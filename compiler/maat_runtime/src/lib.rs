//! Runtime value system for Maat.
//!
//! This crate defines the core object model and built-in functions shared
//! by both the tree-walking interpreter and the bytecode compiler/VM.
#![forbid(unsafe_code)]

mod builtins;
mod env;
mod object;

pub use builtins::{BUILTIN_COUNT, BUILTINS, QUOTE, UNQUOTE, get_builtin};
pub use env::Env;
pub use object::{
    BuiltinFn, Closure, CompiledFunction, EnumVariantObject, FALSE, Function, HashObject, Hashable,
    Macro, NULL, Object, Quote, StructObject, TRUE, TypeDef, VariantInfo,
};
