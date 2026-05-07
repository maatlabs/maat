use maat_lexer::TokenKind;

/// Lowest or minimum binding power (mbp)
pub const MIN_BP: Precedence = Precedence::Lowest;

/// Operator precedence (binding power).
/// Higher == tighter binding.
///
/// Follows Rust's operator precedence order from lowest to highest:
/// `..`/`..=`, `||`, `&&`, `|`, `^`, `&`, `== !=`, `< > <= >=`, `<< >>`,
/// `+ -`, `* / %`, `as`, `prefix`, `call`, `index`, `field`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    /// Lowest binding power (default when no operator applies).
    Lowest = 1,
    /// Range operator (`..` or `..=`)
    Range = 2,
    /// Logical OR `||`
    LogicalOr = 3,
    /// Logical AND `&&`
    LogicalAnd = 4,
    /// Bitwise OR `|`
    BitwiseOr = 5,
    /// Bitwise XOR `^`
    BitwiseXor = 6,
    /// Bitwise AND `&`
    BitwiseAnd = 7,
    /// Equal (`==`), NotEqual (`!=`)
    Equality = 8,
    /// LessThan (`<`), GreaterThan (`>`),
    /// LessThanOrEqual (`<=`), GreaterThanOrEqual (`>=`)
    LessGreater = 9,
    /// Left shift (`<<`) or right shift (`>>`)
    Shift = 10,
    /// Add (`+`) or Subtract (`-`)
    Sum = 11,
    /// Mult (`*`), Div (`/`), or Modulo (`%`)
    Product = 12,
    /// Type cast: `expr as type`
    Cast = 13,
    /// Prefix ops: `-x`, `!x`
    Prefix = 14,
    /// Function calls: `f(x)`
    Call = 15,
    /// Vector/array indexing and index expressions: `expr[i]`
    Index = 16,
    /// Field access and method calls: `expr.field`, `expr.method(args)`
    Field = 17,
    /// Try operator: `expr?`
    Try = 18,
}

impl Precedence {
    #[inline]
    pub fn get(kind: &TokenKind) -> Option<Self> {
        match *kind {
            TokenKind::DotDot | TokenKind::DotDotEqual => Some(Self::Range),
            TokenKind::Or => Some(Self::LogicalOr),
            TokenKind::And => Some(Self::LogicalAnd),
            TokenKind::Pipe => Some(Self::BitwiseOr),
            TokenKind::Caret => Some(Self::BitwiseXor),
            TokenKind::Ampersand => Some(Self::BitwiseAnd),
            TokenKind::Equal | TokenKind::NotEqual => Some(Self::Equality),
            TokenKind::Less
            | TokenKind::Greater
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual => Some(Self::LessGreater),
            TokenKind::ShiftLeft | TokenKind::ShiftRight => Some(Self::Shift),
            TokenKind::Plus | TokenKind::Minus => Some(Self::Sum),
            TokenKind::Asterisk | TokenKind::Slash | TokenKind::Percent => Some(Self::Product),
            TokenKind::As => Some(Self::Cast),
            TokenKind::LParen => Some(Self::Call),
            TokenKind::LBracket => Some(Self::Index),
            TokenKind::Dot => Some(Self::Field),
            TokenKind::Question => Some(Self::Try),
            _ => None,
        }
    }
}
