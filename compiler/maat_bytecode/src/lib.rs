//! Bytecode representation for the Maat virtual machine.
//!
//! This crate provides a compact bytecode format for representing Maat programs.
//! Instructions consist of an opcode byte followed by zero or more operand bytes,
//! with all multi-byte operands encoded in big-endian format.
//!
//! # Example
//!
//! ```
//! use maat_bytecode::{Instructions, Opcode, encode};
//!
//! // Encode some bytecode instructions
//! let const_instr = encode(Opcode::Constant, &[0]);
//! let add_instr = encode(Opcode::Add, &[]);
//!
//! // Combine into an instruction sequence
//! let mut bytecode = Instructions::from(const_instr);
//! bytecode.extend(&Instructions::from(add_instr));
//!
//! // Display as disassembly
//! println!("{}", bytecode);
//! ```

use maat_runtime::Object;

mod instruction;
mod opcode;

pub use instruction::{Instruction, Instructions, decode_operands, encode};
pub use opcode::{Opcode, TypeTag};

/// Maximum number of constants in the constant pool.
///
/// This limit is imposed by the OpConstant instruction's 2-byte index encoding,
/// which can represent indices from 0 to 65,535 (2^16 - 1).
///
/// To support more constants, the operand width would need to be increased to
/// 4 bytes (allowing up to 4,294,967,295 constants), but this comes with a
/// bytecode size trade-off.
pub const MAX_CONSTANT_POOL_SIZE: usize = u16::MAX as usize;

/// Maximum number of elements that can be pushed onto the VM's stack.
pub const MAX_STACK_SIZE: usize = 2048;

/// Maximum number of global variable bindings.
///
/// This limit is imposed by the `OpSetGlobal`/`OpGetGlobal` instructions'
/// 2-byte index encoding, which can represent indices from 0 to 65,535.
pub const MAX_GLOBALS: usize = u16::MAX as usize;

/// Maximum number of local variable bindings per function scope.
///
/// This limit is imposed by the `OpSetLocal`/`OpGetLocal` instructions'
/// 1-byte index encoding, which can represent indices from 0 to 255.
pub const MAX_LOCALS: usize = u8::MAX as usize;

/// Maximum number of call frames on the VM's frame stack.
///
/// This limits the maximum recursion depth. Each function call pushes
/// a new frame, and each return pops one. Exceeding this limit indicates
/// unbounded recursion or excessively deep call chains.
pub const MAX_FRAMES: usize = 1024;

/// Compiled bytecode output containing instructions and constants.
///
/// This represents the complete compiled program ready for execution by the
/// virtual machine. The `Bytecode` struct is the interface between the compiler
/// (which produces it) and the VM (which executes it).
///
/// # Architecture
///
/// The bytecode uses a two-part structure:
///
/// 1. **Instruction Stream** (`instructions`): A linear sequence of bytecode
///    instructions that the VM executes. Each instruction consists of an opcode
///    byte optionally followed by operand bytes.
///
/// 2. **Constant Pool** (`constants`): An array of runtime values referenced by
///    `OpConstant` instructions. The constant pool allows large or frequently-used
///    values to be stored once and referenced by a compact 2-byte index.
///
/// # Constant Pool Design
///
/// Constants are stored separately from instructions for several reasons:
///
/// - **Space efficiency**: A large integer like `999_999_999_999` is stored once
///   in the constant pool, not embedded in every instruction that uses it.
///
/// - **Fixed instruction size**: `OpConstant` instructions are always 3 bytes
///   (1 opcode + 2 index bytes), regardless of the size of the referenced value.
///
/// - **Type flexibility**: The constant pool can hold any `Object` type (integers,
///   booleans, strings, arrays, functions) while the bytecode remains uniform.
///
/// # Example
///
/// For the expression `1 + 2`:
///
/// ```
/// # use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
/// # use maat_runtime::Object;
/// // Constant pool stores the actual values:
/// let constants = vec![
///     Object::I64(1),  // constants[0]
///     Object::I64(2),  // constants[1]
/// ];
///
/// // Instructions reference constants by index:
/// let mut instructions = Instructions::new();
/// instructions.extend(&Instructions::from(encode(Opcode::Constant, &[0])));  // Push constants[0]
/// instructions.extend(&Instructions::from(encode(Opcode::Constant, &[1])));  // Push constants[1]
/// instructions.extend(&Instructions::from(encode(Opcode::Add, &[])));        // Add them
/// instructions.extend(&Instructions::from(encode(Opcode::Pop, &[])));        // Pop result
///
/// let bytecode = Bytecode { instructions, constants };
/// ```
///
/// When executed by the VM:
/// 1. `OpConstant 0` pushes `Object::I64(1)` onto the stack
/// 2. `OpConstant 1` pushes `Object::I64(2)` onto the stack
/// 3. `OpAdd` pops both values, adds them, and pushes `Object::I64(3)`
/// 4. `OpPop` removes the result from the stack
#[derive(Debug, Clone, PartialEq)]
pub struct Bytecode {
    /// The sequence of bytecode instructions to execute.
    ///
    /// Each instruction consists of an opcode byte optionally followed by
    /// operand bytes. Instructions are executed sequentially by the VM.
    pub instructions: Instructions,

    /// The constant pool containing runtime values.
    ///
    /// This array holds all constant values referenced by `OpConstant` instructions.
    /// The index into this array is encoded as a 2-byte operand (allowing up to
    /// 65,535 distinct constants).
    pub constants: Vec<Object>,
}
