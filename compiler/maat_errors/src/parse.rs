use maat_span::Span;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("parse error at {}..{}: {message}", span.start, span.end)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}
