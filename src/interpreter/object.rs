use std::fmt;

use super::Env;
use crate::parser::ast::BlockStatement;

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
    /// A boolean value (true or false).
    Boolean(bool),
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
            Self::Boolean(_) => "Boolean",
            Self::Function(_) => "Function",
            Self::ReturnValue(_) => "ReturnValue",
        }
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        use Object::*;
        match (self, other) {
            (Null, Null) => true,
            (Int64(a), Int64(b)) => a == b,
            (Boolean(a), Boolean(b)) => a == b,
            (Function(f1), Function(f2)) => f1 == f2,
            (ReturnValue(o1), ReturnValue(o2)) => o1 == o2,
            _ => false,
        }
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Int64(int64) => int64.fmt(f),
            Self::Boolean(boolean) => boolean.fmt(f),
            Self::Function(func) => func.fmt(f),
            Self::ReturnValue(ret_val) => ret_val.fmt(f),
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

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}
