mod token;

pub use token::{Token, TokenKind};

pub struct Lexer<'a> {
    source: &'a str,
    // Current position of token under examination
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

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
            b if b >= 0x80 => self.yield_ident(start), // Non-ASCII: potential Unicode

            b if b.is_ascii_digit() => self.yield_number(start),

            _ => self.yield_token(start, TokenKind::Invalid),
        }
    }

    fn eat_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    fn yield_token(&mut self, start: usize, kind: TokenKind) -> Token<'a> {
        self.advance();
        Token::new(kind, &self.source[start..self.pos])
    }

    // Only call this when you've determined you're in a context
    // that allows Unicode (identifiers, strings, comments)
    fn advance_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    // Handle similarly to `self.advance_char`
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

        Token::new(TokenKind::Int, &self.source[start..self.pos])
    }
}
