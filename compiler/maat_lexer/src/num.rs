//! Implements utilities for handling the lexing of number types.

use crate::TokenKind;

/// Metadata produced by number-lexing callbacks.
///
/// Carries the resolved [`TokenKind`] and the byte length of the value portion
/// (digits and radix prefix, excluding any type suffix) so the wrapper can
/// split the current token into the correct `literal` and `span`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumToken {
    pub kind: TokenKind,
    pub value_len: u32,
}

/// Type of numeric suffix found during lexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumSuffix {
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
}

impl NumSuffix {
    /// Converts this suffix to the appropriate token kind.
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
pub fn match_int_suffix(bytes: &[u8]) -> Option<(NumSuffix, usize)> {
    let first_byte = *bytes.first()?;
    if first_byte != b'i' && first_byte != b'u' {
        return None;
    }
    // Check single-digit suffixes: i8/u8
    if bytes.get(1..2) == Some(b"8") && !bytes.get(2).is_some_and(is_ident_continue) {
        let suffix = if first_byte == b'i' {
            NumSuffix::I8
        } else {
            NumSuffix::U8
        };
        return Some((suffix, 2));
    }
    // Check two-digit suffixes: i16/u16, i32/u32, i64/u64
    if let Some(digits) = bytes.get(1..3)
        && !bytes.get(3).is_some_and(is_ident_continue)
    {
        let suffix = match (first_byte, digits) {
            (b'i', b"16") => NumSuffix::I16,
            (b'i', b"32") => NumSuffix::I32,
            (b'i', b"64") => NumSuffix::I64,
            (b'u', b"16") => NumSuffix::U16,
            (b'u', b"32") => NumSuffix::U32,
            (b'u', b"64") => NumSuffix::U64,
            _ => return None,
        };
        return Some((suffix, 3));
    }
    // Check three-digit suffixes: i128/u128
    if bytes.get(1..4) == Some(b"128") && !bytes.get(4).is_some_and(is_ident_continue) {
        let suffix = if first_byte == b'i' {
            NumSuffix::I128
        } else {
            NumSuffix::U128
        };
        return Some((suffix, 4));
    }
    // Check pointer-size suffixes: isize/usize
    if bytes.get(1..5) == Some(b"size") && !bytes.get(5).is_some_and(is_ident_continue) {
        let suffix = if first_byte == b'i' {
            NumSuffix::Isize
        } else {
            NumSuffix::Usize
        };
        return Some((suffix, 5));
    }
    None
}

/// Returns true if this `byte` is an ASCII alphanumeric character or
/// an underscore (`_`).
#[inline]
fn is_ident_continue(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}
