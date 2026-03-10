//! Type system for the Maat programming language.
//!
//! This version implements the [`Hindley-Milner`] type inference (Algorithm W), unification,
//! numeric promotion rules, constant folding (via `maat_ast`), and compile-time type checking.
//! Sits between macro expansion and codegen in the compilation pipeline.
//!
//! [`Hindley-Milner`]: https://en.wikipedia.org/wiki/Hindley%E2%80%93Milner_type_system

pub mod check;
pub mod convert;
pub mod env;
pub mod promote;
pub mod ty;
pub mod unify;

pub use check::TypeChecker;
pub use convert::resolve_type_expr;
pub use env::TypeEnv;
pub use ty::{
    EnumDef, ImplDef, MethodSig, StructDef, TraitDef, Type, TypeScheme, VariantDef, VariantKind,
};
