mod token;

pub use token::{Token, TokenKind};

/// A lexical analyzer for the Maat programming language.
///
/// The lexer converts raw source code into a stream of tokens through iterative
/// scanning. It handles whitespace, operators, keywords, identifiers, and numeric
/// literals while maintaining zero-copy efficiency via string slices.
///
/// # Lifetime
///
/// The `'a` lifetime parameter ties the lexer to the source string it analyzes,
/// ensuring all produced tokens reference the original source without allocation.
///
/// # Examples
///
/// ```
/// # use maat::lexer::{Lexer, TokenKind};
/// let source = "let x = 42;";
/// let mut lexer = Lexer::new(source);
///
/// assert_eq!(lexer.next_token().kind, TokenKind::Let);
/// assert_eq!(lexer.next_token().kind, TokenKind::Ident);
/// assert_eq!(lexer.next_token().kind, TokenKind::Assign);
/// assert_eq!(lexer.next_token().kind, TokenKind::Int64);
/// assert_eq!(lexer.next_token().kind, TokenKind::Semicolon);
/// assert_eq!(lexer.next_token().kind, TokenKind::Eof);
/// ```
pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source code.
    ///
    /// The lexer is initialized at the beginning of the source string and is ready
    /// to produce tokens via [`next_token`](Lexer::next_token).
    ///
    /// # Parameters
    ///
    /// * `source` - The source code to tokenize.
    ///
    /// # Returns
    ///
    /// A new [`Lexer`] instance positioned at the start of the source.
    #[inline]
    pub const fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    /// Advances the lexer and returns the next token from the source.
    ///
    /// This method consumes characters from the source stream and produces a single
    /// token. Whitespace is automatically skipped. The method handles:
    ///
    /// - Single-character operators and delimiters
    /// - Multi-character operators (`==`, `!=`)
    /// - Keywords and identifiers (with Unicode support)
    /// - Integer literals
    /// - Invalid characters
    ///
    /// When the end of the source is reached, this method returns a token with
    /// kind [`TokenKind::Eof`].
    ///
    /// # Returns
    ///
    /// The next [`Token`] in the source stream.
    pub fn next_token(&mut self) -> Token<'a> {
        self.eat_whitespace();

        let start = self.pos;
        let Some(byte) = self.peek_pos() else {
            return Token::new(TokenKind::Eof, &self.source[start..self.pos]);
        };

        match byte {
            b'=' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::Equal)
                } else {
                    Token::new(TokenKind::Assign, &self.source[start..self.pos])
                }
            }
            b'!' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::NotEqual)
                } else {
                    Token::new(TokenKind::Bang, &self.source[start..self.pos])
                }
            }

            b'+' => self.yield_token(start, TokenKind::Plus),
            b'-' => self.yield_token(start, TokenKind::Minus),
            b'*' => self.yield_token(start, TokenKind::Asterisk),
            b'/' => self.yield_token(start, TokenKind::Slash),

            b'<' => self.yield_token(start, TokenKind::Less),
            b'>' => self.yield_token(start, TokenKind::Greater),

            b',' => self.yield_token(start, TokenKind::Comma),
            b';' => self.yield_token(start, TokenKind::Semicolon),
            b':' => self.yield_token(start, TokenKind::Colon),
            b'(' => self.yield_token(start, TokenKind::LParen),
            b')' => self.yield_token(start, TokenKind::RParen),
            b'{' => self.yield_token(start, TokenKind::LBrace),
            b'}' => self.yield_token(start, TokenKind::RBrace),
            b'[' => self.yield_token(start, TokenKind::LBracket),
            b']' => self.yield_token(start, TokenKind::RBracket),
            b'"' => self.yield_string(),

            b if b.is_ascii_alphabetic() || b == b'_' => self.yield_ident(start),
            b if b >= 0x80 => self.yield_ident(start),

            b if b.is_ascii_digit() => self.yield_number(start),

            _ => self.yield_token(start, TokenKind::Invalid),
        }
    }

    #[inline]
    fn eat_whitespace(&mut self) {
        while let Some(b) = self.peek_pos() {
            if b.is_ascii_whitespace() {
                self.advance_pos();
            } else {
                break;
            }
        }
    }

    #[inline]
    fn advance_pos(&mut self) -> Option<u8> {
        let b = self.peek_pos()?;
        self.pos += 1;
        Some(b)
    }

    #[inline]
    fn peek_pos(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    #[inline]
    fn yield_token(&mut self, start: usize, kind: TokenKind) -> Token<'a> {
        self.advance_pos();
        Token::new(kind, &self.source[start..self.pos])
    }

    fn yield_string(&mut self) -> Token<'a> {
        self.advance_pos(); // skip opening quote
        let start = self.pos;

        while let Some(b) = self.peek_pos() {
            if b == b'"' {
                break;
            }
            self.advance_pos();
        }

        let end = self.pos;
        self.advance_pos(); // skip closing quote
        Token::new(TokenKind::String, &self.source[start..end])
    }

    fn yield_ident(&mut self, start: usize) -> Token<'a> {
        self.advance_char();

        loop {
            match self.peek_pos() {
                Some(b) if b.is_ascii_alphanumeric() || b == b'_' => {
                    self.advance_pos();
                }
                Some(b) if b >= 0x80 => {
                    let Some(c) = self.peek_char() else { break };
                    if Self::is_ident_continue(c) {
                        self.advance_char();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        let literal = &self.source[start..self.pos];
        let kind = TokenKind::keyword_or_ident(literal);
        Token::new(kind, literal)
    }

    #[inline]
    fn advance_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    #[inline]
    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn is_ident_continue(c: char) -> bool {
        unicode_xid::UnicodeXID::is_xid_continue(c)
    }

    fn yield_number(&mut self, start: usize) -> Token<'a> {
        let mut is_float = false;
        let mut has_radix = false;

        // Check for radix prefix (0b, 0o, 0x)
        if self.peek_pos() == Some(b'0')
            && let Some(radix) = self.source.as_bytes().get(self.pos + 1)
        {
            match radix {
                b'b' | b'B' => {
                    has_radix = true;
                    self.advance_pos(); // Skip '0'
                    self.advance_pos(); // Skip 'b'
                    self.read_binary_digits();
                }
                b'o' | b'O' => {
                    has_radix = true;
                    self.advance_pos();
                    self.advance_pos();
                    self.read_octal_digits();
                }
                b'x' | b'X' => {
                    has_radix = true;
                    self.advance_pos();
                    self.advance_pos();
                    self.read_hex_digits();
                }
                _ => {}
            }
        }

        if has_radix {
            if let Some(suffix_type) = self.try_read_suffix() {
                is_float = suffix_type.is_float();
            }
            let kind = if is_float {
                TokenKind::Float64
            } else {
                TokenKind::Int64
            };
            return Token::new(kind, &self.source[start..self.pos]);
        }

        self.read_decimal_digits();
        if self.has_fractional_part() {
            is_float = true;
        }
        if self.has_exponent() {
            is_float = true;
        }

        if let Some(suffix_type) = self.try_read_suffix() {
            is_float = suffix_type.is_float();
        }

        let kind = if is_float {
            TokenKind::Float64
        } else {
            TokenKind::Int64
        };
        Token::new(kind, &self.source[start..self.pos])
    }

    #[inline]
    fn read_decimal_digits(&mut self) {
        self.advance_pos();
        while let Some(b) = self.peek_pos() {
            if b.is_ascii_digit() {
                self.advance_pos();
            } else {
                break;
            }
        }
    }

    #[inline]
    fn read_binary_digits(&mut self) {
        while let Some(b) = self.peek_pos() {
            if b == b'0' || b == b'1' {
                self.advance_pos();
            } else {
                break;
            }
        }
    }

    #[inline]
    fn read_octal_digits(&mut self) {
        while let Some(b) = self.peek_pos() {
            if (b'0'..=b'7').contains(&b) {
                self.advance_pos();
            } else {
                break;
            }
        }
    }

    #[inline]
    fn read_hex_digits(&mut self) {
        while let Some(b) = self.peek_pos() {
            if b.is_ascii_hexdigit() {
                self.advance_pos();
            } else {
                break;
            }
        }
    }

    /// Tries to read a type suffix from the current position.
    ///
    /// Recognizes integer suffixes (i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize)
    /// and float suffixes (f32, f64). Supports both `123i64` and `123_i64` styles (with or without
    /// underscore prefix).
    ///
    /// # Returns
    ///
    /// - `Some(Suffix::Int)` if an integer suffix was found
    /// - `Some(Suffix::Float)` if a float suffix was found
    /// - `None` if no valid suffix was found (position is restored)
    fn try_read_suffix(&mut self) -> Option<Suffix> {
        let start = self.pos;

        if self.peek_pos() == Some(b'_') {
            self.advance_pos();
        }

        let bytes = self.source.as_bytes();
        let remaining = &bytes[self.pos..];

        if let Some(len) = Self::match_int_suffix(remaining) {
            self.pos += len;
            return Some(Suffix::Int);
        }
        if let Some(len) = Self::match_float_suffix(remaining) {
            self.pos += len;
            return Some(Suffix::Float);
        }

        // No valid suffix found, restore position
        self.pos = start;
        None
    }

    /// Matches integer suffixes. Returns length if matched.
    /// Supports: i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize.
    ///
    /// Boundary rule: Only match if suffix is followed by non-alphanumeric character.
    /// This prevents matching partial suffixes like "i64" in "42i641", such as in Rust
    /// where invalid suffixes like "i641" cause errors.
    #[inline]
    fn match_int_suffix(bytes: &[u8]) -> Option<usize> {
        #[inline]
        fn is_ident_continue(b: &u8) -> bool {
            b.is_ascii_alphanumeric() || *b == b'_'
        }

        match bytes.first()? {
            b'i' | b'u' => {
                // Check single-digit suffixes: i8/u8
                if bytes.get(1..2) == Some(b"8") && !bytes.get(2).is_some_and(is_ident_continue) {
                    return Some(2);
                }

                // Check two-digit suffixes: i16/u16, i32/u32, i64/u64
                if let Some(suffix) = bytes.get(1..3)
                    && (suffix == b"16" || suffix == b"32" || suffix == b"64")
                    && !bytes.get(3).is_some_and(is_ident_continue)
                {
                    return Some(3);
                }

                // Check three-digit suffixes: i128/u128
                if bytes.get(1..4) == Some(b"128") && !bytes.get(4).is_some_and(is_ident_continue) {
                    return Some(4);
                }

                // Check size suffixes: isize/usize
                if bytes.get(1..5) == Some(b"size") && !bytes.get(5).is_some_and(is_ident_continue)
                {
                    return Some(5);
                }
                None
            }
            _ => None,
        }
    }

    /// Matches float suffixes. Returns length if matched.
    /// Supports: f32, f64.
    ///
    /// Boundary rule: Only match if suffix is followed by non-alphanumeric character.
    /// This prevents matching partial suffixes like "f64" in "42f641", such as in Rust
    /// where invalid suffixes like "f641" cause errors.
    #[inline]
    fn match_float_suffix(bytes: &[u8]) -> Option<usize> {
        #[inline]
        fn is_ident_continue(b: &u8) -> bool {
            b.is_ascii_alphanumeric() || *b == b'_'
        }

        match bytes.first()? {
            b'f' => {
                if let Some(suffix) = bytes.get(1..3)
                    && (suffix == b"32" || suffix == b"64")
                    && !bytes.get(3).is_some_and(is_ident_continue)
                {
                    return Some(3);
                }
                None
            }
            _ => None,
        }
    }

    /// Tries to read fractional part (e.g., .123).
    /// Returns true if fractional part found.
    fn has_fractional_part(&mut self) -> bool {
        if self.peek_pos() != Some(b'.') {
            return false;
        }

        if !self
            .source
            .as_bytes()
            .get(self.pos + 1)
            .is_some_and(|b| b.is_ascii_digit())
        {
            return false;
        }

        self.advance_pos();
        while let Some(b) = self.peek_pos() {
            if b.is_ascii_digit() {
                self.advance_pos();
            } else {
                break;
            }
        }
        true
    }

    /// Tries to read exponent (e.g., e10, E-3, e+5).
    /// Returns true if exponent found.
    fn has_exponent(&mut self) -> bool {
        let Some(b) = self.peek_pos() else {
            return false;
        };
        if b != b'e' && b != b'E' {
            return false;
        }
        self.advance_pos();

        if let Some(sign) = self.peek_pos()
            && (sign == b'+' || sign == b'-')
        {
            self.advance_pos();
        }

        while let Some(b) = self.peek_pos() {
            if b.is_ascii_digit() {
                self.advance_pos();
            } else {
                break;
            }
        }
        true
    }
}

/// Type of numeric suffix found during lexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suffix {
    /// Integer type suffix
    /// (i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize).
    Int,
    /// Floating-point type suffix (f32, f64).
    Float,
}

impl Suffix {
    /// Returns true if this suffix represents a floating-point type.
    #[inline]
    const fn is_float(self) -> bool {
        matches!(self, Self::Float)
    }
}
