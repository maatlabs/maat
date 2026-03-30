//! Public exports collected from a type-checked module.
//!
//! After type checking a module, its public items are captured in a
//! [`ModuleExports`] so that downstream modules can import them via
//! `use` statements.

use maat_ast::{Program, Stmt, TypeExpr};
use maat_types::{EnumDef, ImplDef, StructDef, TraitDef, Type, TypeEnv, TypeScheme};

use crate::imports::{ImportKind, ResolvedImport};

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

/// Extracts public exports from a type-checked module.
///
/// Scans the AST for `pub` items and collects their type information
/// from the type environment.
pub fn extract(program: &Program, env: &TypeEnv) -> ModuleExports {
    let mut exports = ModuleExports::default();
    for stmt in &program.statements {
        match stmt {
            Stmt::FuncDef(func) if func.is_public => {
                if let Some(scheme) = env.lookup_scheme(&func.name) {
                    exports.bindings.push((func.name.clone(), scheme.clone()));
                }
            }
            Stmt::StructDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_struct(&decl.name) {
                    exports.structs.push(def.clone());
                }
            }
            Stmt::EnumDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_enum(&decl.name) {
                    exports.enums.push(def.clone());
                }
            }
            Stmt::TraitDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_trait(&decl.name) {
                    exports.traits.push(def.clone());
                }
            }
            Stmt::ImplBlock(impl_block) => {
                let type_name = match &impl_block.self_type {
                    TypeExpr::Named(n) => &n.name,
                    TypeExpr::Generic(name, _, _) => name,
                    _ => continue,
                };
                let pub_methods = impl_block
                    .methods
                    .iter()
                    .filter(|m| m.is_public)
                    .map(|m| m.name.as_str())
                    .collect::<Vec<&str>>();
                if pub_methods.is_empty() {
                    continue;
                }
                // Find the matching ImplDef and export only public methods.
                for imp in env.all_impls() {
                    let matches = match &imp.self_type {
                        Type::Struct(n, _) | Type::Enum(n, _) => n.as_ref() == type_name,
                        _ => false,
                    };
                    if matches {
                        let filtered = ImplDef {
                            self_type: imp.self_type.clone(),
                            trait_name: imp.trait_name.clone(),
                            methods: imp
                                .methods
                                .iter()
                                .filter(|(name, _)| pub_methods.contains(&name.as_str()))
                                .cloned()
                                .collect(),
                        };
                        if !filtered.methods.is_empty() {
                            exports.impls.push(filtered);
                        }
                    }
                }
            }
            Stmt::Use(use_stmt) if use_stmt.is_public => {
                // Re-exports: `pub use foo::bar;` — forward the binding.
                let item_name = use_stmt
                    .items
                    .as_ref()
                    .and_then(|items| items.first())
                    .or_else(|| use_stmt.path.last());
                if let Some(name) = item_name {
                    if let Some(scheme) = env.lookup_scheme(name) {
                        exports.bindings.push((name.clone(), scheme.clone()));
                    }
                    if let Some(def) = env.lookup_struct(name) {
                        exports.structs.push(def.clone());
                    }
                    if let Some(def) = env.lookup_enum(name) {
                        exports.enums.push(def.clone());
                    }
                    if let Some(def) = env.lookup_trait(name) {
                        exports.traits.push(def.clone());
                    }
                }
            }
            _ => {}
        }
    }
    exports
}

/// Finds all exports matching `name` and appends them to `result`, which is a
/// list of resolved imports.
///
/// When a struct or enum is found, any associated `impl` blocks from the
/// same module are also included so that method resolution works across
/// module boundaries.
pub fn find(exports: &ModuleExports, name: &str, result: &mut Vec<ResolvedImport>) {
    if let Some((_, scheme)) = exports.bindings.iter().find(|(n, _)| n == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Binding(scheme.clone()),
        });
        return;
    }
    // For struct/enum imports, also pull in associated impl blocks.
    let mut is_type_import = false;
    if let Some(def) = exports.structs.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Struct(def.clone()),
        });
        is_type_import = true;
    }
    if let Some(def) = exports.enums.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Enum(def.clone()),
        });
        is_type_import = true;
    }
    if is_type_import {
        for imp in &exports.impls {
            let matches = match &imp.self_type {
                Type::Struct(n, _) | Type::Enum(n, _) => n.as_ref() == name,
                _ => false,
            };
            if matches {
                result.push(ResolvedImport {
                    local_name: String::new(),
                    kind: ImportKind::Impl(imp.clone()),
                });
            }
        }
        return;
    }
    if let Some(def) = exports.traits.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Trait(def.clone()),
        });
    }
}
