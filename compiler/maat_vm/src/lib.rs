//! Virtual machine for executing Maat bytecode.
//!
//! This crate implements a stack-based virtual machine that executes
//! compiled bytecode instructions. The VM uses call frames for function
//! invocations, maintaining a value stack, globals store, and frame stack.
#![forbid(unsafe_code)]

use std::rc::Rc;

use indexmap::IndexMap;
use maat_bytecode::{Bytecode, MAX_FRAMES, MAX_GLOBALS, MAX_STACK_SIZE, Opcode, TypeTag};
use maat_errors::{Result, VmError};
use maat_runtime::{
    BUILTINS, Closure, CompiledFn, EnumVariantVal, FALSE, Felt, Hashable, Integer, Map, StructVal,
    TRUE, TypeDef, UNIT, Value, WideInt,
};
use maat_span::{SourceMap, Span};

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
    constants: Vec<Value>,
    stack: Vec<Value>,
    sp: usize,
    globals: Vec<Value>,
    frames: Vec<Frame>,
    source_map: SourceMap,
    type_registry: Vec<TypeDef>,
}

impl VM {
    /// Creates a new virtual machine from compiled bytecode.
    ///
    /// The top-level program instructions are wrapped in a synthetic
    /// closure and placed as the initial frame.
    pub fn new(bytecode: Bytecode) -> Self {
        let source_map = bytecode.source_map;
        let type_registry = bytecode.type_registry;
        let main_closure = Closure {
            func: CompiledFn {
                instructions: Rc::from(bytecode.instructions.as_bytes()),
                num_locals: 0,
                num_parameters: 0,
                source_map: SourceMap::new(),
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
            source_map,
            type_registry,
        }
    }

    /// Creates a VM with an existing globals store.
    ///
    /// This enables REPL sessions where global variable values persist
    /// across multiple bytecode executions.
    pub fn with_globals(bytecode: Bytecode, globals: Vec<Value>) -> Self {
        let source_map = bytecode.source_map;
        let type_registry = bytecode.type_registry;
        let main_closure = Closure {
            func: CompiledFn {
                instructions: Rc::from(bytecode.instructions.as_bytes()),
                num_locals: 0,
                num_parameters: 0,
                source_map: SourceMap::new(),
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
            source_map,
            type_registry,
        }
    }

    /// Returns a reference to the VM's global variable store.
    ///
    /// Used for REPL sessions where global values must persist across
    /// multiple bytecode executions.
    pub fn globals(&self) -> &[Value] {
        &self.globals
    }

    /// Returns the last value popped from the stack.
    ///
    /// After execution completes, the top-level expression result sits
    /// at the stack position just below `sp` (the slot vacated by the
    /// final `OpPop`). This method retrieves that value without
    /// consuming it.
    pub fn last_popped_stack_elem(&self) -> Option<&Value> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
        }
    }

    /// Looks up the source span for the current instruction pointer.
    ///
    /// Checks the current frame's function-level source map first,
    /// then falls back to the top-level source map.
    fn current_span(&self) -> Option<Span> {
        let frame = self.frames.last()?;
        let ip = frame.ip as usize;
        frame
            .closure
            .func
            .source_map
            .lookup(ip)
            .or_else(|| self.source_map.lookup(ip))
    }

    /// Creates a VM error with the current source span attached.
    fn vm_error(&self, message: impl Into<String>) -> maat_errors::Error {
        match self.current_span() {
            Some(span) => VmError::with_span(message, span).into(),
            None => VmError::new(message).into(),
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
            .ok_or_else(|| self.vm_error("frame stack underflow"))
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
            return Err(self.vm_error("stack overflow: maximum call depth exceeded"));
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
            return Err(self.vm_error("cannot return from top-level code"));
        }
        self.frames
            .pop()
            .ok_or_else(|| self.vm_error("frame stack underflow"))
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
                .ok_or_else(|| self.vm_error(format!("instruction pointer out of bounds: {ip}")))?;
            let op = Opcode::from_byte(op_byte)
                .ok_or_else(|| self.vm_error(format!("unknown opcode: {op_byte}")))?;

            match op {
                Opcode::Constant => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let constant = self.constants.get(index).cloned().ok_or_else(|| {
                        self.vm_error(format!(
                            "constant pool access out of bounds at index {index}"
                        ))
                    })?;
                    self.push_stack(constant)?;
                }
                Opcode::Pop => {
                    self.pop_stack()?;
                }
                Opcode::Add
                | Opcode::Sub
                | Opcode::Mul
                | Opcode::Div
                | Opcode::Mod
                | Opcode::BitAnd
                | Opcode::BitOr
                | Opcode::BitXor
                | Opcode::Shl
                | Opcode::Shr => {
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
                Opcode::Unit => {
                    self.push_stack(UNIT)?;
                }
                Opcode::SetGlobal => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let value = self.pop_stack()?;
                    if index >= self.globals.len() {
                        self.globals.resize(index + 1, Value::Unit);
                    }
                    self.globals[index] = value;
                }
                Opcode::GetGlobal => {
                    let index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let value = self.globals.get(index).cloned().ok_or_else(|| {
                        self.vm_error(format!("undefined global variable at index {index}"))
                    })?;
                    self.push_stack(value)?;
                }
                Opcode::Vector => {
                    let num_elements = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let vector = self.build_vector(num_elements)?;
                    self.push_stack(vector)?;
                }
                Opcode::Tuple => {
                    let num_elements = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let tuple = self.build_tuple(num_elements)?;
                    self.push_stack(tuple)?;
                }
                Opcode::Map => {
                    let num_elements = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    let map = self.build_map(num_elements)?;
                    self.push_stack(map)?;
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
                        self.vm_error(format!("builtin index out of bounds: {index}"))
                    })?;
                    self.push_stack(Value::Builtin(*builtin_fn))?;
                }
                Opcode::Closure => {
                    let const_index = self.read_u16_operand(ip + 1)?;
                    let num_free = self.read_u8_operand(ip + 3)?;
                    self.current_frame_mut()?.ip += 3;
                    let func = match self.constants.get(const_index) {
                        Some(Value::CompiledFn(f)) => f.clone(),
                        _ => {
                            return Err(self.vm_error(format!(
                                "expected CompiledFn at constant pool index {const_index}"
                            )));
                        }
                    };
                    let base = self
                        .sp
                        .checked_sub(num_free)
                        .ok_or_else(|| self.vm_error("stack underflow reading free variables"))?;
                    let free_vars = (0..num_free)
                        .map(|i| {
                            self.stack.get(base + i).cloned().ok_or_else(|| {
                                self.vm_error("stack underflow reading free variables")
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    self.sp = base;
                    self.push_stack(Value::Closure(Closure { func, free_vars }))?;
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
                            self.vm_error(format!("free variable index out of bounds: {index}"))
                        })?;
                    self.push_stack(value)?;
                }
                Opcode::CurrentClosure => {
                    let closure = self.current_frame()?.closure.clone();
                    self.push_stack(Value::Closure(closure))?;
                }
                Opcode::Convert => {
                    let tag_byte = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    self.execute_convert(tag_byte)?;
                }
                Opcode::Construct => {
                    let type_index = self.read_u16_operand(ip + 1)?;
                    let num_fields = self.read_u8_operand(ip + 3)?;
                    self.current_frame_mut()?.ip += 3;
                    self.execute_construct(type_index, num_fields)?;
                }
                Opcode::GetField => {
                    let field_index = self.read_u16_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 2;
                    self.execute_get_field(field_index)?;
                }
                Opcode::MatchTag => {
                    let expected_tag = self.read_u16_operand(ip + 1)?;
                    let jump_target = self.read_u16_operand(ip + 3)?;
                    self.current_frame_mut()?.ip += 4;
                    self.execute_match_tag(expected_tag, jump_target)?;
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
                    self.push_stack(UNIT)?;
                }
                Opcode::SetLocal => {
                    let local_index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let base_pointer = self.current_frame()?.base_pointer;
                    let value = self.pop_stack()?;
                    let slot = base_pointer + local_index;
                    if slot >= self.stack.len() {
                        self.stack.resize(slot + 1, Value::Unit);
                    }
                    self.stack[slot] = value;
                }
                Opcode::GetLocal => {
                    let local_index = self.read_u8_operand(ip + 1)?;
                    self.current_frame_mut()?.ip += 1;
                    let base_pointer = self.current_frame()?.base_pointer;
                    let slot = base_pointer + local_index;
                    let value = self.stack.get(slot).cloned().ok_or_else(|| {
                        self.vm_error(format!(
                            "local variable access out of bounds at slot {slot}"
                        ))
                    })?;
                    self.push_stack(value)?;
                }
                Opcode::MakeRange => {
                    let end = self.pop_integer("Range")?;
                    let start = self.pop_integer("Range")?;
                    self.push_stack(Value::Range(start, end))?;
                }
                Opcode::MakeRangeInclusive => {
                    let end = self.pop_integer("RangeInclusive")?;
                    let start = self.pop_integer("RangeInclusive")?;
                    self.push_stack(Value::RangeInclusive(start, end))?;
                }
                Opcode::FeltAdd | Opcode::FeltSub | Opcode::FeltMul => {
                    self.execute_felt_binop(op)?;
                }
                Opcode::FeltInv => {
                    self.execute_felt_inv()?;
                }
                Opcode::FeltPow => {
                    self.execute_felt_pow()?;
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
            .ok_or_else(|| self.vm_error("instruction stream truncated: missing operand byte"))?;
        let lo = *instructions
            .get(offset + 1)
            .ok_or_else(|| self.vm_error("instruction stream truncated: missing operand byte"))?;
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
            .ok_or_else(|| self.vm_error("instruction stream truncated: missing operand byte"))
    }

    /// Dispatches a function call to the appropriate handler based on the callee type.
    fn execute_function_call(&mut self, num_args: usize) -> Result<()> {
        let fn_slot = self
            .sp
            .checked_sub(1 + num_args)
            .ok_or_else(|| self.vm_error("stack underflow in function call"))?;
        let callee = self
            .stack
            .get(fn_slot)
            .cloned()
            .ok_or_else(|| self.vm_error("stack underflow in function call"))?;
        match callee {
            Value::Closure(cl) => self.call_closure(cl, num_args),
            Value::Builtin(f) => self.call_builtin_fn(f, num_args),
            _ => Err(self.vm_error("calling non-function")),
        }
    }

    /// Handles closure call execution.
    fn call_closure(&mut self, closure: Closure, num_args: usize) -> Result<()> {
        if num_args != closure.func.num_parameters {
            return Err(self.vm_error(format!(
                "wrong number of arguments: want={}, got={num_args}",
                closure.func.num_parameters
            )));
        }
        let base_pointer = self.sp - num_args;
        let num_locals = closure.func.num_locals;
        let frame = Frame::new(closure, base_pointer);
        self.push_frame(frame)?;
        self.sp = base_pointer + num_locals;

        if self.sp > self.stack.len() {
            self.stack.resize(self.sp, Value::Unit);
        }
        Ok(())
    }

    /// Executes a built-in function call.
    fn call_builtin_fn(
        &mut self,
        func: fn(&[Value]) -> Result<Value>,
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
    fn push_stack(&mut self, val: Value) -> Result<()> {
        if self.sp >= MAX_STACK_SIZE {
            return Err(self.vm_error("stack overflow"));
        }
        if self.sp >= self.stack.len() {
            self.stack.push(val);
        } else {
            self.stack[self.sp] = val;
        }
        self.sp += 1;
        Ok(())
    }

    /// Pops a value from the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack is empty (stack underflow).
    fn pop_stack(&mut self) -> Result<Value> {
        if self.sp == 0 {
            return Err(self.vm_error("stack underflow"));
        }
        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    /// Pops a value from the stack and extracts it as an [`Integer`].
    ///
    /// Used by range construction opcodes where both bounds must be integers.
    fn pop_integer(&mut self, context: &str) -> Result<Integer> {
        match self.pop_stack()? {
            Value::Integer(v) => Ok(v),
            other => Err(self.vm_error(format!(
                "{context} bounds must be integer, got {}",
                other.type_name()
            ))),
        }
    }

    /// Executes a binary arithmetic, bitwise, or shift operation across all numeric types.
    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        let (l_name, r_name) = (left.type_name(), right.type_name());

        // String concatenation (only for '+')
        if op == Opcode::Add
            && let (Value::Str(l), Value::Str(r)) = (&left, &right)
        {
            return self.push_stack(Value::Str(format!("{}{}", l, r)));
        }

        // Integer operations
        match (left, right) {
            (Value::Integer(l), Value::Integer(r)) => {
                let result = match op {
                    Opcode::Add => l
                        .checked_add(r)
                        .ok_or_else(|| self.vm_error("arithmetic overflow"))?,
                    Opcode::Sub => l
                        .checked_sub(r)
                        .ok_or_else(|| self.vm_error("arithmetic overflow"))?,
                    Opcode::Mul => l
                        .checked_mul(r)
                        .ok_or_else(|| self.vm_error("arithmetic overflow"))?,
                    Opcode::Div => l
                        .checked_div(r)
                        .ok_or_else(|| self.vm_error("division by zero or overflow"))?,
                    Opcode::Mod => l
                        .checked_rem_euclid(r)
                        .ok_or_else(|| self.vm_error("modulo by zero or overflow"))?,
                    Opcode::Shl | Opcode::Shr => {
                        let shift = r
                            .to_usize()
                            .and_then(|u| u32::try_from(u).ok())
                            .ok_or_else(|| {
                                self.vm_error(
                                    "shift amount must be a non-negative integer <= u32::MAX",
                                )
                            })?;
                        match op {
                            Opcode::Shl => l.checked_shl(shift).ok_or_else(|| {
                                self.vm_error("shift value exceeds type bit width")
                            })?,
                            Opcode::Shr => l.checked_shr(shift).ok_or_else(|| {
                                self.vm_error("shift value exceeds type bit width")
                            })?,
                            _ => unreachable!(),
                        }
                    }
                    Opcode::BitAnd => l
                        .bitwise_and(r)
                        .ok_or_else(|| self.vm_error("type mismatch in bitwise AND"))?,
                    Opcode::BitOr => l
                        .bitwise_or(r)
                        .ok_or_else(|| self.vm_error("type mismatch in bitwise OR"))?,
                    Opcode::BitXor => l
                        .bitwise_xor(r)
                        .ok_or_else(|| self.vm_error("type mismatch in bitwise XOR"))?,
                    _ => {
                        return Err(
                            self.vm_error(format!("unsupported binary operation: {:?}", op))
                        );
                    }
                };
                self.push_stack(Value::Integer(result))
            }
            _ => Err(self.vm_error(format!(
                "unsupported types for binary operation: {l_name} {r_name}"
            ))),
        }
    }

    /// Executes a comparison operation across all numeric and non-numeric types.
    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        // Same-type numeric comparison
        if let Some(result) = self.compare_ordered(op, &left, &right) {
            return self.push_stack(Value::Bool(result));
        }
        // Cross-type integer comparison via i128 widening
        if let (Some(l), Some(r)) = (left.to_i128(), right.to_i128()) {
            let result = match op {
                Opcode::Equal => l == r,
                Opcode::NotEqual => l != r,
                Opcode::GreaterThan => l > r,
                Opcode::LessThan => l < r,
                _ => unreachable!(),
            };
            return self.push_stack(Value::Bool(result));
        }
        // Non-numeric equality (booleans, strings, vectors, etc.)
        match op {
            Opcode::Equal => self.push_stack(Value::Bool(left == right)),
            Opcode::NotEqual => self.push_stack(Value::Bool(left != right)),
            _ => Err(self.vm_error(format!(
                "unsupported comparison: {:?} ({} {})",
                op,
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    /// Attempts same-type ordered comparison for integers, characters, and strings.
    ///
    /// Returns `None` if the operands are not a supported same-type pair.
    fn compare_ordered(&self, op: Opcode, left: &Value, right: &Value) -> Option<bool> {
        match (left, right) {
            (Value::Integer(l), Value::Integer(r)) => {
                let ordering = l.partial_cmp(r)?;
                Some(match op {
                    Opcode::Equal => ordering.is_eq(),
                    Opcode::NotEqual => ordering.is_ne(),
                    Opcode::GreaterThan => ordering.is_gt(),
                    Opcode::LessThan => ordering.is_lt(),
                    _ => return None,
                })
            }
            (Value::Char(l), Value::Char(r)) => Some(match op {
                Opcode::Equal => l == r,
                Opcode::NotEqual => l != r,
                Opcode::GreaterThan => l > r,
                Opcode::LessThan => l < r,
                _ => return None,
            }),
            (Value::Str(l), Value::Str(r)) => Some(match op {
                Opcode::Equal => l == r,
                Opcode::NotEqual => l != r,
                Opcode::GreaterThan => l > r,
                Opcode::LessThan => l < r,
                _ => return None,
            }),
            (Value::Felt(l), Value::Felt(r)) => match op {
                Opcode::Equal => Some(l == r),
                Opcode::NotEqual => Some(l != r),
                _ => None,
            },
            _ => None,
        }
    }

    /// Executes the logical NOT (bang) operator.
    fn execute_bang_operator(&mut self) -> Result<()> {
        let result = match self.pop_stack()? {
            Value::Bool(b) => Value::Bool(!b),
            other => {
                return Err(self.vm_error(format!("cannot apply `!` to {}", other.type_name())));
            }
        };
        self.push_stack(result)
    }

    /// Executes the unary minus operator for all signed integer types.
    fn execute_minus_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;
        match operand {
            Value::Integer(int_val) => match int_val.checked_neg() {
                Some(neg) => self.push_stack(Value::Integer(neg)),
                None => Err(self.vm_error("integer negation overflow")),
            },
            _ => Err(self.vm_error(format!(
                "unsupported type for negation: {}",
                operand.type_name()
            ))),
        }
    }

    /// Executes an explicit type conversion (`as` operator).
    ///
    /// Pops the top stack value, converts it to the target type specified
    /// by the type tag operand, and pushes the result. Rejects lossy
    /// conversions at runtime (e.g., value out of range for the target type).
    fn execute_convert(&mut self, tag_byte: usize) -> Result<()> {
        let tag = TypeTag::from_byte(tag_byte as u8)
            .ok_or_else(|| self.vm_error(format!("unknown type tag: {tag_byte}")))?;
        let value = self.pop_stack()?;
        let converted = self.convert_value(&value, tag)?;
        self.push_stack(converted)
    }

    /// Converts a runtime value to the specified target type.
    ///
    /// Widening conversions always succeed. Narrowing conversions that would
    /// lose data produce a runtime error.
    fn convert_value(&self, value: &Value, target: TypeTag) -> Result<Value> {
        if target == TypeTag::Char {
            return match value {
                Value::Integer(val) => {
                    let scalar = match val.to_wide() {
                        WideInt::Signed(v) => u32::try_from(v).ok().and_then(char::from_u32),
                        WideInt::Unsigned(v) => u32::try_from(v).ok().and_then(char::from_u32),
                    };
                    scalar.map(Value::Char).ok_or_else(|| {
                        self.vm_error(format!("value {} is not a valid Unicode scalar value", val,))
                    })
                }
                other => Err(self.vm_error(format!("cannot cast {} as char", other.type_name(),))),
            };
        }

        let num_kind = target
            .to_num_kind()
            .ok_or_else(|| self.vm_error(format!("unknown conversion target: {target:?}")))?;

        if target == TypeTag::Felt {
            return self.convert_to_felt(value);
        }

        match value {
            Value::Char(ch) => {
                Integer::from_wide(WideInt::Unsigned(u128::from(*ch as u32)), num_kind)
                    .map(Value::Integer)
                    .map_err(|e| self.vm_error(e))
            }
            Value::Integer(val) => val
                .cast_to(num_kind)
                .map(Value::Integer)
                .map_err(|e| self.vm_error(e)),
            Value::Felt(_) => Err(self.vm_error(format!(
                "cannot cast Felt to {}; field elements are non-narrowing",
                num_kind.as_str(),
            ))),
            other => Err(self.vm_error(format!(
                "cannot cast {} to {}",
                other.type_name(),
                num_kind.as_str(),
            ))),
        }
    }

    /// Converts an integer-typed value into a Goldilocks field element.
    fn convert_to_felt(&self, value: &Value) -> Result<Value> {
        use maat_runtime::Integer as I;

        let felt = match value {
            Value::Felt(f) => return Ok(Value::Felt(*f)),
            Value::Integer(I::I8(v)) => Felt::from_i64(*v as i64),
            Value::Integer(I::I16(v)) => Felt::from_i64(*v as i64),
            Value::Integer(I::I32(v)) => Felt::from_i64(*v as i64),
            Value::Integer(I::I64(v)) => Felt::from_i64(*v),
            Value::Integer(I::Isize(v)) => Felt::from_i64(*v as i64),
            Value::Integer(I::U8(v)) => Felt::new(u64::from(*v)),
            Value::Integer(I::U16(v)) => Felt::new(u64::from(*v)),
            Value::Integer(I::U32(v)) => Felt::new(u64::from(*v)),
            Value::Integer(I::U64(v)) => Felt::new(*v),
            Value::Integer(I::Usize(v)) => Felt::new(*v as u64),
            Value::Integer(I::I128(_)) | Value::Integer(I::U128(_)) => {
                return Err(
                    self.vm_error("cannot cast 128-bit integer to Felt; use explicit `Felt::new`")
                );
            }
            other => {
                return Err(self.vm_error(format!("cannot cast {} to Felt", other.type_name())));
            }
        };
        Ok(Value::Felt(felt))
    }

    /// Pops a [`Value::Felt`] off the stack, or errors with a descriptive message.
    fn pop_felt(&mut self, context: &str) -> Result<Felt> {
        match self.pop_stack()? {
            Value::Felt(f) => Ok(f),
            other => Err(self.vm_error(format!(
                "{context} expects Felt operand, got {}",
                other.type_name()
            ))),
        }
    }

    /// Executes a `Felt`-typed binary operator ([`Opcode::FeltAdd`],
    /// [`Opcode::FeltSub`], [`Opcode::FeltMul`]).
    fn execute_felt_binop(&mut self, op: Opcode) -> Result<()> {
        let rhs = self.pop_felt("Felt arithmetic")?;
        let lhs = self.pop_felt("Felt arithmetic")?;
        let result = match op {
            Opcode::FeltAdd => lhs + rhs,
            Opcode::FeltSub => lhs - rhs,
            Opcode::FeltMul => lhs * rhs,
            _ => unreachable!("non-Felt opcode in execute_felt_binary"),
        };
        self.push_stack(Value::Felt(result))
    }

    /// Executes [`Opcode::FeltInv`], replacing the top stack element with
    /// its multiplicative inverse. Errors at runtime if the operand is zero.
    fn execute_felt_inv(&mut self) -> Result<()> {
        let operand = self.pop_felt("Felt inverse")?;
        let inv = operand
            .inv()
            .map_err(|e| self.vm_error(format!("Felt inverse error: {e}")))?;
        self.push_stack(Value::Felt(inv))
    }

    /// Executes [`Opcode::FeltPow`]. Pops the `u64` exponent and the base
    /// field element, pushing `base^exponent` computed by square-and-multiply.
    /// The exponent must be a `u64`-typed integer.
    fn execute_felt_pow(&mut self) -> Result<()> {
        let exp_value = self.pop_stack()?;
        let exponent = match exp_value {
            Value::Integer(Integer::U64(v)) => v,
            Value::Integer(int) => int
                .to_usize()
                .and_then(|u| u64::try_from(u).ok())
                .ok_or_else(|| {
                    self.vm_error("Felt exponent must be a non-negative integer <= u64::MAX")
                })?,
            other => {
                return Err(self.vm_error(format!(
                    "Felt exponent must be an integer, got {}",
                    other.type_name()
                )));
            }
        };
        let base = self.pop_felt("Felt power")?;
        self.push_stack(Value::Felt(base.pow(exponent)))
    }

    /// Builds a vector from the top `num_elements` stack values.
    fn build_vector(&mut self, num_elements: usize) -> Result<Value> {
        if num_elements > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in vector construction: need {num_elements} elements, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - num_elements;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        Ok(Value::Vector(elements))
    }

    /// Builds a tuple from the top `num_elements` stack values.
    fn build_tuple(&mut self, num_elements: usize) -> Result<Value> {
        if num_elements > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in tuple construction: need {num_elements} elements, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - num_elements;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        Ok(Value::Tuple(elements))
    }

    /// Builds a map from the top `num_elements` stack values.
    ///
    /// Elements are expected in alternating key-value order on the stack.
    fn build_map(&mut self, num_elements: usize) -> Result<Value> {
        if num_elements > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in map construction: need {num_elements} elements, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - num_elements;
        let mut pairs = IndexMap::with_capacity(num_elements / 2);
        for i in (start..self.sp).step_by(2) {
            let key = self.stack[i].clone();
            let value = self.stack[i + 1].clone();
            let key = Hashable::try_from(key).map_err(|e| self.vm_error(e.to_string()))?;
            pairs.insert(key, value);
        }
        self.sp = start;
        Ok(Value::Map(Map { pairs }))
    }

    /// Dispatches index operations to the appropriate handler.
    fn execute_index_expression(&mut self, container: Value, index: Value) -> Result<()> {
        match (&container, &index) {
            (Value::Vector(elements), _) => self.execute_vector_index(elements, &index),
            (Value::Map(map), _) => self.execute_map_index(map, index),
            _ => Err(self.vm_error(format!(
                "index operator not supported: {}",
                container.type_name()
            ))),
        }
    }

    /// Indexes into a vector with bounds checking.
    fn execute_vector_index(&mut self, elements: &[Value], index: &Value) -> Result<()> {
        if !index.is_integer() {
            return Err(self.vm_error(format!(
                "vector index must be an integer, got {}",
                index.type_name()
            )));
        }
        match index.to_vector_index() {
            Some(idx) if idx < elements.len() => self.push_stack(elements[idx].clone()),
            _ => Err(self.vm_error(format!(
                "index out of bounds: index is {index}, length is {}",
                elements.len()
            ))),
        }
    }

    /// Indexes into a map by key.
    fn execute_map_index(&mut self, map: &Map, index: Value) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| self.vm_error(e.to_string()))?;
        match map.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => Err(self.vm_error(format!("key not found: {key}"))),
        }
    }

    /// Constructs a struct or enum variant from stack values.
    ///
    /// The type registry index encodes whether this is a struct or an enum
    /// variant. For enums, the variant tag is packed into the high bits of
    /// the type index: `type_index = (registry_index << 8) | variant_tag`.
    fn execute_construct(&mut self, type_index: usize, num_fields: usize) -> Result<()> {
        let registry_index = type_index >> 8;
        let variant_tag = (type_index & 0xFF) as u16;
        if num_fields > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in construct: need {num_fields} fields, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - num_fields;
        let fields = self.stack[start..self.sp].to_vec();
        self.sp = start;
        let type_def = self.type_registry.get(registry_index).ok_or_else(|| {
            self.vm_error(format!(
                "type registry index out of bounds: {registry_index}"
            ))
        })?;
        let val = match type_def {
            TypeDef::Struct { .. } => Value::Struct(StructVal {
                type_index: registry_index as u16,
                fields,
            }),
            TypeDef::Enum { .. } => Value::EnumVariant(EnumVariantVal {
                type_index: registry_index as u16,
                tag: variant_tag,
                fields,
            }),
        };
        self.push_stack(val)
    }

    /// Reads a field from a struct or enum variant on top of the stack.
    fn execute_get_field(&mut self, field_index: usize) -> Result<()> {
        let val = self.pop_stack()?;
        let fields = match &val {
            Value::Struct(s) => &s.fields,
            Value::EnumVariant(v) => &v.fields,
            Value::Tuple(elems) => elems,
            _ => {
                return Err(self.vm_error(format!("cannot access field on {}", val.type_name())));
            }
        };
        let value = fields.get(field_index).cloned().ok_or_else(|| {
            self.vm_error(format!(
                "field index {field_index} out of bounds (value has {} fields)",
                fields.len()
            ))
        })?;
        self.push_stack(value)
    }

    /// Tests an enum variant's tag and conditionally jumps.
    ///
    /// Peeks at the top of the stack (does not pop). If the variant's tag
    /// matches `expected_tag`, execution continues to the next instruction.
    /// Otherwise, the instruction pointer jumps to `jump_target`.
    fn execute_match_tag(&mut self, expected_tag: usize, jump_target: usize) -> Result<()> {
        let val = self
            .stack
            .get(self.sp - 1)
            .ok_or_else(|| self.vm_error("stack underflow in match_tag"))?;
        let actual_tag = match val {
            Value::EnumVariant(v) => v.tag as usize,
            _ => {
                return Err(self.vm_error(format!(
                    "match_tag requires an enum variant, got {}",
                    val.type_name()
                )));
            }
        };
        if actual_tag != expected_tag {
            self.current_frame_mut()?.ip = jump_target as isize - 1;
        }
        Ok(())
    }
}
