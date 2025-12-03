#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub literal: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Keywords
    Let,
    If,
    Else,
    True,
    False,
    Function,
    Return,

    // Identifiers and literals
    Ident,
    Int,

    // Operators
    Assign,
    Plus,
    Minus,
    Bang,
    Asterisk,
    Slash,

    Less,
    Greater,

    Equal,
    NotEqual,

    // Delimiters
    Comma,
    Semicolon,

    LParen,
    RParen,
    LBrace,
    RBrace,

    Invalid,
    Eof,
}

impl TokenKind {
    /// Maps the given input to a valid keyword,
    /// defaulting to `Self::Ident` if no match is found.
    #[inline]
    pub fn keyword_or_ident(k: &str) -> Self {
        match k {
            "let" => Self::Let,
            "if" => Self::If,
            "else" => Self::Else,
            "true" => Self::True,
            "false" => Self::False,
            "fn" => Self::Function,
            "return" => Self::Return,
            _ => Self::Ident,
        }
    }
}

impl<'a> Token<'a> {
    pub fn new(kind: TokenKind, literal: &'a str) -> Self {
        Self { kind, literal }
    }
}

impl<'a> core::fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.kind {
            TokenKind::Ident => write!(f, "{}", self.literal),
            _ => write!(f, "{:?}", self.kind),
        }
    }
}
