//! Runtime value system for Maat.
//!
//! This crate defines the core values and built-in functions shared
//! by both the tree-walking interpreter and the bytecode compiler/VM.

#![forbid(unsafe_code)]

mod builtins;
mod memory;
mod num;

use std::fmt;
use std::rc::Rc;

pub use builtins::{BUILTIN_COUNT, BUILTINS, get_builtin};
use indexmap::{IndexMap, IndexSet};
use maat_ast::Number;
pub use maat_ast::{CastTarget, NumKind};
use maat_errors::{Error, EvalError, Result};
use maat_field::{Encodable as _, FieldElement};
pub use maat_field::{Felt, StarkField, from_i64, try_div, try_inv};
use maat_span::SourceMap;
pub use memory::{
    MaybeRelocatable, MemorySegmentManager, RELOCATION_BASE, Relocatable, SEG_EXECUTION,
    SEG_PRIVATE_INPUT, SEG_PROGRAM, SEG_PUBLIC_INPUT, SEG_PUBLIC_OUTPUT,
};
pub use num::{Integer, WideInt};
use serde::{Deserialize, Serialize};

/// Builtin function signature.
pub type BuiltinFn = fn(&[BuiltinArg<'_>]) -> Result<BuiltinReturn>;

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
    /// A fixed-size array of homogeneous values.
    Array(Vec<Value>),
    /// An ordered map of key-value pairs, backed by [`IndexMap`].
    Map(Map),
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
    /// A logical address within a memory segment.
    Relocatable(Relocatable),
    /// The user-facing runtime form of a vector: a fat pointer to per-instance
    /// segment storage (`base`) paired with the inline length (`len`).
    Vector { base: Relocatable, len: u32 },
}

impl Value {
    /// Encodes this runtime value as a single Goldilocks field element.
    ///
    /// Primitive types encode losslessly within the 64‑bit field. 128‑bit
    /// integers (`I128`, `U128`) cannot fit into one element; they are encoded
    /// as [`Felt::ZERO`] through this method (use [`Encodable::encode`](maat_field))
    /// if you need the full two‑element representation.
    ///
    /// Composite types (`Vector`, `Map`, `Struct`, closures, etc.) also encode
    /// as [`Felt::ZERO`].
    pub fn to_felt(&self) -> Felt {
        match self {
            Self::Felt(f) => *f,
            Self::Integer(int) => int.to_felt().unwrap_or(Felt::ZERO),
            Self::Bool(b) => b.encode()[0],
            Self::Char(c) => c.encode()[0],
            Self::Unit => ().encode()[0],
            _ => Felt::ZERO,
        }
    }

    pub fn to_maybe_relocatable(&self) -> MaybeRelocatable {
        match self {
            Self::Relocatable(r) => MaybeRelocatable::Relocatable(*r),
            other => MaybeRelocatable::Felt(other.to_felt()),
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
            Self::Array(_) => "Array",
            Self::Map(_) => "Map",
            Self::Builtin(_) => "fn",
            Self::CompiledFn(_) => "fn",
            Self::Closure(_) => "fn",
            Self::Struct(_) => "struct",
            Self::EnumVariant(_) => "enum",
            Self::Set(_) => "Set",
            Self::Range(..) => "Range",
            Self::RangeInclusive(..) => "RangeInclusive",
            Self::Relocatable(_) => "Relocatable",
            Self::Vector { .. } => "Vector",
        }
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
            (Array(a1), Array(a2)) => a1 == a2,
            (Map(m1), Map(m2)) => m1 == m2,
            (Builtin(f1), Builtin(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            (CompiledFn(c1), CompiledFn(c2)) => c1 == c2,
            (Closure(c1), Closure(c2)) => c1 == c2,
            (Struct(s1), Struct(s2)) => s1 == s2,
            (EnumVariant(e1), EnumVariant(e2)) => e1 == e2,
            (Set(s1), Set(s2)) => s1 == s2,
            (Range(s1, e1), Range(s2, e2)) => s1 == s2 && e1 == e2,
            (RangeInclusive(s1, e1), RangeInclusive(s2, e2)) => s1 == s2 && e1 == e2,
            (Relocatable(a), Relocatable(b)) => a == b,
            (Vector { base: ba, len: la }, Vector { base: bb, len: lb }) => ba == bb && la == lb,
            _ => false,
        }
    }
}

/// Borrowed view of a builtin function argument.
#[derive(Debug, Clone)]
pub enum BuiltinArg<'a> {
    Unit,
    Integer(Integer),
    Felt(Felt),
    Bool(bool),
    Char(char),
    Str(&'a str),
    Tuple(&'a [Value]),
    Vector(&'a [Value]),
    Array(&'a [Value]),
    Map(&'a Map),
    Set(&'a Set),
    Builtin(BuiltinFn),
    CompiledFn(&'a CompiledFn),
    Closure(&'a Closure),
    Struct(&'a StructVal),
    EnumVariant(&'a EnumVariantVal),
    Range(Integer, Integer),
    RangeInclusive(Integer, Integer),
    Relocatable(Relocatable),
}

impl BuiltinArg<'_> {
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
            Self::Builtin(_) => "fn",
            Self::CompiledFn(_) => "fn",
            Self::Closure(_) => "fn",
            Self::Struct(_) => "struct",
            Self::EnumVariant(_) => "enum",
            Self::Set(_) => "Set",
            Self::Range(..) => "Range",
            Self::RangeInclusive(..) => "RangeInclusive",
            Self::Relocatable(_) => "Relocatable",
        }
    }
}

impl fmt::Display for BuiltinArg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("()"),
            Self::Integer(v) => v.fmt(f),
            Self::Felt(v) => v.fmt(f),
            Self::Bool(v) => v.fmt(f),
            Self::Char(v) => v.fmt(f),
            Self::Str(s) => f.write_str(s),
            Self::Tuple(elems) => {
                f.write_str("(")?;
                write_comma_separated(f, *elems)?;
                f.write_str(")")
            }
            Self::Vector(elems) | Self::Array(elems) => {
                f.write_str("[")?;
                write_comma_separated(f, *elems)?;
                f.write_str("]")
            }
            Self::Map(v) => v.fmt(f),
            Self::Builtin(_) => f.write_str("builtin function"),
            Self::CompiledFn(v) => write!(f, "CompiledFn[{v:p}]"),
            Self::Closure(v) => write!(f, "Closure[{:p}]", &v.func),
            Self::Struct(s) => write!(f, "Struct({}, fields={})", s.type_index, s.fields.len()),
            Self::EnumVariant(v) => write!(
                f,
                "EnumVariant({}::{}, fields={})",
                v.type_index,
                v.tag,
                v.fields.len()
            ),
            Self::Set(v) => v.fmt(f),
            Self::Range(s, e) => write!(f, "{s}..{e}"),
            Self::RangeInclusive(s, e) => write!(f, "{s}..={e}"),
            Self::Relocatable(r) => r.fmt(f),
        }
    }
}

/// Owned return shape produced by a builtin function.
#[derive(Debug, Clone)]
pub enum BuiltinReturn {
    Value(Value),
    Str(String),
    Vector(Vec<BuiltinReturn>),
}

impl BuiltinReturn {
    #[inline]
    pub fn value(v: Value) -> Self {
        Self::Value(v)
    }

    /// Wraps a `Vec<Value>` as the elements of a fresh segment-backed Vector.
    /// Each element is wrapped via [`Self::value`] (already-canonical).
    pub fn vector_from_values(values: Vec<Value>) -> Self {
        Self::Vector(values.into_iter().map(Self::Value).collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledFn {
    pub instructions: Rc<[u8]>,
    pub num_locals: usize,
    pub num_parameters: usize,
    pub source_map: SourceMap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Closure {
    pub func: CompiledFn,
    pub base: Option<Relocatable>,
    pub num_free: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructVal {
    pub type_index: u16,
    pub fields: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map {
    pub pairs: IndexMap<Hashable, Value>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Set(pub IndexSet<Hashable>);

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

impl TryFrom<BuiltinArg<'_>> for Hashable {
    type Error = Error;

    fn try_from(value: BuiltinArg<'_>) -> Result<Self> {
        match value {
            BuiltinArg::Integer(i) => Ok(Self::Integer(i)),
            BuiltinArg::Felt(f) => Ok(Self::Felt(f.as_int())),
            BuiltinArg::Bool(b) => Ok(Self::Bool(b)),
            BuiltinArg::Char(c) => Ok(Self::Char(c)),
            BuiltinArg::Str(s) => Ok(Self::Str(s.to_owned())),
            other => Err(EvalError::NotHashable(other.type_name().to_owned()).into()),
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
            Self::Array(arr) => {
                f.write_str("[")?;
                write_comma_separated(f, arr)?;
                f.write_str("]")
            }
            Self::Map(v) => v.fmt(f),
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
            Self::Relocatable(r) => r.fmt(f),
            Self::Vector { base, len } => write!(f, "Vector[base={base}, len={len}]"),
        }
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
