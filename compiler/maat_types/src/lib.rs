//! Type system for the Maat programming language.
//!
//! Implements [`Hindley-Milner`] type inference (Algorithm W), unification,
//! constant folding (via `maat_ast`), and compile-time type checking.
//! Sits between macro expansion and codegen in the compilation pipeline.
//!
//! [`Hindley-Milner`]: https://en.wikipedia.org/wiki/Hindley%E2%80%93Milner_type_system
#![forbid(unsafe_code)]

pub mod check;
pub mod convert;
pub mod env;
pub mod ty;
pub mod unify;

pub use check::TypeChecker;
pub use convert::resolve_type_expr;
pub use env::TypeEnv;
pub use ty::{
    EnumDef, FnType, ImplDef, MethodSig, StructDef, TraitDef, Type, TypeScheme, TypeVarId,
    VariantDef, VariantKind,
};
