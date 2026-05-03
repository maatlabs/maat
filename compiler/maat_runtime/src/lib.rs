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

pub use builtins::{BUILTIN_COUNT, BUILTINS, get_builtin};
pub use env::Env;
use indexmap::{IndexMap, IndexSet};
use maat_ast::{BlockStmt, MaatAst, Number};
pub use maat_ast::{CastTarget, NumKind};
use maat_errors::{Error, EvalError, Result};
use maat_field::FieldElement;
pub use maat_field::{Felt, StarkField, from_i64, try_div, try_inv};
use maat_span::SourceMap;
pub use num::{Integer, WideInt};
use serde::{Deserialize, Serialize};

pub type BuiltinFn = fn(&[Value]) -> Result<Value>;

pub const TRUE: Value = Value::Bool(true);
pub const FALSE: Value = Value::Bool(false);
/// The unit value `()`, representing the result of expressions that produce
/// no meaningful value (e.g., statements, void function returns, `print!`).
pub const UNIT: Value = Value::Unit;

#[derive(Debug, Clone)]
pub enum Value {
    /// The unit type `()`, representing the absence of a meaningful value.
    Unit,
    /// Runtime integer types
    Integer(Integer),
    /// A field-element value over the Goldilocks base field used by the ZK backend.
    Felt(Felt),
    /// A boolean value (true or false).
    Bool(bool),
    /// A Unicode scalar value.
    Char(char),
    /// A string literal.
    Str(String),
    /// An ordered, fixed-size collection of heterogeneous values.
    Tuple(Vec<Value>),
    /// A vector of values.
    Vector(Vec<Value>),
    /// A fixed-size array of homogeneous values.
    Array(Vec<Value>),
    /// An ordered map of key-value pairs, backed by [`IndexMap`].
    Map(Map),
    /// A runtime function with parameters, body, and closure environment.
    Function(Function),
    /// A macro with parameters, body, and closure environment.
    Macro(Macro),
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
    /// An ordered set of unique hashable values, backed by [`IndexSet`].
    Set(Set),
    /// A half-open range `start..end`, generic over all integer types.
    Range(Integer, Integer),
    /// An inclusive range `start..=end`, generic over all integer types.
    RangeInclusive(Integer, Integer),
}

impl Value {
    /// Encodes this runtime value as a single Goldilocks field element.
    ///
    /// Primitive types encode losslessly within the 64-bit field; composite types
    /// (`Vector`, `Map`, `Struct`, closures, etc.) currently encode as
    /// [`Felt::ZERO`].
    pub fn to_felt(&self) -> Felt {
        match self {
            Self::Integer(int) => int.to_felt(),
            Self::Felt(f) => *f,
            Self::Bool(b) => Felt::new(*b as u64),
            Self::Char(c) => Felt::new(*c as u32 as u64),
            Self::Unit => Felt::ZERO,
            _ => Felt::ZERO,
        }
    }

    pub fn from_number_literal(lit: &Number) -> std::result::Result<Self, String> {
        macro_rules! narrow {
            ($variant:ident, $ty:ty) => {
                <$ty>::try_from(lit.value)
                    .map(|v| Self::Integer(Integer::$variant(v)))
                    .map_err(|_| format!("{} out of range for {}", lit.value, stringify!($ty)))
            };
        }
        match lit.kind {
            NumKind::I8 => narrow!(I8, i8),
            NumKind::I16 => narrow!(I16, i16),
            NumKind::I32 => narrow!(I32, i32),
            NumKind::I64 | NumKind::Int { .. } => narrow!(I64, i64),
            NumKind::I128 => Ok(Self::Integer(Integer::I128(lit.value))),
            NumKind::Isize => narrow!(Isize, isize),
            NumKind::U8 => narrow!(U8, u8),
            NumKind::U16 => narrow!(U16, u16),
            NumKind::U32 => narrow!(U32, u32),
            NumKind::U64 => narrow!(U64, u64),
            NumKind::U128 => narrow!(U128, u128),
            NumKind::Usize => narrow!(Usize, usize),
            NumKind::Fe => u64::try_from(lit.value)
                .map(|v| Self::Felt(Felt::new(v)))
                .map_err(|_| format!("{} out of range for Felt", lit.value)),
        }
    }

    pub fn to_ast_node(val: &Self) -> Option<MaatAst> {
        use maat_ast::*;
        use maat_span::Span;

        match val {
            Value::Integer(i) => {
                let (kind, value) = i.to_ast_literal()?;
                Some(MaatAst::Expr(Expr::Number(Number {
                    kind,
                    value,
                    radix: Radix::Dec,
                    span: Span::ZERO,
                })))
            }
            Value::Bool(b) => Some(MaatAst::Expr(Expr::Bool(BoolLit {
                value: *b,
                span: Span::ZERO,
            }))),
            Value::Quote(q) => Some(q.node.clone()),
            _ => None,
        }
    }

    #[inline]
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Bool(false))
    }

    pub fn to_vector_index(&self) -> Option<usize> {
        match self {
            Self::Integer(n) => n.to_usize(),
            _ => None,
        }
    }

    pub fn to_i128(&self) -> Option<i128> {
        match self {
            Self::Integer(n) => n.to_i128(),
            _ => None,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "()",
            Self::Integer(n) => n.type_name(),
            Self::Felt(_) => "Felt",
            Self::Bool(_) => "bool",
            Self::Char(_) => "char",
            Self::Str(_) => "str",
            Self::Tuple(_) => "tuple",
            Self::Vector(_) => "Vector",
            Self::Array(_) => "Array",
            Self::Map(_) => "Map",
            Self::Function(_) => "fn",
            Self::Macro(_) => "macro",
            Self::Quote(_) => "quote",
            Self::ReturnValue(_) => "return",
            Self::Break(_) => "break",
            Self::Continue => "continue",
            Self::Builtin(_) => "fn",
            Self::CompiledFn(_) => "fn",
            Self::Closure(_) => "fn",
            Self::Struct(_) => "struct",
            Self::EnumVariant(_) => "enum",
            Self::Set(_) => "Set",
            Self::Range(..) => "Range",
            Self::RangeInclusive(..) => "RangeInclusive",
        }
    }
}

/// Serialization proxy containing only the [`Value`] variants that can be
/// represented in the bytecode binary format.
#[derive(Serialize, Deserialize)]
enum SerVal {
    Unit,
    Integer(Integer),
    Felt(u64),
    Bool(bool),
    Char(char),
    Str(String),
    Tuple(Vec<Value>),
    Vector(Vec<Value>),
    Array(Vec<Value>),
    Map(Map),
    CompiledFn(CompiledFn),
    Closure(Closure),
    Struct(StructVal),
    EnumVariant(EnumVariantVal),
    Set(Set),
    Range(Integer, Integer),
    RangeInclusive(Integer, Integer),
}

impl Serialize for Value {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let val = match self {
            Self::Unit => SerVal::Unit,
            Self::Integer(v) => SerVal::Integer(*v),
            Self::Felt(v) => SerVal::Felt(v.as_int()),
            Self::Bool(v) => SerVal::Bool(*v),
            Self::Char(v) => SerVal::Char(*v),
            Self::Str(v) => SerVal::Str(v.clone()),
            Self::Tuple(v) => SerVal::Tuple(v.clone()),
            Self::Vector(v) => SerVal::Vector(v.clone()),
            Self::Array(v) => SerVal::Array(v.clone()),
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
            SerVal::Unit => Self::Unit,
            SerVal::Integer(v) => Self::Integer(v),
            SerVal::Felt(v) => Self::Felt(Felt::new(v)),
            SerVal::Bool(v) => Self::Bool(v),
            SerVal::Char(v) => Self::Char(v),
            SerVal::Str(v) => Self::Str(v),
            SerVal::Tuple(v) => Self::Tuple(v),
            SerVal::Vector(v) => Self::Vector(v),
            SerVal::Array(v) => Self::Array(v),
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
            (Unit, Unit) => true,
            (Integer(n1), Integer(n2)) => n1 == n2,
            (Felt(a), Felt(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Char(a), Char(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Tuple(t1), Tuple(t2)) => t1 == t2,
            (Vector(v1), Vector(v2)) => v1 == v2,
            (Array(a1), Array(a2)) => a1 == a2,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: Env,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Macro {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: Env,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub node: MaatAst,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledFn {
    pub instructions: Rc<[u8]>,
    pub num_locals: usize,
    pub num_parameters: usize,
    pub source_map: SourceMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Closure {
    pub func: CompiledFn,
    pub free_vars: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructVal {
    pub type_index: u16,
    pub fields: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariantVal {
    pub type_index: u16,
    pub tag: u16,
    pub fields: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeDef {
    Struct {
        name: String,
        field_names: Vec<String>,
    },
    Enum {
        name: String,
        variants: Vec<VariantInfo>,
    },
}

/// Metadata for a single enum variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantInfo {
    pub name: String,
    pub field_count: u8,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Map {
    pub pairs: IndexMap<Hashable, Value>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Set(IndexSet<Hashable>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hashable {
    Integer(Integer),
    Felt(u64),
    Bool(bool),
    Char(char),
    Str(String),
}

impl TryFrom<Value> for Hashable {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Integer(i) => Ok(Self::Integer(i)),
            Value::Felt(f) => Ok(Self::Felt(f.as_int())),
            Value::Bool(b) => Ok(Self::Bool(b)),
            Value::Char(c) => Ok(Self::Char(c)),
            Value::Str(s) => Ok(Self::Str(s)),
            val => Err(EvalError::NotHashable(val.type_name().to_owned()).into()),
        }
    }
}

fn write_comma_separated<I, T>(f: &mut fmt::Formatter<'_>, iter: I) -> fmt::Result
where
    I: IntoIterator<Item = T>,
    T: fmt::Display,
{
    let mut iter = iter.into_iter();
    if let Some(first) = iter.next() {
        write!(f, "{first}")?;
        for item in iter {
            write!(f, ", {item}")?;
        }
    }
    Ok(())
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("()"),
            Self::Integer(v) => v.fmt(f),
            Self::Felt(v) => v.fmt(f),
            Self::Bool(v) => v.fmt(f),
            Self::Char(v) => v.fmt(f),
            Self::Str(v) => v.fmt(f),
            Self::Tuple(elems) => {
                f.write_str("(")?;
                write_comma_separated(f, elems)?;
                f.write_str(")")
            }
            Self::Vector(vector) => {
                f.write_str("[")?;
                write_comma_separated(f, vector)?;
                f.write_str("]")
            }
            Self::Array(arr) => {
                f.write_str("[")?;
                write_comma_separated(f, arr)?;
                f.write_str("]")
            }
            Self::Map(v) => v.fmt(f),
            Self::Function(v) => v.fmt(f),
            Self::Macro(v) => v.fmt(f),
            Self::Quote(v) => v.fmt(f),
            Self::ReturnValue(v) => v.fmt(f),
            Self::Break(v) => write!(f, "break {v}"),
            Self::Continue => f.write_str("continue"),
            Self::Builtin(_) => f.write_str("builtin function"),
            Self::CompiledFn(v) => write!(f, "CompiledFn[{v:p}]"),
            Self::Closure(v) => write!(f, "Closure[{:p}]", &v.func),
            Self::Struct(s) => {
                write!(f, "Struct({}", s.type_index)?;
                if !s.fields.is_empty() {
                    f.write_str(" { ")?;
                    write_comma_separated(f, &s.fields)?;
                    f.write_str(" }")?;
                }
                f.write_str(")")
            }
            Self::EnumVariant(v) => {
                write!(f, "EnumVariant({}::{})", v.type_index, v.tag)?;
                if !v.fields.is_empty() {
                    f.write_str("(")?;
                    write_comma_separated(f, &v.fields)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::Set(v) => v.fmt(f),
            Self::Range(start, end) => write!(f, "{start}..{end}"),
            Self::RangeInclusive(start, end) => write!(f, "{start}..={end}"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fn(")?;
        write_comma_separated(f, &self.params)?;
        write!(f, ") {{\n{}\n}}", self.body)
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("macro(")?;
        write_comma_separated(f, &self.params)?;
        write!(f, ") {{\n{}\n}}", self.body)
    }
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote({})", self.node)
    }
}

impl fmt::Display for Map {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        let mut iter = self.pairs.iter();
        if let Some((k, v)) = iter.next() {
            write!(f, "{k}: {v}")?;
            for (k, v) in iter {
                write!(f, ", {k}: {v}")?;
            }
        }
        f.write_str("}")
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Set({")?;
        write_comma_separated(f, &self.0)?;
        f.write_str("})")
    }
}

impl fmt::Display for Hashable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(v) => v.fmt(f),
            Self::Felt(v) => v.fmt(f),
            Self::Bool(b) => b.fmt(f),
            Self::Char(c) => c.fmt(f),
            Self::Str(s) => s.fmt(f),
        }
    }
}
