use maat_ast::{Program, Stmt, TypeExpr};
use maat_types::{EnumDef, ImplDef, StructDef, TraitDef, Type, TypeEnv, TypeScheme};

use crate::{ImportKind, ResolvedImport};

#[derive(Debug, Clone, Default)]
pub struct ModuleExports {
    pub bindings: Vec<(String, TypeScheme)>,
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub traits: Vec<TraitDef>,
    pub impls: Vec<ImplDef>,
}

impl ModuleExports {
    pub fn from_checked(program: &Program, env: &TypeEnv) -> Self {
        let mut exports = Self::default();
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

    pub fn resolve_item(&self, name: &str, result: &mut Vec<ResolvedImport>) {
        if let Some((_, scheme)) = self.bindings.iter().find(|(n, _)| n == name) {
            result.push(ResolvedImport {
                local_name: name.to_string(),
                kind: ImportKind::Binding(scheme.clone()),
            });
            return;
        }
        // For struct/enum imports, also pull in associated impl blocks.
        let mut is_type_import = false;
        if let Some(def) = self.structs.iter().find(|d| d.name == name) {
            result.push(ResolvedImport {
                local_name: name.to_string(),
                kind: ImportKind::Struct(def.clone()),
            });
            is_type_import = true;
        }
        if let Some(def) = self.enums.iter().find(|d| d.name == name) {
            result.push(ResolvedImport {
                local_name: name.to_string(),
                kind: ImportKind::Enum(def.clone()),
            });
            is_type_import = true;
        }
        if is_type_import {
            for imp in &self.impls {
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
        if let Some(def) = self.traits.iter().find(|d| d.name == name) {
            result.push(ResolvedImport {
                local_name: name.to_string(),
                kind: ImportKind::Trait(def.clone()),
            });
        }
    }
}
