//! Trace-generating virtual machine.
//!
//! [`TraceVM`] mirrors the standard `maat_vm::VM` dispatch loop but records
//! every instruction step into a [`TraceTable`]. Memory accesses use an
//! append-only physical address allocation scheme: every write receives a
//! fresh contiguous address, preserving the write-once invariant required
//! by the memory permutation argument. Physical address 0 is a sentinel
//! (value zero) for dummy reads before the first real memory operation.

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;
use maat_bytecode::{Bytecode, MAX_FRAMES, MAX_GLOBALS, MAX_STACK_SIZE, Opcode, TypeTag};
use maat_errors::{Result, VmError};
use maat_field::Felt;
use maat_runtime::{
    BUILTINS, Closure, CompiledFn, EnumVariantVal, FALSE, Hashable, Integer, Map, StructVal, TRUE,
    TypeDef, UNIT, Value, WideInt,
};
use maat_span::{SourceMap, Span};

use crate::encode::value_to_felt;
use crate::selector::{SEL_NOP, class_index};
use crate::table::{
    COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_NONZERO_INV, COL_OPCODE, COL_OPERAND_0,
    COL_OPERAND_1, COL_OUT, COL_PC, COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3, COL_RC_VAL, COL_S0,
    COL_S1, COL_S2, COL_SEL_BASE, COL_SP, TRACE_WIDTH, TraceRow, TraceTable,
};

/// Trace metadata captured for a single instruction step.
#[derive(Default)]
struct TraceData {
    /// Result value pushed to the stack (or zero).
    out: Felt,
    /// Flat memory address accessed.
    mem_addr: Felt,
    /// Memory value at `mem_addr`.
    mem_val: Felt,
    /// `1` if memory read, `0` if write.
    is_read: Felt,
    /// Whether this instruction performs a real memory access.
    has_mem_access: bool,
    /// Value being range-checked (zero on non-trigger rows).
    rc_val: Felt,
    /// 16-bit limb decomposition of `rc_val`.
    rc_limbs: [Felt; 4],
    /// Multiplicative inverse of the divisor on `sel_div_mod` rows.
    nonzero_inv: Felt,
}

/// Decomposes a 64-bit value into four 16-bit limbs.
///
/// Returns `[l0, l1, l2, l3]` such that
/// `val = l0 + 2^16 * l1 + 2^32 * l2 + 2^48 * l3`
/// with each limb in `[0, 2^16)`.
fn decompose_limbs(val: u64) -> [Felt; 4] {
    [
        Felt::new(val & 0xFFFF),
        Felt::new((val >> 16) & 0xFFFF),
        Felt::new((val >> 32) & 0xFFFF),
        Felt::new((val >> 48) & 0xFFFF),
    ]
}

/// A single call frame on the trace VM's frame stack.
#[derive(Debug, Clone)]
struct Frame {
    closure: Closure,
    ip: isize,
    base_pointer: usize,
    /// Number of local variable slots for this frame.
    num_locals: usize,
}

impl Frame {
    fn new(closure: Closure, base_pointer: usize) -> Self {
        let num_locals = closure.func.num_locals;
        Self {
            closure,
            ip: -1,
            base_pointer,
            num_locals,
        }
    }

    #[inline]
    fn instructions(&self) -> &[u8] {
        &self.closure.func.instructions
    }
}

/// Trace-generating virtual machine.
///
/// Executes bytecode identically to `maat_vm::VM` while recording a
/// 36-column execution trace. The trace records program counter, stack
/// state, memory accesses, opcode selectors, and range-check witness
/// data for each instruction step.
pub struct TraceVM {
    constants: Vec<Value>,
    stack: Vec<Value>,
    sp: usize,
    globals: Vec<Value>,
    frames: Vec<Frame>,
    source_map: SourceMap,
    type_registry: Vec<TypeDef>,

    trace: TraceTable,
    /// Next physical address to allocate. Starts at 1; address 0 is a
    /// sentinel with value zero that absorbs dummy reads before the
    /// first real memory operation.
    alloc_ptr: usize,
    /// Maps logical addresses to their latest physical address.
    /// Logical addresses: globals at `[0, MAX_GLOBALS)`, locals at
    /// `[fp, fp + frame_depth)`.
    addr_map: HashMap<usize, usize>,
    /// Current frame pointer (logical address space).
    fp: usize,
    /// Saved frame pointers for return.
    fp_stack: Vec<usize>,
    /// Last physical address accessed (for dummy reads on non-memory rows).
    last_mem_addr: Felt,
    /// Last memory value at `last_mem_addr`.
    last_mem_val: Felt,
}

impl TraceVM {
    /// Creates a new trace-generating VM from compiled bytecode.
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
            trace: TraceTable::new(),
            alloc_ptr: 1,
            addr_map: HashMap::new(),
            fp: MAX_GLOBALS,
            fp_stack: Vec::new(),
            last_mem_addr: Felt::ZERO,
            last_mem_val: Felt::ZERO,
        }
    }

    /// Consumes the VM and returns the completed trace table.
    pub fn into_trace(self) -> TraceTable {
        self.trace
    }

    /// Returns the last value popped from the stack.
    pub fn last_popped_stack_elem(&self) -> Option<&Value> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
        }
    }

    /// Reads the top N stack elements as felt values (without popping).
    fn peek_stack_felts(&self) -> (Felt, Felt, Felt) {
        let s0 = if self.sp >= 1 {
            value_to_felt(&self.stack[self.sp - 1])
        } else {
            Felt::ZERO
        };
        let s1 = if self.sp >= 2 {
            value_to_felt(&self.stack[self.sp - 2])
        } else {
            Felt::ZERO
        };
        let s2 = if self.sp >= 3 {
            value_to_felt(&self.stack[self.sp - 3])
        } else {
            Felt::ZERO
        };
        (s0, s1, s2)
    }

    /// Builds a trace row with the pre-instruction snapshot and selector.
    fn begin_row(&self, ip: usize, op: Opcode, operand0: usize, operand1: usize) -> TraceRow {
        let (s0, s1, s2) = self.peek_stack_felts();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(ip as u64);
        row[COL_SP] = Felt::new(self.sp as u64);
        row[COL_FP] = Felt::new(self.fp as u64);
        row[COL_OPCODE] = Felt::new(op.to_byte() as u64);
        row[COL_OPERAND_0] = Felt::new(operand0 as u64);
        row[COL_OPERAND_1] = Felt::new(operand1 as u64);
        row[COL_S0] = s0;
        row[COL_S1] = s1;
        row[COL_S2] = s2;

        let sel = class_index(op);
        row[COL_SEL_BASE + sel as usize] = Felt::ONE;
        row
    }

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

    fn vm_error(&self, message: impl Into<String>) -> maat_errors::Error {
        match self.current_span() {
            Some(span) => VmError::with_span(message, span).into(),
            None => VmError::new(message).into(),
        }
    }

    #[inline]
    fn current_frame(&self) -> Result<&Frame> {
        self.frames
            .last()
            .ok_or_else(|| self.vm_error("frame stack underflow"))
    }

    #[inline]
    fn current_frame_mut(&mut self) -> Result<&mut Frame> {
        self.frames
            .last_mut()
            .ok_or_else(|| VmError::new("frame stack underflow").into())
    }

    fn push_frame(&mut self, frame: Frame) -> Result<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(self.vm_error("stack overflow: maximum call depth exceeded"));
        }
        self.frames.push(frame);
        Ok(())
    }

    fn pop_frame(&mut self) -> Result<Frame> {
        if self.frames.len() <= 1 {
            return Err(self.vm_error("cannot return from top-level code"));
        }
        self.frames
            .pop()
            .ok_or_else(|| self.vm_error("frame stack underflow"))
    }

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

    fn pop_stack(&mut self) -> Result<Value> {
        if self.sp == 0 {
            return Err(self.vm_error("stack underflow"));
        }
        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    fn pop_integer(&mut self, context: &str) -> Result<Integer> {
        match self.pop_stack()? {
            Value::Integer(v) => Ok(v),
            other => Err(self.vm_error(format!(
                "{context} bounds must be integer, got {}",
                other.type_name()
            ))),
        }
    }

    fn pop_felt(&mut self, context: &str) -> Result<Felt> {
        match self.pop_stack()? {
            Value::Felt(f) => Ok(f),
            other => Err(self.vm_error(format!(
                "{context} expects Felt operand, got {}",
                other.type_name()
            ))),
        }
    }

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

    #[inline]
    fn read_u8_operand(&self, offset: usize) -> Result<usize> {
        self.current_frame()?
            .instructions()
            .get(offset)
            .map(|&b| b as usize)
            .ok_or_else(|| self.vm_error("instruction stream truncated: missing operand byte"))
    }

    /// Executes the bytecode and records the execution trace.
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

            let (operand0, operand1) = self.read_operands(ip, op)?;
            let mut row = self.begin_row(ip, op, operand0, operand1);

            let trace_info = self.dispatch(op, ip)?;

            row[COL_OUT] = trace_info.out;

            if trace_info.has_mem_access {
                row[COL_MEM_ADDR] = trace_info.mem_addr;
                row[COL_MEM_VAL] = trace_info.mem_val;
                row[COL_IS_READ] = trace_info.is_read;
                self.last_mem_addr = trace_info.mem_addr;
                self.last_mem_val = trace_info.mem_val;
            } else {
                // Dummy read from the last-accessed address preserves
                // memory consistency in the sorted permutation argument.
                row[COL_MEM_ADDR] = self.last_mem_addr;
                row[COL_MEM_VAL] = self.last_mem_val;
                row[COL_IS_READ] = Felt::ONE;
            }

            row[COL_RC_VAL] = trace_info.rc_val;
            row[COL_RC_L0] = trace_info.rc_limbs[0];
            row[COL_RC_L1] = trace_info.rc_limbs[1];
            row[COL_RC_L2] = trace_info.rc_limbs[2];
            row[COL_RC_L3] = trace_info.rc_limbs[3];
            row[COL_NONZERO_INV] = trace_info.nonzero_inv;

            self.trace.push_row(row);
        }

        self.emit_final_row();

        Ok(())
    }

    /// Emits a NOP row reflecting the VM state after the last instruction.
    ///
    /// The execution trace records pre-instruction state; this row makes the
    /// post-execution state (final SP, PC, etc.) visible so that the last
    /// real instruction's transition constraints (e.g. `sel_store * (sp_next - sp + 1)`)
    /// evaluate against the correct successor values.
    fn emit_final_row(&mut self) {
        let ip = self.current_frame().map(|f| f.ip as usize).unwrap_or(0);
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(ip as u64);
        row[COL_SP] = Felt::new(self.sp as u64);
        row[COL_FP] = Felt::new(self.fp as u64);
        row[COL_SEL_BASE + SEL_NOP as usize] = Felt::ONE;
        // Dummy memory read from last-accessed location.
        row[COL_MEM_ADDR] = self.last_mem_addr;
        row[COL_MEM_VAL] = self.last_mem_val;
        row[COL_IS_READ] = Felt::ONE;
        self.trace.push_row(row);
    }

    /// Reads operands for the given opcode without advancing `ip`.
    fn read_operands(&self, ip: usize, op: Opcode) -> Result<(usize, usize)> {
        let widths = op.operand_widths();
        let mut operand0 = 0;
        let mut operand1 = 0;
        let mut offset = ip + 1;

        if !widths.is_empty() {
            operand0 = match widths[0] {
                1 => self.read_u8_operand(offset)?,
                2 => self.read_u16_operand(offset)?,
                _ => 0,
            };
            offset += widths[0];
        }
        if widths.len() > 1 {
            operand1 = match widths[1] {
                1 => self.read_u8_operand(offset)?,
                2 => self.read_u16_operand(offset)?,
                _ => 0,
            };
        }
        Ok((operand0, operand1))
    }

    /// Dispatches a single opcode and returns trace metadata for the row.
    fn dispatch(&mut self, op: Opcode, ip: usize) -> Result<TraceData> {
        let mut info = TraceData::default();

        match op {
            Opcode::Constant => {
                let index = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let constant = self.constants.get(index).cloned().ok_or_else(|| {
                    self.vm_error(format!(
                        "constant pool access out of bounds at index {index}"
                    ))
                })?;
                info.out = value_to_felt(&constant);
                self.push_stack(constant)?;
            }
            Opcode::Pop => {
                self.pop_stack()?;
            }
            Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::BitAnd
            | Opcode::BitOr
            | Opcode::BitXor
            | Opcode::Shl
            | Opcode::Shr => {
                self.execute_binary_operation(op)?;
                info.out = self.peek_top_felt();
            }
            Opcode::Div | Opcode::Mod => {
                let divisor_felt = self.peek_top_felt();
                self.execute_binary_operation(op)?;
                info.out = self.peek_top_felt();

                if divisor_felt != Felt::ZERO {
                    info.nonzero_inv = divisor_felt
                        .inv()
                        .expect("non-zero field element has inverse");
                }
            }
            Opcode::True => {
                self.push_stack(TRUE)?;
                info.out = Felt::ONE;
            }
            Opcode::False => {
                self.push_stack(FALSE)?;
            }
            Opcode::Equal | Opcode::NotEqual | Opcode::GreaterThan | Opcode::LessThan => {
                self.execute_comparison(op)?;
                info.out = self.peek_top_felt();
            }
            Opcode::Bang => {
                self.execute_bang_operator()?;
                info.out = self.peek_top_felt();
            }
            Opcode::Minus => {
                self.execute_minus_operator()?;
                info.out = self.peek_top_felt();
            }
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
                let felt = value_to_felt(&value);
                if index >= self.globals.len() {
                    self.globals.resize(index + 1, Value::Unit);
                }
                self.globals[index] = value;

                let physical = self.alloc_ptr;
                self.alloc_ptr += 1;
                self.addr_map.insert(index, physical);
                info.mem_addr = Felt::new(physical as u64);
                info.mem_val = felt;
                info.has_mem_access = true;
            }
            Opcode::GetGlobal => {
                let index = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let value = self.globals.get(index).cloned().ok_or_else(|| {
                    self.vm_error(format!("undefined global variable at index {index}"))
                })?;
                let felt = value_to_felt(&value);
                self.push_stack(value)?;

                let physical = self.addr_map[&index];
                info.out = felt;
                info.mem_addr = Felt::new(physical as u64);
                info.mem_val = felt;
                info.is_read = Felt::ONE;
                info.has_mem_access = true;
            }
            Opcode::SetLocal => {
                let local_index = self.read_u8_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 1;
                let base_pointer = self.current_frame()?.base_pointer;
                let value = self.pop_stack()?;
                let felt = value_to_felt(&value);
                let slot = base_pointer + local_index;
                if slot >= self.stack.len() {
                    self.stack.resize(slot + 1, Value::Unit);
                }
                self.stack[slot] = value;

                let logical = self.fp + local_index;
                let physical = self.alloc_ptr;
                self.alloc_ptr += 1;
                self.addr_map.insert(logical, physical);
                info.mem_addr = Felt::new(physical as u64);
                info.mem_val = felt;
                info.has_mem_access = true;
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
                let felt = value_to_felt(&value);
                self.push_stack(value)?;

                let logical = self.fp + local_index;
                let physical = self.addr_map[&logical];
                info.out = felt;
                info.mem_addr = Felt::new(physical as u64);
                info.mem_val = felt;
                info.is_read = Felt::ONE;
                info.has_mem_access = true;
            }
            Opcode::Vector => {
                let n = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let v = self.build_collection(n, Value::Vector)?;
                self.push_stack(v)?;
            }
            Opcode::Tuple => {
                let n = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let v = self.build_collection(n, Value::Tuple)?;
                self.push_stack(v)?;
            }
            Opcode::Array => {
                let n = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let v = self.build_collection(n, Value::Array)?;
                self.push_stack(v)?;
            }
            Opcode::Map => {
                let n = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let map = self.build_map(n)?;
                self.push_stack(map)?;
            }
            Opcode::Index => {
                let index = self.pop_stack()?;
                let container = self.pop_stack()?;
                self.execute_index_expression(container, index)?;
                info.out = self.peek_top_felt();
            }
            Opcode::Call => {
                let num_args = self.read_u8_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 1;
                self.execute_function_call(num_args, &mut info)?;
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
                        self.stack
                            .get(base + i)
                            .cloned()
                            .ok_or_else(|| self.vm_error("stack underflow reading free variables"))
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
                info.out = value_to_felt(&value);
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
                info.out = self.peek_top_felt();
                // Range-check: prove the converted value fits its target type.
                let rc = info.out.as_u64();
                info.rc_val = info.out;
                info.rc_limbs = decompose_limbs(rc);
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
                info.out = self.peek_top_felt();
            }
            Opcode::MatchTag => {
                let expected_tag = self.read_u16_operand(ip + 1)?;
                let jump_target = self.read_u16_operand(ip + 3)?;
                self.current_frame_mut()?.ip += 4;
                self.execute_match_tag(expected_tag, jump_target)?;
            }
            Opcode::ReturnValue => {
                let return_value = self.pop_stack()?;
                info.out = value_to_felt(&return_value);
                let frame = self.pop_frame()?;
                self.sp = frame.base_pointer.saturating_sub(1);
                self.push_stack(return_value)?;
                self.fp = self.fp_stack.pop().unwrap_or(MAX_GLOBALS);
            }
            Opcode::Return => {
                let frame = self.pop_frame()?;
                self.sp = frame.base_pointer.saturating_sub(1);
                self.push_stack(UNIT)?;
                self.fp = self.fp_stack.pop().unwrap_or(MAX_GLOBALS);
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
                info.out = self.peek_top_felt();
            }
            Opcode::FeltInv => {
                self.execute_felt_inv()?;
                info.out = self.peek_top_felt();
            }
            Opcode::FeltPow => {
                self.execute_felt_pow()?;
                info.out = self.peek_top_felt();
            }
        }

        Ok(info)
    }

    /// Peeks at the top of the stack as a felt (without popping).
    fn peek_top_felt(&self) -> Felt {
        if self.sp >= 1 {
            value_to_felt(&self.stack[self.sp - 1])
        } else {
            Felt::ZERO
        }
    }

    fn execute_function_call(&mut self, num_args: usize, info: &mut TraceData) -> Result<()> {
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
            Value::Builtin(f) => self.call_builtin_fn(f, num_args, info),
            _ => Err(self.vm_error("calling non-function")),
        }
    }

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

        self.fp_stack.push(self.fp);
        let new_fp = self.fp + self.current_frame()?.num_locals;
        self.fp = new_fp;
        Ok(())
    }

    fn call_builtin_fn(
        &mut self,
        func: fn(&[Value]) -> Result<Value>,
        num_args: usize,
        info: &mut TraceData,
    ) -> Result<()> {
        let args_start = self.sp - num_args;
        let args = self.stack[args_start..self.sp].to_vec();
        let result = func(&args)?;

        // Builtin calls don't change FP. Store current FP in `out` for
        // constraint 33: fp_next = out.
        info.out = Felt::new(self.fp as u64);

        self.sp = args_start - 1;
        self.push_stack(result)?;
        Ok(())
    }

    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        let (l_name, r_name) = (left.type_name(), right.type_name());

        if op == Opcode::Add
            && let (Value::Str(l), Value::Str(r)) = (&left, &right)
        {
            return self.push_stack(Value::Str(format!("{}{}", l, r)));
        }

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

    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        if let Some(result) = self.compare_ordered(op, &left, &right) {
            return self.push_stack(Value::Bool(result));
        }
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

    fn execute_bang_operator(&mut self) -> Result<()> {
        let result = match self.pop_stack()? {
            Value::Bool(b) => Value::Bool(!b),
            other => {
                return Err(self.vm_error(format!("cannot apply `!` to {}", other.type_name())));
            }
        };
        self.push_stack(result)
    }

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

    fn execute_convert(&mut self, tag_byte: usize) -> Result<()> {
        let tag = TypeTag::from_byte(tag_byte as u8)
            .ok_or_else(|| self.vm_error(format!("unknown type tag: {tag_byte}")))?;
        let value = self.pop_stack()?;
        let converted = self.convert_value(&value, tag)?;
        self.push_stack(converted)
    }

    fn convert_value(&self, value: &Value, target: TypeTag) -> Result<Value> {
        if target == TypeTag::Char {
            return match value {
                Value::Integer(val) => {
                    let scalar = match val.to_wide() {
                        WideInt::Signed(v) => u32::try_from(v).ok().and_then(char::from_u32),
                        WideInt::Unsigned(v) => u32::try_from(v).ok().and_then(char::from_u32),
                    };
                    scalar.map(Value::Char).ok_or_else(|| {
                        self.vm_error(format!("value {} is not a valid Unicode scalar value", val))
                    })
                }
                other => Err(self.vm_error(format!("cannot cast {} as char", other.type_name()))),
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

    fn execute_felt_binop(&mut self, op: Opcode) -> Result<()> {
        let rhs = self.pop_felt("Felt arithmetic")?;
        let lhs = self.pop_felt("Felt arithmetic")?;
        let result = match op {
            Opcode::FeltAdd => lhs + rhs,
            Opcode::FeltSub => lhs - rhs,
            Opcode::FeltMul => lhs * rhs,
            _ => unreachable!("non-Felt opcode in execute_felt_binop"),
        };
        self.push_stack(Value::Felt(result))
    }

    fn execute_felt_inv(&mut self) -> Result<()> {
        let operand = self.pop_felt("Felt inverse")?;
        let inv = operand
            .inv()
            .map_err(|e| self.vm_error(format!("Felt inverse error: {e}")))?;
        self.push_stack(Value::Felt(inv))
    }

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

    /// Builds a vector, tuple, or array from the top N stack elements.
    fn build_collection(&mut self, n: usize, ctor: fn(Vec<Value>) -> Value) -> Result<Value> {
        if n > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in collection construction: need {n} elements, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - n;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        Ok(ctor(elements))
    }

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

    fn execute_index_expression(&mut self, container: Value, index: Value) -> Result<()> {
        match (&container, &index) {
            (Value::Vector(elements) | Value::Array(elements), _) => {
                self.execute_vector_index(elements, &index)
            }
            (Value::Map(map), _) => self.execute_map_index(map, index),
            _ => Err(self.vm_error(format!(
                "index operator not supported: {}",
                container.type_name()
            ))),
        }
    }

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

    fn execute_map_index(&mut self, map: &Map, index: Value) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| self.vm_error(e.to_string()))?;
        match map.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => Err(self.vm_error(format!("key not found: {key}"))),
        }
    }

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
