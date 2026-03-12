use maat_span::Span;

/// A lexical token in Maat.
///
/// Tokens represent the smallest meaningful units of source code, such as
/// keywords, operators, identifiers, and literals. Each token carries both its
/// syntactic classification ([`TokenKind`]), the exact source text it represents,
/// and its position in the source for error reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'a> {
    /// The syntactic classification of this token.
    pub kind: TokenKind,
    /// The raw source text that produced this token.
    pub literal: &'a str,
    /// The source position of this token.
    pub span: Span,
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
    Fn,
    /// The `return` keyword for early returns.
    Return,
    /// The `macro` keyword for macro definitions.
    Macro,
    /// The `as` keyword for type cast expressions.
    As,
    /// The `loop` keyword for infinite loops.
    Loop,
    /// The `while` keyword for conditional loops.
    While,
    /// The `for` keyword for iterator loops.
    For,
    /// The `in` keyword for iterator binding.
    In,
    /// The `break` keyword for loop termination.
    Break,
    /// The `continue` keyword for loop continuation.
    Continue,
    /// The `where` keyword for trait bound clauses.
    Where,
    /// The `struct` keyword for structure type declarations.
    Struct,
    /// The `enum` keyword for enumeration type declarations.
    Enum,
    /// The `match` keyword for pattern-matching expressions.
    Match,
    /// The `impl` keyword for inherent and trait implementation blocks.
    Impl,
    /// The `trait` keyword for trait declarations.
    Trait,
    /// The `self` value keyword for method receivers.
    SelfValue,
    /// The `Self` type keyword for the implementing type in `impl` and `trait` blocks.
    SelfType,
    /// The `mod` keyword for module declarations.
    Mod,
    /// The `use` keyword for importing items from other modules.
    Use,
    /// The `pub` keyword for visibility modifiers.
    Pub,

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
    /// The modulo (remainder) operator `%`.
    Percent,

    /// The compound addition assignment operator `+=`.
    AddAssign,
    /// The compound subtraction assignment operator `-=`.
    SubAssign,
    /// The compound multiplication assignment operator `*=`.
    MulAssign,
    /// The compound division assignment operator `/=`.
    DivAssign,
    /// The compound modulo assignment operator `%=`.
    RemAssign,

    /// The less-than comparison operator or
    /// left angle bracket `<`.
    Less,
    /// The greater-than comparison operator or
    /// right angle bracket `>`.
    Greater,
    /// The less-than-or-equal comparison operator `<=`.
    LessEqual,
    /// The greater-than-or-equal comparison operator `>=`.
    GreaterEqual,

    /// The equality comparison operator `==`.
    Equal,
    /// The inequality comparison operator `!=`.
    NotEqual,
    /// The logical AND operator `&&`.
    And,
    /// The logical OR operator `||`.
    Or,

    /// The bitwise AND operator `&`.
    Ampersand,
    /// The bitwise OR operator `|`.
    Pipe,
    /// The bitwise XOR operator `^`.
    Caret,
    /// The left shift operator `<<`.
    ShiftLeft,
    /// The right shift operator `>>`.
    ShiftRight,

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
    /// The return type arrow `->`.
    Arrow,
    /// The fat arrow `=>` used in match arms.
    FatArrow,
    /// The path separator `::` for qualified paths.
    PathSep,
    /// The dot `.` for field access and method calls.
    Dot,

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
    /// # use maat_lexer::TokenKind;
    /// assert_eq!(TokenKind::keyword_or_ident("let"), TokenKind::Let);
    /// assert_eq!(TokenKind::keyword_or_ident("fn"), TokenKind::Fn);
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
            "fn" => Self::Fn,
            "return" => Self::Return,
            "macro" => Self::Macro,
            "as" => Self::As,
            "loop" => Self::Loop,
            "while" => Self::While,
            "for" => Self::For,
            "in" => Self::In,
            "break" => Self::Break,
            "continue" => Self::Continue,
            "where" => Self::Where,
            "struct" => Self::Struct,
            "enum" => Self::Enum,
            "match" => Self::Match,
            "impl" => Self::Impl,
            "trait" => Self::Trait,
            "self" => Self::SelfValue,
            "Self" => Self::SelfType,
            "mod" => Self::Mod,
            "use" => Self::Use,
            "pub" => Self::Pub,
            _ => Self::Ident,
        }
    }
}

impl<'a> Token<'a> {
    /// Constructs a new token with the given kind, literal text, and span.
    ///
    /// # Parameters
    ///
    /// * `kind` - The syntactic classification of the token.
    /// * `literal` - The raw source text that produced this token.
    /// * `span` - The source position of this token.
    ///
    /// # Returns
    ///
    /// A new [`Token`] instance.
    #[inline]
    pub const fn new(kind: TokenKind, literal: &'a str, span: Span) -> Self {
        Self {
            kind,
            literal,
            span,
        }
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
