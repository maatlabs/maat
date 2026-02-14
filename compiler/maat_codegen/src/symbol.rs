use std::collections::HashMap;

/// Compile-time symbols table for tracking variable bindings.
///
/// Maps variable names to storage indices, enabling the compiler
/// to resolve identifiers to global and local storage slots.
/// Each defined variable receives a unique, monotonically increasing index.
#[derive(Debug, Clone, Default)]
pub struct Table {
    store: HashMap<String, Symbol>,
    num_definitions: usize,
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
    pub scope: Scope,
    /// The storage index assigned to this symbol.
    pub index: usize,
}

/// Scope classification for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// A globally-scoped variable, accessible from any point in the program.
    Global,
}

impl Table {
    /// Creates a new empty symbols table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Defines a new symbol, assigning it the next available index.
    ///
    /// Returns a reference to the newly created symbol.
    pub fn define_symbol(&mut self, name: &str) -> &Symbol {
        let symbol = Symbol {
            name: name.to_string(),
            scope: Scope::Global,
            index: self.num_definitions,
        };
        self.store.insert(name.to_string(), symbol);
        self.num_definitions += 1;
        &self.store[name]
    }

    /// Resolves a symbol by name.
    ///
    /// Returns `None` if the symbol has not been defined.
    pub fn resolve_symbol(&self, name: &str) -> Option<&Symbol> {
        self.store.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table_define() {
        let mut table = Table::new();

        let a = table.define_symbol("a");
        assert_eq!(a.name, "a");
        assert_eq!(a.scope, Scope::Global);
        assert_eq!(a.index, 0);

        let b = table.define_symbol("b");
        assert_eq!(b.name, "b");
        assert_eq!(b.scope, Scope::Global);
        assert_eq!(b.index, 1);
    }

    #[test]
    fn symbol_table_resolve() {
        let mut table = Table::new();
        table.define_symbol("a");
        table.define_symbol("b");

        let a = table.resolve_symbol("a").expect("'a' should be defined");
        assert_eq!(a.index, 0);
        assert_eq!(a.scope, Scope::Global);

        let b = table.resolve_symbol("b").expect("'b' should be defined");
        assert_eq!(b.index, 1);
        assert_eq!(b.scope, Scope::Global);

        assert!(
            table.resolve_symbol("c").is_none(),
            "undefined symbol should return None"
        );
    }
}
