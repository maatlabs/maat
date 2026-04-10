#![forbid(unsafe_code)]

mod codex;
mod compile;
mod eval;
mod module;
mod parse;
mod ty;
mod vm;

pub use codex::{DecodeError, SerializationError};
pub use compile::{CompileError, CompileErrorKind};
pub use eval::EvalError;
pub use module::{ModuleError, ModuleErrorKind};
pub use parse::ParseError;
pub use ty::{
    MissingTraitMethodError, TraitMethodSignatureMismatchError, TypeError, TypeErrorKind,
};
pub use vm::VmError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error("eval error: {0}")]
    Eval(#[from] EvalError),

    #[error("compile error: {0}")]
    Compile(#[from] CompileError),

    #[error("type error: {0}")]
    Type(#[from] TypeError),

    #[error("vm error: {0}")]
    Vm(#[from] VmError),

    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),

    #[error("serialization error: {0}")]
    Serialization(#[from] SerializationError),

    #[error("module error: {0}")]
    Module(#[from] ModuleError),
}
