use std::collections::HashMap;
use std::fmt;

use super::Env;
use crate::error::EvalError;
use crate::parser::ast::BlockStatement;
use crate::{Error, Result};

/// Runtime value representation in the interpreter.
///
/// Objects are the evaluated results of expressions and can be integers,
/// booleans, functions, or special values like null and return wrappers.
#[derive(Debug, Clone)]
pub enum Object {
    /// The null object, representing absence of a value.
    Null,
    /// A 64-bit signed integer.
    Int64(i64),
    /// A 64-bit floating-point number.
    Float64(f64),
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
}

impl Object {
    /// Returns a string representation of the object's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Int64(_) => "Int64",
            Self::Float64(_) => "Float64",
            Self::Boolean(_) => "Boolean",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Hash(_) => "Hashable",
            Self::Function(_) => "Function",
            Self::ReturnValue(_) => "ReturnValue",
        }
    }

    pub fn is_hashable(&self) -> bool {
        matches!(self, Self::Int64(_) | Self::Boolean(_) | Self::String(_))
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        use Object::*;
        match (self, other) {
            (Null, Null) => true,
            (Int64(a), Int64(b)) => a == b,
            (Float64(a), Float64(b)) => a == b,
            (Boolean(a), Boolean(b)) => a == b,
            (String(a), String(b)) => a == b,
            (Array(a1), Array(a2)) => a1 == a2,
            (Hash(h1), Hash(h2)) => h1 == h2,
            (Function(f1), Function(f2)) => f1 == f2,
            (ReturnValue(o1), ReturnValue(o2)) => o1 == o2,
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
    Int64(i64),
    Boolean(bool),
    String(String),
}

impl TryFrom<Object> for Hashable {
    type Error = Error;

    fn try_from(value: Object) -> Result<Self> {
        match value {
            Object::Int64(i) => Ok(Self::Int64(i)),
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
            Self::Int64(int64) => int64.fmt(f),
            Self::Float64(float64) => float64.fmt(f),
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
            Self::Int64(i) => i.fmt(f),
            Self::Boolean(b) => b.fmt(f),
            Self::String(s) => s.fmt(f),
        }
    }
}
