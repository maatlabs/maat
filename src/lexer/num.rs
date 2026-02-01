//! Implements utilities for handling the lexing of number types.

use super::TokenKind;

/// Type of numeric suffix found during lexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suffix {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
}

impl Suffix {
    /// Converts this suffix to the appropriate TokenKind.
    #[inline]
    pub const fn to_token_kind(self) -> TokenKind {
        match self {
            Self::I8 => TokenKind::I8,
            Self::I16 => TokenKind::I16,
            Self::I32 => TokenKind::I32,
            Self::I64 => TokenKind::I64,
            Self::I128 => TokenKind::I128,
            Self::Isize => TokenKind::Isize,
            Self::U8 => TokenKind::U8,
            Self::U16 => TokenKind::U16,
            Self::U32 => TokenKind::U32,
            Self::U64 => TokenKind::U64,
            Self::U128 => TokenKind::U128,
            Self::Usize => TokenKind::Usize,
            Self::F32 => TokenKind::F32,
            Self::F64 => TokenKind::F64,
        }
    }
}

/// Matches integer suffixes. Returns the specific suffix type and its length.
/// Supports: i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize.
///
/// Boundary rule: Only match if suffix is followed by non-alphanumeric character.
/// This prevents matching partial suffixes like "i64" in "42i641", such as in Rust
/// where invalid suffixes like "i641" cause errors.
#[inline]
pub(super) fn match_int_suffix(bytes: &[u8]) -> Option<(Suffix, usize)> {
    #[inline]
    fn is_ident_continue(b: &u8) -> bool {
        b.is_ascii_alphanumeric() || *b == b'_'
    }

    let prefix = *bytes.first()?;

    // Check single-digit suffixes: i8/u8
    if bytes.get(1..2) == Some(b"8") && !bytes.get(2).is_some_and(is_ident_continue) {
        let suffix = if prefix == b'i' {
            Suffix::I8
        } else {
            Suffix::U8
        };
        return Some((suffix, 2));
    }

    // Check two-digit suffixes: i16/u16, i32/u32, i64/u64
    if let Some(digits) = bytes.get(1..3)
        && !bytes.get(3).is_some_and(is_ident_continue)
    {
        let suffix = match (prefix, digits) {
            (b'i', b"16") => Suffix::I16,
            (b'i', b"32") => Suffix::I32,
            (b'i', b"64") => Suffix::I64,
            (b'u', b"16") => Suffix::U16,
            (b'u', b"32") => Suffix::U32,
            (b'u', b"64") => Suffix::U64,
            _ => return None,
        };
        return Some((suffix, 3));
    }

    // Check three-digit suffixes: i128/u128
    if bytes.get(1..4) == Some(b"128") && !bytes.get(4).is_some_and(is_ident_continue) {
        let suffix = if prefix == b'i' {
            Suffix::I128
        } else {
            Suffix::U128
        };
        return Some((suffix, 4));
    }

    // Check size suffixes: isize/usize
    if bytes.get(1..5) == Some(b"size") && !bytes.get(5).is_some_and(is_ident_continue) {
        let suffix = if prefix == b'i' {
            Suffix::Isize
        } else {
            Suffix::Usize
        };
        return Some((suffix, 5));
    }

    None
}

/// Matches float suffixes. Returns the specific suffix type and its length.
/// Supports: f32, f64.
///
/// Boundary rule: Only match if suffix is followed by non-alphanumeric character.
/// This prevents matching partial suffixes like "f64" in "42f641", such as in Rust
/// where invalid suffixes like "f641" cause errors.
#[inline]
pub(super) fn match_float_suffix(bytes: &[u8]) -> Option<(Suffix, usize)> {
    #[inline]
    fn is_ident_continue(b: &u8) -> bool {
        b.is_ascii_alphanumeric() || *b == b'_'
    }

    if bytes.first()? != &b'f' {
        return None;
    }

    if let Some(digits) = bytes.get(1..3)
        && !bytes.get(3).is_some_and(is_ident_continue)
    {
        let suffix = match digits {
            b"32" => Suffix::F32,
            b"64" => Suffix::F64,
            _ => return None,
        };
        return Some((suffix, 3));
    }

    None
}
