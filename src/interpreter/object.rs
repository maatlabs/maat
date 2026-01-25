use std::fmt;

use super::Env;
use crate::parser::ast::BlockStatement;

pub const TRUE: Object = Object::Boolean(true);
pub const FALSE: Object = Object::Boolean(false);
pub const NULL: Object = Object::Null;

#[derive(Debug, Clone)]
pub enum Object {
    Null,
    Error(String),
    Int64(i64),
    Boolean(bool),
    Function(Function),
    ReturnValue(Box<Object>),
}

impl Object {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Error(_) => "Error",
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
            Self::Error(err) => err.fmt(f),
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
