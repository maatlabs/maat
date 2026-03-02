//! Operator precedence (binding power).
//! Higher == tighter binding.

use maat_lexer::TokenKind;

/// Lowest binding power (default when no operator applies).
pub const LOWEST: u8 = 1;
/// `||`
pub const LOGICAL_OR: u8 = 2;
/// `&&`
pub const LOGICAL_AND: u8 = 3;
/// `==`, `!=`
pub const EQUALITY: u8 = 4;
/// `<`, `>`, `<=`, `>=`
pub const LESSGREATER: u8 = 5;
/// `+`, `-`
pub const SUM: u8 = 6;
/// `*`, `/`
pub const PRODUCT: u8 = 7;
/// Type cast: `expr as type`
pub const CAST: u8 = 8;
/// Prefix ops: `-x`, `!x`
pub const PREFIX: u8 = 9;
/// Function calls: `f(x)`
pub const CALL: u8 = 10;
/// Array indexing and index expressions: `expr[i]`
pub const INDEX: u8 = 11;

pub struct Precedence;

impl Precedence {
    /// Returns the precedence for a token kind, or None if
    /// it has no special binding power (caller typically falls back to `LOWEST`).
    #[inline]
    pub fn get(&self, kind: &TokenKind) -> Option<u8> {
        match *kind {
            TokenKind::Or => Some(LOGICAL_OR),
            TokenKind::And => Some(LOGICAL_AND),
            TokenKind::Equal | TokenKind::NotEqual => Some(EQUALITY),
            TokenKind::Less
            | TokenKind::Greater
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual => Some(LESSGREATER),
            TokenKind::Plus | TokenKind::Minus => Some(SUM),
            TokenKind::Asterisk | TokenKind::Slash => Some(PRODUCT),
            TokenKind::As => Some(CAST),
            TokenKind::LParen => Some(CALL),
            TokenKind::LBracket => Some(INDEX),
            _ => None,
        }
    }
}
