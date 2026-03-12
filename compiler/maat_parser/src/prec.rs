//! Operator precedence (binding power).
//! Higher == tighter binding.
//!
//! Follows Rust's operator precedence order from lowest to highest:
//! `||`, `&&`, `|`, `^`, `&`, `== !=`, `< > <= >=`, `<< >>`, `+ -`, `* / %`,
//! `as`, prefix, call, index, field.

use maat_lexer::TokenKind;

/// Lowest binding power (default when no operator applies).
pub const LOWEST: u8 = 1;
/// `||`
pub const LOGICAL_OR: u8 = 2;
/// `&&`
pub const LOGICAL_AND: u8 = 3;
/// `|`
pub const BITWISE_OR: u8 = 4;
/// `^`
pub const BITWISE_XOR: u8 = 5;
/// `&`
pub const BITWISE_AND: u8 = 6;
/// `==`, `!=`
pub const EQUALITY: u8 = 7;
/// `<`, `>`, `<=`, `>=`
pub const LESSGREATER: u8 = 8;
/// `<<`, `>>`
pub const SHIFT: u8 = 9;
/// `+`, `-`
pub const SUM: u8 = 10;
/// `*`, `/`, `%`
pub const PRODUCT: u8 = 11;
/// Type cast: `expr as type`
pub const CAST: u8 = 12;
/// Prefix ops: `-x`, `!x`
pub const PREFIX: u8 = 13;
/// Function calls: `f(x)`
pub const CALL: u8 = 14;
/// Array indexing and index expressions: `expr[i]`
pub const INDEX: u8 = 15;
/// Field access and method calls: `expr.field`, `expr.method(args)`
pub const FIELD: u8 = 16;

pub struct Precedence;

impl Precedence {
    /// Returns the precedence for a token kind, or None if
    /// it has no special binding power (caller typically falls back to `LOWEST`).
    #[inline]
    pub fn get(kind: &TokenKind) -> Option<u8> {
        match *kind {
            TokenKind::Or => Some(LOGICAL_OR),
            TokenKind::And => Some(LOGICAL_AND),
            TokenKind::Pipe => Some(BITWISE_OR),
            TokenKind::Caret => Some(BITWISE_XOR),
            TokenKind::Ampersand => Some(BITWISE_AND),
            TokenKind::Equal | TokenKind::NotEqual => Some(EQUALITY),
            TokenKind::Less
            | TokenKind::Greater
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual => Some(LESSGREATER),
            TokenKind::ShiftLeft | TokenKind::ShiftRight => Some(SHIFT),
            TokenKind::Plus | TokenKind::Minus => Some(SUM),
            TokenKind::Asterisk | TokenKind::Slash | TokenKind::Percent => Some(PRODUCT),
            TokenKind::As => Some(CAST),
            TokenKind::LParen => Some(CALL),
            TokenKind::LBracket => Some(INDEX),
            TokenKind::Dot => Some(FIELD),
            _ => None,
        }
    }
}
