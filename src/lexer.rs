mod num;
mod span;
mod token;

use num::Suffix;
pub use span::Span;
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
/// # use maat::{Lexer, TokenKind};
/// let source = "let x = 42;";
/// let mut lexer = Lexer::new(source);
///
/// assert_eq!(lexer.next_token().kind, TokenKind::Let);
/// assert_eq!(lexer.next_token().kind, TokenKind::Ident);
/// assert_eq!(lexer.next_token().kind, TokenKind::Assign);
/// assert_eq!(lexer.next_token().kind, TokenKind::I64);
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
    /// - Multi-character operators (`==`, `!=`, `<=`, `>=`)
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
            let end = self.pos;
            return Token::new(
                TokenKind::Eof,
                &self.source[start..end],
                Span::new(start, end),
            );
        };

        match byte {
            b'=' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::Equal)
                } else {
                    let end = self.pos;
                    Token::new(
                        TokenKind::Assign,
                        &self.source[start..end],
                        Span::new(start, end),
                    )
                }
            }
            b'!' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::NotEqual)
                } else {
                    let end = self.pos;
                    Token::new(
                        TokenKind::Bang,
                        &self.source[start..end],
                        Span::new(start, end),
                    )
                }
            }

            b'+' => self.yield_token(start, TokenKind::Plus),
            b'-' => self.yield_token(start, TokenKind::Minus),
            b'*' => self.yield_token(start, TokenKind::Asterisk),
            b'/' => self.yield_token(start, TokenKind::Slash),

            b'<' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::LessEqual)
                } else {
                    let end = self.pos;
                    Token::new(
                        TokenKind::Less,
                        &self.source[start..end],
                        Span::new(start, end),
                    )
                }
            }
            b'>' => {
                self.advance_pos();
                if self.peek_pos() == Some(b'=') {
                    self.yield_token(start, TokenKind::GreaterEqual)
                } else {
                    let end = self.pos;
                    Token::new(
                        TokenKind::Greater,
                        &self.source[start..end],
                        Span::new(start, end),
                    )
                }
            }

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
        let end = self.pos;
        Token::new(kind, &self.source[start..end], Span::new(start, end))
    }

    fn yield_string(&mut self) -> Token<'a> {
        let span_start = self.pos;
        self.advance_pos(); // skip opening quote
        let start = self.pos;

        while let Some(b) = self.peek_pos() {
            if b == b'"' {
                break;
            }
            if b == b'\\' {
                self.advance_pos(); // skip backslash
                if self.peek_pos().is_some() {
                    self.advance_pos(); // skip escaped character
                }
            } else {
                self.advance_pos();
            }
        }

        let end = self.pos;
        self.advance_pos(); // skip closing quote
        let span_end = self.pos;
        Token::new(
            TokenKind::String,
            &self.source[start..end],
            Span::new(span_start, span_end),
        )
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

        let end = self.pos;
        let literal = &self.source[start..end];
        let kind = TokenKind::keyword_or_ident(literal);
        Token::new(kind, literal, Span::new(start, end))
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

        // Check for radix prefix (0b, 0o, 0x)
        if self.peek_pos() == Some(b'0')
            && let Some(radix) = self.source.as_bytes().get(self.pos + 1)
        {
            match radix {
                b'b' | b'B' => {
                    self.advance_pos(); // Skip '0'
                    self.advance_pos(); // Skip 'b'
                    self.read_binary_digits();
                    let kind = self
                        .try_read_suffix()
                        .map(|s| s.to_token_kind())
                        .unwrap_or(TokenKind::I64);
                    let end = self.pos;
                    return Token::new(kind, &self.source[start..end], Span::new(start, end));
                }
                b'o' | b'O' => {
                    self.advance_pos();
                    self.advance_pos();
                    self.read_octal_digits();
                    let kind = self
                        .try_read_suffix()
                        .map(|s| s.to_token_kind())
                        .unwrap_or(TokenKind::I64);
                    let end = self.pos;
                    return Token::new(kind, &self.source[start..end], Span::new(start, end));
                }
                b'x' | b'X' => {
                    self.advance_pos();
                    self.advance_pos();
                    self.read_hex_digits();
                    let kind = self
                        .try_read_suffix()
                        .map(|s| s.to_token_kind())
                        .unwrap_or(TokenKind::I64);
                    let end = self.pos;
                    return Token::new(kind, &self.source[start..end], Span::new(start, end));
                }
                _ => {}
            }
        }

        self.read_decimal_digits();
        if self.has_fractional_part() {
            is_float = true;
        }
        if self.has_exponent() {
            is_float = true;
        }

        let kind = if let Some(suf) = self.try_read_suffix() {
            suf.to_token_kind()
        } else if is_float {
            TokenKind::F64
        } else {
            TokenKind::I64
        };

        let end = self.pos;
        Token::new(kind, &self.source[start..end], Span::new(start, end))
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
    fn try_read_suffix(&mut self) -> Option<Suffix> {
        let start = self.pos;

        if self.peek_pos() == Some(b'_') {
            self.advance_pos();
        }

        let bytes = self.source.as_bytes();
        let remaining = &bytes[self.pos..];

        if let Some((suffix, len)) = num::match_int_suffix(remaining) {
            self.pos += len;
            return Some(suffix);
        }
        if let Some((suffix, len)) = num::match_float_suffix(remaining) {
            self.pos += len;
            return Some(suffix);
        }

        // No valid suffix found, restore position
        self.pos = start;
        None
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
