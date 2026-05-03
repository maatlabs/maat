//! Bytecode representation for the Maat virtual machine.

#![forbid(unsafe_code)]

use maat_runtime::{TypeDef, Value};
use maat_span::SourceMap;

mod instruction;
mod opcode;
mod serialize;

pub use instruction::{Instruction, Instructions, decode_operands, encode};
pub use opcode::{Opcode, TypeTag};

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
