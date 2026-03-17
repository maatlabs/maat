#![forbid(unsafe_code)]

use std::path::PathBuf;

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

    #[error("type error: {0}")]
    Type(#[from] TypeError),

    #[error("vm error: {0}")]
    Vm(#[from] VmError),

    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),

    #[error("serialization error: {0}")]
    Serialization(#[from] SerializationError),

    #[error("module error: {0}")]
    Module(#[from] ModuleError),
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
    Ident(String),

    #[error("{0}")]
    IndexExpr(String),

    #[error("{0}")]
    PrefixExpr(String),

    #[error("{0}")]
    InfixExpr(String),

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
}

/// A type-checking error with a source span.
///
/// Wraps [`TypeErrorKind`] with location information for rich diagnostics.
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

/// The underlying variant of a type-checking error.
#[derive(Debug, thiserror::Error)]
pub enum TypeErrorKind {
    #[error("type mismatch: expected `{expected}`, found `{found}`")]
    Mismatch { expected: String, found: String },

    #[error("infinite type: `{0}` occurs in its own definition")]
    OccursCheck(String),

    #[error("wrong number of arguments: expected {expected}, found {found}")]
    WrongArity { expected: usize, found: usize },

    #[error("expression of type `{0}` is not callable")]
    NotCallable(String),

    #[error("numeric overflow: `{value}` out of range for `{target}`")]
    NumericOverflow { value: String, target: String },

    #[error("unknown type `{0}`")]
    UnknownType(String),

    #[error("no field `{field}` on type `{ty}`")]
    UnknownField { ty: String, field: String },

    #[error("no method `{method}` found for type `{ty}`")]
    UnknownMethod { ty: String, method: String },

    #[error("duplicate type definition `{0}`")]
    DuplicateType(String),

    #[error("{0}")]
    MissingTraitMethod(Box<MissingTraitMethodError>),

    #[error("{0}")]
    TraitMethodSignatureMismatch(Box<TraitMethodSignatureMismatchError>),

    #[error("non-exhaustive patterns in `match`: {missing}")]
    NonExhaustiveMatch { missing: String },

    #[error("unknown trait `{0}`")]
    UnknownTrait(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("item `{item}` is private to module `{module}`")]
    PrivateAccess { item: String, module: String },
}

/// Detail for a missing trait method error.
#[derive(Debug, thiserror::Error)]
#[error("missing trait method `{method}` in impl of `{trait_name}` for `{self_type}`")]
pub struct MissingTraitMethodError {
    pub trait_name: String,
    pub self_type: String,
    pub method: String,
}

/// Detail for a trait method signature mismatch error.
#[derive(Debug, thiserror::Error)]
#[error(
    "method `{method}` has wrong signature in impl of `{trait_name}` for `{self_type}`: expected `{expected}`, found `{found}`"
)]
pub struct TraitMethodSignatureMismatchError {
    pub trait_name: String,
    pub self_type: String,
    pub method: String,
    pub expected: String,
    pub found: String,
}

impl TypeErrorKind {
    /// Attaches a source span to this error kind, producing a [`TypeError`].
    pub fn at(self, span: Span) -> TypeError {
        TypeError { kind: self, span }
    }
}

/// A compile-time error with an optional source span.
///
/// Wraps [`CompileErrorKind`] with location information for rich diagnostics.
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Option<Span>,
}

impl CompileError {
    /// Creates a compile error from a kind with no associated span.
    pub fn new(kind: CompileErrorKind) -> Self {
        Self { kind, span: None }
    }
}

/// The underlying variant of a compile-time error.
#[derive(Debug, thiserror::Error)]
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

    #[error("cannot re-assign to immutable variable `{name}`")]
    ImmutableAssignment { name: String },

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

/// A runtime VM error with an optional source span.
#[derive(Debug, thiserror::Error)]
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

    /// Creates a VM error with an associated source span.
    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
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

/// Errors arising during bytecode serialization or deserialization.
///
/// Header-level errors (`InvalidMagic`, `UnsupportedVersion`, `UnexpectedEof`)
/// are checked before the payload is decoded. Payload-level errors are
/// reported by `postcard` via `serde` and surfaced as `PostcardEncode` or
/// `PostcardDecode`.
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    /// The file does not begin with the expected `MAAT` magic bytes.
    #[error("invalid magic bytes: expected MAAT header")]
    InvalidMagic,

    /// The format version in the header is not supported by this build.
    #[error("unsupported bytecode format version: {0}")]
    UnsupportedVersion(u32),

    /// The byte stream was truncated before the header could be fully read.
    #[error("unexpected end of bytecode at offset {offset}: needed {needed} more bytes")]
    UnexpectedEof { offset: usize, needed: usize },

    /// An error occurred while encoding bytecode with postcard.
    #[error("bytecode encode error: {0}")]
    PostcardEncode(String),

    /// An error occurred while decoding bytecode with postcard.
    #[error("bytecode decode error: {0}")]
    PostcardDecode(String),

    /// The bytecode payload exceeds the maximum allowed size.
    #[error("bytecode payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    /// A deserialized field exceeds its allowed resource limit.
    #[error("{field} too large: {size} exceeds limit of {limit}")]
    ResourceLimitExceeded {
        field: &'static str,
        size: usize,
        limit: usize,
    },
}

/// A module resolution error with file context and source span.
///
/// Wraps [`ModuleErrorKind`] with the originating file path and span
/// for rich diagnostic output.
#[derive(Debug, thiserror::Error)]
#[error("{file}: {kind}", file = file.display())]
pub struct ModuleError {
    pub kind: ModuleErrorKind,
    pub span: Span,
    /// The file in which the error was encountered.
    pub file: PathBuf,
}

/// The underlying variant of a module resolution error.
#[derive(Debug, thiserror::Error)]
pub enum ModuleErrorKind {
    /// A `mod foo;` declaration could not be resolved to a source file.
    #[error("module `{module_name}` not found; searched: {}", candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    FileNotFound {
        module_name: String,
        candidates: Vec<PathBuf>,
    },

    /// A cycle was detected in the module dependency graph.
    #[error("cyclic module dependency: {}", cycle.join(" -> "))]
    CyclicDependency {
        /// The module names forming the cycle, in visitation order.
        cycle: Vec<String>,
    },

    /// A module was declared more than once in the same parent module.
    #[error("duplicate module declaration `{module_name}`")]
    DuplicateModule { module_name: String },

    /// The parser encountered errors in a module source file.
    #[error("parse errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    ParseErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// Type errors encountered during module type checking.
    #[error("type errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    TypeErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// Compilation errors encountered during module code generation.
    #[error("compile errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    CompileErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// An I/O error occurred while reading a source file.
    #[error("cannot read `{}`: {message}", path.display())]
    Io { path: PathBuf, message: String },
}

impl ModuleErrorKind {
    /// Attaches a source span and file path to this error kind,
    /// producing a [`ModuleError`].
    pub fn at(self, span: Span, file: PathBuf) -> ModuleError {
        ModuleError {
            kind: self,
            span,
            file,
        }
    }
}
