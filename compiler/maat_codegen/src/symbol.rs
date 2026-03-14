use std::collections::HashSet;

use indexmap::IndexMap;
use maat_bytecode::{MAX_GLOBALS, MAX_LOCALS};
use maat_errors::{CompileError, CompileErrorKind, Result};

/// Compile-time symbols table for tracking variable bindings.
///
/// Maps variable names to storage indices, enabling the compiler
/// to resolve identifiers to global, local, free, and function-scoped
/// storage slots. Supports nested scopes via an optional outer table
/// reference, allowing local bindings to shadow globals and enabling
/// scope-chain resolution during identifier lookup.
///
/// When resolving a symbol that lives in a non-global, non-builtin
/// enclosing scope, the table automatically promotes it to a free
/// variable, recording the original symbol for the compiler to emit
/// the appropriate load instructions at closure-creation time.
///
/// Symbols can be masked to temporarily hide them from resolution
/// without removing their storage indices. This supports multi-module
/// compilation where a shared compiler must prevent cross-module
/// symbol leaks while preserving global index assignments.
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

/// Tracks symbols defined within a single block scope.
///
/// When the block exits, all symbols recorded here are removed from the
/// store and `num_definitions` is restored, enabling local slot reuse
/// across non-overlapping blocks within the same function. Any symbols
/// that were shadowed are restored to their original bindings.
#[derive(Debug, Clone)]
struct BlockScope {
    num_definitions_at_entry: usize,
    names: Vec<String>,
    shadowed: Vec<Symbol>,
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
    /// Whether this binding was declared with `let mut`.
    pub mutable: bool,
}

/// Scope classification for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolScope {
    /// A globally-scoped variable, accessible from any point in the program.
    Global,
    /// A locally-scoped variable, accessible only within its enclosing function.
    Local,
    /// A built-in function, resolved at compile time by index.
    Builtin,
    /// A free variable captured from an enclosing scope.
    Free,
    /// The enclosing function's own name, enabling recursive self-reference.
    Function,
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
            store: IndexMap::new(),
            num_definitions: 0,
            max_definitions: 0,
            outer: Some(Box::new(outer)),
            free_vars: Vec::new(),
            masked: HashSet::new(),
            block_scopes: Vec::new(),
        }
    }

    /// Returns the number of local slots the VM must allocate for this scope.
    ///
    /// This is the high-water mark of simultaneously live locals,
    /// accounting for slot reuse across non-overlapping block scopes.
    pub fn max_definitions(&self) -> usize {
        self.max_definitions
    }

    /// Enters a new block scope, saving the current definition count.
    ///
    /// Variables defined after this call are tracked so they can be
    /// removed when `pop_block_scope` is called, enabling lexical
    /// block scoping within a single function.
    pub fn push_block_scope(&mut self) {
        self.block_scopes.push(BlockScope {
            num_definitions_at_entry: self.num_definitions,
            names: Vec::new(),
            shadowed: Vec::new(),
        });
    }

    /// Exits the current block scope, removing locally defined symbols.
    ///
    /// All symbols defined since the matching `push_block_scope` are
    /// removed from the store and `num_definitions` is restored,
    /// allowing the local slot indices to be reused by subsequent blocks.
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

    /// Defines a symbol, assigning it the next available index.
    ///
    /// If a symbol with the same name already exists at the same scope
    /// level (Global or Local), the existing index is reused. This
    /// enables idiomatic rebinding inside loops (`let x = x + 1;`)
    /// without allocating a new storage slot on each iteration.
    ///
    /// The symbol's scope is determined by whether this table has an
    /// outer (enclosing) table: global scope if no outer, local scope otherwise.
    ///
    /// # Errors
    ///
    /// Returns `CompileErrorKind::SymbolsTableOverflow` if the maximum number
    /// of global bindings has been reached (only checked for global scope).
    pub fn define_symbol(&mut self, name: &str, mutable: bool) -> Result<&Symbol> {
        let scope = if self.outer.is_some() {
            SymbolScope::Local
        } else {
            SymbolScope::Global
        };
        // Reuse the existing index when rebinding a name at the same scope level,
        // but only if the symbol was defined within the current block scope.
        // If the existing symbol predates the current block, this is a shadow
        // and must receive a new slot so the outer binding is preserved on exit.
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

    /// Defines a built-in function symbol with a fixed index.
    ///
    /// Built-in symbols are stored in the table but do not increment
    /// `num_definitions`, as they occupy no global or local storage slots.
    /// They are resolved by index at runtime via `OpGetBuiltin`.
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

    /// Defines the enclosing function's own name for recursive self-reference.
    ///
    /// The symbol is stored at index 0 with `SymbolScope::Function` but does
    /// **not** increment `num_definitions`, since it occupies no local slot.
    /// At runtime, the VM resolves this via `OpCurrentClosure`.
    pub fn define_function_name(&mut self, name: &str) {
        let symbol = Symbol {
            name: name.to_string(),
            scope: SymbolScope::Function,
            index: 0,
            mutable: false,
        };
        self.store.insert(name.to_string(), symbol);
    }

    /// Resolves a symbol by name, walking up the scope chain.
    ///
    /// When a symbol is found in an enclosing (non-global, non-builtin) scope,
    /// it is automatically promoted to a free variable in the current scope.
    /// This records the capture chain so the compiler can emit the correct
    /// load instructions when creating closures.
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

    /// Returns the free variables captured by this scope.
    ///
    /// The returned slice is ordered by free variable index. The compiler
    /// uses this to emit load instructions for each captured variable
    /// before creating the closure.
    pub fn free_vars(&self) -> &[Symbol] {
        &self.free_vars
    }

    /// Extracts the outer table from this enclosed table.
    ///
    /// Returns `None` if this is a top-level (global) table.
    pub fn take_outer(self) -> Option<SymbolsTable> {
        self.outer.map(|b| *b)
    }

    /// Returns the names of all unmasked, globally-scoped symbols.
    ///
    /// Useful for snapshotting the global namespace before compiling a
    /// module, so that newly-defined globals can be identified afterward.
    pub fn global_symbol_names(&self) -> Vec<String> {
        self.store
            .iter()
            .filter(|(name, sym)| sym.scope == SymbolScope::Global && !self.masked.contains(*name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Masks a symbol, hiding it from resolution without removing its
    /// storage index.
    ///
    /// Masked symbols retain their global indices so that re-defining
    /// them (via import injection) reuses the same slot. This prevents
    /// cross-module symbol leaks in shared-compiler pipelines while
    /// keeping the bytecode's index references valid.
    pub fn mask_symbol(&mut self, name: &str) {
        self.masked.insert(name.to_string());
    }

    /// Promotes an outer-scope symbol to a free variable in the current scope.
    ///
    /// The original symbol is appended to `free_vars` (preserving the
    /// capture order) and a new `Free`-scoped symbol is stored in the
    /// current table. Returns the newly created free symbol.
    fn define_free(&mut self, original: Symbol) -> Symbol {
        let index = self.free_vars.len();
        let mutable = original.mutable;
        self.free_vars.push(original);

        let symbol = Symbol {
            name: self.free_vars[index].name.clone(),
            scope: SymbolScope::Free,
            index,
            mutable,
        };
        self.store.insert(symbol.name.clone(), symbol.clone());
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
