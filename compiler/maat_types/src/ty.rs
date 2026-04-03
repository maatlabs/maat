//! Core type representations for the type system.

use std::collections::HashSet;
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
    Str,
    /// The unit type `()`, representing expressions that produce no value.
    Unit,
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

    /// Collects all free type variables in a type.
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

    /// Returns `true` if this is any integer type (signed or unsigned).
    pub fn is_integer(&self) -> bool {
        self.is_signed() || self.is_unsigned() || matches!(self, Self::IntVar(_))
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

    /// Converts a concrete `NumKind` to an internal `Type`.
    ///
    /// # Panics
    ///
    /// Panics if called with `NumKind::Int`; use the type checker's
    /// inference machinery for unsuffixed literals instead.
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
            NumKind::Int { .. } => {
                unreachable!("Int literals must be resolved by type inference")
            }
        }
    }

    /// Maps a resolved integer `Type` to the corresponding `NumKind`.
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
            _ => NumKind::I64,
        }
    }

    /// Maps a cast target to the corresponding type.
    pub fn from_cast_target(target: &CastTarget) -> Self {
        match target {
            CastTarget::Num(k) => Self::from_number_kind(k),
            CastTarget::Char => Self::Char,
        }
    }

    /// Infers the type of a literal expression used in a pattern context.
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
            },
            Expr::Bool(_) => Self::Bool,
            Expr::Char(_) => Self::Char,
            Expr::Str(_) => Self::Str,
            _ => Self::Unit,
        }
    }

    /// Maps a resolved type to the dispatch prefix used in builtin qualified names.
    ///
    /// Returns `Some("Vector")` for vector types, `Some("str")` for strings,
    /// `Some("Map")` for map types, `Some("Set")` for set types, and
    /// `Some(name)` for user-defined structs/enums. Returns `None` for
    /// unresolved type variables or primitive types that have no inherent methods.
    pub fn receiver_name(&self) -> Option<String> {
        match self {
            Self::Vector(_) => Some("Vector".to_string()),
            Self::Char => Some("char".to_string()),
            Self::Str => Some("str".to_string()),
            Self::Map(..) => Some("Map".to_string()),
            Self::Set(_) => Some("Set".to_string()),
            Self::Struct(name, _) | Self::Enum(name, _) => Some(name.to_string()),
            _ => None,
        }
    }
}

/// Returns `true` if converting `source` to `target` is a lossless widening.
///
/// Accepted conversions mirror Rust's `From` impls for integer types:
/// - Signed widening: `i8-->i16-->i32-->i64-->i128`
/// - Unsigned widening: `u8-->u16-->u32-->u64-->u128`
/// - Unsigned-->signed where the target is strictly wider:
///   `u8-->i16`, `u16-->i32`, `u32-->i64`, `u64-->i128`
pub fn is_lossless_conversion(source: &Type, target: &Type) -> bool {
    use Type::*;
    matches!(
        (source, target),
        // Signed widening chain
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
            // Unsigned widening chain
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
            // Safe cross-sign (unsigned --> strictly wider signed)
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
            Self::Bool => f.write_str("bool"),
            Self::Char => f.write_str("char"),
            Self::Str => f.write_str("str"),
            Self::Unit => f.write_str("()"),
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
