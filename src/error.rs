use core::fmt;
use std::error::Error as StdError;

#[derive(Debug)]
pub enum Error {
    Eval(EvalError),
}

#[derive(Debug)]
pub enum EvalError {
    Identifier(String),
    IndexExpression(String),
    PrefixExpression(String),
    InfixExpression(String),
    Boolean(String),
    Number(String),
    NotAFunction(String),
    NotHashable(String),
    ValueNotFound,
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Eval(inner) => Some(inner),
        }
    }
}

impl StdError for EvalError {}

impl From<EvalError> for Error {
    fn from(e: EvalError) -> Self {
        Self::Eval(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(inner) => write!(f, "eval error: {inner}"),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(msg) => write!(f, "{msg}"),
            Self::IndexExpression(msg) => write!(f, "{msg}"),
            Self::PrefixExpression(msg) => write!(f, "{msg}"),
            Self::InfixExpression(msg) => write!(f, "{msg}"),
            Self::Boolean(msg) => write!(f, "{msg}"),
            Self::Number(msg) => write!(f, "{msg}"),
            Self::NotAFunction(msg) => write!(f, "{msg}"),
            Self::NotHashable(msg) => write!(f, "unusable as hash key: {msg}"),
            Self::ValueNotFound => write!(f, "value not found"),
        }
    }
}
