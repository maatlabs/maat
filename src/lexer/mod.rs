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
/// assert_eq!(lexer.next_token().kind, TokenKind::Int);
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
        let Some(byte) = self.peek() else {
            return Token::new(TokenKind::Eof, &self.source[start..self.pos]);
        };

        match byte {
            b'=' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.yield_token(start, TokenKind::Equal)
                } else {
                    Token::new(TokenKind::Assign, &self.source[start..self.pos])
                }
            }
            b'!' => {
                self.advance();
                if self.peek() == Some(b'=') {
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
            b'(' => self.yield_token(start, TokenKind::LParen),
            b')' => self.yield_token(start, TokenKind::RParen),
            b'{' => self.yield_token(start, TokenKind::LBrace),
            b'}' => self.yield_token(start, TokenKind::RBrace),

            b if b.is_ascii_alphabetic() || b == b'_' => self.yield_ident(start),
            b if b >= 0x80 => self.yield_ident(start),

            b if b.is_ascii_digit() => self.yield_number(start),

            _ => self.yield_token(start, TokenKind::Invalid),
        }
    }

    #[inline]
    fn eat_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    #[inline]
    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    #[inline]
    fn yield_token(&mut self, start: usize, kind: TokenKind) -> Token<'a> {
        self.advance();
        Token::new(kind, &self.source[start..self.pos])
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

    fn yield_ident(&mut self, start: usize) -> Token<'a> {
        self.advance_char();

        loop {
            match self.peek() {
                Some(b) if b.is_ascii_alphanumeric() || b == b'_' => {
                    self.advance();
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

    fn is_ident_continue(c: char) -> bool {
        unicode_xid::UnicodeXID::is_xid_continue(c)
    }

    fn yield_number(&mut self, start: usize) -> Token<'a> {
        self.advance();

        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        Token::new(TokenKind::Int64, &self.source[start..self.pos])
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
}
