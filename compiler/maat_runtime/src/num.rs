//! Runtime integer handling for Maat.

use std::fmt;

use maat_ast::NumKind;
use maat_field::{Felt, FieldElement, from_i64};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Integer {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
}

pub enum WideInt {
    Signed(i128),
    Unsigned(u128),
}

macro_rules! checked_binop {
    ($lhs:expr, $rhs:expr, $method:ident) => {
        match ($lhs, $rhs) {
            (Integer::I8(l), Integer::I8(r)) => l.$method(r).map(Integer::I8),
            (Integer::I16(l), Integer::I16(r)) => l.$method(r).map(Integer::I16),
            (Integer::I32(l), Integer::I32(r)) => l.$method(r).map(Integer::I32),
            (Integer::I64(l), Integer::I64(r)) => l.$method(r).map(Integer::I64),
            (Integer::I128(l), Integer::I128(r)) => l.$method(r).map(Integer::I128),
            (Integer::Isize(l), Integer::Isize(r)) => l.$method(r).map(Integer::Isize),
            (Integer::U8(l), Integer::U8(r)) => l.$method(r).map(Integer::U8),
            (Integer::U16(l), Integer::U16(r)) => l.$method(r).map(Integer::U16),
            (Integer::U32(l), Integer::U32(r)) => l.$method(r).map(Integer::U32),
            (Integer::U64(l), Integer::U64(r)) => l.$method(r).map(Integer::U64),
            (Integer::U128(l), Integer::U128(r)) => l.$method(r).map(Integer::U128),
            (Integer::Usize(l), Integer::Usize(r)) => l.$method(r).map(Integer::Usize),
            _ => None,
        }
    };
}

macro_rules! bitwise_binop {
    ($lhs:expr, $rhs:expr, $op:tt) => {
        match ($lhs, $rhs) {
            (Integer::I8(l), Integer::I8(r)) => Some(Integer::I8(l $op r)),
            (Integer::I16(l), Integer::I16(r)) => Some(Integer::I16(l $op r)),
            (Integer::I32(l), Integer::I32(r)) => Some(Integer::I32(l $op r)),
            (Integer::I64(l), Integer::I64(r)) => Some(Integer::I64(l $op r)),
            (Integer::I128(l), Integer::I128(r)) => Some(Integer::I128(l $op r)),
            (Integer::Isize(l), Integer::Isize(r)) => Some(Integer::Isize(l $op r)),
            (Integer::U8(l), Integer::U8(r)) => Some(Integer::U8(l $op r)),
            (Integer::U16(l), Integer::U16(r)) => Some(Integer::U16(l $op r)),
            (Integer::U32(l), Integer::U32(r)) => Some(Integer::U32(l $op r)),
            (Integer::U64(l), Integer::U64(r)) => Some(Integer::U64(l $op r)),
            (Integer::U128(l), Integer::U128(r)) => Some(Integer::U128(l $op r)),
            (Integer::Usize(l), Integer::Usize(r)) => Some(Integer::Usize(l $op r)),
            _ => None,
        }
    };
}

macro_rules! checked_unary {
    ($self:expr, $arg:expr, $method:ident) => {
        match $self {
            Integer::I8(v) => v.$method($arg).map(Integer::I8),
            Integer::I16(v) => v.$method($arg).map(Integer::I16),
            Integer::I32(v) => v.$method($arg).map(Integer::I32),
            Integer::I64(v) => v.$method($arg).map(Integer::I64),
            Integer::I128(v) => v.$method($arg).map(Integer::I128),
            Integer::Isize(v) => v.$method($arg).map(Integer::Isize),
            Integer::U8(v) => v.$method($arg).map(Integer::U8),
            Integer::U16(v) => v.$method($arg).map(Integer::U16),
            Integer::U32(v) => v.$method($arg).map(Integer::U32),
            Integer::U64(v) => v.$method($arg).map(Integer::U64),
            Integer::U128(v) => v.$method($arg).map(Integer::U128),
            Integer::Usize(v) => v.$method($arg).map(Integer::Usize),
        }
    };
}

impl Integer {
    /// Encodes this integer value as a field element.
    pub fn to_felt(&self) -> Felt {
        match self {
            Self::I8(v) => from_i64(i64::from(*v)),
            Self::I16(v) => from_i64(i64::from(*v)),
            Self::I32(v) => from_i64(i64::from(*v)),
            Self::I64(v) => from_i64(*v),
            Self::Isize(v) => from_i64(*v as i64),
            Self::U8(v) => Felt::new(u64::from(*v)),
            Self::U16(v) => Felt::new(u64::from(*v)),
            Self::U32(v) => Felt::new(u64::from(*v)),
            Self::U64(v) => Felt::new(*v),
            Self::Usize(v) => Felt::new(*v as u64),
            Self::I128(_) | Self::U128(_) => Felt::ZERO,
        }
    }

    pub fn zero(&self) -> Self {
        match self {
            Self::I8(_) => Self::I8(0),
            Self::I16(_) => Self::I16(0),
            Self::I32(_) => Self::I32(0),
            Self::I64(_) => Self::I64(0),
            Self::I128(_) => Self::I128(0),
            Self::Isize(_) => Self::Isize(0),
            Self::U8(_) => Self::U8(0),
            Self::U16(_) => Self::U16(0),
            Self::U32(_) => Self::U32(0),
            Self::U64(_) => Self::U64(0),
            Self::U128(_) => Self::U128(0),
            Self::Usize(_) => Self::Usize(0),
        }
    }

    pub fn one(&self) -> Self {
        match self {
            Self::I8(_) => Self::I8(1),
            Self::I16(_) => Self::I16(1),
            Self::I32(_) => Self::I32(1),
            Self::I64(_) => Self::I64(1),
            Self::I128(_) => Self::I128(1),
            Self::Isize(_) => Self::Isize(1),
            Self::U8(_) => Self::U8(1),
            Self::U16(_) => Self::U16(1),
            Self::U32(_) => Self::U32(1),
            Self::U64(_) => Self::U64(1),
            Self::U128(_) => Self::U128(1),
            Self::Usize(_) => Self::Usize(1),
        }
    }

    pub fn cast_to(self, target: NumKind) -> Result<Self, String> {
        Self::from_wide(self.to_wide(), target)
    }

    pub fn from_wide(wide: WideInt, target: NumKind) -> Result<Self, String> {
        macro_rules! narrow {
            ($ty:ty, $variant:ident, $name:expr) => {
                match wide {
                    WideInt::Signed(v) => <$ty>::try_from(v)
                        .map(|v| Integer::$variant(v))
                        .map_err(|_| format!("value {} out of range for {}", v, $name)),
                    WideInt::Unsigned(v) => <$ty>::try_from(v)
                        .map(|v| Integer::$variant(v))
                        .map_err(|_| format!("value {} out of range for {}", v, $name)),
                }
            };
        }

        match target {
            NumKind::Int { .. } | NumKind::I64 => narrow!(i64, I64, "i64"),
            NumKind::I8 => narrow!(i8, I8, "i8"),
            NumKind::I16 => narrow!(i16, I16, "i16"),
            NumKind::I32 => narrow!(i32, I32, "i32"),
            NumKind::I128 => narrow!(i128, I128, "i128"),
            NumKind::Isize => narrow!(isize, Isize, "isize"),
            NumKind::U8 => narrow!(u8, U8, "u8"),
            NumKind::U16 => narrow!(u16, U16, "u16"),
            NumKind::U32 => narrow!(u32, U32, "u32"),
            NumKind::U64 => narrow!(u64, U64, "u64"),
            NumKind::U128 => narrow!(u128, U128, "u128"),
            NumKind::Usize => narrow!(usize, Usize, "usize"),
            NumKind::Fe => Err(
                "field element is not an integer variant; cast through `Value::Felt` instead"
                    .to_string(),
            ),
        }
    }

    pub fn to_wide(self) -> WideInt {
        match self {
            Self::I8(v) => WideInt::Signed(v as i128),
            Self::I16(v) => WideInt::Signed(v as i128),
            Self::I32(v) => WideInt::Signed(v as i128),
            Self::I64(v) => WideInt::Signed(v as i128),
            Self::I128(v) => WideInt::Signed(v),
            Self::Isize(v) => WideInt::Signed(v as i128),
            Self::U8(v) => WideInt::Unsigned(v as u128),
            Self::U16(v) => WideInt::Unsigned(v as u128),
            Self::U32(v) => WideInt::Unsigned(v as u128),
            Self::U64(v) => WideInt::Unsigned(v as u128),
            Self::U128(v) => WideInt::Unsigned(v),
            Self::Usize(v) => WideInt::Unsigned(v as u128),
        }
    }

    pub fn to_i128(&self) -> Option<i128> {
        match *self {
            Self::I8(v) => Some(v as i128),
            Self::I16(v) => Some(v as i128),
            Self::I32(v) => Some(v as i128),
            Self::I64(v) => Some(v as i128),
            Self::I128(v) => Some(v),
            Self::Isize(v) => Some(v as i128),
            Self::U8(v) => Some(v as i128),
            Self::U16(v) => Some(v as i128),
            Self::U32(v) => Some(v as i128),
            Self::U64(v) => Some(v as i128),
            Self::U128(v) => i128::try_from(v).ok(),
            Self::Usize(v) => Some(v as i128),
        }
    }

    pub fn to_usize(&self) -> Option<usize> {
        match *self {
            Self::I8(v) => usize::try_from(v).ok(),
            Self::I16(v) => usize::try_from(v).ok(),
            Self::I32(v) => usize::try_from(v).ok(),
            Self::I64(v) => usize::try_from(v).ok(),
            Self::I128(v) => usize::try_from(v).ok(),
            Self::Isize(v) => usize::try_from(v).ok(),
            Self::U8(v) => Some(v as usize),
            Self::U16(v) => Some(v as usize),
            Self::U32(v) => Some(v as usize),
            Self::U64(v) => usize::try_from(v).ok(),
            Self::U128(v) => usize::try_from(v).ok(),
            Self::Usize(v) => Some(v),
        }
    }

    /// Returns the multiplicative identity (1) for the given `NumKind`.
    pub fn one_of_kind(kind: &NumKind) -> Self {
        match kind {
            NumKind::I8 => Self::I8(1),
            NumKind::I16 => Self::I16(1),
            NumKind::I32 => Self::I32(1),
            NumKind::I64 | NumKind::Int { .. } => Self::I64(1),
            NumKind::I128 => Self::I128(1),
            NumKind::Isize => Self::Isize(1),
            NumKind::U8 => Self::U8(1),
            NumKind::U16 => Self::U16(1),
            NumKind::U32 => Self::U32(1),
            NumKind::U64 => Self::U64(1),
            NumKind::U128 => Self::U128(1),
            NumKind::Usize => Self::Usize(1),
            NumKind::Fe => unreachable!("Felt is not an Integer variant"),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::I8(_) => "i8",
            Self::I16(_) => "i16",
            Self::I32(_) => "i32",
            Self::I64(_) => "i64",
            Self::I128(_) => "i128",
            Self::Isize(_) => "isize",
            Self::U8(_) => "u8",
            Self::U16(_) => "u16",
            Self::U32(_) => "u32",
            Self::U64(_) => "u64",
            Self::U128(_) => "u128",
            Self::Usize(_) => "usize",
        }
    }

    pub fn to_ast_literal(&self) -> Option<(NumKind, i128)> {
        match *self {
            Self::I8(v) => Some((NumKind::I8, v as i128)),
            Self::I16(v) => Some((NumKind::I16, v as i128)),
            Self::I32(v) => Some((NumKind::I32, v as i128)),
            Self::I64(v) => Some((NumKind::I64, v as i128)),
            Self::I128(v) => Some((NumKind::I128, v)),
            Self::Isize(v) => Some((NumKind::Isize, v as i128)),
            Self::U8(v) => Some((NumKind::U8, v as i128)),
            Self::U16(v) => Some((NumKind::U16, v as i128)),
            Self::U32(v) => Some((NumKind::U32, v as i128)),
            Self::U64(v) => Some((NumKind::U64, v as i128)),
            Self::U128(v) => i128::try_from(v).ok().map(|v| (NumKind::U128, v)),
            Self::Usize(v) => Some((NumKind::Usize, v as i128)),
        }
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        checked_binop!(self, rhs, checked_add)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        checked_binop!(self, rhs, checked_sub)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        checked_binop!(self, rhs, checked_mul)
    }

    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        checked_binop!(self, rhs, checked_div)
    }

    pub fn checked_rem_euclid(self, rhs: Self) -> Option<Self> {
        checked_binop!(self, rhs, checked_rem_euclid)
    }

    pub fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::I8(l), Self::I8(r)) => l.partial_cmp(r),
            (Self::I16(l), Self::I16(r)) => l.partial_cmp(r),
            (Self::I32(l), Self::I32(r)) => l.partial_cmp(r),
            (Self::I64(l), Self::I64(r)) => l.partial_cmp(r),
            (Self::I128(l), Self::I128(r)) => l.partial_cmp(r),
            (Self::Isize(l), Self::Isize(r)) => l.partial_cmp(r),
            (Self::U8(l), Self::U8(r)) => l.partial_cmp(r),
            (Self::U16(l), Self::U16(r)) => l.partial_cmp(r),
            (Self::U32(l), Self::U32(r)) => l.partial_cmp(r),
            (Self::U64(l), Self::U64(r)) => l.partial_cmp(r),
            (Self::U128(l), Self::U128(r)) => l.partial_cmp(r),
            (Self::Usize(l), Self::Usize(r)) => l.partial_cmp(r),
            _ => None,
        }
    }

    pub fn bitwise_and(self, rhs: Self) -> Option<Self> {
        bitwise_binop!(self, rhs, &)
    }

    pub fn bitwise_or(self, rhs: Self) -> Option<Self> {
        bitwise_binop!(self, rhs, |)
    }

    pub fn bitwise_xor(self, rhs: Self) -> Option<Self> {
        bitwise_binop!(self, rhs, ^)
    }

    pub fn checked_shl(self, rhs: u32) -> Option<Self> {
        checked_unary!(self, rhs, checked_shl)
    }

    pub fn checked_shr(self, rhs: u32) -> Option<Self> {
        checked_unary!(self, rhs, checked_shr)
    }

    pub fn checked_neg(self) -> Option<Self> {
        match self {
            Self::I8(v) => v.checked_neg().map(Self::I8),
            Self::I16(v) => v.checked_neg().map(Self::I16),
            Self::I32(v) => v.checked_neg().map(Self::I32),
            Self::I64(v) => v.checked_neg().map(Self::I64),
            Self::I128(v) => v.checked_neg().map(Self::I128),
            Self::Isize(v) => v.checked_neg().map(Self::Isize),
            _ => None,
        }
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8(v) => v.fmt(f),
            Self::I16(v) => v.fmt(f),
            Self::I32(v) => v.fmt(f),
            Self::I64(v) => v.fmt(f),
            Self::I128(v) => v.fmt(f),
            Self::Isize(v) => v.fmt(f),
            Self::U8(v) => v.fmt(f),
            Self::U16(v) => v.fmt(f),
            Self::U32(v) => v.fmt(f),
            Self::U64(v) => v.fmt(f),
            Self::U128(v) => v.fmt(f),
            Self::Usize(v) => v.fmt(f),
        }
    }
}
