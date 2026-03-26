//! Core type representations for the type system.

use std::fmt;
use std::rc::Rc;

use maat_ast::*;

/// Unique identifier for type variables during inference.
pub type TypeVarId = u32;

/// A concrete or polymorphic type in the type system.
///
/// Mirrors the runtime value categories: numeric primitives, booleans, strings,
/// compound types (vectors, maps, functions), user-defined types (structs,
/// enums), and inference-time placeholders (type variables and generics).
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
    Char,
    String,
    Null,
    Vector(Box<Type>),
    /// A map type with key and value types (e.g., `Map<str, i64>`).
    Map(Box<Type>, Box<Type>),
    /// A set type parameterised by its element type (e.g., `Set<i64>`).
    Set(Box<Type>),
    /// A range type parameterised by its element type (e.g., `Range<i64>`).
    Range(Box<Type>),
    /// A tuple type with ordered element types (e.g., `(i64, bool, str)`).
    Tuple(Vec<Type>),
    Function(FnType),
    /// A user-defined struct type, identified by name with instantiated type arguments.
    Struct(Rc<str>, Vec<Type>),
    /// A user-defined enum type, identified by name with instantiated type arguments.
    Enum(Rc<str>, Vec<Type>),
    /// A type variable introduced during inference (Algorithm W).
    Var(TypeVarId),
    /// A named generic type parameter with optional trait bounds.
    Generic(Rc<str>, Vec<String>),
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
    /// Rewrites a literal `expr`ession to match `self`'s numeric type.
    ///
    /// Called after the `TypeChecker`'s range checking has confirmed the value fits. For negated
    /// literals, the prefix is collapsed into a single signed literal node.
    pub fn coerce_literal(&self, expr: &mut Expr) {
        let Some(value) = expr.extract_integer_value() else {
            return;
        };
        let span = expr.span();

        let kind = match *self {
            Self::I8 => NumKind::I8,
            Self::I16 => NumKind::I16,
            Self::I32 => NumKind::I32,
            Self::I64 => NumKind::I64,
            Self::I128 => NumKind::I128,
            Self::Isize => NumKind::Isize,
            Self::U8 => NumKind::U8,
            Self::U16 => NumKind::U16,
            Self::U32 => NumKind::U32,
            Self::U64 => NumKind::U64,
            Self::U128 => NumKind::U128,
            Self::Usize => NumKind::Usize,
            _ => return,
        };

        *expr = Expr::Number(Number {
            kind,
            value,
            radix: Radix::Dec,
            span,
        });
    }

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

    /// Converts an internal `Type` to a `NumKind` for generating cast nodes.
    ///
    /// Returns `None` for non-numeric types since cast nodes only support numeric targets.
    pub fn to_number_kind(&self) -> Option<NumKind> {
        match self {
            Self::I8 => Some(NumKind::I8),
            Self::I16 => Some(NumKind::I16),
            Self::I32 => Some(NumKind::I32),
            Self::I64 => Some(NumKind::I64),
            Self::I128 => Some(NumKind::I128),
            Self::Isize => Some(NumKind::Isize),
            Self::U8 => Some(NumKind::U8),
            Self::U16 => Some(NumKind::U16),
            Self::U32 => Some(NumKind::U32),
            Self::U64 => Some(NumKind::U64),
            Self::U128 => Some(NumKind::U128),
            Self::Usize => Some(NumKind::Usize),
            _ => None,
        }
    }

    /// Converts a `NumKind` (for `as` casts) to an internal `Type`.
    pub fn from_number_kind(num: &NumKind) -> Self {
        match num {
            NumKind::I8 => Self::I8,
            NumKind::I16 => Self::I16,
            NumKind::I32 => Self::I32,
            NumKind::I64 => Self::I64,
            NumKind::I128 => Self::I128,
            NumKind::Isize => Self::Isize,
            NumKind::U8 => Self::U8,
            NumKind::U16 => Self::U16,
            NumKind::U32 => Self::U32,
            NumKind::U64 => Self::U64,
            NumKind::U128 => Self::U128,
            NumKind::Usize => Self::Usize,
        }
    }
}

/// A registered struct definition in the type registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    /// The struct's name (e.g., `Point`).
    pub name: String,
    /// Generic type parameter names declared on this struct.
    pub generic_params: Vec<String>,
    /// Ordered fields: `(field_name, field_type)`.
    ///
    /// Field types may reference generic parameters by name via `Type::Generic`.
    pub fields: Vec<(String, Type)>,
}

/// A registered enum definition in the type registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    /// The enum's name (e.g., `Option`).
    pub name: String,
    /// Generic type parameter names declared on this enum.
    pub generic_params: Vec<String>,
    /// Ordered variants.
    pub variants: Vec<VariantDef>,
}

/// A single enum variant definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    /// Variant name (e.g., `Some`, `None`).
    pub name: String,
    /// The payload shape.
    pub kind: VariantKind,
}

/// The payload of an enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantKind {
    /// A unit variant carrying no data (e.g., `None`).
    Unit,
    /// A tuple variant carrying positional fields (e.g., `Some(T)`).
    Tuple(Vec<Type>),
    /// A struct variant carrying named fields (e.g., `Point { x: i64, y: i64 }`).
    Struct(Vec<(String, Type)>),
}

/// A registered trait definition in the type registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDef {
    /// The trait's name (e.g., `Display`).
    pub name: String,
    /// Generic type parameter names declared on this trait.
    pub generic_params: Vec<String>,
    /// Required method signatures.
    pub methods: Vec<MethodSig>,
}

/// A method signature in a trait definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    /// Method name.
    pub name: String,
    /// Parameter types (excluding `self`).
    pub params: Vec<Type>,
    /// Return type.
    pub ret: Type,
    /// Whether the method has a default implementation.
    pub has_default: bool,
    /// Whether the method takes `self` as its first parameter.
    pub takes_self: bool,
}

/// A registered `impl` block (inherent or trait) in the type registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplDef {
    /// The concrete type this impl applies to (e.g., `Point`).
    pub self_type: Type,
    /// If this is a trait impl, the trait name; `None` for inherent impls.
    pub trait_name: Option<String>,
    /// Methods defined in this impl block: `(method_name, function_type)`.
    pub methods: Vec<(String, Type)>,
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
            Self::Char => f.write_str("char"),
            Self::String => f.write_str("String"),
            Self::Null => f.write_str("null"),
            Self::Vector(elem) => write!(f, "[{elem}]"),
            Self::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            Self::Set(elem) => write!(f, "Set<{elem}>"),
            Self::Range(elem) => write!(f, "Range<{elem}>"),
            Self::Tuple(elems) => {
                f.write_str("(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                if elems.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Self::Function(fn_ty) => {
                let params = fn_ty
                    .params
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fn({params}) -> {}", fn_ty.ret)
            }
            Self::Struct(name, args) | Self::Enum(name, args) => {
                f.write_str(name)?;
                if !args.is_empty() {
                    f.write_str("<")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{arg}")?;
                    }
                    f.write_str(">")?;
                }
                Ok(())
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
