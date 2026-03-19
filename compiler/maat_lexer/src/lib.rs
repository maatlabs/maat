#![forbid(unsafe_code)]

mod lexer;
mod num;
pub mod token;

pub use lexer::Lexer;
pub use token::{Token, TokenKind};
