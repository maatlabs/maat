#![forbid(unsafe_code)]

mod lexer;
mod num;
pub mod token;

pub use lexer::Lexer;
pub use maat_span::Span;
pub use token::{Token, TokenKind};
