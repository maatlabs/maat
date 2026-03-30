use maat_codegen::Compiler;
use maat_runtime::{TypeDef, VariantInfo};
use maat_types::{EnumDef, ImplDef, StructDef, TraitDef, Type, TypeEnv, TypeScheme, VariantKind};

/// A resolved import ready to be injected into a module's type environment.
#[derive(Debug)]
pub struct ResolvedImport {
    /// The local name under which this item is available.
    pub local_name: String,
    /// The exported item.
    pub kind: ImportKind,
}

/// The kind of item being imported.
#[derive(Debug)]
pub enum ImportKind {
    Binding(TypeScheme),
    Struct(StructDef),
    Enum(EnumDef),
    Trait(TraitDef),
    Impl(ImplDef),
}

/// Injects a resolved import into a module's type environment.
pub fn inject_into_env(env: &mut TypeEnv, import: &ResolvedImport) {
    match &import.kind {
        ImportKind::Binding(scheme) => {
            env.define_scheme(&import.local_name, scheme.clone());
        }
        ImportKind::Struct(def) => {
            env.register_struct(def.clone());
        }
        ImportKind::Enum(def) => {
            env.register_enum(def.clone());
        }
        ImportKind::Trait(def) => {
            env.register_trait(def.clone());
        }
        ImportKind::Impl(def) => {
            env.register_impl(def.clone());
        }
    }
}

/// Injects a resolved import into the compiler's symbol table and type registry.
///
/// For bindings, `define_symbol` reuses the existing global index if the symbol
/// was already defined by a prior module's compilation (this is the mechanism
/// by which the shared compiler avoids duplicate global slots for cross-module
/// references).
pub fn inject_into_compiler(compiler: &mut Compiler, import: &ResolvedImport) {
    match &import.kind {
        ImportKind::Binding(_) => {
            let _ = compiler
                .symbols_table_mut()
                .define_symbol(&import.local_name, false);
        }
        ImportKind::Struct(def) => {
            compiler.register_type(TypeDef::Struct {
                name: def.name.clone(),
                field_names: def.fields.iter().map(|(n, _)| n.clone()).collect(),
            });
        }
        ImportKind::Enum(def) => {
            compiler.register_type(TypeDef::Enum {
                name: def.name.clone(),
                variants: def
                    .variants
                    .iter()
                    .map(|v| VariantInfo {
                        name: v.name.clone(),
                        field_count: match &v.kind {
                            VariantKind::Unit => 0,
                            VariantKind::Tuple(fields) => fields.len() as u8,
                            VariantKind::Struct(fields) => fields.len() as u8,
                        },
                    })
                    .collect(),
            });
        }
        ImportKind::Trait(_) => {
            // Traits have no runtime representation; they only affect
            // type checking.
        }
        ImportKind::Impl(def) => {
            // Register each method as a global symbol so that method
            // calls compile correctly.
            let type_name = match &def.self_type {
                Type::Struct(n, _) | Type::Enum(n, _) => n.to_string(),
                _ => return,
            };
            for (method_name, _) in &def.methods {
                let qualified = format!("{type_name}::{method_name}");
                let _ = compiler
                    .symbols_table_mut()
                    .define_symbol(&qualified, false);
            }
        }
    }
}
