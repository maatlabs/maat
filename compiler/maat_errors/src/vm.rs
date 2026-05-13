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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MemoryError {
    #[error("segment count exceeds u32::MAX")]
    SegmentCountOverflow,
    #[error("segment {0} does not exist")]
    SegmentNotFound(u32),
    #[error("segment {0} holds more than u32::MAX cells")]
    SegmentTooLarge(u32),
    #[error("offset overflow: segment {segment}, offset {offset}, addend {addend}")]
    OffsetOverflow {
        segment: u32,
        offset: u32,
        addend: u64,
    },
    #[error("cross-segment subtraction: lhs segment {lhs_segment}, rhs segment {rhs_segment}")]
    CrossSegmentSubtraction { lhs_segment: u32, rhs_segment: u32 },
    #[error("write-once violation at {segment}:{offset}")]
    WriteOnceViolation { segment: u32, offset: u32 },
    #[error(
        "declared size {declared} for segment {segment} is below highest written offset {actual}"
    )]
    DeclaredSizeUnderflow {
        segment: u32,
        declared: u32,
        actual: u32,
    },
    #[error("relocation table overflow: cumulative base exceeds u32::MAX")]
    RelocationOverflow,
}
