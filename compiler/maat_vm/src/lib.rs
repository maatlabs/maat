//! Virtual machine for executing Maat bytecode.
//!
//! This crate implements a stack-based virtual machine that executes
//! compiled bytecode instructions. The VM uses call frames for function
//! invocations, maintaining a value stack, globals store, and frame stack.

use std::rc::Rc;

use indexmap::IndexMap;
use maat_bytecode::{Bytecode, MAX_FRAMES, MAX_GLOBALS, MAX_STACK_SIZE, Opcode, TypeTag};
use maat_errors::{Result, VmError};
use maat_runtime::{
    BUILTINS, Closure, CompiledFunction, EnumVariantObject, FALSE, HashObject, Hashable, NULL,
    Object, StructObject, TRUE, TypeDef,
};
use maat_span::{SourceMap, Span};

/// Dispatches checked integer arithmetic across all 12 integer variants.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operands are not the same integer type
/// - inner `None`: arithmetic overflow
macro_rules! int_binop {
    ($left:expr, $right:expr, $method:ident) => {
        match ($left, $right) {
            (Object::I8(l), Object::I8(r)) => Some(l.$method(*r).map(Object::I8)),
            (Object::I16(l), Object::I16(r)) => Some(l.$method(*r).map(Object::I16)),
            (Object::I32(l), Object::I32(r)) => Some(l.$method(*r).map(Object::I32)),
            (Object::I64(l), Object::I64(r)) => Some(l.$method(*r).map(Object::I64)),
            (Object::I128(l), Object::I128(r)) => Some(l.$method(*r).map(Object::I128)),
            (Object::Isize(l), Object::Isize(r)) => Some(l.$method(*r).map(Object::Isize)),
            (Object::U8(l), Object::U8(r)) => Some(l.$method(*r).map(Object::U8)),
            (Object::U16(l), Object::U16(r)) => Some(l.$method(*r).map(Object::U16)),
            (Object::U32(l), Object::U32(r)) => Some(l.$method(*r).map(Object::U32)),
            (Object::U64(l), Object::U64(r)) => Some(l.$method(*r).map(Object::U64)),
            (Object::U128(l), Object::U128(r)) => Some(l.$method(*r).map(Object::U128)),
            (Object::Usize(l), Object::Usize(r)) => Some(l.$method(*r).map(Object::Usize)),
            _ => None,
        }
    };
}

/// Dispatches ordered comparison across all 12 integer variants.
///
/// Returns `None` if the operands are not the same integer type.
macro_rules! int_cmp {
    ($left:expr, $right:expr, $op:tt) => {
        match ($left, $right) {
            (Object::I8(l), Object::I8(r)) => Some(*l $op *r),
            (Object::I16(l), Object::I16(r)) => Some(*l $op *r),
            (Object::I32(l), Object::I32(r)) => Some(*l $op *r),
            (Object::I64(l), Object::I64(r)) => Some(*l $op *r),
            (Object::I128(l), Object::I128(r)) => Some(*l $op *r),
            (Object::Isize(l), Object::Isize(r)) => Some(*l $op *r),
            (Object::U8(l), Object::U8(r)) => Some(*l $op *r),
            (Object::U16(l), Object::U16(r)) => Some(*l $op *r),
            (Object::U32(l), Object::U32(r)) => Some(*l $op *r),
            (Object::U64(l), Object::U64(r)) => Some(*l $op *r),
            (Object::U128(l), Object::U128(r)) => Some(*l $op *r),
            (Object::Usize(l), Object::Usize(r)) => Some(*l $op *r),
            _ => None,
        }
    };
}

/// Dispatches checked negation for signed integer types.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operand is not a signed integer
/// - inner `None`: negation overflow (e.g., `i8::MIN`)
macro_rules! checked_neg {
    ($val:expr) => {
        match $val {
            Object::I8(v) => Some(v.checked_neg().map(Object::I8)),
            Object::I16(v) => Some(v.checked_neg().map(Object::I16)),
            Object::I32(v) => Some(v.checked_neg().map(Object::I32)),
            Object::I64(v) => Some(v.checked_neg().map(Object::I64)),
            Object::I128(v) => Some(v.checked_neg().map(Object::I128)),
            Object::Isize(v) => Some(v.checked_neg().map(Object::Isize)),
            _ => None,
        }
    };
}

/// Dispatches a bitwise binary operation across all 12 integer variants.
///
/// Returns `Option<Object>`:
/// - `None`: operands are not the same integer type
/// - `Some(result)`: the result of the operation
macro_rules! int_bitwise {
    ($left:expr, $right:expr, $op:tt) => {
        match ($left, $right) {
            (Object::I8(l), Object::I8(r)) => Some(Object::I8(*l $op *r)),
            (Object::I16(l), Object::I16(r)) => Some(Object::I16(*l $op *r)),
            (Object::I32(l), Object::I32(r)) => Some(Object::I32(*l $op *r)),
            (Object::I64(l), Object::I64(r)) => Some(Object::I64(*l $op *r)),
            (Object::I128(l), Object::I128(r)) => Some(Object::I128(*l $op *r)),
            (Object::Isize(l), Object::Isize(r)) => Some(Object::Isize(*l $op *r)),
            (Object::U8(l), Object::U8(r)) => Some(Object::U8(*l $op *r)),
            (Object::U16(l), Object::U16(r)) => Some(Object::U16(*l $op *r)),
            (Object::U32(l), Object::U32(r)) => Some(Object::U32(*l $op *r)),
            (Object::U64(l), Object::U64(r)) => Some(Object::U64(*l $op *r)),
            (Object::U128(l), Object::U128(r)) => Some(Object::U128(*l $op *r)),
            (Object::Usize(l), Object::Usize(r)) => Some(Object::Usize(*l $op *r)),
            _ => None,
        }
    };
}

/// Dispatches a checked shift operation across all 12 integer variants.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operands are not the same integer type
/// - inner `None`: shift amount too large
macro_rules! int_shift {
    ($left:expr, $right:expr, $method:ident) => {
        match ($left, $right) {
            (Object::I8(l), Object::I8(r)) => Some(l.$method(*r as u32).map(Object::I8)),
            (Object::I16(l), Object::I16(r)) => Some(l.$method(*r as u32).map(Object::I16)),
            (Object::I32(l), Object::I32(r)) => Some(l.$method(*r as u32).map(Object::I32)),
            (Object::I64(l), Object::I64(r)) => Some(l.$method(*r as u32).map(Object::I64)),
            (Object::I128(l), Object::I128(r)) => Some(l.$method(*r as u32).map(Object::I128)),
            (Object::Isize(l), Object::Isize(r)) => Some(l.$method(*r as u32).map(Object::Isize)),
            (Object::U8(l), Object::U8(r)) => Some(l.$method(*r as u32).map(Object::U8)),
            (Object::U16(l), Object::U16(r)) => Some(l.$method(*r as u32).map(Object::U16)),
            (Object::U32(l), Object::U32(r)) => Some(l.$method(*r as u32).map(Object::U32)),
            (Object::U64(l), Object::U64(r)) => Some(l.$method(*r as u32).map(Object::U64)),
            (Object::U128(l), Object::U128(r)) => Some(l.$method(*r as u32).map(Object::U128)),
            (Object::Usize(l), Object::Usize(r)) => Some(l.$method(*r as u32).map(Object::Usize)),
            _ => None,
        }
    };
}

/// Intermediate representation for type conversion.
///
/// All numeric source values are widened into one of these variants
/// before narrowing to the target type, simplifying the conversion matrix.
enum WideValue {
    Int(i128),
    Uint(u128),
}

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
            func: CompiledFunction {
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
    pub fn with_globals(bytecode: Bytecode, globals: Vec<Object>) -> Self {
        let source_map = bytecode.source_map;
        let type_registry = bytecode.type_registry;
        let main_closure = Closure {
            func: CompiledFunction {
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
    pub fn globals(&self) -> &[Object] {
        &self.globals
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
                        self.vm_error(format!("undefined global variable at index {index}"))
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
                        self.vm_error(format!("builtin index out of bounds: {index}"))
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
                            return Err(self.vm_error(format!(
                                "expected CompiledFunction at constant pool index {const_index}"
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
                            self.vm_error(format!("free variable index out of bounds: {index}"))
                        })?;
                    self.push_stack(value)?;
                }
                Opcode::CurrentClosure => {
                    let closure = self.current_frame()?.closure.clone();
                    self.push_stack(Object::Closure(closure))?;
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
                        self.vm_error(format!(
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
            Object::Closure(cl) => self.call_closure(cl, num_args),
            Object::Builtin(f) => self.call_builtin_fn(f, num_args),
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
            return Err(self.vm_error("stack overflow"));
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
            return Err(self.vm_error("stack underflow"));
        }

        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    /// Executes a binary arithmetic, bitwise, or shift operation across all numeric types.
    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

        if let (Object::Str(l), Object::Str(r)) = (&left, &right) {
            return self.execute_binary_string_operation(op, l, r);
        }

        let checked_result = match op {
            Opcode::Add => int_binop!(&left, &right, checked_add),
            Opcode::Sub => int_binop!(&left, &right, checked_sub),
            Opcode::Mul => int_binop!(&left, &right, checked_mul),
            Opcode::Div => int_binop!(&left, &right, checked_div),
            Opcode::Mod => int_binop!(&left, &right, checked_rem),
            Opcode::Shl => int_shift!(&left, &right, checked_shl),
            Opcode::Shr => int_shift!(&left, &right, checked_shr),
            _ => None,
        };
        if let Some(maybe_val) = checked_result {
            let val = maybe_val.ok_or_else(|| self.vm_error("integer arithmetic overflow"))?;
            return self.push_stack(val);
        }

        let bitwise_result = match op {
            Opcode::BitAnd => int_bitwise!(&left, &right, &),
            Opcode::BitOr => int_bitwise!(&left, &right, |),
            Opcode::BitXor => int_bitwise!(&left, &right, ^),
            _ => None,
        };
        if let Some(val) = bitwise_result {
            return self.push_stack(val);
        }

        Err(self.vm_error(format!(
            "unsupported types for binary operation: {} {}",
            left.type_name(),
            right.type_name()
        )))
    }

    /// Executes a comparison operation across all numeric and non-numeric types.
    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

        // Same-type numeric comparison
        if let Some(result) = self.compare_numeric(op, &left, &right) {
            return self.push_stack(Object::Bool(result));
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
            return self.push_stack(Object::Bool(result));
        }

        // Non-numeric equality (booleans, strings, arrays, etc.)
        match op {
            Opcode::Equal => self.push_stack(Object::Bool(left == right)),
            Opcode::NotEqual => self.push_stack(Object::Bool(left != right)),
            _ => Err(self.vm_error(format!(
                "unsupported comparison: {:?} ({} {})",
                op,
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    /// Attempts same-type numeric comparison across all integer variants.
    ///
    /// Returns `None` if the operands are not the same integer type.
    fn compare_numeric(&self, op: Opcode, left: &Object, right: &Object) -> Option<bool> {
        match op {
            Opcode::Equal => int_cmp!(left, right, ==),
            Opcode::NotEqual => int_cmp!(left, right, !=),
            Opcode::GreaterThan => int_cmp!(left, right, >),
            Opcode::LessThan => int_cmp!(left, right, <),
            _ => None,
        }
    }

    /// Executes the logical NOT (bang) operator.
    fn execute_bang_operator(&mut self) -> Result<()> {
        let result = match self.pop_stack()? {
            Object::Bool(false) | Object::Null => TRUE,
            _ => FALSE,
        };
        self.push_stack(result)
    }

    /// Executes the unary minus operator for all signed integer types.
    fn execute_minus_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;

        if let Some(result) = checked_neg!(&operand) {
            let val = result.ok_or_else(|| self.vm_error("integer negation overflow"))?;
            return self.push_stack(val);
        }

        Err(self.vm_error(format!(
            "unsupported type for negation: {}",
            operand.type_name()
        )))
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
    fn convert_value(&self, value: &Object, target: TypeTag) -> Result<Object> {
        let wide = self.to_wide_value(value)?;

        match target {
            TypeTag::I8 => self.narrow_int::<i8>(wide, "i8").map(Object::I8),
            TypeTag::I16 => self.narrow_int::<i16>(wide, "i16").map(Object::I16),
            TypeTag::I32 => self.narrow_int::<i32>(wide, "i32").map(Object::I32),
            TypeTag::I64 => self.narrow_int::<i64>(wide, "i64").map(Object::I64),
            TypeTag::I128 => self.narrow_int::<i128>(wide, "i128").map(Object::I128),
            TypeTag::Isize => self.narrow_int::<isize>(wide, "isize").map(Object::Isize),
            TypeTag::U8 => self.narrow_int::<u8>(wide, "u8").map(Object::U8),
            TypeTag::U16 => self.narrow_int::<u16>(wide, "u16").map(Object::U16),
            TypeTag::U32 => self.narrow_int::<u32>(wide, "u32").map(Object::U32),
            TypeTag::U64 => self.narrow_int::<u64>(wide, "u64").map(Object::U64),
            TypeTag::U128 => self.narrow_int::<u128>(wide, "u128").map(Object::U128),
            TypeTag::Usize => self.narrow_int::<usize>(wide, "usize").map(Object::Usize),
        }
    }

    /// Extracts a widened value for conversion dispatch.
    ///
    /// Integer types are widened to `WideValue::Int(i128)` or `WideValue::Uint(u128)`.
    /// Float types are preserved for float-specific conversion.
    fn to_wide_value(&self, value: &Object) -> Result<WideValue> {
        match value {
            Object::I8(v) => Ok(WideValue::Int(*v as i128)),
            Object::I16(v) => Ok(WideValue::Int(*v as i128)),
            Object::I32(v) => Ok(WideValue::Int(*v as i128)),
            Object::I64(v) => Ok(WideValue::Int(*v as i128)),
            Object::I128(v) => Ok(WideValue::Int(*v)),
            Object::Isize(v) => Ok(WideValue::Int(*v as i128)),
            Object::U8(v) => Ok(WideValue::Uint(*v as u128)),
            Object::U16(v) => Ok(WideValue::Uint(*v as u128)),
            Object::U32(v) => Ok(WideValue::Uint(*v as u128)),
            Object::U64(v) => Ok(WideValue::Uint(*v as u128)),
            Object::U128(v) => Ok(WideValue::Uint(*v)),
            Object::Usize(v) => Ok(WideValue::Uint(*v as u128)),
            _ => Err(self.vm_error(format!(
                "cannot cast {} to a numeric type",
                value.type_name()
            ))),
        }
    }

    /// Narrows a wide value to a target integer type, rejecting out-of-range values.
    fn narrow_int<T>(&self, wide: WideValue, type_name: &str) -> Result<T>
    where
        T: TryFrom<i128> + TryFrom<u128>,
    {
        match wide {
            WideValue::Int(v) => T::try_from(v)
                .map_err(|_| self.vm_error(format!("value {v} out of range for {type_name}"))),
            WideValue::Uint(v) => T::try_from(v)
                .map_err(|_| self.vm_error(format!("value {v} out of range for {type_name}"))),
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
            return Err(self.vm_error(format!("unknown string operator: {}", op.name())));
        }
        let mut result = String::with_capacity(left.len() + right.len());
        result.push_str(left);
        result.push_str(right);
        self.push_stack(Object::Str(result))
    }

    /// Builds an array object from the top `num_elements` stack values.
    fn build_array(&mut self, num_elements: usize) -> Result<Object> {
        if num_elements > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in array construction: need {num_elements} elements, stack has {}",
                self.sp
            )));
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
            return Err(self.vm_error(format!(
                "stack underflow in hash construction: need {num_elements} elements, stack has {}",
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
        Ok(Object::Hash(HashObject { pairs }))
    }

    /// Dispatches index operations to the appropriate handler.
    fn execute_index_expression(&mut self, container: Object, index: Object) -> Result<()> {
        match (&container, &index) {
            (Object::Array(elements), _) => self.execute_array_index(elements, &index),
            (Object::Hash(hash), _) => self.execute_hash_index(hash, index),
            _ => Err(self.vm_error(format!(
                "index operator not supported: {}",
                container.type_name()
            ))),
        }
    }

    /// Indexes into an array with bounds checking.
    fn execute_array_index(&mut self, elements: &[Object], index: &Object) -> Result<()> {
        if !index.is_integer() {
            return Err(self.vm_error(format!(
                "array index must be an integer, got {}",
                index.type_name()
            )));
        }

        match index.to_array_index() {
            Some(idx) if idx < elements.len() => self.push_stack(elements[idx].clone()),
            _ => self.push_stack(NULL),
        }
    }

    /// Indexes into a hash by key.
    fn execute_hash_index(&mut self, hash: &HashObject, index: Object) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| self.vm_error(e.to_string()))?;

        match hash.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => self.push_stack(NULL),
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

        let obj = match type_def {
            TypeDef::Struct { .. } => Object::Struct(StructObject {
                type_index: registry_index as u16,
                fields,
            }),
            TypeDef::Enum { .. } => Object::EnumVariant(EnumVariantObject {
                type_index: registry_index as u16,
                tag: variant_tag,
                fields,
            }),
        };

        self.push_stack(obj)
    }

    /// Reads a field from a struct or enum variant on top of the stack.
    fn execute_get_field(&mut self, field_index: usize) -> Result<()> {
        let obj = self.pop_stack()?;
        let fields = match &obj {
            Object::Struct(s) => &s.fields,
            Object::EnumVariant(v) => &v.fields,
            _ => {
                return Err(self.vm_error(format!("cannot access field on {}", obj.type_name())));
            }
        };

        let value = fields.get(field_index).cloned().ok_or_else(|| {
            self.vm_error(format!(
                "field index {field_index} out of bounds (object has {} fields)",
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
        let obj = self
            .stack
            .get(self.sp - 1)
            .ok_or_else(|| self.vm_error("stack underflow in match_tag"))?;

        let actual_tag = match obj {
            Object::EnumVariant(v) => v.tag as usize,
            _ => {
                return Err(self.vm_error(format!(
                    "match_tag requires an enum variant, got {}",
                    obj.type_name()
                )));
            }
        };

        if actual_tag != expected_tag {
            self.current_frame_mut()?.ip = jump_target as isize - 1;
        }
        Ok(())
    }
}
