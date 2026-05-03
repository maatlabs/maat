//! Core type representations for the type system.

use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use maat_ast::*;

/// Unique identifier for type variables during inference.
pub type TypeVarId = u32;

/// A concrete or polymorphic type in the type system.
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
    /// A field element over the Goldilocks prime.
    Felt,
    Bool,
    Char,
    Str,
    /// The unit type `()`, representing expressions that produce no value.
    Unit,
    Vector(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Set(Box<Type>),
    Range(Box<Type>),
    Array(Box<Type>, usize),
    Tuple(Vec<Type>),
    Function(FnType),
    Struct(Rc<str>, Vec<Type>),
    Enum(Rc<str>, Vec<Type>),
    /// A type variable introduced during inference (Algorithm W).
    Var(TypeVarId),
    /// An integer-constrained type variable for unsuffixed integer literals.
    ///
    /// Behaves like `Var` during unification but only accepts integer types.
    /// Defaults to `I64` when no constraint is found after inference completes.
    IntVar(TypeVarId),
    /// A named generic type parameter with optional trait bounds.
    Generic(Rc<str>, Vec<String>),
    /// The bottom type (diverging expressions like `break`, `continue`, `return`).
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnType {
    pub params: Vec<Type>,
    pub ret: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScheme {
    pub forall: Vec<TypeVarId>,
    pub ty: Type,
}

impl TypeScheme {
    pub fn monomorphic(ty: Type) -> Self {
        Self {
            forall: Vec::new(),
            ty,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    pub generic_params: Vec<String>,
    pub fields: Vec<(String, Type)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub generic_params: Vec<String>,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    pub name: String,
    pub kind: VariantKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantKind {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<(String, Type)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDef {
    pub name: String,
    pub generic_params: Vec<String>,
    pub methods: Vec<MethodSig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    pub name: String,
    pub params: Vec<Type>,
    pub ret: Type,
    pub has_default: bool,
    pub takes_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplDef {
    pub self_type: Type,
    pub trait_name: Option<String>,
    pub methods: Vec<(String, Type)>,
}

impl Type {
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
            Self::Felt => NumKind::Fe,
            _ => return,
        };

        *expr = Expr::Number(Number {
            kind,
            value,
            radix: Radix::Dec,
            span,
        });
    }

    pub fn free_type_vars(&self) -> HashSet<TypeVarId> {
        let mut vars = HashSet::new();
        self.collect_free_vars(&mut vars);
        vars
    }

    fn collect_free_vars(&self, vars: &mut HashSet<TypeVarId>) {
        match self {
            Self::Var(id) | Self::IntVar(id) => {
                vars.insert(*id);
            }
            Self::Vector(elem) | Self::Set(elem) | Self::Range(elem) => {
                elem.collect_free_vars(vars)
            }
            Self::Map(key, val) => {
                key.collect_free_vars(vars);
                val.collect_free_vars(vars);
            }
            Self::Function(FnType { params, ret }) => {
                for param in params {
                    param.collect_free_vars(vars);
                }
                ret.collect_free_vars(vars);
            }
            Self::Struct(_, args) | Self::Enum(_, args) => {
                for arg in args {
                    arg.collect_free_vars(vars);
                }
            }
            _ => {}
        }
    }

    pub fn is_integer(&self) -> bool {
        self.is_signed() || self.is_unsigned() || matches!(self, Self::IntVar(_))
    }

    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Isize
        )
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::Usize
        )
    }

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
            NumKind::Fe => Self::Felt,
            NumKind::Int { .. } => {
                unreachable!("Int literals must be resolved by type inference")
            }
        }
    }

    pub fn to_number_kind(&self) -> NumKind {
        match self {
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
            Self::Felt => NumKind::Fe,
            _ => NumKind::I64,
        }
    }

    pub fn from_cast_target(target: &CastTarget) -> Self {
        match target {
            CastTarget::Num(k) => Self::from_number_kind(k),
            CastTarget::Char => Self::Char,
        }
    }

    pub fn from_literal_expr(expr: &Expr) -> Self {
        match expr {
            Expr::Number(lit) => match lit.kind {
                NumKind::I8 => Self::I8,
                NumKind::I16 => Self::I16,
                NumKind::I32 => Self::I32,
                NumKind::I64 | NumKind::Int { .. } => Self::I64,
                NumKind::I128 => Self::I128,
                NumKind::Isize => Self::Isize,
                NumKind::U8 => Self::U8,
                NumKind::U16 => Self::U16,
                NumKind::U32 => Self::U32,
                NumKind::U64 => Self::U64,
                NumKind::U128 => Self::U128,
                NumKind::Usize => Self::Usize,
                NumKind::Fe => Self::Felt,
            },
            Expr::Bool(_) => Self::Bool,
            Expr::Char(_) => Self::Char,
            Expr::Str(_) => Self::Str,
            _ => Self::Unit,
        }
    }

    pub fn receiver_name(&self) -> Option<String> {
        match self {
            Self::Vector(_) | Self::Array(..) => Some("Vector".to_string()),
            Self::Char => Some("char".to_string()),
            Self::Str => Some("str".to_string()),
            Self::Map(..) => Some("Map".to_string()),
            Self::Set(_) => Some("Set".to_string()),
            Self::Struct(name, _) | Self::Enum(name, _) => Some(name.to_string()),
            _ => None,
        }
    }

    pub fn is_bitwise_safe_unsigned(&self) -> bool {
        matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::Usize
        )
    }

    pub fn is_shift_safe_unsigned(&self) -> bool {
        matches!(self, Self::U64 | Self::Usize)
    }
}

pub fn is_lossless_conversion(source: &Type, target: &Type) -> bool {
    use Type::*;
    matches!(
        (source, target),
        (I8, I16)
            | (I8, I32)
            | (I8, I64)
            | (I8, I128)
            | (I16, I32)
            | (I16, I64)
            | (I16, I128)
            | (I32, I64)
            | (I32, I128)
            | (I64, I128)
            | (U8, U16)
            | (U8, U32)
            | (U8, U64)
            | (U8, U128)
            | (U16, U32)
            | (U16, U64)
            | (U16, U128)
            | (U32, U64)
            | (U32, U128)
            | (U64, U128)
            | (U8, I16)
            | (U8, I32)
            | (U8, I64)
            | (U8, I128)
            | (U16, I32)
            | (U16, I64)
            | (U16, I128)
            | (U32, I64)
            | (U32, I128)
            | (U64, I128)
    )
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
            Self::Felt => f.write_str("Felt"),
            Self::Bool => f.write_str("bool"),
            Self::Char => f.write_str("char"),
            Self::Str => f.write_str("str"),
            Self::Unit => f.write_str("()"),
            Self::Vector(elem) => write!(f, "[{elem}]"),
            Self::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            Self::Set(elem) => write!(f, "Set<{elem}>"),
            Self::Range(elem) => write!(f, "Range<{elem}>"),
            Self::Array(elem, n) => write!(f, "[{elem}; {n}]"),
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
                f.write_str("fn(")?;
                for (i, p) in fn_ty.params.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {}", fn_ty.ret)
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
            Self::IntVar(id) => write!(f, "?Int{id}"),
            Self::Generic(name, bounds) => {
                f.write_str(name)?;
                if !bounds.is_empty() {
                    f.write_str(": ")?;
                    for (i, bound) in bounds.iter().enumerate() {
                        if i > 0 {
                            f.write_str(" + ")?;
                        }
                        f.write_str(bound)?;
                    }
                }
                Ok(())
            }
            Self::Never => f.write_str("!"),
        }
    }
}
