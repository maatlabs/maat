use std::collections::HashMap;

use maat_bytecode::{MAX_GLOBALS, MAX_LOCALS};
use maat_errors::{CompileError, Result};

/// Compile-time symbols table for tracking variable bindings.
///
/// Maps variable names to storage indices, enabling the compiler
/// to resolve identifiers to global and local storage slots.
/// Supports nested scopes via an optional outer table reference,
/// allowing local bindings to shadow globals and enabling scope-chain
/// resolution during identifier lookup.
#[derive(Debug, Clone, Default)]
pub struct SymbolsTable {
    store: HashMap<String, Symbol>,
    num_definitions: usize,
    outer: Option<Box<SymbolsTable>>,
}

/// A resolved symbol with its scope and storage index.
///
/// Symbols are created during compilation when variables are defined,
/// and looked up when variables are referenced in expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The original variable name from source code.
    pub name: String,
    /// The scope in which this symbol was defined.
    pub scope: SymbolScope,
    /// The storage index assigned to this symbol.
    pub index: usize,
}

/// Scope classification for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolScope {
    /// A globally-scoped variable, accessible from any point in the program.
    Global,
    /// A locally-scoped variable, accessible only within its enclosing function.
    Local,
}

impl SymbolsTable {
    /// Creates a new empty symbols table at global scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new symbols table enclosed by the given outer table.
    ///
    /// Symbols defined in the enclosed table receive `SymbolScope::Local`,
    /// while resolution walks up to the outer table for undefined names.
    pub fn new_enclosed(outer: SymbolsTable) -> Self {
        Self {
            store: HashMap::new(),
            num_definitions: 0,
            outer: Some(Box::new(outer)),
        }
    }

    /// Returns the number of symbols defined in this scope.
    pub fn num_definitions(&self) -> usize {
        self.num_definitions
    }

    /// Defines a new symbol, assigning it the next available index.
    ///
    /// The symbol's scope is determined by whether this table has an
    /// outer (enclosing) table: global scope if no outer, local scope otherwise.
    ///
    /// # Errors
    ///
    /// Returns `CompileError::SymbolsTableOverflow` if the maximum number
    /// of global bindings has been reached (only checked for global scope).
    pub fn define_symbol(&mut self, name: &str) -> Result<&Symbol> {
        let scope = if self.outer.is_some() {
            SymbolScope::Local
        } else {
            SymbolScope::Global
        };

        match scope {
            SymbolScope::Global if self.num_definitions > MAX_GLOBALS => {
                return Err(CompileError::SymbolsTableOverflow {
                    max: MAX_GLOBALS,
                    name: name.to_string(),
                }
                .into());
            }
            SymbolScope::Local if self.num_definitions > MAX_LOCALS => {
                return Err(CompileError::LocalsOverflow {
                    max: MAX_LOCALS,
                    name: name.to_string(),
                }
                .into());
            }
            _ => {}
        }

        let symbol = Symbol {
            name: name.to_string(),
            scope,
            index: self.num_definitions,
        };
        self.store.insert(name.to_string(), symbol);
        self.num_definitions += 1;
        Ok(&self.store[name])
    }

    /// Resolves a symbol by name, walking up the scope chain.
    ///
    /// First checks the current scope, then delegates to the outer table
    /// if the symbol is not found locally.
    pub fn resolve_symbol(&self, name: &str) -> Option<&Symbol> {
        self.store.get(name).or_else(|| {
            self.outer
                .as_ref()
                .and_then(|outer| outer.resolve_symbol(name))
        })
    }

    /// Extracts the outer table from this enclosed table.
    ///
    /// Returns `None` if this is a top-level (global) table.
    pub fn take_outer(self) -> Option<SymbolsTable> {
        self.outer.map(|b| *b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table_define() {
        let mut table = SymbolsTable::new();

        let a = table.define_symbol("a").expect("should define 'a'");
        assert_eq!(a.name, "a");
        assert_eq!(a.scope, SymbolScope::Global);
        assert_eq!(a.index, 0);

        let b = table.define_symbol("b").expect("should define 'b'");
        assert_eq!(b.name, "b");
        assert_eq!(b.scope, SymbolScope::Global);
        assert_eq!(b.index, 1);
    }

    #[test]
    fn symbol_table_resolve() {
        let mut table = SymbolsTable::new();
        table.define_symbol("a").expect("should define 'a'");
        table.define_symbol("b").expect("should define 'b'");

        let a = table.resolve_symbol("a").expect("'a' should be defined");
        assert_eq!(a.index, 0);
        assert_eq!(a.scope, SymbolScope::Global);

        let b = table.resolve_symbol("b").expect("'b' should be defined");
        assert_eq!(b.index, 1);
        assert_eq!(b.scope, SymbolScope::Global);

        assert!(
            table.resolve_symbol("c").is_none(),
            "undefined symbol should return None"
        );
    }

    #[test]
    fn define_resolve_local() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a").expect("should define 'a'");
        global.define_symbol("b").expect("should define 'b'");

        let mut local = SymbolsTable::new_enclosed(global);
        local.define_symbol("c").expect("should define 'c'");
        local.define_symbol("d").expect("should define 'd'");

        let expected = [
            ("a", SymbolScope::Global, 0),
            ("b", SymbolScope::Global, 1),
            ("c", SymbolScope::Local, 0),
            ("d", SymbolScope::Local, 1),
        ];

        for (name, expected_scope, expected_index) in expected {
            let symbol = local
                .resolve_symbol(name)
                .unwrap_or_else(|| panic!("'{name}' should be resolvable"));
            assert_eq!(symbol.scope, expected_scope, "wrong scope for '{name}'");
            assert_eq!(symbol.index, expected_index, "wrong index for '{name}'");
        }
    }

    #[test]
    fn resolve_nested_local() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a").expect("should define 'a'");
        global.define_symbol("b").expect("should define 'b'");

        let mut first_local = SymbolsTable::new_enclosed(global);
        first_local.define_symbol("c").expect("should define 'c'");
        first_local.define_symbol("d").expect("should define 'd'");

        let mut second_local = SymbolsTable::new_enclosed(first_local);
        second_local.define_symbol("e").expect("should define 'e'");
        second_local.define_symbol("f").expect("should define 'f'");

        let expected = [
            ("a", SymbolScope::Global, 0),
            ("b", SymbolScope::Global, 1),
            ("e", SymbolScope::Local, 0),
            ("f", SymbolScope::Local, 1),
        ];

        for (name, expected_scope, expected_index) in expected {
            let symbol = second_local
                .resolve_symbol(name)
                .unwrap_or_else(|| panic!("'{name}' should be resolvable"));
            assert_eq!(symbol.scope, expected_scope, "wrong scope for '{name}'");
            assert_eq!(symbol.index, expected_index, "wrong index for '{name}'");
        }
    }
}
