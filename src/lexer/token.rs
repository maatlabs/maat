/// A lexical token in Maat.
///
/// Tokens represent the smallest meaningful units of source code, such as
/// keywords, operators, identifiers, and literals. Each token carries both its
/// syntactic classification ([`TokenKind`]) and the exact source text it represents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'a> {
    /// The syntactic classification of this token.
    pub kind: TokenKind,
    /// The raw source text that produced this token.
    pub literal: &'a str,
}

/// The syntactic classification of a lexical token.
///
/// This enum defines all possible token types in Maat, including
/// keywords, operators, delimiters, identifiers, and literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// The `let` keyword for variable bindings.
    Let,
    /// The `if` keyword for conditional expressions.
    If,
    /// The `else` keyword for alternative branches.
    Else,
    /// The `true` boolean literal.
    True,
    /// The `false` boolean literal.
    False,
    /// The `fn` keyword for function definitions.
    Function,
    /// The `return` keyword for early returns.
    Return,

    /// A user-defined identifier (variable, function name, etc.).
    Ident,
    /// A string literal.
    String,

    /// An 8-bit signed integer literal.
    I8,
    /// A 16-bit signed integer literal.
    I16,
    /// A 32-bit signed integer literal.
    I32,
    /// A 64-bit signed integer literal.
    I64,
    /// A 128-bit signed integer literal.
    I128,
    /// A pointer-sized signed integer literal.
    Isize,

    /// An 8-bit unsigned integer literal.
    U8,
    /// A 16-bit unsigned integer literal.
    U16,
    /// A 32-bit unsigned integer literal.
    U32,
    /// A 64-bit unsigned integer literal.
    U64,
    /// A 128-bit unsigned integer literal.
    U128,
    /// A pointer-sized unsigned integer literal.
    Usize,

    /// A 32-bit floating-point number.
    F32,
    /// A 64-bit floating-point number.
    F64,

    /// The assignment operator `=`.
    Assign,
    /// The addition operator `+`.
    Plus,
    /// The subtraction operator `-`.
    Minus,
    /// The logical NOT operator `!`.
    Bang,
    /// The multiplication operator `*`.
    Asterisk,
    /// The division operator `/`.
    Slash,

    /// The less-than comparison operator `<`.
    Less,
    /// The greater-than comparison operator `>`.
    Greater,

    /// The equality comparison operator `==`.
    Equal,
    /// The inequality comparison operator `!=`.
    NotEqual,

    /// The comma delimiter `,`.
    Comma,
    /// The semicolon delimiter `;`.
    Semicolon,
    /// The colon delimiter `:`.
    Colon,
    /// The left parenthesis `(`.
    LParen,
    /// The right parenthesis `)`.
    RParen,
    /// The left brace `{`.
    LBrace,
    /// The right brace `}`.
    RBrace,
    /// The left bracket `[`.
    LBracket,
    /// The right bracket `]`.
    RBracket,

    /// An invalid or unrecognized token.
    Invalid,
    /// End of file marker.
    Eof,
}

impl TokenKind {
    /// Classifies an identifier string as either a keyword or a regular identifier.
    ///
    /// This method performs keyword recognition by matching the input string against
    /// all reserved keywords in the language. If no keyword matches, the identifier
    /// is classified as a regular user-defined identifier.
    ///
    /// # Parameters
    ///
    /// * `ident` - The identifier string to classify.
    ///
    /// # Returns
    ///
    /// The corresponding keyword variant if the string is a reserved word,
    /// otherwise [`TokenKind::Ident`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use maat::TokenKind;
    /// assert_eq!(TokenKind::keyword_or_ident("let"), TokenKind::Let);
    /// assert_eq!(TokenKind::keyword_or_ident("fn"), TokenKind::Function);
    /// assert_eq!(TokenKind::keyword_or_ident("myvar"), TokenKind::Ident);
    /// ```
    #[inline]
    pub fn keyword_or_ident(ident: &str) -> Self {
        match ident {
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
    /// Constructs a new token with the given kind and literal text.
    ///
    /// # Parameters
    ///
    /// * `kind` - The syntactic classification of the token.
    /// * `literal` - The raw source text that produced this token.
    ///
    /// # Returns
    ///
    /// A new [`Token`] instance.
    #[inline]
    pub const fn new(kind: TokenKind, literal: &'a str) -> Self {
        Self { kind, literal }
    }
}

impl<'a> core::fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TokenKind::Ident => write!(f, "{}", self.literal),
            _ => write!(f, "{:?}", self.kind),
        }
    }
}
