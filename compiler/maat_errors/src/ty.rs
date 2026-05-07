use maat_span::Span;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

#[derive(Debug, Error)]
pub enum TypeErrorKind {
    #[error("type mismatch: expected `{expected}`, found `{found}`")]
    Mismatch { expected: String, found: String },

    #[error(
        "type mismatch: expected `{expected}`, found `{found}`\n  help: consider using `as {expected}` for explicit numeric conversion"
    )]
    NumericMismatch { expected: String, found: String },

    #[error("infinite type: `{0}` occurs in its own definition")]
    OccursCheck(String),

    #[error("wrong number of arguments: expected {expected}, found {found}")]
    WrongArity { expected: usize, found: usize },

    #[error("expression of type `{0}` is not callable")]
    NotCallable(String),

    #[error("numeric overflow: `{value}` out of range for `{target}`")]
    NumericOverflow { value: String, target: String },

    #[error("division by zero: `{value}`")]
    DivisionByZero { value: String },

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

#[derive(Debug, Error)]
#[error("missing trait method `{method}` in impl of `{trait_name}` for `{self_type}`")]
pub struct MissingTraitMethodError {
    pub trait_name: String,
    pub self_type: String,
    pub method: String,
}

#[derive(Debug, Error)]
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
