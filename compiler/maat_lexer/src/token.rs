//! Implements [`Token`] and [`TokenKind`].

use maat_span::Span;

/// Language keywords recognized by the lexer.
///
/// This is the canonical list of keywords that the lexer produces as
/// dedicated [`TokenKind`] variants (rather than [`TokenKind::Ident`]).
/// Sorted lexicographically for binary search in completion engines.
pub const KEYWORDS: &[&str] = &[
    "Self", "as", "break", "continue", "else", "enum", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "macro", "match", "mod", "mut", "pub", "return", "self", "struct", "trait",
    "true", "use", "where", "while",
];

/// A lexical token in Maat.
///
/// Tokens represent the smallest meaningful units of source code, such as
/// keywords, operators, identifiers, and literals. Each token carries both its
/// syntactic classification ([`TokenKind`]), the exact source text it represents,
/// and its position in the source for error reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'src> {
    /// The syntactic classification of this token.
    pub kind: TokenKind,
    /// The raw source text that produced this token.
    pub literal: &'src str,
    /// The source position of this token.
    pub span: Span,
}

impl<'src> Token<'src> {
    /// Constructs a new token with the given token kind, the raw text
    /// from the source code, and its span.
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
    pub const fn new(kind: TokenKind, literal: &'src str, span: Span) -> Self {
        Self {
            kind,
            literal,
            span,
        }
    }
}

impl<'src> core::fmt::Display for Token<'src> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TokenKind::Ident => write!(f, "{}", self.literal),
            _ => write!(f, "{:?}", self.kind),
        }
    }
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
    /// The `else` keyword for alternative branches in conditionals.
    Else,
    /// The `true` keyword/boolean literal.
    True,
    /// The `false` keyword/boolean literal.
    False,
    /// The `fn` keyword for function definitions and closures.
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
    /// The `for` keyword for iterator-based loops.
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
    /// The `use` keyword for importing items from other modules/crates.
    Use,
    /// The `pub` keyword for marking an item as public/visible.
    Pub,
    /// The `mut` keyword for mutable bindings.
    Mut,

    /// A user-defined identifier (variable, function name, etc.).
    Ident,
    /// A lifetime-style label for loops (`'outer`, `'inner`).
    Label,
    /// A string literal.
    String,
    /// A character literal (`'a'`, `'\n'`, `'\u{1F600}'`).
    Char,

    /// An unsuffixed integer literal whose concrete type is determined by inference.
    Int,
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

    /// A field-element literal over the Goldilocks base field (suffix `_fe`).
    Fe,

    /// The assignment operator `=`.
    Assign,
    /// The addition/concat operator `+`.
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
    /// The range operator `..` for half-open ranges.
    DotDot,
    /// The inclusive range operator `..=` for closed ranges.
    DotDotEqual,
    /// The try operator `?` for error propagation.
    Question,
    /// The hash symbol `#` for attribute annotations.
    Hash,

    /// A documentation comment (`///`).
    DocComment,

    /// An invalid or unrecognized token.
    Invalid,
    /// End of file marker.
    Eof,
}
