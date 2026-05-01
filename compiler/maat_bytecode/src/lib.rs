//! Bytecode representation for the Maat virtual machine.

#![forbid(unsafe_code)]

use maat_runtime::{TypeDef, Value};
use maat_span::SourceMap;

mod instruction;
mod opcode;
mod selector;
mod serialize;

pub use instruction::{Instruction, Instructions, decode_operands, encode};
pub use opcode::{Opcode, TypeTag};
pub use selector::{
    NUM_SELECTORS, NUM_SUB_SELECTORS, OpcodeInfo, SEL_ARITH, SEL_BITWISE, SEL_CALL, SEL_CMP,
    SEL_COLLECTION, SEL_COND_JUMP, SEL_CONSTRUCT, SEL_CONVERT, SEL_DIV_MOD, SEL_FELT,
    SEL_HEAP_ALLOC, SEL_HEAP_READ, SEL_HEAP_WRITE, SEL_JUMP, SEL_LOAD, SEL_NOP, SEL_PUSH,
    SEL_RETURN, SEL_STORE, SEL_UNARY, SUB_SEL_ADD, SUB_SEL_AND, SUB_SEL_DIV, SUB_SEL_EQ,
    SUB_SEL_FELT_ADD, SUB_SEL_FELT_MUL, SUB_SEL_FELT_SUB, SUB_SEL_NEG, SUB_SEL_NEQ, SUB_SEL_OR,
    SUB_SEL_SHL, SUB_SEL_SHR, SUB_SEL_SUB, SUB_SEL_XOR, selector_index, sub_selector_index,
};

/// Maximum number of constants in the constant pool.
pub const MAX_CONSTANT_POOL_SIZE: usize = u16::MAX as usize;

/// Maximum number of variants an enum can have.
pub const MAX_ENUM_VARIANTS: usize = 256;

/// Maximum number of elements that can be pushed onto the VM's stack.
pub const MAX_STACK_SIZE: usize = 2048;

/// Maximum number of global variable bindings.
pub const MAX_GLOBALS: usize = u16::MAX as usize;

/// Maximum number of local variable bindings per function scope.
pub const MAX_LOCALS: usize = u8::MAX as usize;

/// Maximum number of call frames on the VM's frame stack.
pub const MAX_FRAMES: usize = 1024;

/// Compiled bytecode output containing instructions and constants.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bytecode {
    pub instructions: Instructions,
    pub constants: Vec<Value>,
    pub source_map: SourceMap,
    pub type_registry: Vec<TypeDef>,
}
