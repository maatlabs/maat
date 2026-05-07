use maat_span::Span;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct VmError {
    pub message: String,
    pub span: Option<Span>,
}

impl VmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn bound_exceeded(bound: u64) -> Self {
        Self::new(format!(
            "loop exceeded its declared bound of {bound} iterations"
        ))
    }
}

impl From<String> for VmError {
    fn from(message: String) -> Self {
        Self {
            message,
            span: None,
        }
    }
}

impl From<&str> for VmError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            span: None,
        }
    }
}
