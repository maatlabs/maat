use crate::TokenKind;

/// Metadata produced by number-lexing callbacks.
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
    Fe,
}

impl NumSuffix {
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
            Self::Fe => TokenKind::Fe,
        }
    }
}

/// Matches numeric type suffixes. Returns the specific suffix type and its length.
/// Supports: i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, fe.
#[inline]
pub fn match_num_suffix(bytes: &[u8]) -> Option<(NumSuffix, usize)> {
    let first_byte = *bytes.first()?;
    if first_byte == b'f' {
        if bytes.get(1..2) == Some(b"e") && !bytes.get(2).is_some_and(is_ident_continue) {
            return Some((NumSuffix::Fe, 2));
        }
        return None;
    }
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

#[inline]
fn is_ident_continue(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}
