use std::collections::HashSet;

use indexmap::IndexMap;
use maat_bytecode::{MAX_GLOBALS, MAX_LOCALS};
use maat_errors::{CompileError, CompileErrorKind, Result};

/// Compile-time symbols table for tracking variable bindings.
#[derive(Debug, Clone, Default)]
pub struct SymbolsTable {
    store: IndexMap<String, Symbol>,
    num_definitions: usize,
    max_definitions: usize,
    outer: Option<Box<SymbolsTable>>,
    free_vars: Vec<Symbol>,
    masked: HashSet<String>,
    block_scopes: Vec<BlockScope>,
}

#[derive(Debug, Clone)]
struct BlockScope {
    num_definitions_at_entry: usize,
    names: Vec<String>,
    shadowed: Vec<Symbol>,
}

/// A resolved symbol with its scope and storage index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub scope: SymbolScope,
    pub index: usize,
    pub mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolScope {
    Global,
    Local,
    Builtin,
    Free,
    Function,
}

impl SymbolsTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_enclosed(outer: SymbolsTable) -> Self {
        Self {
            store: IndexMap::new(),
            num_definitions: 0,
            max_definitions: 0,
            outer: Some(Box::new(outer)),
            free_vars: Vec::new(),
            masked: HashSet::new(),
            block_scopes: Vec::new(),
        }
    }

    pub fn max_definitions(&self) -> usize {
        self.max_definitions
    }

    pub fn push_block_scope(&mut self) {
        self.block_scopes.push(BlockScope {
            num_definitions_at_entry: self.num_definitions,
            names: Vec::new(),
            shadowed: Vec::new(),
        });
    }

    pub fn pop_block_scope(&mut self) {
        if let Some(block) = self.block_scopes.pop() {
            for name in &block.names {
                self.store.swap_remove(name);
            }
            for sym in block.shadowed {
                self.store.insert(sym.name.clone(), sym);
            }
            self.num_definitions = block.num_definitions_at_entry;
        }
    }

    pub fn define_symbol(&mut self, name: &str, mutable: bool) -> Result<&Symbol> {
        let scope = if self.outer.is_some() {
            SymbolScope::Local
        } else {
            SymbolScope::Global
        };

        if self
            .store
            .get(name)
            .is_some_and(|existing| existing.scope == scope)
        {
            let is_shadow = self
                .block_scopes
                .last()
                .is_some_and(|block| self.store[name].index < block.num_definitions_at_entry);
            if !is_shadow {
                self.masked.remove(name);
                // Update mutability on rebinding.
                if let Some(sym) = self.store.get_mut(name) {
                    sym.mutable = mutable;
                }
                return Ok(&self.store[name]);
            }
            // Save the original symbol so it can be restored when the
            // block scope exits.
            if let Some(block) = self.block_scopes.last_mut() {
                block.shadowed.push(self.store[name].clone());
            }
        }
        match scope {
            SymbolScope::Global if self.num_definitions > MAX_GLOBALS => {
                return Err(CompileError::new(CompileErrorKind::SymbolsTableOverflow {
                    max: MAX_GLOBALS,
                    name: name.to_string(),
                })
                .into());
            }
            SymbolScope::Local if self.num_definitions > MAX_LOCALS => {
                return Err(CompileError::new(CompileErrorKind::LocalsOverflow {
                    max: MAX_LOCALS,
                    name: name.to_string(),
                })
                .into());
            }
            _ => {}
        }
        let symbol = Symbol {
            name: name.to_string(),
            scope,
            index: self.num_definitions,
            mutable,
        };
        self.store.insert(name.to_string(), symbol);
        self.num_definitions += 1;
        if self.num_definitions > self.max_definitions {
            self.max_definitions = self.num_definitions;
        }
        if let Some(block) = self.block_scopes.last_mut() {
            block.names.push(name.to_string());
        }
        Ok(&self.store[name])
    }

    pub fn define_builtin(&mut self, index: usize, name: &str) -> &Symbol {
        let symbol = Symbol {
            name: name.to_string(),
            scope: SymbolScope::Builtin,
            index,
            mutable: false,
        };
        self.store.insert(name.to_string(), symbol);
        &self.store[name]
    }

    pub fn define_function_name(&mut self, name: &str) {
        let symbol = Symbol {
            name: name.to_string(),
            scope: SymbolScope::Function,
            index: 0,
            mutable: false,
        };
        self.store.insert(name.to_string(), symbol);
    }

    pub fn resolve_symbol(&mut self, name: &str) -> Option<Symbol> {
        if let Some(symbol) = self.store.get(name)
            && !self.masked.contains(name)
        {
            return Some(symbol.clone());
        }
        let outer = self.outer.as_mut()?;
        let outer_symbol = outer.resolve_symbol(name)?;
        match outer_symbol.scope {
            SymbolScope::Global | SymbolScope::Builtin => Some(outer_symbol),
            _ => Some(self.define_free(outer_symbol)),
        }
    }

    pub fn free_vars(&self) -> &[Symbol] {
        &self.free_vars
    }

    pub fn take_outer(self) -> Option<SymbolsTable> {
        self.outer.map(|b| *b)
    }

    pub fn global_symbol_names(&self) -> Vec<String> {
        self.store
            .iter()
            .filter(|(name, sym)| sym.scope == SymbolScope::Global && !self.masked.contains(*name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn mask_symbol(&mut self, name: &str) {
        self.masked.insert(name.to_string());
    }

    fn define_free(&mut self, original: Symbol) -> Symbol {
        let index = self.free_vars.len();
        let mutable = original.mutable;
        let name = original.name.clone();
        self.free_vars.push(original);

        let symbol = Symbol {
            name: name.clone(),
            scope: SymbolScope::Free,
            index,
            mutable,
        };
        self.store.insert(name, symbol.clone());
        symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table_define() {
        let mut table = SymbolsTable::new();

        let a = table.define_symbol("a", false).expect("should define 'a'");
        assert_eq!(a.name, "a");
        assert_eq!(a.scope, SymbolScope::Global);
        assert_eq!(a.index, 0);

        let b = table.define_symbol("b", false).expect("should define 'b'");
        assert_eq!(b.name, "b");
        assert_eq!(b.scope, SymbolScope::Global);
        assert_eq!(b.index, 1);
    }

    #[test]
    fn symbol_table_resolve() {
        let mut table = SymbolsTable::new();
        table.define_symbol("a", false).expect("should define 'a'");
        table.define_symbol("b", false).expect("should define 'b'");

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
        global.define_symbol("a", false).expect("should define 'a'");
        global.define_symbol("b", false).expect("should define 'b'");

        let mut local = SymbolsTable::new_enclosed(global);
        local.define_symbol("c", false).expect("should define 'c'");
        local.define_symbol("d", false).expect("should define 'd'");

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
    fn define_resolve_builtins() {
        let mut global = SymbolsTable::new();
        let expected = [
            ("a", SymbolScope::Builtin, 0),
            ("c", SymbolScope::Builtin, 1),
            ("e", SymbolScope::Builtin, 2),
            ("f", SymbolScope::Builtin, 3),
        ];
        for (i, &(name, _, _)) in expected.iter().enumerate() {
            global.define_builtin(i, name);
        }
        let first_local = SymbolsTable::new_enclosed(global);
        let mut second_local = SymbolsTable::new_enclosed(first_local);

        for &(name, expected_scope, expected_index) in &expected {
            let symbol = second_local
                .resolve_symbol(name)
                .unwrap_or_else(|| panic!("'{name}' should be resolvable"));
            assert_eq!(symbol.scope, expected_scope, "wrong scope for '{name}'");
            assert_eq!(symbol.index, expected_index, "wrong index for '{name}'");
        }
    }

    #[test]
    fn resolve_nested_local() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a", false).expect("should define 'a'");
        global.define_symbol("b", false).expect("should define 'b'");

        let mut first_local = SymbolsTable::new_enclosed(global);
        first_local
            .define_symbol("c", false)
            .expect("should define 'c'");
        first_local
            .define_symbol("d", false)
            .expect("should define 'd'");
        let mut second_local = SymbolsTable::new_enclosed(first_local);
        second_local
            .define_symbol("e", false)
            .expect("should define 'e'");
        second_local
            .define_symbol("f", false)
            .expect("should define 'f'");

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

    #[test]
    fn resolve_free() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a", false).expect("should define 'a'");
        global.define_symbol("b", false).expect("should define 'b'");

        let mut first_local = SymbolsTable::new_enclosed(global);
        first_local
            .define_symbol("c", false)
            .expect("should define 'c'");
        first_local
            .define_symbol("d", false)
            .expect("should define 'd'");
        let mut second_local = SymbolsTable::new_enclosed(first_local);
        second_local
            .define_symbol("e", false)
            .expect("should define 'e'");
        second_local
            .define_symbol("f", false)
            .expect("should define 'f'");

        let expected = [
            ("a", SymbolScope::Global, 0),
            ("b", SymbolScope::Global, 1),
            ("c", SymbolScope::Free, 0),
            ("d", SymbolScope::Free, 1),
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
        let expected_free = [
            Symbol {
                name: "c".to_string(),
                scope: SymbolScope::Local,
                index: 0,
                mutable: false,
            },
            Symbol {
                name: "d".to_string(),
                scope: SymbolScope::Local,
                index: 1,
                mutable: false,
            },
        ];
        assert_eq!(
            second_local.free_vars().len(),
            expected_free.len(),
            "wrong number of free variables"
        );
        for (actual, expected) in second_local.free_vars().iter().zip(&expected_free) {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn resolve_unresolvable_free() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a", false).expect("should define 'a'");

        let mut first_local = SymbolsTable::new_enclosed(global);
        first_local
            .define_symbol("c", false)
            .expect("should define 'c'");
        let mut second_local = SymbolsTable::new_enclosed(first_local);
        second_local
            .define_symbol("e", false)
            .expect("should define 'e'");
        second_local
            .define_symbol("f", false)
            .expect("should define 'f'");

        let expected = [
            ("a", SymbolScope::Global, 0),
            ("c", SymbolScope::Free, 0),
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
        assert!(
            second_local.resolve_symbol("b").is_none(),
            "should not resolve undefined 'b'"
        );
        assert!(
            second_local.resolve_symbol("d").is_none(),
            "should not resolve undefined 'd'"
        );
    }

    #[test]
    fn define_and_resolve_function_name() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a", false).expect("should define 'a'");

        let mut local = SymbolsTable::new_enclosed(global);
        local.define_function_name("my_fn");
        local.define_symbol("b", false).expect("should define 'b'");

        let fn_sym = local
            .resolve_symbol("my_fn")
            .expect("function name should resolve");
        assert_eq!(fn_sym.scope, SymbolScope::Function);
        assert_eq!(fn_sym.index, 0);

        let b_sym = local.resolve_symbol("b").expect("'b' should resolve");
        assert_eq!(b_sym.scope, SymbolScope::Local);
        assert_eq!(b_sym.index, 0);

        assert_eq!(
            local.max_definitions(),
            1,
            "function name should not increment num_definitions"
        );
    }

    #[test]
    fn shadowing_function_name() {
        let mut global = SymbolsTable::new();
        global.define_symbol("a", false).expect("should define 'a'");

        let mut local = SymbolsTable::new_enclosed(global);
        local.define_function_name("a");

        let sym = local.resolve_symbol("a").expect("'a' should resolve");
        assert_eq!(
            sym.scope,
            SymbolScope::Function,
            "function name should shadow outer 'a'"
        );
        assert_eq!(sym.index, 0);
    }
}
