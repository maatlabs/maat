use maat_span::Span;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Option<Span>,
}

impl CompileError {
    pub fn new(kind: CompileErrorKind) -> Self {
        Self { kind, span: None }
    }
}

#[derive(Debug, Error)]
pub enum CompileErrorKind {
    #[error(
        "constant pool overflow: exceeded maximum of {max} constants (attempted index: {attempted})"
    )]
    ConstantPoolOverflow { max: usize, attempted: usize },

    #[error("unsupported operator '{operator}' in {context}")]
    UnsupportedOperator { operator: String, context: String },

    #[error("unsupported expression type '{expr_type}'")]
    UnsupportedExpr { expr_type: String },

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

    #[error("`break` outside of a loop")]
    BreakOutsideLoop,

    #[error("`continue` outside of a loop")]
    ContinueOutsideLoop,

    #[error("undeclared label `'{label}`")]
    UndeclaredLabel { label: String },

    #[error("cannot re-assign to immutable variable `{name}`")]
    ImmutableAssignment { name: String },

    #[error("unknown builtin macro `{name}!`")]
    UnknownMacro { name: String },

    #[error(
        "format string has {placeholders} placeholder(s) but {arguments} argument(s) were supplied"
    )]
    FormatArgCountMismatch {
        placeholders: usize,
        arguments: usize,
    },

    #[error("`{macro_name}!` requires a format string literal as its first argument")]
    MacroExpectsFormatString { macro_name: String },

    #[error(
        "enum `{name}` has {count} variants, exceeding the maximum of {max} (variant tags must fit in 8 bits)"
    )]
    VariantTagOverflow {
        name: String,
        count: usize,
        max: usize,
    },
}

impl CompileErrorKind {
    /// Attaches a source span to this error kind, producing a [`CompileError`].
    pub fn at(self, span: Span) -> CompileError {
        CompileError {
            kind: self,
            span: Some(span),
        }
    }
}

impl From<CompileErrorKind> for CompileError {
    fn from(kind: CompileErrorKind) -> Self {
        Self { kind, span: None }
    }
}
