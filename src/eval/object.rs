use std::collections::HashMap;
use std::fmt;

use super::Env;
use crate::ast::BlockStatement;
use crate::{Error, EvalError, Result};

pub type BuiltinFn = fn(&[Object]) -> Result<Object>;

pub const TRUE: Object = Object::Boolean(true);
pub const FALSE: Object = Object::Boolean(false);
pub const NULL: Object = Object::Null;

/// Runtime value representation in the interpreter.
///
/// Objects are the evaluated results of expressions and can be integers,
/// booleans, functions, or special values like null and return wrappers.
#[derive(Debug, Clone)]
pub enum Object {
    /// The null object, representing absence of a value.
    Null,

    /// 8-bit signed integer.
    I8(i8),
    /// 16-bit signed integer.
    I16(i16),
    /// 32-bit signed integer.
    I32(i32),
    /// 64-bit signed integer.
    I64(i64),
    /// 128-bit signed integer.
    I128(i128),
    /// Pointer-sized signed integer.
    Isize(isize),

    /// 8-bit unsigned integer.
    U8(u8),
    /// 16-bit unsigned integer.
    U16(u16),
    /// 32-bit unsigned integer.
    U32(u32),
    /// 64-bit unsigned integer.
    U64(u64),
    /// 128-bit unsigned integer.
    U128(u128),
    /// Pointer-sized unsigned integer.
    Usize(usize),

    /// 32-bit floating-point number.
    F32(f32),
    /// 64-bit floating-point number.
    F64(f64),

    /// A boolean value (true or false).
    Boolean(bool),
    /// A string literal.
    String(String),
    /// An array literal.
    Array(Vec<Object>),
    /// A hashable object.
    Hash(HashObject),
    /// A function object with parameters, body, and closure environment.
    Function(Function),
    /// Wraps a return value for early function/block termination.
    ReturnValue(Box<Object>),
    /// A builtin function.
    Builtin(BuiltinFn),
}

impl Object {
    /// Returns a string representation of the object's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::I8(_) => "I8",
            Self::I16(_) => "I16",
            Self::I32(_) => "I32",
            Self::I64(_) => "I64",
            Self::I128(_) => "I128",
            Self::Isize(_) => "Isize",
            Self::U8(_) => "U8",
            Self::U16(_) => "U16",
            Self::U32(_) => "U32",
            Self::U64(_) => "U64",
            Self::U128(_) => "U128",
            Self::Usize(_) => "Usize",
            Self::F32(_) => "F32",
            Self::F64(_) => "F64",
            Self::Boolean(_) => "Boolean",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Hash(_) => "Hashable",
            Self::Function(_) => "Function",
            Self::ReturnValue(_) => "ReturnValue",
            Self::Builtin(_) => "BuiltinFn",
        }
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        use Object::*;
        match (self, other) {
            (Null, Null) => true,
            (I8(a), I8(b)) => a == b,
            (I16(a), I16(b)) => a == b,
            (I32(a), I32(b)) => a == b,
            (I64(a), I64(b)) => a == b,
            (I128(a), I128(b)) => a == b,
            (Isize(a), Isize(b)) => a == b,
            (U8(a), U8(b)) => a == b,
            (U16(a), U16(b)) => a == b,
            (U32(a), U32(b)) => a == b,
            (U64(a), U64(b)) => a == b,
            (U128(a), U128(b)) => a == b,
            (Usize(a), Usize(b)) => a == b,
            (F32(a), F32(b)) => a == b,
            (F64(a), F64(b)) => a == b,
            (Boolean(a), Boolean(b)) => a == b,
            (String(a), String(b)) => a == b,
            (Array(a1), Array(a2)) => a1 == a2,
            (Hash(h1), Hash(h2)) => h1 == h2,
            (Function(f1), Function(f2)) => f1 == f2,
            (ReturnValue(o1), ReturnValue(o2)) => o1 == o2,
            (Builtin(f1), Builtin(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            _ => false,
        }
    }
}

/// Represents a function object.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub env: Env,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HashObject {
    pub pairs: HashMap<Hashable, Object>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Hashable {
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
    Boolean(bool),
    String(String),
}

impl TryFrom<Object> for Hashable {
    type Error = Error;

    fn try_from(value: Object) -> Result<Self> {
        match value {
            Object::I8(i) => Ok(Self::I8(i)),
            Object::I16(i) => Ok(Self::I16(i)),
            Object::I32(i) => Ok(Self::I32(i)),
            Object::I64(i) => Ok(Self::I64(i)),
            Object::I128(i) => Ok(Self::I128(i)),
            Object::Isize(i) => Ok(Self::Isize(i)),
            Object::U8(i) => Ok(Self::U8(i)),
            Object::U16(i) => Ok(Self::U16(i)),
            Object::U32(i) => Ok(Self::U32(i)),
            Object::U64(i) => Ok(Self::U64(i)),
            Object::U128(i) => Ok(Self::U128(i)),
            Object::Usize(i) => Ok(Self::Usize(i)),
            Object::Boolean(b) => Ok(Self::Boolean(b)),
            Object::String(s) => Ok(Self::String(s)),
            obj => Err(EvalError::NotHashable(obj.type_name().to_owned()).into()),
        }
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
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
            Self::F32(v) => v.fmt(f),
            Self::F64(v) => v.fmt(f),
            Self::Boolean(boolean) => boolean.fmt(f),
            Self::String(string) => string.fmt(f),
            Self::Array(array) => {
                write!(
                    f,
                    "[{}]",
                    array
                        .iter()
                        .map(|obj| format!("{obj}"))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Self::Hash(hash) => hash.fmt(f),
            Self::Function(func) => func.fmt(f),
            Self::ReturnValue(ret_val) => ret_val.fmt(f),
            Self::Builtin(_) => write!(f, "builtin function"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for HashObject {
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
            Self::Boolean(b) => b.fmt(f),
            Self::String(s) => s.fmt(f),
        }
    }
}
