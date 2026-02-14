//! Code generation for the compiler.
//!
//! This crate translates AST nodes into bytecode instructions that can be
//! executed by the virtual machine. The compiler performs a single-pass
//! traversal of the AST, emitting stack-based bytecode operations.

mod compile;
mod symbol;

pub use compile::Compiler;
