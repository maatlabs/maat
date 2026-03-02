//! Core type representations for the type system.

use std::fmt;

use maat_ast::*;

/// Unique identifier for type variables during inference.
pub type TypeVarId = u32;

/// A concrete or polymorphic type in the type system.
///
/// Mirrors the runtime value categories: numeric primitives, booleans, strings,
/// compound types (arrays, hashes, functions), and inference-time placeholders
/// (type variables and generics).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
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

    Bool,
    String,
    Null,

    Array(Box<Type>),
    Hash(Box<Type>, Box<Type>),
    Function(FnType),

    /// A type variable introduced during inference (Algorithm W).
    Var(TypeVarId),
    /// A named generic type parameter with optional trait bounds.
    Generic(String, Vec<String>),
    /// The bottom type (diverging expressions like `break`, `continue`, `return`).
    Never,
}

/// Function type signature: parameter types and return type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnType {
    pub params: Vec<Type>,
    pub ret: Box<Type>,
}

/// A polymorphic type scheme.
///
/// Generalizes a type over a set of type variables that are not free in
/// the surrounding environment. At each use site, `instantiate` replaces
/// the quantified variables with fresh inference variables, enabling
/// let-polymorphism (e.g., `let id = fn(x) { x }; id(5); id(true);`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScheme {
    /// Type variables universally quantified by this scheme.
    pub forall: Vec<TypeVarId>,
    /// The underlying type (may contain variables listed in `forall`).
    pub ty: Type,
}

impl TypeScheme {
    /// Creates a monomorphic scheme (no quantified variables).
    pub fn monomorphic(ty: Type) -> Self {
        Self {
            forall: Vec::new(),
            ty,
        }
    }
}

impl Type {
    /// Returns `true` if this is any integer type (signed or unsigned).
    pub fn is_integer(&self) -> bool {
        self.is_signed() || self.is_unsigned()
    }

    /// Returns `true` if this is a signed integer type.
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Isize
        )
    }

    /// Returns `true` if this is an unsigned integer type.
    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::Usize
        )
    }

    /// Returns the bit width for integer types, treating `isize`/`usize` as 64-bit.
    ///
    /// Returns `None` for non-integer types.
    pub fn int_bit_width(&self) -> Option<u32> {
        match self {
            Self::I8 | Self::U8 => Some(8),
            Self::I16 | Self::U16 => Some(16),
            Self::I32 | Self::U32 => Some(32),
            Self::I64 | Self::U64 | Self::Isize | Self::Usize => Some(64),
            Self::I128 | Self::U128 => Some(128),
            _ => None,
        }
    }

    /// Returns `(is_signed, bit_width)` for an integer type.
    pub fn int_sign_bit_width(&self) -> Option<(bool, u32)> {
        let width = self.int_bit_width()?;
        Some((self.is_signed(), width))
    }

    /// Converts an internal `self` to a `TypeAnnotation` (for generating cast nodes).
    ///
    /// Returns `None` for non-numeric types since cast nodes only support numeric targets.
    pub fn to_annotation(&self) -> Option<TypeAnnotation> {
        match self {
            Self::I8 => Some(TypeAnnotation::I8),
            Self::I16 => Some(TypeAnnotation::I16),
            Self::I32 => Some(TypeAnnotation::I32),
            Self::I64 => Some(TypeAnnotation::I64),
            Self::I128 => Some(TypeAnnotation::I128),
            Self::Isize => Some(TypeAnnotation::Isize),
            Self::U8 => Some(TypeAnnotation::U8),
            Self::U16 => Some(TypeAnnotation::U16),
            Self::U32 => Some(TypeAnnotation::U32),
            Self::U64 => Some(TypeAnnotation::U64),
            Self::U128 => Some(TypeAnnotation::U128),
            Self::Usize => Some(TypeAnnotation::Usize),
            _ => None,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8 => f.write_str("i8"),
            Self::I16 => f.write_str("i16"),
            Self::I32 => f.write_str("i32"),
            Self::I64 => f.write_str("i64"),
            Self::I128 => f.write_str("i128"),
            Self::Isize => f.write_str("isize"),
            Self::U8 => f.write_str("u8"),
            Self::U16 => f.write_str("u16"),
            Self::U32 => f.write_str("u32"),
            Self::U64 => f.write_str("u64"),
            Self::U128 => f.write_str("u128"),
            Self::Usize => f.write_str("usize"),
            Self::Bool => f.write_str("bool"),
            Self::String => f.write_str("String"),
            Self::Null => f.write_str("null"),
            Self::Array(elem) => write!(f, "[{elem}]"),
            Self::Hash(k, v) => write!(f, "{{{k}: {v}}}"),
            Self::Function(fn_ty) => {
                let params = fn_ty
                    .params
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fn({params}) -> {}", fn_ty.ret)
            }
            Self::Var(id) => write!(f, "?T{id}"),
            Self::Generic(name, bounds) => {
                if bounds.is_empty() {
                    f.write_str(name)
                } else {
                    write!(f, "{}: {}", name, bounds.join(" + "))
                }
            }
            Self::Never => f.write_str("!"),
        }
    }
}
