//! Stack-based virtual machine for executing Maat bytecode.
//!
//! The VM uses a stack-based architecture with call frames. Operands are
//! pushed onto a value stack, operations pop operands and push results, and
//! function calls create new frames with their own instruction pointers.

#![forbid(unsafe_code)]

pub mod trace;

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;
use maat_bytecode::{Bytecode, Constant, MAX_FRAMES, MAX_GLOBALS, MAX_STACK_SIZE, Opcode, TypeTag};
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement, from_i64, try_inv};
use maat_runtime::{
    BUILTINS, BuiltinArg, BuiltinFn, BuiltinReturn, Closure, CompiledFn, EnumVariantVal, FALSE,
    Hashable, Integer, Map, MaybeRelocatable, MemorySegmentManager, Relocatable, Set, StructVal,
    TRUE, TypeDef, UNIT, Value, WideInt,
};
use maat_span::{SourceMap, Span};

use crate::trace::{CallCtx, DispatchCtx, NoOpRecorder, Tracer};

#[derive(Debug, Clone)]
struct Frame {
    closure: Closure,
    ip: isize,
    base_pointer: usize,
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

#[derive(Debug)]
pub struct VM {
    constants: Vec<Constant>,
    stack: Vec<Value>,
    sp: usize,
    globals: Vec<Value>,
    frames: Vec<Frame>,
    source_map: SourceMap,
    type_registry: Vec<TypeDef>,
    segments: MemorySegmentManager,
    heap_values: HashMap<Relocatable, Value>,
    current_segment: Option<u32>,
    default_segment: Option<u32>,
}

impl VM {
    pub fn new(bytecode: Bytecode) -> Self {
        Self::with_globals(bytecode, Vec::with_capacity(MAX_GLOBALS))
    }

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
            segments: MemorySegmentManager::with_reserved_segments(),
            heap_values: HashMap::new(),
            current_segment: None,
            default_segment: None,
        }
    }

    pub fn globals(&self) -> &[Value] {
        &self.globals
    }

    pub fn segments(&self) -> &MemorySegmentManager {
        &self.segments
    }

    pub fn last_popped_stack_elem(&self) -> Option<&Value> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
        }
    }

    /// Reads the elements of a segment-backed `Value::Vector { base, len }`
    /// out of the heap and returns them as a `Vec<Value>`. Returns `None`
    /// for non-Vector values.
    pub fn inspect_vector(&self, value: &Value) -> Result<Option<Vec<Value>>> {
        match value {
            Value::Vector { base, len } => self.read_vector_cells(*base, *len).map(Some),
            _ => Ok(None),
        }
    }

    pub fn materialize_for_inspection(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Vector { base, len } => {
                let cells = self.read_vector_cells(*base, *len)?;
                Ok(Value::Array(cells))
            }
            other => Ok(other.clone()),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.run_with_recorder(&mut NoOpRecorder)
    }

    pub fn run_with_recorder<R: Tracer>(&mut self, recorder: &mut R) -> Result<()> {
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
            let (s0_mr, s1_mr, s2_mr) = self.peek_stack_maybe_reloc();
            let s0 = s0_mr.as_felt().unwrap_or(Felt::ZERO);
            let s1 = s1_mr.as_felt().unwrap_or(Felt::ZERO);
            let s2 = s2_mr.as_felt().unwrap_or(Felt::ZERO);
            recorder.before_dispatch(DispatchCtx {
                ip,
                op,
                operand0,
                operand1,
                sp: self.sp,
                s0,
                s1,
                s2,
                s0_reloc: s0_mr.as_relocatable(),
                s1_reloc: s1_mr.as_relocatable(),
                s2_reloc: s2_mr.as_relocatable(),
            });

            self.dispatch(op, ip, s0, s1, recorder)?;

            recorder.end_row();
        }

        let final_pc = self
            .current_frame()
            .map(|f| (f.ip + 1) as usize)
            .unwrap_or(0);
        recorder.finalize(final_pc, self.sp);

        Ok(())
    }

    fn dispatch<R: Tracer>(
        &mut self,
        op: Opcode,
        ip: usize,
        s0_pre: Felt,
        s1_pre: Felt,
        recorder: &mut R,
    ) -> Result<()> {
        match op {
            Opcode::Constant => {
                let index = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let entry = self.constants.get(index).cloned().ok_or_else(|| {
                    self.vm_error(format!(
                        "constant pool access out of bounds at index {index}"
                    ))
                })?;
                let value = self.materialize_constant(entry, recorder)?;
                recorder.record_out(value.to_maybe_relocatable());
                self.push_stack(value)?;
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
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::Div | Opcode::Mod => {
                self.execute_binary_operation(op)?;
                let result = self.peek_top_maybe_reloc();
                let result_felt = result.as_felt().unwrap_or(Felt::ZERO);
                recorder.record_out(result);
                recorder.record_div_mod_witness(op, s0_pre, s1_pre, result_felt);
            }
            Opcode::True => {
                self.push_stack(TRUE)?;
                recorder.record_out(MaybeRelocatable::Felt(Felt::ONE));
            }
            Opcode::False => {
                self.push_stack(FALSE)?;
            }
            Opcode::Equal | Opcode::NotEqual | Opcode::GreaterThan | Opcode::LessThan => {
                self.execute_comparison(op)?;
                let result = self.peek_top_maybe_reloc();
                let result_felt = result.as_felt().unwrap_or(Felt::ZERO);
                recorder.record_out(result);
                recorder.record_cmp_witness(s0_pre, s1_pre);
                if matches!(op, Opcode::LessThan | Opcode::GreaterThan) {
                    recorder.record_lt_gt_witness(op, s0_pre, s1_pre, result_felt);
                }
            }
            Opcode::Bang => {
                self.execute_bang_operator()?;
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::Minus => {
                self.execute_minus_operator()?;
                recorder.record_out(self.peek_top_maybe_reloc());
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
                let mr = value.to_maybe_relocatable();
                if index >= self.globals.len() {
                    self.globals.resize(index + 1, Value::Unit);
                }
                self.globals[index] = value;
                recorder.record_global_access(index, mr, false);
            }
            Opcode::GetGlobal => {
                let index = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let value = self.globals.get(index).cloned().ok_or_else(|| {
                    self.vm_error(format!("undefined global variable at index {index}"))
                })?;
                let mr = value.to_maybe_relocatable();
                recorder.record_out(mr);
                recorder.record_global_access(index, mr, true);
                self.push_stack(value)?;
            }
            Opcode::Vector => {
                let n = self.read_u16_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 2;
                let v = self.build_vector_segment(n, recorder)?;
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
                if let Value::Vector { base, len } = container {
                    let addr = self.index_vector_seg(base, len, &index)?;
                    let value = self.heap_values.get(&addr).cloned().ok_or_else(|| {
                        self.vm_error(format!("vector cell {addr} is unallocated"))
                    })?;
                    let value_mr = value.to_maybe_relocatable();
                    recorder.record_heap_access(addr.segment_index, addr.offset, value_mr, true);
                    self.push_stack(value)?;
                } else {
                    self.execute_index_expression(container, index)?;
                }
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::Call => {
                let num_args = self.read_u8_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 1;
                self.execute_function_call(num_args, ip, recorder)?;
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
                    Some(Constant::CompiledFn(f)) => f.clone(),
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
                recorder.record_out(value.to_maybe_relocatable());
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
                let result = self.peek_top_maybe_reloc();
                let result_felt = result.as_felt().unwrap_or(Felt::ZERO);
                recorder.record_out(result);
                recorder.record_convert_witness(result_felt);
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
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::MatchTag => {
                let expected_tag = self.read_u16_operand(ip + 1)?;
                let jump_target = self.read_u16_operand(ip + 3)?;
                self.current_frame_mut()?.ip += 4;
                self.execute_match_tag(expected_tag, jump_target)?;
            }
            Opcode::ReturnValue => {
                let return_value = self.pop_stack()?;
                let return_mr = return_value.to_maybe_relocatable();
                let frame = self.pop_frame()?;
                self.sp = frame.base_pointer.saturating_sub(1);
                self.push_stack(return_value)?;
                recorder.record_out(return_mr);
                recorder.record_return()?;
            }
            Opcode::Return => {
                let frame = self.pop_frame()?;
                self.sp = frame.base_pointer.saturating_sub(1);
                self.push_stack(UNIT)?;
                recorder.record_return()?;
            }
            Opcode::SetLocal => {
                let local_index = self.read_u8_operand(ip + 1)?;
                self.current_frame_mut()?.ip += 1;
                let base_pointer = self.current_frame()?.base_pointer;
                let value = self.pop_stack()?;
                let mr = value.to_maybe_relocatable();
                let slot = base_pointer + local_index;
                if slot >= self.stack.len() {
                    self.stack.resize(slot + 1, Value::Unit);
                }
                self.stack[slot] = value;
                recorder.record_local_access(local_index, mr, false);
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
                let mr = value.to_maybe_relocatable();
                recorder.record_out(mr);
                recorder.record_local_access(local_index, mr, true);
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
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::FeltInv => {
                self.execute_felt_inv()?;
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::FeltPow => {
                self.execute_felt_pow()?;
                recorder.record_out(self.peek_top_maybe_reloc());
            }
            Opcode::SegmentNew => {
                let base = self
                    .segments
                    .add()
                    .map_err(|e| self.vm_error(format!("SegmentNew: {e}")))?;
                self.current_segment = Some(base.segment_index);
                self.push_stack(Value::Relocatable(base))?;
                recorder.record_out(MaybeRelocatable::Relocatable(base));
            }
            Opcode::HeapAlloc => {
                let initial = self.pop_stack()?;
                let initial_mr = initial.to_maybe_relocatable();
                let segment = self.heap_target_segment()?;
                let addr = self
                    .segments
                    .append(segment, initial_mr)
                    .map_err(|e| self.vm_error(format!("HeapAlloc: {e}")))?;
                self.heap_values.insert(addr, initial);
                self.push_stack(Value::Relocatable(addr))?;
                recorder.record_out(MaybeRelocatable::Relocatable(addr));
                recorder.record_heap_access(addr.segment_index, addr.offset, initial_mr, false);
            }
            Opcode::HeapRead => {
                let addr = self.pop_relocatable("HeapRead")?;
                let value = self.heap_values.get(&addr).cloned().ok_or_else(|| {
                    self.vm_error(format!("heap read of unallocated address {addr}"))
                })?;
                let value_mr = value.to_maybe_relocatable();
                recorder.record_out(value_mr);
                recorder.record_heap_access(addr.segment_index, addr.offset, value_mr, true);
                self.push_stack(value)?;
            }
            Opcode::HeapWrite => {
                let value = self.pop_stack()?;
                let value_mr = value.to_maybe_relocatable();
                let addr = self.pop_relocatable("HeapWrite")?;
                self.segments
                    .write(addr, value_mr)
                    .map_err(|e| self.vm_error(format!("HeapWrite: {e}")))?;
                self.heap_values.insert(addr, value);
                recorder.record_heap_access(addr.segment_index, addr.offset, value_mr, false);
            }
            Opcode::ArenaNew => {
                let arena_base = self.pop_relocatable("ArenaNew")?;
                let (allocated_base, info_addr) = self
                    .segments
                    .arena_new(arena_base.segment_index)
                    .map_err(|e| self.vm_error(format!("ArenaNew: {e}")))?;
                let id = MaybeRelocatable::Felt(Felt::new(u64::from(allocated_base.segment_index)));
                self.heap_values.insert(
                    info_addr,
                    Value::Felt(Felt::new(u64::from(allocated_base.segment_index))),
                );
                self.push_stack(Value::Relocatable(allocated_base))?;
                recorder.record_out(MaybeRelocatable::Relocatable(allocated_base));
                recorder.record_heap_access(info_addr.segment_index, info_addr.offset, id, false);
            }
            Opcode::ArenaFinalize => {
                let target_base = self.pop_relocatable("ArenaFinalize")?;
                let arena_base = self.pop_relocatable("ArenaFinalize")?;
                let (_size, marker_addr) = self
                    .segments
                    .arena_finalize(arena_base.segment_index, target_base.segment_index)
                    .map_err(|e| self.vm_error(format!("ArenaFinalize: {e}")))?;
                let marker =
                    MaybeRelocatable::Felt(Felt::new(u64::from(target_base.segment_index)));
                self.heap_values.insert(
                    marker_addr,
                    Value::Felt(Felt::new(u64::from(target_base.segment_index))),
                );
                recorder.record_heap_access(
                    marker_addr.segment_index,
                    marker_addr.offset,
                    marker,
                    false,
                );
            }
            Opcode::VectorNew => {
                let base = self
                    .segments
                    .add()
                    .map_err(|e| self.vm_error(format!("VectorNew: {e}")))?;
                self.current_segment = Some(base.segment_index);
                self.push_stack(Value::Vector { base, len: 0 })?;
                recorder.record_out(MaybeRelocatable::Relocatable(base));
            }
            Opcode::VectorPush => {
                let val = self.pop_stack()?;
                let vec = self.pop_stack()?;
                match vec {
                    Value::Vector { base, len } => {
                        let val_mr = val.to_maybe_relocatable();
                        let cell_addr = Relocatable::new(base.segment_index, len);
                        self.segments
                            .write(cell_addr, val_mr)
                            .map_err(|e| self.vm_error(format!("VectorPush: {e}")))?;
                        self.heap_values.insert(cell_addr, val);
                        let new_len = len
                            .checked_add(1)
                            .ok_or_else(|| self.vm_error("VectorPush: vector length overflow"))?;
                        self.push_stack(Value::Vector { base, len: new_len })?;
                        self.push_stack(Value::Relocatable(cell_addr))?;
                        recorder.record_out(MaybeRelocatable::Relocatable(cell_addr));
                        recorder.record_heap_access(
                            cell_addr.segment_index,
                            cell_addr.offset,
                            val_mr,
                            false,
                        );
                    }
                    other => {
                        return Err(self.vm_error(format!(
                            "VectorPush: receiver must be a Vector, got {}",
                            other.type_name()
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn heap_target_segment(&mut self) -> Result<u32> {
        if let Some(seg) = self.current_segment {
            return Ok(seg);
        }
        if let Some(seg) = self.default_segment {
            return Ok(seg);
        }
        let base = self
            .segments
            .add()
            .map_err(|e| self.vm_error(format!("HeapAlloc default segment: {e}")))?;
        self.default_segment = Some(base.segment_index);
        Ok(base.segment_index)
    }

    fn pop_relocatable(&mut self, context: &str) -> Result<Relocatable> {
        match self.pop_stack()? {
            Value::Relocatable(r) => Ok(r),
            other => Err(self.vm_error(format!(
                "{context} expects relocatable heap address, got {}",
                other.type_name()
            ))),
        }
    }

    fn execute_function_call<R: Tracer>(
        &mut self,
        num_args: usize,
        call_ip: usize,
        recorder: &mut R,
    ) -> Result<()> {
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
            Value::Closure(cl) => self.call_closure(cl, num_args, call_ip, recorder),
            Value::Builtin(f) => self.call_builtin_fn(f, num_args, recorder),
            _ => Err(self.vm_error("calling non-function")),
        }
    }

    fn call_closure<R: Tracer>(
        &mut self,
        closure: Closure,
        num_args: usize,
        call_ip: usize,
        recorder: &mut R,
    ) -> Result<()> {
        if num_args != closure.func.num_parameters {
            return Err(self.vm_error(format!(
                "wrong number of arguments: want={}, got={num_args}",
                closure.func.num_parameters
            )));
        }

        let caller_num_locals = self.current_frame()?.num_locals;
        let sp_at_call = self.sp;
        let args_start = self
            .sp
            .checked_sub(num_args)
            .ok_or_else(|| self.vm_error("stack underflow in function call"))?;
        let arg_mrs = self.stack[args_start..args_start + num_args]
            .iter()
            .map(Value::to_maybe_relocatable)
            .collect::<Vec<MaybeRelocatable>>();

        recorder.record_call_closure(CallCtx {
            call_ip,
            sp_at_call,
            caller_num_locals,
            args: &arg_mrs,
        })?;

        let base_pointer = args_start;
        let num_locals = closure.func.num_locals;
        let frame = Frame::new(closure, base_pointer);
        self.push_frame(frame)?;
        self.sp = base_pointer
            .checked_add(num_locals)
            .ok_or_else(|| self.vm_error("stack pointer overflow"))?;

        if self.sp > self.stack.len() {
            self.stack.resize(self.sp, Value::Unit);
        }
        Ok(())
    }

    fn call_builtin_fn<R: Tracer>(
        &mut self,
        func: BuiltinFn,
        num_args: usize,
        recorder: &mut R,
    ) -> Result<()> {
        let args_start = self.sp - num_args;
        let raw_args: Vec<Value> = self.stack[args_start..self.sp].to_vec();

        let base_vals: Vec<Vec<Value>> = raw_args
            .iter()
            .map(|v| match v {
                Value::Vector { base, len } => self.read_vector_cells(*base, *len),
                _ => Ok(Vec::new()),
            })
            .collect::<Result<_>>()?;

        let args: Vec<BuiltinArg<'_>> = raw_args
            .iter()
            .zip(base_vals.iter())
            .map(|(v, b)| Self::value_to_builtin_arg(v, b))
            .collect();

        let ret = func(&args)?;
        drop(args);
        drop(base_vals);
        drop(raw_args);

        let value = self.segment_allocate_return(ret, recorder)?;

        self.sp = args_start - 1;
        self.push_stack(value)?;
        recorder.record_call_builtin();
        Ok(())
    }

    fn value_to_builtin_arg<'a>(value: &'a Value, base_vals: &'a [Value]) -> BuiltinArg<'a> {
        match value {
            Value::Unit => BuiltinArg::Unit,
            Value::Integer(i) => BuiltinArg::Integer(*i),
            Value::Felt(f) => BuiltinArg::Felt(*f),
            Value::Bool(b) => BuiltinArg::Bool(*b),
            Value::Char(c) => BuiltinArg::Char(*c),
            Value::Str(s) => BuiltinArg::Str(s.as_str()),
            Value::Tuple(t) => BuiltinArg::Tuple(t.as_slice()),
            Value::Array(a) => BuiltinArg::Array(a.as_slice()),
            Value::Map(m) => BuiltinArg::Map(m),
            Value::Builtin(f) => BuiltinArg::Builtin(*f),
            Value::CompiledFn(f) => BuiltinArg::CompiledFn(f),
            Value::Closure(c) => BuiltinArg::Closure(c),
            Value::Struct(s) => BuiltinArg::Struct(s),
            Value::EnumVariant(ev) => BuiltinArg::EnumVariant(ev),
            Value::Set(s) => BuiltinArg::Set(s),
            Value::Range(s, e) => BuiltinArg::Range(*s, *e),
            Value::RangeInclusive(s, e) => BuiltinArg::RangeInclusive(*s, *e),
            Value::Relocatable(r) => BuiltinArg::Relocatable(*r),
            Value::Vector { .. } => BuiltinArg::Vector(base_vals),
        }
    }

    /// Recursively converts a [`BuiltinReturn`] tree into a runtime [`Value`].
    fn segment_allocate_return<R: Tracer>(
        &mut self,
        ret: BuiltinReturn,
        recorder: &mut R,
    ) -> Result<Value> {
        match ret {
            BuiltinReturn::Value(v) => Ok(v),
            BuiltinReturn::Str(s) => Ok(Value::Str(s)),
            BuiltinReturn::Vector(entries) => {
                let values = entries
                    .into_iter()
                    .map(|e| self.segment_allocate_return(e, recorder))
                    .collect::<Result<Vec<_>>>()?;
                self.allocate_vector_segment(values, recorder)
            }
        }
    }

    fn read_vector_cells(&self, base: Relocatable, len: u32) -> Result<Vec<Value>> {
        let mut cells = Vec::with_capacity(len as usize);
        for off in 0..len {
            let addr = Relocatable::new(base.segment_index, off);
            let cell = self.heap_values.get(&addr).cloned().ok_or_else(|| {
                self.vm_error(format!(
                    "Vector materialization: cell {addr} is unallocated"
                ))
            })?;
            cells.push(cell);
        }
        Ok(cells)
    }

    /// Materializes a [`Constant`] into a runtime [`Value`].
    fn materialize_constant<R: Tracer>(
        &mut self,
        entry: Constant,
        recorder: &mut R,
    ) -> Result<Value> {
        match entry {
            Constant::Unit => Ok(Value::Unit),
            Constant::Integer(i) => Ok(Value::Integer(i)),
            Constant::Felt(f) => Ok(Value::Felt(Felt::new(f))),
            Constant::Bool(b) => Ok(Value::Bool(b)),
            Constant::Char(c) => Ok(Value::Char(c)),
            Constant::Str(s) => Ok(Value::Str(s)),
            Constant::Tuple(entries) => {
                let values = entries
                    .into_iter()
                    .map(|e| self.materialize_constant(e, recorder))
                    .collect::<Result<_>>()?;
                Ok(Value::Tuple(values))
            }
            Constant::Vector(entries) => {
                let values = entries
                    .into_iter()
                    .map(|e| self.materialize_constant(e, recorder))
                    .collect::<Result<Vec<_>>>()?;
                self.allocate_vector_segment(values, recorder)
            }
            Constant::Array(entries) => {
                let values = entries
                    .into_iter()
                    .map(|e| self.materialize_constant(e, recorder))
                    .collect::<Result<_>>()?;
                Ok(Value::Array(values))
            }
            Constant::Map(map) => {
                let mut pairs = IndexMap::with_capacity(map.len());
                for (k, v) in map {
                    let v = self.materialize_constant(v, recorder)?;
                    pairs.insert(k, v);
                }
                Ok(Value::Map(Map { pairs }))
            }
            Constant::Set(set) => Ok(Value::Set(Set(set))),
            Constant::CompiledFn(f) => Ok(Value::CompiledFn(f)),
            Constant::Struct { type_index, fields } => {
                let fields = fields
                    .into_iter()
                    .map(|e| self.materialize_constant(e, recorder))
                    .collect::<Result<_>>()?;
                Ok(Value::Struct(StructVal { type_index, fields }))
            }
            Constant::EnumVariant {
                type_index,
                tag,
                fields,
            } => {
                let fields = fields
                    .into_iter()
                    .map(|e| self.materialize_constant(e, recorder))
                    .collect::<Result<_>>()?;
                Ok(Value::EnumVariant(EnumVariantVal {
                    type_index,
                    tag,
                    fields,
                }))
            }
            Constant::Range(s, e) => Ok(Value::Range(s, e)),
            Constant::RangeInclusive(s, e) => Ok(Value::RangeInclusive(s, e)),
            Constant::Relocatable(r) => Ok(Value::Relocatable(r)),
        }
    }

    fn allocate_vector_segment<R: Tracer>(
        &mut self,
        elements: Vec<Value>,
        _recorder: &mut R,
    ) -> Result<Value> {
        let base = self
            .segments
            .add()
            .map_err(|e| self.vm_error(format!("Vector segment allocation: {e}")))?;
        let len = u32::try_from(elements.len())
            .map_err(|_| self.vm_error("Vector length exceeds u32 representable range"))?;
        for (offset, element) in elements.into_iter().enumerate() {
            let offset = u32::try_from(offset)
                .map_err(|_| self.vm_error("Vector offset exceeds u32 representable range"))?;
            let addr = Relocatable::new(base.segment_index, offset);
            self.heap_values.insert(addr, element);
        }
        Ok(Value::Vector { base, len })
    }

    fn build_vector_segment<R: Tracer>(&mut self, n: usize, recorder: &mut R) -> Result<Value> {
        if n > self.sp {
            return Err(self.vm_error(format!(
                "stack underflow in vector construction: need {n} elements, stack has {}",
                self.sp
            )));
        }
        let start = self.sp - n;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        self.allocate_vector_segment(elements, recorder)
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

    fn peek_stack_maybe_reloc(&self) -> (MaybeRelocatable, MaybeRelocatable, MaybeRelocatable) {
        (self.peek_at(1), self.peek_at(2), self.peek_at(3))
    }

    fn peek_top_maybe_reloc(&self) -> MaybeRelocatable {
        self.peek_at(1)
    }

    fn peek_at(&self, depth: usize) -> MaybeRelocatable {
        if self.sp >= depth {
            self.stack[self.sp - depth].to_maybe_relocatable()
        } else {
            MaybeRelocatable::Felt(Felt::ZERO)
        }
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

        if op == Opcode::Add
            && let (Value::Relocatable(addr), Value::Integer(idx)) = (&left, &right)
        {
            let addend = idx.to_felt().ok_or_else(|| {
                self.vm_error("Relocatable offset addend does not fit in the base field")
            })?;
            let next = addr
                .add_offset(addend)
                .map_err(|e| self.vm_error(format!("Relocatable arithmetic: {e}")))?;
            return self.push_stack(Value::Relocatable(next));
        }

        if op == Opcode::Add
            && let (Value::Vector { base, .. }, Value::Integer(idx)) = (&left, &right)
        {
            let addend = idx.to_felt().ok_or_else(|| {
                self.vm_error("Vector index addend does not fit in the base field")
            })?;
            let next = base
                .add_offset(addend)
                .map_err(|e| self.vm_error(format!("Vector index arithmetic: {e}")))?;
            return self.push_stack(Value::Relocatable(next));
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
            Value::Integer(I::I8(v)) => from_i64(*v as i64),
            Value::Integer(I::I16(v)) => from_i64(*v as i64),
            Value::Integer(I::I32(v)) => from_i64(*v as i64),
            Value::Integer(I::I64(v)) => from_i64(*v),
            Value::Integer(I::Isize(v)) => from_i64(*v as i64),
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
        let inv =
            try_inv(operand).map_err(|e| self.vm_error(format!("Felt inverse error: {e}")))?;
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
        self.push_stack(Value::Felt(base.exp(exponent)))
    }

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
            (Value::Array(elements), _) => self.execute_vector_index(elements, &index),
            (Value::Map(map), _) => self.execute_map_index(map, index),
            _ => Err(self.vm_error(format!(
                "index operator not supported: {}",
                container.type_name()
            ))),
        }
    }

    fn index_vector_seg(&self, base: Relocatable, len: u32, index: &Value) -> Result<Relocatable> {
        if !index.is_integer() {
            return Err(self.vm_error(format!(
                "vector index must be an integer, got {}",
                index.type_name()
            )));
        }
        let idx = index.to_vector_index().ok_or_else(|| {
            self.vm_error(format!(
                "index out of bounds: index is {index}, length is {len}"
            ))
        })?;
        let idx_u32 = u32::try_from(idx).map_err(|_| {
            self.vm_error(format!(
                "index out of bounds: index {idx} exceeds u32 representable range"
            ))
        })?;
        if idx_u32 >= len {
            return Err(self.vm_error(format!(
                "index out of bounds: index is {idx_u32}, length is {len}"
            )));
        }
        Ok(Relocatable::new(base.segment_index, idx_u32))
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
