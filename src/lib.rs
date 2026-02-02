pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod repl;

pub use error::{Error, EvalError, ParseError};
pub use eval::{Env, Hashable, NULL, Object, eval};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{Parser, ast};

pub type Result<T> = std::result::Result<T, Error>;
