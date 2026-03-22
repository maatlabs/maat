//! Runtime value system for Maat.
//!
//! This crate defines the core values and built-in functions shared
//! by both the tree-walking interpreter and the bytecode compiler/VM.
#![forbid(unsafe_code)]

mod builtins;
mod env;
mod num;

use std::fmt;
use std::rc::Rc;

pub use builtins::{BUILTIN_COUNT, BUILTINS, QUOTE, UNQUOTE, get_builtin};
pub use env::Env;
use indexmap::{IndexMap, IndexSet};
pub use maat_ast::NumberKind;
use maat_ast::{BlockStmt, Node, Number};
use maat_errors::{Error, EvalError, Result};
use maat_span::SourceMap;
pub use num::{Integer, WideInt};
use serde::{Deserialize, Serialize};

pub type BuiltinFn = fn(&[Value]) -> Result<Value>;

pub const TRUE: Value = Value::Bool(true);
pub const FALSE: Value = Value::Bool(false);
pub const NULL: Value = Value::Null;

/// Runtime value representation.
///
/// Values are the evaluated results of expressions and can be integers,
/// booleans, functions, or special values like [`NULL`] and return wrappers.
#[derive(Debug, Clone)]
pub enum Value {
    /// [`NULL`], representing absence of a value.
    Null,
    /// Runtime integer types
    Integer(Integer),
    /// A boolean value (true or false).
    Bool(bool),
    /// A string literal.
    Str(String),
    /// A vector of values.
    Vector(Vec<Value>),
    /// A map (key-value collection).
    Map(MapVal),
    /// A runtime function with parameters, body, and closure environment.
    Function(Function),
    /// A macro with parameters, body, and closure environment.
    Macro(MacroVal),
    /// A quoted AST node for metaprogramming.
    Quote(Box<Quote>),
    /// Wraps a return value for early function/block termination.
    ReturnValue(Box<Value>),
    /// Signals a `break` from a loop, optionally carrying a value.
    Break(Box<Value>),
    /// Signals a `continue` to the next loop iteration.
    Continue,
    /// A builtin function.
    Builtin(BuiltinFn),
    /// A compiled function containing bytecode instructions.
    CompiledFn(CompiledFn),
    /// A closure wrapping a compiled function with captured free variables.
    Closure(Closure),
    /// A user-defined struct instance.
    Struct(StructVal),
    /// A user-defined enum variant instance.
    EnumVariant(EnumVariantVal),
    /// An ordered set of unique hashable values, backed by `IndexSet`.
    Set(IndexSet<Hashable>),
    /// A half-open range `start..end`.
    Range(i64, i64),
    /// An inclusive range `start..=end`.
    RangeInclusive(i64, i64),
}

impl Value {
    /// Converts a `Number` AST node into its corresponding runtime `Value`.
    ///
    /// The type checker validates that `lit.value` fits within the target type
    /// before this function is called. The `TryFrom` conversions enforce this
    /// invariant at runtime as a defense-in-depth measure, returning an error
    /// rather than silently truncating if the value is out of range.
    pub fn from_number_literal(lit: &Number) -> std::result::Result<Self, String> {
        macro_rules! narrow {
            ($variant:ident, $ty:ty) => {
                <$ty>::try_from(lit.value)
                    .map(|v| Self::Integer(Integer::$variant(v)))
                    .map_err(|_| format!("{} out of range for {}", lit.value, stringify!($ty)))
            };
        }
        match lit.kind {
            NumberKind::I8 => narrow!(I8, i8),
            NumberKind::I16 => narrow!(I16, i16),
            NumberKind::I32 => narrow!(I32, i32),
            NumberKind::I64 => narrow!(I64, i64),
            NumberKind::I128 => Ok(Self::Integer(Integer::I128(lit.value))),
            NumberKind::Isize => narrow!(Isize, isize),
            NumberKind::U8 => narrow!(U8, u8),
            NumberKind::U16 => narrow!(U16, u16),
            NumberKind::U32 => narrow!(U32, u32),
            NumberKind::U64 => narrow!(U64, u64),
            NumberKind::U128 => narrow!(U128, u128),
            NumberKind::Usize => narrow!(Usize, usize),
        }
    }

    /// Converts a runtime value back to an AST node.
    ///
    /// Used to splice evaluated values back into quoted code.
    pub fn to_ast_node(val: &Self) -> Option<Node> {
        use maat_ast::*;
        use maat_span::Span;

        match val {
            Value::Integer(i) => {
                let (kind, value) = i.to_ast_literal()?;
                Some(Node::Expr(Expr::Number(Number {
                    kind,
                    value,
                    radix: Radix::Dec,
                    span: Span::ZERO,
                })))
            }
            Value::Bool(b) => Some(Node::Expr(Expr::Bool(Bool {
                value: *b,
                span: Span::ZERO,
            }))),
            Value::Quote(q) => Some(q.node.clone()),
            _ => None,
        }
    }

    /// Determines whether this value is truthy.
    ///
    /// Booleans return their value directly; null is falsy;
    /// all other values (including integers) are truthy.
    #[inline]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            _ => true,
        }
    }

    /// Attempts to convert this value to a `usize` vector index.
    ///
    /// Returns `Some(index)` for any integer type whose value fits in `usize`.
    /// Returns `None` for negative values, out-of-range values, or non-integer types.
    pub fn to_vector_index(&self) -> Option<usize> {
        match self {
            Self::Integer(n) => n.to_usize(),
            _ => None,
        }
    }

    /// Converts any integer variant to `i128` for cross-type comparison.
    ///
    /// Returns `None` for non-integer types or `U128` values exceeding `i128::MAX`.
    pub fn to_i128(&self) -> Option<i128> {
        match self {
            Self::Integer(n) => n.to_i128(),
            _ => None,
        }
    }

    /// Returns `true` if this value is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    /// Returns a string representation of the value's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Integer(n) => n.type_name(),
            Self::Bool(_) => "Bool",
            Self::Str(_) => "Str",
            Self::Vector(_) => "Vector",
            Self::Map(_) => "Map",
            Self::Function(_) => "Function",
            Self::Macro(_) => "Macro",
            Self::Quote(_) => "Quote",
            Self::ReturnValue(_) => "ReturnValue",
            Self::Break(_) => "Break",
            Self::Continue => "Continue",
            Self::Builtin(_) => "Builtin",
            Self::CompiledFn(_) => "CompiledFn",
            Self::Closure(_) => "Closure",
            Self::Struct(_) => "Struct",
            Self::EnumVariant(_) => "EnumVariant",
            Self::Set(_) => "Set",
            Self::Range(..) => "Range",
            Self::RangeInclusive(..) => "RangeInclusive",
        }
    }
}

/// Serialization proxy containing only the [`Value`] variants that can be
/// represented in the bytecode binary format.
///
/// Non-serializable variants (`Function`, `Macro`, `Quote`, `ReturnValue`,
/// `Builtin`) exist only at runtime and cannot appear in compiled bytecode.
/// Attempting to serialize them produces an error.
#[derive(Serialize, Deserialize)]
enum SerVal {
    Null,
    Integer(Integer),
    Bool(bool),
    Str(String),
    Vector(Vec<Value>),
    Map(MapVal),
    CompiledFn(CompiledFn),
    Closure(Closure),
    Struct(StructVal),
    EnumVariant(EnumVariantVal),
    Set(IndexSet<Hashable>),
    Range(i64, i64),
    RangeInclusive(i64, i64),
}

impl Serialize for Value {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let val = match self {
            Self::Null => SerVal::Null,
            Self::Integer(v) => SerVal::Integer(*v),
            Self::Bool(v) => SerVal::Bool(*v),
            Self::Str(v) => SerVal::Str(v.clone()),
            Self::Vector(v) => SerVal::Vector(v.clone()),
            Self::Map(v) => SerVal::Map(v.clone()),
            Self::CompiledFn(v) => SerVal::CompiledFn(v.clone()),
            Self::Closure(v) => SerVal::Closure(v.clone()),
            Self::Struct(v) => SerVal::Struct(v.clone()),
            Self::EnumVariant(v) => SerVal::EnumVariant(v.clone()),
            Self::Set(v) => SerVal::Set(v.clone()),
            Self::Range(s, e) => SerVal::Range(*s, *e),
            Self::RangeInclusive(s, e) => SerVal::RangeInclusive(*s, *e),
            other => {
                return Err(serde::ser::Error::custom(format!(
                    "non-serializable value: {}",
                    other.type_name()
                )));
            }
        };
        val.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        SerVal::deserialize(deserializer).map(|val| match val {
            SerVal::Null => Self::Null,
            SerVal::Integer(v) => Self::Integer(v),
            SerVal::Bool(v) => Self::Bool(v),
            SerVal::Str(v) => Self::Str(v),
            SerVal::Vector(v) => Self::Vector(v),
            SerVal::Map(v) => Self::Map(v),
            SerVal::CompiledFn(v) => Self::CompiledFn(v),
            SerVal::Closure(v) => Self::Closure(v),
            SerVal::Struct(v) => Self::Struct(v),
            SerVal::EnumVariant(v) => Self::EnumVariant(v),
            SerVal::Set(v) => Self::Set(v),
            SerVal::Range(s, e) => Self::Range(s, e),
            SerVal::RangeInclusive(s, e) => Self::RangeInclusive(s, e),
        })
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Null, Null) => true,
            (Integer(n1), Integer(n2)) => n1 == n2,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Vector(v1), Vector(v2)) => v1 == v2,
            (Map(m1), Map(m2)) => m1 == m2,
            (Function(f1), Function(f2)) => f1 == f2,
            (Macro(m1), Macro(m2)) => m1 == m2,
            (Quote(q1), Quote(q2)) => q1 == q2,
            (ReturnValue(o1), ReturnValue(o2)) => o1 == o2,
            (Break(o1), Break(o2)) => o1 == o2,
            (Continue, Continue) => true,
            (Builtin(f1), Builtin(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            (CompiledFn(c1), CompiledFn(c2)) => c1 == c2,
            (Closure(c1), Closure(c2)) => c1 == c2,
            (Struct(s1), Struct(s2)) => s1 == s2,
            (EnumVariant(e1), EnumVariant(e2)) => e1 == e2,
            (Set(s1), Set(s2)) => s1 == s2,
            (Range(s1, e1), Range(s2, e2)) => s1 == s2 && e1 == e2,
            (RangeInclusive(s1, e1), RangeInclusive(s2, e2)) => s1 == s2 && e1 == e2,
            _ => false,
        }
    }
}

/// Represents a runtime function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: Env,
}

/// Represents a runtime macro.
#[derive(Debug, Clone, PartialEq)]
pub struct MacroVal {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: Env,
}

/// Represents a quoted AST node.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub node: Node,
}

/// A compiled function containing bytecode instructions.
///
/// Functions are compiled into bytecode and stored in the constant pool.
/// The VM creates a new call frame for each invocation, using the
/// `num_locals` field to reserve stack space for local bindings.
///
/// Instructions are stored behind `Rc<[u8]>` so that closures created
/// from the same function literal share instruction memory rather than
/// cloning the entire byte vector on every call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledFn {
    /// The bytecode instructions for this function's body.
    ///
    /// Reference-counted to allow zero-copy sharing across closures
    /// instantiated from the same compiled function.
    pub instructions: Rc<[u8]>,
    /// The number of local bindings (parameters + let bindings) in this function.
    pub num_locals: usize,
    /// The number of parameters this function expects.
    pub num_parameters: usize,
    /// Maps instruction byte offsets to source spans within this function.
    pub source_map: SourceMap,
}

/// A closure binding a compiled function with its captured free variables.
///
/// At runtime, every function is wrapped in a closure, even those with zero
/// free variables. This uniform representation simplifies the VM's call
/// dispatch: there is a single code path for invoking user-defined functions.
///
/// Free variables are resolved at closure-creation time (`OpClosure`) and
/// stored by value. Nested closures capture through the chain: an inner
/// closure's free variable may itself be a free variable of its enclosing
/// closure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Closure {
    /// The underlying compiled function.
    pub func: CompiledFn,
    /// Captured free variables from enclosing scopes.
    pub free_vars: Vec<Value>,
}

/// A user-defined struct instance at runtime.
///
/// Stores the type registry index and field values in declaration order.
/// Field names are resolved at compile time; the VM accesses fields by index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructVal {
    /// Index into the shared type registry.
    pub type_index: u16,
    /// Field values in declaration order.
    pub fields: Vec<Value>,
}

/// A user-defined enum variant instance at runtime.
///
/// Stores the type registry index, variant tag (discriminant), and
/// any associated data fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariantVal {
    /// Index into the shared type registry.
    pub type_index: u16,
    /// Variant discriminant (positional index within the enum definition).
    pub tag: u16,
    /// Associated data fields (empty for unit variants).
    pub fields: Vec<Value>,
}

/// A type definition in the shared type registry.
///
/// Shared between the compiler (which registers types during compilation)
/// and the VM (which reads type metadata for field access and pattern matching).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeDef {
    /// A struct type with ordered named fields.
    Struct {
        name: String,
        field_names: Vec<String>,
    },
    /// An enum type with ordered variants.
    Enum {
        name: String,
        variants: Vec<VariantInfo>,
    },
}

/// Metadata for a single enum variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantInfo {
    /// Variant name (e.g., `Some`, `None`).
    pub name: String,
    /// Number of data fields this variant carries.
    pub field_count: u8,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MapVal {
    pub pairs: IndexMap<Hashable, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hashable {
    Integer(Integer),
    Bool(bool),
    Str(String),
}

impl TryFrom<Value> for Hashable {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Integer(i) => Ok(Self::Integer(i)),
            Value::Bool(b) => Ok(Self::Bool(b)),
            Value::Str(s) => Ok(Self::Str(s)),
            val => Err(EvalError::NotHashable(val.type_name().to_owned()).into()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Integer(v) => v.fmt(f),
            Self::Bool(v) => v.fmt(f),
            Self::Str(v) => v.fmt(f),
            Self::Vector(vector) => {
                write!(
                    f,
                    "[{}]",
                    vector
                        .iter()
                        .map(|e| format!("{e}"))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Self::Map(v) => v.fmt(f),
            Self::Function(v) => v.fmt(f),
            Self::Macro(v) => v.fmt(f),
            Self::Quote(v) => v.fmt(f),
            Self::ReturnValue(v) => v.fmt(f),
            Self::Break(v) => write!(f, "break {v}"),
            Self::Continue => write!(f, "continue"),
            Self::Builtin(_) => write!(f, "builtin function"),
            Self::CompiledFn(v) => write!(f, "CompiledFn[{v:p}]"),
            Self::Closure(v) => write!(f, "Closure[{:p}]", &v.func),
            Self::Struct(s) => {
                write!(f, "Struct({}", s.type_index)?;
                if !s.fields.is_empty() {
                    write!(
                        f,
                        " {{ {} }}",
                        s.fields
                            .iter()
                            .map(|v| format!("{v}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                }
                write!(f, ")")
            }
            Self::EnumVariant(v) => {
                write!(f, "EnumVariant({}::{})", v.type_index, v.tag)?;
                if !v.fields.is_empty() {
                    write!(
                        f,
                        "({})",
                        v.fields
                            .iter()
                            .map(|e| format!("{e}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                }
                Ok(())
            }
            Self::Set(set) => {
                write!(
                    f,
                    "Set({{{}}})",
                    set.iter()
                        .map(|v| format!("{v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Range(start, end) => write!(f, "{start}..{end}"),
            Self::RangeInclusive(start, end) => write!(f, "{start}..={end}"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for MacroVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macro({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote({})", self.node)
    }
}

impl fmt::Display for MapVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}",
            self.pairs
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl fmt::Display for Hashable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(v) => v.fmt(f),
            Self::Bool(b) => b.fmt(f),
            Self::Str(s) => s.fmt(f),
        }
    }
}
