pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;

pub use error::Error;
pub use interpreter::Env;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;

pub type Result<T> = std::result::Result<T, Error>;
