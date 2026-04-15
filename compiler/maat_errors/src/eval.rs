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

    #[error("loop exceeded its declared bound of {0} iterations")]
    BoundExceeded(u64),
}
