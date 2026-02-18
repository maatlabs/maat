//! Virtual machine for executing Maat bytecode.
//!
//! This crate implements a stack-based virtual machine that executes
//! compiled bytecode instructions. The VM uses call frames for function
//! invocations, maintaining a value stack, globals store, and frame stack.

use std::collections::HashMap;

use maat_bytecode::{Bytecode, MAX_FRAMES, MAX_GLOBALS, MAX_STACK_SIZE, Opcode};
use maat_errors::{Result, VmError};
use maat_runtime::{
    BUILTINS, Closure, CompiledFunction, FALSE, HashObject, Hashable, NULL, Object, TRUE,
};

/// A single call frame on the VM's frame stack.
///
/// Each function invocation creates a new frame that tracks the closure's
/// bytecode, current instruction pointer, and base pointer into the stack
/// where this frame's local variables begin.
#[derive(Debug, Clone)]
struct Frame {
    closure: Closure,
    ip: isize,
    base_pointer: usize,
}

impl Frame {
    /// Creates a new call frame for the given closure.
    fn new(closure: Closure, base_pointer: usize) -> Self {
        Self {
            closure,
            ip: -1,
            base_pointer,
        }
    }

    /// Returns a reference to this frame's instruction bytes.
    #[inline]
    fn instructions(&self) -> &[u8] {
        &self.closure.func.instructions
    }
}

/// Virtual machine for executing bytecode instructions.
///
/// The VM uses a stack-based architecture with call frames. Operands are
/// pushed onto a value stack, operations pop operands and push results,
/// and function calls create new frames with their own instruction pointers.
#[derive(Debug)]
pub struct VM {
    constants: Vec<Object>,
    stack: Vec<Object>,
    sp: usize,
    globals: Vec<Object>,
    frames: Vec<Frame>,
}

impl VM {
    /// Creates a new virtual machine from compiled bytecode.
    ///
    /// The top-level program instructions are wrapped in a synthetic
    /// closure and placed as the initial frame.
    pub fn new(bytecode: Bytecode) -> Self {
        let main_closure = Closure {
            func: CompiledFunction {
                instructions: bytecode.instructions.into(),
                num_locals: 0,
                num_parameters: 0,
            },
            free_vars: vec![],
        };
        let main_frame = Frame::new(main_closure, 0);

        Self {
            constants: bytecode.constants,
            stack: Vec::with_capacity(MAX_STACK_SIZE),
            sp: 0,
            globals: Vec::with_capacity(MAX_GLOBALS),
            frames: vec![main_frame],
        }
    }

    /// Creates a VM with an existing globals store.
    ///
    /// This enables REPL sessions where global variable values persist
    /// across multiple bytecode executions.
    pub fn with_globals(bytecode: Bytecode, globals: Vec<Object>) -> Self {
        let main_closure = Closure {
            func: CompiledFunction {
                instructions: bytecode.instructions.into(),
                num_locals: 0,
                num_parameters: 0,
            },
            free_vars: vec![],
        };
        let main_frame = Frame::new(main_closure, 0);

        Self {
            constants: bytecode.constants,
            stack: Vec::with_capacity(MAX_STACK_SIZE),
            sp: 0,
            globals,
            frames: vec![main_frame],
        }
    }

    /// Returns the last value popped from the stack.
    ///
    /// After execution completes, the top-level expression result sits
    /// at the stack position just below `sp` (the slot vacated by the
    /// final `OpPop`). This method retrieves that value without
    /// consuming it.
    pub fn last_popped_stack_elem(&self) -> Option<&Object> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
        }
    }

    /// Returns the current (topmost) call frame.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the frame stack is empty, which indicates
    /// an internal VM invariant violation.
    #[inline]
    fn current_frame(&self) -> Result<&Frame> {
        self.frames
            .last()
            .ok_or_else(|| VmError::new("frame stack underflow").into())
    }

    /// Returns a mutable reference to the current call frame.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the frame stack is empty, which indicates
    /// an internal VM invariant violation.
    #[inline]
    fn current_frame_mut(&mut self) -> Result<&mut Frame> {
        self.frames
            .last_mut()
            .ok_or_else(|| VmError::new("frame stack underflow").into())
    }

    /// Pushes a new call frame onto the frame stack.
    fn push_frame(&mut self, frame: Frame) -> Result<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(VmError::new("stack overflow: maximum call depth exceeded").into());
        }
        self.frames.push(frame);
        Ok(())
    }

    /// Pops the current call frame from the frame stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if only the main frame remains, preventing
    /// an invalid return from the top-level program.
    fn pop_frame(&mut self) -> Result<Frame> {
        if self.frames.len() <= 1 {
            return Err(VmError::new("cannot return from top-level code").into());
        }
        self.frames
            .pop()
            .ok_or_else(|| VmError::new("frame stack underflow").into())
    }

    /// Executes the bytecode instructions.
    ///
    /// Iterates through instructions in the current frame, dispatching
    /// to appropriate handlers for each opcode. Function calls push new
    /// frames; returns pop them.
    pub fn run(&mut self) -> Result<()> {
        loop {
            let frame = self.current_frame()?;
            if frame.ip >= frame.instructions().len() as isize - 1 {
                break;
            }

            self.current_frame_mut()?.ip += 1;
            let ip = self.current_frame()?.ip as usize;
            let op_byte = *self
                .current_frame()?
                .instructions()
                .get(ip)
                .ok_or_else(|| VmError::new(format!("instruction pointer out of bounds: {ip}")))?;
            let op = Opcode::from_byte(op_byte)
                .ok_or_else(|| VmError::new(format!("unknown opcode: {op_byte}")))?;

            match op {
                Opcode::Constant => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let constant = self.constants.get(index).cloned().ok_or_else(|| {
                        VmError::new(format!(
                            "constant pool access out of bounds at index {index}"
                        ))
                    })?;
                    self.push_stack(constant)?;
                }
                Opcode::Pop => {
                    self.pop_stack()?;
                }
                Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => {
                    self.execute_binary_operation(op)?;
                }
                Opcode::True => self.push_stack(TRUE)?,
                Opcode::False => self.push_stack(FALSE)?,

                Opcode::Equal | Opcode::NotEqual | Opcode::GreaterThan | Opcode::LessThan => {
                    self.execute_comparison(op)?
                }
                Opcode::Bang => self.execute_bang_operator()?,
                Opcode::Minus => self.execute_minus_operator()?,
                Opcode::Jump => {
                    let target = self.read_u16_operand(ip + 1)? as isize;
                    self.current_frame_mut()?.ip = target - 1;
                }
                Opcode::CondJump => {
                    let target = self.read_u16_operand(ip + 1)? as isize;
                    self.current_frame_mut()?.ip += 2;
                    let condition = self.pop_stack()?;
                    if !condition.is_truthy() {
                        self.current_frame_mut()?.ip = target - 1;
                    }
                }
                Opcode::Null => {
                    self.push_stack(NULL)?;
                }
                Opcode::SetGlobal => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let value = self.pop_stack()?;
                    if index >= self.globals.len() {
                        self.globals.resize(index + 1, Object::Null);
                    }
                    self.globals[index] = value;
                }
                Opcode::GetGlobal => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let value = self.globals.get(index).cloned().ok_or_else(|| {
                        VmError::new(format!("undefined global variable at index {index}"))
                    })?;
                    self.push_stack(value)?;
                }
                Opcode::Array => {
                    let num_elements = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let array = self.build_array(num_elements)?;
                    self.push_stack(array)?;
                }
                Opcode::Hash => {
                    let num_elements = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let hash = self.build_hash(num_elements)?;
                    self.push_stack(hash)?;
                }
                Opcode::Index => {
                    let index = self.pop_stack()?;
                    let container = self.pop_stack()?;
                    self.execute_index_expression(container, index)?;
                }
                Opcode::Call => {
                    let num_args = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    self.execute_function_call(num_args)?;
                }
                Opcode::GetBuiltin => {
                    let index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let (_, builtin_fn) = BUILTINS.get(index).ok_or_else(|| {
                        VmError::new(format!("builtin index out of bounds: {index}"))
                    })?;
                    self.push_stack(Object::Builtin(*builtin_fn))?;
                }
                Opcode::Closure => {
                    let const_index = self.read_u16_operand(ip + 1)?;
                    let num_free = self.read_u8_operand(ip + 3)?;
                    self.current_frame_mut()?.ip += 3;

                    let func = match self.constants.get(const_index) {
                        Some(Object::CompiledFunction(f)) => f.clone(),
                        _ => {
                            return Err(VmError::new(format!(
                                "expected CompiledFunction at constant pool index {const_index}"
                            ))
                            .into());
                        }
                    };

                    let base = self
                        .sp
                        .checked_sub(num_free)
                        .ok_or_else(|| VmError::new("stack underflow reading free variables"))?;
                    let free_vars = (0..num_free)
                        .map(|i| {
                            self.stack.get(base + i).cloned().ok_or_else(|| {
                                VmError::new("stack underflow reading free variables").into()
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    self.sp = base;

                    self.push_stack(Object::Closure(Closure { func, free_vars }))?;
                }
                Opcode::GetFree => {
                    let index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let value = self
                        .current_frame()?
                        .closure
                        .free_vars
                        .get(index)
                        .cloned()
                        .ok_or_else(|| {
                            VmError::new(format!("free variable index out of bounds: {index}"))
                        })?;
                    self.push_stack(value)?;
                }
                Opcode::CurrentClosure => {
                    let closure = self.current_frame()?.closure.clone();
                    self.push_stack(Object::Closure(closure))?;
                }
                Opcode::ReturnValue => {
                    let return_value = self.pop_stack()?;
                    let frame = self.pop_frame()?;
                    self.sp = frame.base_pointer.saturating_sub(1);
                    self.push_stack(return_value)?;
                }
                Opcode::Return => {
                    let frame = self.pop_frame()?;
                    self.sp = frame.base_pointer.saturating_sub(1);
                    self.push_stack(NULL)?;
                }
                Opcode::SetLocal => {
                    let local_index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let base_pointer = self.current_frame()?.base_pointer;
                    let value = self.pop_stack()?;
                    let slot = base_pointer + local_index;
                    if slot >= self.stack.len() {
                        self.stack.resize(slot + 1, Object::Null);
                    }
                    self.stack[slot] = value;
                }
                Opcode::GetLocal => {
                    let local_index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let base_pointer = self.current_frame()?.base_pointer;
                    let slot = base_pointer + local_index;
                    let value = self.stack.get(slot).cloned().ok_or_else(|| {
                        VmError::new(format!(
                            "local variable access out of bounds at slot {slot}"
                        ))
                    })?;
                    self.push_stack(value)?;
                }
            }
        }

        Ok(())
    }

    /// Reads a 16-bit big-endian operand from the current frame's instructions.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the offset extends beyond the instruction stream.
    #[inline]
    fn read_u16_operand(&self, offset: usize) -> Result<usize> {
        let instructions = self.current_frame()?.instructions();
        let hi = *instructions
            .get(offset)
            .ok_or_else(|| VmError::new("instruction stream truncated: missing operand byte"))?;
        let lo = *instructions
            .get(offset + 1)
            .ok_or_else(|| VmError::new("instruction stream truncated: missing operand byte"))?;
        Ok(u16::from_be_bytes([hi, lo]) as usize)
    }

    /// Reads an 8-bit operand from the current frame's instructions.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the offset is beyond the instruction stream.
    #[inline]
    fn read_u8_operand(&self, offset: usize) -> Result<usize> {
        self.current_frame()?
            .instructions()
            .get(offset)
            .map(|&b| b as usize)
            .ok_or_else(|| {
                VmError::new("instruction stream truncated: missing operand byte").into()
            })
    }

    /// Dispatches a function call to the appropriate handler based on the callee type.
    fn execute_function_call(&mut self, num_args: usize) -> Result<()> {
        let fn_slot = self
            .sp
            .checked_sub(1 + num_args)
            .ok_or_else(|| VmError::new("stack underflow in function call"))?;
        let callee = self
            .stack
            .get(fn_slot)
            .cloned()
            .ok_or_else(|| VmError::new("stack underflow in function call"))?;

        match callee {
            Object::Closure(cl) => self.call_closure(cl, num_args),
            Object::Builtin(f) => self.call_builtin_fn(f, num_args),
            _ => Err(VmError::new("calling non-function").into()),
        }
    }

    /// Handles closure call execution.
    fn call_closure(&mut self, closure: Closure, num_args: usize) -> Result<()> {
        if num_args != closure.func.num_parameters {
            return Err(VmError::new(format!(
                "wrong number of arguments: want={}, got={num_args}",
                closure.func.num_parameters
            ))
            .into());
        }

        let base_pointer = self.sp - num_args;
        let num_locals = closure.func.num_locals;
        let frame = Frame::new(closure, base_pointer);
        self.push_frame(frame)?;
        self.sp = base_pointer + num_locals;

        if self.sp > self.stack.len() {
            self.stack.resize(self.sp, Object::Null);
        }

        Ok(())
    }

    /// Executes a built-in function call.
    fn call_builtin_fn(
        &mut self,
        func: fn(&[Object]) -> Result<Object>,
        num_args: usize,
    ) -> Result<()> {
        let args_start = self.sp - num_args;
        let args = self.stack[args_start..self.sp].to_vec();
        let result = func(&args)?;

        self.sp = args_start - 1;
        self.push_stack(result)?;

        Ok(())
    }

    /// Pushes a value onto the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack is full (stack overflow).
    fn push_stack(&mut self, obj: Object) -> Result<()> {
        if self.sp >= MAX_STACK_SIZE {
            return Err(VmError::new("stack overflow").into());
        }

        if self.sp >= self.stack.len() {
            self.stack.push(obj);
        } else {
            self.stack[self.sp] = obj;
        }

        self.sp += 1;
        Ok(())
    }

    /// Pops a value from the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack is empty (stack underflow).
    fn pop_stack(&mut self) -> Result<Object> {
        if self.sp == 0 {
            return Err(VmError::new("stack underflow").into());
        }

        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    /// Executes a binary arithmetic operation.
    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

        match (&left, &right) {
            (Object::I64(l), Object::I64(r)) => self.execute_binary_integer_operation(op, *l, *r),
            (Object::String(l), Object::String(r)) => {
                self.execute_binary_string_operation(op, l, r)
            }
            _ => Err(VmError::new(format!(
                "unsupported types for binary operation: {} {}",
                left.type_name(),
                right.type_name()
            ))
            .into()),
        }
    }

    /// Executes a binary integer arithmetic operation.
    fn execute_binary_integer_operation(
        &mut self,
        op: Opcode,
        left: i64,
        right: i64,
    ) -> Result<()> {
        let result = match op {
            Opcode::Add => left.checked_add(right),
            Opcode::Sub => left.checked_sub(right),
            Opcode::Mul => left.checked_mul(right),
            Opcode::Div => left.checked_div(right),
            _ => return Err(VmError::new(format!("unknown integer operator: {:?}", op)).into()),
        };

        let result = result.ok_or_else(|| VmError::new("integer arithmetic overflow"))?;
        self.push_stack(Object::I64(result))
    }

    /// Executes a comparison operation.
    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

        match (&left, &right) {
            (Object::I64(l), Object::I64(r)) => self.execute_integer_comparison(op, *l, *r),
            _ => {
                let result = match op {
                    Opcode::Equal => left == right,
                    Opcode::NotEqual => left != right,
                    _ => {
                        return Err(VmError::new(format!(
                            "unknown operator: {:?} ({} {})",
                            op,
                            left.type_name(),
                            right.type_name()
                        ))
                        .into());
                    }
                };
                self.push_stack(Object::Boolean(result))
            }
        }
    }

    /// Executes an integer comparison operation.
    fn execute_integer_comparison(&mut self, op: Opcode, left: i64, right: i64) -> Result<()> {
        let result = match op {
            Opcode::Equal => left == right,
            Opcode::NotEqual => left != right,
            Opcode::GreaterThan => left > right,
            Opcode::LessThan => left < right,
            _ => return Err(VmError::new(format!("unknown operator: {:?}", op)).into()),
        };

        self.push_stack(Object::Boolean(result))
    }

    /// Executes the logical NOT (bang) operator.
    fn execute_bang_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;
        let result = match operand {
            Object::Boolean(false) | Object::Null => TRUE,
            _ => FALSE,
        };
        self.push_stack(result)
    }

    /// Executes the unary minus operator.
    fn execute_minus_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;
        match operand {
            Object::I64(value) => {
                let result = value
                    .checked_neg()
                    .ok_or_else(|| VmError::new("integer negation overflow"))?;
                self.push_stack(Object::I64(result))
            }
            _ => Err(VmError::new(format!(
                "unsupported type for negation: {}",
                operand.type_name()
            ))
            .into()),
        }
    }

    /// Executes string concatenation via `OpAdd`.
    fn execute_binary_string_operation(
        &mut self,
        op: Opcode,
        left: &str,
        right: &str,
    ) -> Result<()> {
        if op != Opcode::Add {
            return Err(VmError::new(format!("unknown string operator: {}", op.name())).into());
        }
        let mut result = String::with_capacity(left.len() + right.len());
        result.push_str(left);
        result.push_str(right);
        self.push_stack(Object::String(result))
    }

    /// Builds an array object from the top `num_elements` stack values.
    fn build_array(&mut self, num_elements: usize) -> Result<Object> {
        if num_elements > self.sp {
            return Err(VmError::new(format!(
                "stack underflow in array construction: need {num_elements} elements, stack has {}",
                self.sp
            ))
            .into());
        }
        let start = self.sp - num_elements;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        Ok(Object::Array(elements))
    }

    /// Builds a hash object from the top `num_elements` stack values.
    ///
    /// Elements are expected in alternating key-value order on the stack.
    fn build_hash(&mut self, num_elements: usize) -> Result<Object> {
        if num_elements > self.sp {
            return Err(VmError::new(format!(
                "stack underflow in hash construction: need {num_elements} elements, stack has {}",
                self.sp
            ))
            .into());
        }
        let start = self.sp - num_elements;
        let mut pairs = HashMap::with_capacity(num_elements / 2);

        for i in (start..self.sp).step_by(2) {
            let key = self.stack[i].clone();
            let value = self.stack[i + 1].clone();
            let key = Hashable::try_from(key).map_err(|e| VmError::new(e.to_string()))?;
            pairs.insert(key, value);
        }

        self.sp = start;
        Ok(Object::Hash(HashObject { pairs }))
    }

    /// Dispatches index operations to the appropriate handler.
    fn execute_index_expression(&mut self, container: Object, index: Object) -> Result<()> {
        match (&container, &index) {
            (Object::Array(elements), _) => self.execute_array_index(elements, &index),
            (Object::Hash(hash), _) => self.execute_hash_index(hash, index),
            _ => Err(VmError::new(format!(
                "index operator not supported: {}",
                container.type_name()
            ))
            .into()),
        }
    }

    /// Indexes into an array with bounds checking.
    fn execute_array_index(&mut self, elements: &[Object], index: &Object) -> Result<()> {
        if !index.is_integer() {
            return Err(VmError::new(format!(
                "array index must be an integer, got {}",
                index.type_name()
            ))
            .into());
        }

        match index.to_array_index() {
            Some(idx) if idx < elements.len() => self.push_stack(elements[idx].clone()),
            _ => self.push_stack(NULL),
        }
    }

    /// Indexes into a hash by key.
    fn execute_hash_index(&mut self, hash: &HashObject, index: Object) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| VmError::new(e.to_string()))?;

        match hash.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => self.push_stack(NULL),
        }
    }
}
