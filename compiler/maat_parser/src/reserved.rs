pub const RESERVED_KEYWORDS: &[&str] = &[
    "abstract", "async", "await", "become", "box", "const", "crate", "do", "dyn", "extern",
    "final", "move", "override", "priv", "ref", "static", "super", "try", "type", "typeof",
    "unsafe", "unsized", "virtual", "yield",
];

pub const RESERVED_TYPE_NAMES: &[&str] = &[
    "Felt",
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

#[inline]
pub fn is_reserved_keyword(name: &str) -> bool {
    RESERVED_KEYWORDS.binary_search(&name).is_ok()
}

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
