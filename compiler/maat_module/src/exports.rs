//! Public exports collected from a type-checked module.
//!
//! After type checking a module, its public items are captured in a
//! [`ModuleExports`] so that downstream modules can import them via
//! `use` statements.

use maat_types::{EnumDef, ImplDef, StructDef, TraitDef, TypeScheme};

/// The set of publicly visible items exported by a module.
///
/// Collected after type checking and used to populate the type environments
/// of downstream modules that import from this module.
#[derive(Debug, Clone, Default)]
pub struct ModuleExports {
    /// Public function and variable bindings: `(name, type_scheme)`.
    pub bindings: Vec<(String, TypeScheme)>,
    /// Public struct definitions.
    pub structs: Vec<StructDef>,
    /// Public enum definitions.
    pub enums: Vec<EnumDef>,
    /// Public trait definitions.
    pub traits: Vec<TraitDef>,
    /// All impl blocks (visibility is per-method, not per-block).
    pub impls: Vec<ImplDef>,
}
