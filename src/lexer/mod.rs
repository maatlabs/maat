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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_token() {
        let source = r#"let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
	return true;
} else {
	return false;
}

10 == 10;
10 != 9;
"#;

        let expected = [
            (TokenKind::Let, "let"),
            (TokenKind::Ident, "five"),
            (TokenKind::Assign, "="),
            (TokenKind::Int64, "5"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Let, "let"),
            (TokenKind::Ident, "ten"),
            (TokenKind::Assign, "="),
            (TokenKind::Int64, "10"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Let, "let"),
            (TokenKind::Ident, "add"),
            (TokenKind::Assign, "="),
            (TokenKind::Function, "fn"),
            (TokenKind::LParen, "("),
            (TokenKind::Ident, "x"),
            (TokenKind::Comma, ","),
            (TokenKind::Ident, "y"),
            (TokenKind::RParen, ")"),
            (TokenKind::LBrace, "{"),
            (TokenKind::Ident, "x"),
            (TokenKind::Plus, "+"),
            (TokenKind::Ident, "y"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::RBrace, "}"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Let, "let"),
            (TokenKind::Ident, "result"),
            (TokenKind::Assign, "="),
            (TokenKind::Ident, "add"),
            (TokenKind::LParen, "("),
            (TokenKind::Ident, "five"),
            (TokenKind::Comma, ","),
            (TokenKind::Ident, "ten"),
            (TokenKind::RParen, ")"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Bang, "!"),
            (TokenKind::Minus, "-"),
            (TokenKind::Slash, "/"),
            (TokenKind::Asterisk, "*"),
            (TokenKind::Int64, "5"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Int64, "5"),
            (TokenKind::Less, "<"),
            (TokenKind::Int64, "10"),
            (TokenKind::Greater, ">"),
            (TokenKind::Int64, "5"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::If, "if"),
            (TokenKind::LParen, "("),
            (TokenKind::Int64, "5"),
            (TokenKind::Less, "<"),
            (TokenKind::Int64, "10"),
            (TokenKind::RParen, ")"),
            (TokenKind::LBrace, "{"),
            (TokenKind::Return, "return"),
            (TokenKind::True, "true"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::RBrace, "}"),
            (TokenKind::Else, "else"),
            (TokenKind::LBrace, "{"),
            (TokenKind::Return, "return"),
            (TokenKind::False, "false"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::RBrace, "}"),
            (TokenKind::Int64, "10"),
            (TokenKind::Equal, "=="),
            (TokenKind::Int64, "10"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Int64, "10"),
            (TokenKind::NotEqual, "!="),
            (TokenKind::Int64, "9"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (i, (kind, literal)) in expected.iter().enumerate() {
            let token = lexer.next_token();

            assert_eq!(
                token.kind, *kind,
                "tests[{}]: token kind wrong. expected={:?}, got={:?}",
                i, kind, token.kind
            );
            assert_eq!(
                token.literal, *literal,
                "tests[{}]: literal wrong. expected={:?}, got={:?}",
                i, literal, token.literal
            );
        }
    }

    #[test]
    fn single_character_tokens() {
        let source = "=+(){},;";
        let expected = [
            (TokenKind::Assign, "="),
            (TokenKind::Plus, "+"),
            (TokenKind::LParen, "("),
            (TokenKind::RParen, ")"),
            (TokenKind::LBrace, "{"),
            (TokenKind::RBrace, "}"),
            (TokenKind::Comma, ","),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn two_character_tokens() {
        let source = "== !=";
        let expected = [
            (TokenKind::Equal, "=="),
            (TokenKind::NotEqual, "!="),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn keywords() {
        let source = "let fn if else return true false";
        let expected = [
            (TokenKind::Let, "let"),
            (TokenKind::Function, "fn"),
            (TokenKind::If, "if"),
            (TokenKind::Else, "else"),
            (TokenKind::Return, "return"),
            (TokenKind::True, "true"),
            (TokenKind::False, "false"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn identifiers() {
        let source = "foo bar _baz qux123";
        let expected = [
            (TokenKind::Ident, "foo"),
            (TokenKind::Ident, "bar"),
            (TokenKind::Ident, "_baz"),
            (TokenKind::Ident, "qux123"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn int64() {
        let source = "0 1 42 1234567890";
        let expected = [
            (TokenKind::Int64, "0"),
            (TokenKind::Int64, "1"),
            (TokenKind::Int64, "42"),
            (TokenKind::Int64, "1234567890"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn operators() {
        let source = "+ - * / < > ! = == !=";
        let expected = [
            (TokenKind::Plus, "+"),
            (TokenKind::Minus, "-"),
            (TokenKind::Asterisk, "*"),
            (TokenKind::Slash, "/"),
            (TokenKind::Less, "<"),
            (TokenKind::Greater, ">"),
            (TokenKind::Bang, "!"),
            (TokenKind::Assign, "="),
            (TokenKind::Equal, "=="),
            (TokenKind::NotEqual, "!="),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn whitespace() {
        let source = "  let   \t\n  x  \r\n =   \t 5  ";
        let expected = [
            (TokenKind::Let, "let"),
            (TokenKind::Ident, "x"),
            (TokenKind::Assign, "="),
            (TokenKind::Int64, "5"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn empty_input() {
        let source = "";
        let mut lexer = Lexer::new(source);
        let token = lexer.next_token();
        assert_eq!(token.kind, TokenKind::Eof);
        assert_eq!(token.literal, "");
    }

    #[test]
    fn invalid_characters() {
        let source = "@ # $";
        let expected = [
            (TokenKind::Invalid, "@"),
            (TokenKind::Invalid, "#"),
            (TokenKind::Invalid, "$"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn string_literals() {
        let source = r#""hello world" "foo bar" "" "with\nnewlines""#;
        let expected = [
            (TokenKind::String, "hello world"),
            (TokenKind::String, "foo bar"),
            (TokenKind::String, ""),
            (TokenKind::String, "with\\nnewlines"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn float64_literals() {
        let source = "3.14 0.5 123.456 0.0 999.999";
        let expected = [
            (TokenKind::Float64, "3.14"),
            (TokenKind::Float64, "0.5"),
            (TokenKind::Float64, "123.456"),
            (TokenKind::Float64, "0.0"),
            (TokenKind::Float64, "999.999"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn number_suffixes() {
        let source = "123_i64 456_f64 0_i64 999_f64";
        let expected = [
            (TokenKind::Int64, "123_i64"),
            (TokenKind::Float64, "456_f64"),
            (TokenKind::Int64, "0_i64"),
            (TokenKind::Float64, "999_f64"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn scientific_notation() {
        let source = "1e10 1.5e10 2E5 3.14E-2 1e+5 6.022e23 0e0";
        let expected = [
            (TokenKind::Float64, "1e10"),
            (TokenKind::Float64, "1.5e10"),
            (TokenKind::Float64, "2E5"),
            (TokenKind::Float64, "3.14E-2"),
            (TokenKind::Float64, "1e+5"),
            (TokenKind::Float64, "6.022e23"),
            (TokenKind::Float64, "0e0"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn mixed_numbers() {
        let source = "3.14_f64 1.5e10_f64 100_i64";
        let expected = [
            (TokenKind::Float64, "3.14_f64"),
            (TokenKind::Float64, "1.5e10_f64"),
            (TokenKind::Int64, "100_i64"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn integer_followed_by_dot_method() {
        let source = "5.abs()";
        let expected = [
            (TokenKind::Int64, "5"),
            (TokenKind::Invalid, "."),
            (TokenKind::Ident, "abs"),
            (TokenKind::LParen, "("),
            (TokenKind::RParen, ")"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn binary_literals() {
        let source = "0b1010 0B1111 0b0";
        let expected = [
            (TokenKind::Int64, "0b1010"),
            (TokenKind::Int64, "0B1111"),
            (TokenKind::Int64, "0b0"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn octal_literals() {
        let source = "0o755 0O644 0o0";
        let expected = [
            (TokenKind::Int64, "0o755"),
            (TokenKind::Int64, "0O644"),
            (TokenKind::Int64, "0o0"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn hex_literals() {
        let source = "0xff 0xFF 0x0 0xDEADBEEF 0X1a2B";
        let expected = [
            (TokenKind::Int64, "0xff"),
            (TokenKind::Int64, "0xFF"),
            (TokenKind::Int64, "0x0"),
            (TokenKind::Int64, "0xDEADBEEF"),
            (TokenKind::Int64, "0X1a2B"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn rust_style_suffixes() {
        let source = "123i64 456f64 0i64 999f64";
        let expected = [
            (TokenKind::Int64, "123i64"),
            (TokenKind::Float64, "456f64"),
            (TokenKind::Int64, "0i64"),
            (TokenKind::Float64, "999f64"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn mixed_radix_and_suffixes() {
        let source = "0b1010i64 0xFFi64 3.14f64";
        let expected = [
            (TokenKind::Int64, "0b1010i64"),
            (TokenKind::Int64, "0xFFi64"),
            (TokenKind::Float64, "3.14f64"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn arrays_and_hashes() {
        let source = r#"[1, 2, 3]; {"key": "value"}; arr[0]"#;
        let expected = [
            (TokenKind::LBracket, "["),
            (TokenKind::Int64, "1"),
            (TokenKind::Comma, ","),
            (TokenKind::Int64, "2"),
            (TokenKind::Comma, ","),
            (TokenKind::Int64, "3"),
            (TokenKind::RBracket, "]"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::LBrace, "{"),
            (TokenKind::String, "key"),
            (TokenKind::Colon, ":"),
            (TokenKind::String, "value"),
            (TokenKind::RBrace, "}"),
            (TokenKind::Semicolon, ";"),
            (TokenKind::Ident, "arr"),
            (TokenKind::LBracket, "["),
            (TokenKind::Int64, "0"),
            (TokenKind::RBracket, "]"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (i, (kind, literal)) in expected.iter().enumerate() {
            let token = lexer.next_token();
            assert_eq!(
                token.kind, *kind,
                "tests[{}]: token kind wrong. expected={:?}, got={:?}",
                i, kind, token.kind
            );
            assert_eq!(
                token.literal, *literal,
                "tests[{}]: literal wrong. expected={:?}, got={:?}",
                i, literal, token.literal
            );
        }
    }

    #[test]
    fn signed_integer_suffixes() {
        let source = "42i8 42_i8 127i16 32767i32 2147483647i64 170141183460469231731687303715884105727i128 42isize";
        let expected = [
            (TokenKind::Int64, "42i8"),
            (TokenKind::Int64, "42_i8"),
            (TokenKind::Int64, "127i16"),
            (TokenKind::Int64, "32767i32"),
            (TokenKind::Int64, "2147483647i64"),
            (
                TokenKind::Int64,
                "170141183460469231731687303715884105727i128",
            ),
            (TokenKind::Int64, "42isize"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn unsigned_integer_suffixes() {
        let source = "42u8 42_u8 255u16 65535u32 4294967295u64 340282366920938463463374607431768211455u128 42usize";
        let expected = [
            (TokenKind::Int64, "42u8"),
            (TokenKind::Int64, "42_u8"),
            (TokenKind::Int64, "255u16"),
            (TokenKind::Int64, "65535u32"),
            (TokenKind::Int64, "4294967295u64"),
            (
                TokenKind::Int64,
                "340282366920938463463374607431768211455u128",
            ),
            (TokenKind::Int64, "42usize"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn float_suffixes() {
        let source = "3.14f32 3.14_f32 2.718f64 2.718_f64 1e10f32 1.5e-5f64";
        let expected = [
            (TokenKind::Float64, "3.14f32"),
            (TokenKind::Float64, "3.14_f32"),
            (TokenKind::Float64, "2.718f64"),
            (TokenKind::Float64, "2.718_f64"),
            (TokenKind::Float64, "1e10f32"),
            (TokenKind::Float64, "1.5e-5f64"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn suffix_boundary_checking() {
        let source = "42i641 42u641 42f641 42i12 42u12 42isizes";
        let expected = [
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "i641"),
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "u641"),
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "f641"),
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "i12"),
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "u12"),
            (TokenKind::Int64, "42"),
            (TokenKind::Ident, "isizes"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind, "for literal: {}", literal);
            assert_eq!(token.literal, literal);
        }
    }

    #[test]
    fn radix_with_suffix() {
        let source = "0b1010i8 0o755u16 0xFFi32 0b11111111u8 0xDEADBEEFu64 0o777isize";
        let expected = [
            (TokenKind::Int64, "0b1010i8"),
            (TokenKind::Int64, "0o755u16"),
            (TokenKind::Int64, "0xFFi32"),
            (TokenKind::Int64, "0b11111111u8"),
            (TokenKind::Int64, "0xDEADBEEFu64"),
            (TokenKind::Int64, "0o777isize"),
            (TokenKind::Eof, ""),
        ];

        let mut lexer = Lexer::new(source);

        for (kind, literal) in expected {
            let token = lexer.next_token();
            assert_eq!(token.kind, kind);
            assert_eq!(token.literal, literal);
        }
    }
}
