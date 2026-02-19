use maat_span::Span;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error("eval error: {0}")]
    Eval(#[from] EvalError),

    #[error("compile error: {0}")]
    Compile(#[from] CompileError),

    #[error("vm error: {0}")]
    Vm(#[from] VmError),

    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),
}

#[derive(Debug, thiserror::Error)]
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

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("{0}")]
    Identifier(String),

    #[error("{0}")]
    IndexExpression(String),

    #[error("{0}")]
    PrefixExpression(String),

    #[error("{0}")]
    InfixExpression(String),

    #[error("{0}")]
    Boolean(String),

    #[error("{0}")]
    Number(String),

    #[error("{0}")]
    NotAFunction(String),

    #[error("unusable as hash key: {0}")]
    NotHashable(String),

    #[error("{0}")]
    Builtin(String),

    #[error("value not found")]
    ValueNotFound,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error(
        "constant pool overflow: exceeded maximum of {max} constants (attempted index: {attempted})"
    )]
    ConstantPoolOverflow { max: usize, attempted: usize },

    #[error("unsupported operator '{operator}' in {context}")]
    UnsupportedOperator { operator: String, context: String },

    #[error(
        "unsupported expression type '{expr_type}' (not yet implemented in this compiler phase)"
    )]
    UnsupportedExpression { expr_type: String },

    #[error("invalid opcode 0x{opcode:02x} at instruction position {position}")]
    InvalidOpcode { opcode: u8, position: usize },

    #[error("undefined variable '{name}'")]
    UndefinedVariable { name: String },

    #[error(
        "symbols table overflow: exceeded maximum of {max} global bindings (attempted to define '{name}')"
    )]
    SymbolsTableOverflow { max: usize, name: String },

    #[error(
        "local variable overflow: exceeded maximum of {max} local bindings in function scope (attempted to define '{name}')"
    )]
    LocalsOverflow { max: usize, name: String },

    #[error("scope stack underflow: attempted to leave scope with no enclosing scope")]
    ScopeUnderflow,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct VmError {
    pub message: String,
}

impl VmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<String> for VmError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for VmError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error(
        "bytecode truncated: needed {needed} bytes at offset {offset}, but only {available} bytes available"
    )]
    UnexpectedEndOfBytecode {
        offset: usize,
        needed: usize,
        available: usize,
    },

    #[error("unsupported operand width: {0} (valid widths: 1, 2, 4, 8)")]
    UnsupportedOperandWidth(usize),

    #[error("invalid opcode: 0x{0:02x}")]
    InvalidOpcode(u8),
}
