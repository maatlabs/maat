use core::fmt;
use std::error::Error as StdError;

use crate::lexer::Span;

#[derive(Debug)]
pub enum Error {
    Parse(ParseError),
    Eval(EvalError),
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
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
    Builtin(String),
    ValueNotFound,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Parse(_) => None,
            Self::Eval(inner) => Some(inner),
        }
    }
}

impl StdError for EvalError {}
impl StdError for ParseError {}

impl From<EvalError> for Error {
    fn from(e: EvalError) -> Self {
        Self::Eval(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(inner) => write!(f, "{inner}"),
            Self::Eval(inner) => write!(f, "eval error: {inner}"),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
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
            Self::Builtin(msg) => write!(f, "{msg}"),
            Self::ValueNotFound => write!(f, "value not found"),
        }
    }
}
