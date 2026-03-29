use std::collections::HashMap;

use maat_runtime::{BUILTINS, TypeDef, VariantInfo};

use crate::SymbolsTable;

/// Built-in type prefixes for method dispatch fallback.
pub const BUILTIN_METHOD_PREFIXES: &[&str] =
    &["Vector", "str", "char", "Set", "Map", "Option", "Result"];

/// Enum types whose variants are available as bare names in patterns.
///
/// Mirrors Rust's prelude: `Option` (`Some`, `None`) and `Result`
/// (`Ok`, `Err`) are directly accessible. All other enum variants
/// require a qualified path (e.g., `ParseIntError::InvalidDigit`).
const PRELUDE_ENUMS: &[&str] = &["Option", "Result"];

/// Pre-computed enum variant lookup entry, indexed by variant name.
#[derive(Debug, Clone, Copy)]
pub struct VariantEntry {
    pub registry_index: usize,
    pub tag: usize,
    pub field_count: usize,
}

/// Registers all built-in functions in the given symbols table.
pub fn register_builtins(table: &mut SymbolsTable) {
    for (i, (name, _)) in BUILTINS.iter().enumerate() {
        table.define_builtin(i, name);
    }
}

/// Resolves a builtin function name to its index in the [`BUILTINS`] registry.
///
/// Panics if the name is not found. Internal builtins are guaranteed
/// to be present at compile time.
pub fn resolve_builtin_index(name: &str) -> usize {
    BUILTINS
        .iter()
        .position(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("internal builtin `{name}` not found in registry"))
}

/// Returns the type registry pre-populated with built-in enum types.
pub fn builtin_type_registry() -> Vec<TypeDef> {
    vec![
        TypeDef::Enum {
            name: "Option".to_string(),
            variants: vec![
                VariantInfo {
                    name: "Some".to_string(),
                    field_count: 1,
                },
                VariantInfo {
                    name: "None".to_string(),
                    field_count: 0,
                },
            ],
        },
        TypeDef::Enum {
            name: "Result".to_string(),
            variants: vec![
                VariantInfo {
                    name: "Ok".to_string(),
                    field_count: 1,
                },
                VariantInfo {
                    name: "Err".to_string(),
                    field_count: 1,
                },
            ],
        },
        TypeDef::Enum {
            name: "ParseIntError".to_string(),
            variants: vec![
                VariantInfo {
                    name: "Empty".to_string(),
                    field_count: 0,
                },
                VariantInfo {
                    name: "InvalidDigit".to_string(),
                    field_count: 0,
                },
                VariantInfo {
                    name: "Overflow".to_string(),
                    field_count: 0,
                },
            ],
        },
    ]
}

/// Builds the enum variant index from an existing type registry.
pub fn build_variant_index(registry: &[TypeDef]) -> HashMap<String, VariantEntry> {
    let mut index = HashMap::new();
    for (registry_index, td) in registry.iter().enumerate() {
        if let TypeDef::Enum { name, variants } = td {
            let in_prelude = PRELUDE_ENUMS.contains(&name.as_str());
            index_variants(&mut index, registry_index, variants, name, in_prelude);
        }
    }
    index
}

/// Inserts entries for each variant of an enum at the given registry index.
///
/// Qualified keys (`EnumName::VariantName`) are always registered. Bare
/// keys (`VariantName`) are only registered when `include_bare` is true,
/// which is the case for prelude enums like `Option` and `Result`.
pub fn index_variants(
    index: &mut HashMap<String, VariantEntry>,
    registry_index: usize,
    variants: &[VariantInfo],
    enum_name: &str,
    include_bare: bool,
) {
    for (tag, v) in variants.iter().enumerate() {
        let entry = VariantEntry {
            registry_index,
            tag,
            field_count: v.field_count as usize,
        };
        if include_bare {
            index.insert(v.name.clone(), entry);
        }
        index.insert(format!("{enum_name}::{}", v.name), entry);
    }
}
