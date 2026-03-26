//! Reserved identifiers and type names for Maat.
//!
//! This module centralizes all name-reservation logic, preventing user code
//! from shadowing language keywords, future keywords, or builtin type names.
//!
//! # Categories
//!
//! - **Reserved keywords**: Rust keywords that are not yet Maat lexer keywords
//!   but must be reserved for forward compatibility. These cannot be used as
//!   variable names, function names, parameter names, struct field names, or
//!   any other binding site.
//!
//! - **Reserved type names**: Primitive types and standard-library types that
//!   cannot be redefined by user code via `struct`, `enum`, or `trait`
//!   declarations.
//!
//! # Lookup strategy
//!
//! Both tables are compile-time sorted slices searched via binary search,
//! giving O(log n) lookups with zero allocation.

/// Rust keywords that are not yet Maat lexer keywords.
///
/// These arrive at the parser as `TokenKind::Ident` because the lexer does
/// not recognize them, but they must be rejected in all binding positions
/// to maintain forward compatibility with Rust's keyword set.
///
/// Sorted lexicographically for binary search.
pub const RESERVED_KEYWORDS: &[&str] = &[
    "abstract", "async", "await", "become", "box", "const", "crate", "do", "dyn", "extern",
    "final", "move", "override", "priv", "ref", "static", "super", "try", "type", "typeof",
    "unsafe", "unsized", "virtual", "yield",
];

/// Builtin type names that cannot be redefined by user declarations.
///
/// Includes primitive types, standard-library collection types, and
/// builtin enum types. Sorted lexicographically for binary search.
pub const RESERVED_TYPE_NAMES: &[&str] = &[
    "Map",
    "Option",
    "ParseIntError",
    "Result",
    "Set",
    "Vector",
    "bool",
    "char",
    "i128",
    "i16",
    "i32",
    "i64",
    "i8",
    "isize",
    "str",
    "u128",
    "u16",
    "u32",
    "u64",
    "u8",
    "usize",
];

/// Returns `true` if `name` is a reserved keyword that cannot appear as a
/// binding identifier.
#[inline]
pub fn is_reserved_keyword(name: &str) -> bool {
    RESERVED_KEYWORDS.binary_search(&name).is_ok()
}

/// Returns `true` if `name` is a reserved type name that cannot be used
/// for user-defined `struct`, `enum`, or `trait` declarations.
#[inline]
pub fn is_reserved_type_name(name: &str) -> bool {
    RESERVED_TYPE_NAMES.binary_search(&name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_keywords_sorted() {
        let mut sorted = RESERVED_KEYWORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            RESERVED_KEYWORDS,
            &sorted[..],
            "RESERVED_KEYWORDS must be sorted for binary search"
        );
    }

    #[test]
    fn reserved_type_names_sorted() {
        let mut sorted = RESERVED_TYPE_NAMES.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            RESERVED_TYPE_NAMES,
            &sorted[..],
            "RESERVED_TYPE_NAMES must be sorted for binary search"
        );
    }

    #[test]
    fn keyword_lookup() {
        assert!(is_reserved_keyword("type"));
        assert!(is_reserved_keyword("async"));
        assert!(is_reserved_keyword("const"));
        assert!(is_reserved_keyword("yield"));
        assert!(!is_reserved_keyword("let"));
        assert!(!is_reserved_keyword("foo"));
    }

    #[test]
    fn type_name_lookup() {
        assert!(is_reserved_type_name("Option"));
        assert!(is_reserved_type_name("i64"));
        assert!(is_reserved_type_name("Vector"));
        assert!(!is_reserved_type_name("Foo"));
        assert!(!is_reserved_type_name("MyStruct"));
    }
}
