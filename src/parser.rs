//! Recursive descent parser using Pratt parsing for operator precedence.
//!
//! # Example
//!
//! ```
//! use maat::{Lexer, Parser};
//!
//! let input = "let x = 5 + 10;";
//! let lexer = Lexer::new(input);
//! let mut parser = Parser::new(lexer);
//! let program = parser.parse_program();
//!
//! assert_eq!(parser.errors().len(), 0);
//! assert_eq!(program.statements.len(), 1);
//! ```

pub mod ast;
mod prec;

use ast::*;
use prec::{LOWEST, PREFIX, Precedence};

use crate::{Lexer, Token, TokenKind};

/// A recursive descent parser that builds an AST from a token stream.
///
/// The parser maintains two-token lookahead (`current` and `peek`) to enable
/// predictive parsing. Errors encountered during parsing are collected
/// rather than immediately halting execution, allowing multiple errors to be
/// reported at once.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token<'a>,
    peek: Token<'a>,
    errors: Vec<String>,
}

impl<'a> Parser<'a> {
    /// Creates a new parser from a lexer.
    ///
    /// We read two tokens from the lexer to initialize
    /// both `current` and `peek` positions, enabling two-token lookahead.
    ///
    /// # Example
    ///
    /// ```
    /// use maat::{Lexer, Parser};
    ///
    /// let lexer = Lexer::new("let x = 42;");
    /// let parser = Parser::new(lexer);
    /// ```
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        Self {
            current: lexer.next_token(),
            peek: lexer.next_token(),
            lexer,
            errors: vec![],
        }
    }

    /// Returns a reference to the errors encountered during parsing.
    ///
    /// # Example
    ///
    /// ```
    /// use maat::{Lexer, Parser};
    ///
    /// let lexer = Lexer::new("let = 5;");
    /// let mut parser = Parser::new(lexer);
    /// let _program = parser.parse_program();
    ///
    /// assert!(!parser.errors().is_empty());
    /// ```
    pub fn errors(&self) -> &Vec<String> {
        &self.errors
    }

    /// Parses the input source code into a complete program AST.
    ///
    /// This method consumes tokens until EOF is reached, attempting to parse
    /// each top-level statement. Parsing errors are collected in the parser's
    /// error vector and can be retrieved via [`Parser::errors`].
    ///
    /// # Example
    ///
    /// ```
    /// use maat::{Lexer, Parser};
    ///
    /// let input = r#"
    ///     let x = 5;
    ///     let y = 10;
    ///     return x + y;
    /// "#;
    /// let lexer = Lexer::new(input);
    /// let mut parser = Parser::new(lexer);
    /// let program = parser.parse_program();
    ///
    /// assert_eq!(parser.errors().len(), 0);
    /// assert_eq!(program.statements.len(), 3);
    /// ```
    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };
        while !self.cur_token_is(TokenKind::Eof) {
            if let Some(stmt) = self.parse_statement() {
                program.statements.push(stmt);
            }
            self.next_token();
        }
        program
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current.kind {
            TokenKind::Let => self.parse_let_statement().map(Statement::Let),
            TokenKind::Return => self.parse_return_statement().map(Statement::Return),
            _ => self.parse_expression_statement().map(Statement::Expression),
        }
    }

    fn parse_let_statement(&mut self) -> Option<LetStatement> {
        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let ident = self.current.literal.to_string();
        if !self.expect_peek(TokenKind::Assign) {
            return None;
        }
        self.next_token();
        let value = self.parse_expression(LOWEST)?;
        if self.peek_token_is(TokenKind::Semicolon) {
            self.next_token();
        }
        Some(LetStatement { ident, value })
    }

    fn parse_return_statement(&mut self) -> Option<ReturnStatement> {
        self.next_token();
        let value = self.parse_expression(LOWEST)?;
        if self.peek_token_is(TokenKind::Semicolon) {
            self.next_token();
        }
        Some(ReturnStatement { value })
    }

    fn parse_expression_statement(&mut self) -> Option<ExpressionStatement> {
        let value = self.parse_expression(LOWEST)?;
        if self.peek.kind == TokenKind::Semicolon {
            self.next_token();
        }
        Some(ExpressionStatement { value })
    }

    /// Parse an expression using Pratt‐parsing:
    /// 1. Parse a prefix subexpression.
    /// 2. While the next token’s precedence is higher than `prec`,
    ///    consume it and parse an infix operation.
    fn parse_expression(&mut self, prec: u8) -> Option<Expression> {
        let mut lhs = match self.current.kind {
            TokenKind::Ident => self.parse_identifier()?,

            // All numeric types
            TokenKind::I8
            | TokenKind::I16
            | TokenKind::I32
            | TokenKind::I64
            | TokenKind::I128
            | TokenKind::Isize
            | TokenKind::U8
            | TokenKind::U16
            | TokenKind::U32
            | TokenKind::U64
            | TokenKind::U128
            | TokenKind::Usize => self.parse_int()?,
            TokenKind::F32 | TokenKind::F64 => self.parse_float()?,

            TokenKind::String => self.parse_string_literal()?,
            TokenKind::Bang | TokenKind::Minus => self.parse_prefix_expression()?,
            TokenKind::True | TokenKind::False => self.parse_boolean()?,
            TokenKind::LParen => self.parse_grouped_expression()?,
            TokenKind::If => self.parse_conditional_expression()?,
            TokenKind::Function => self.parse_function()?,
            TokenKind::LBracket => self.parse_array_literal()?,
            TokenKind::LBrace => self.parse_hash_literal()?,
            kind => {
                self.prefix_parse_error(kind);
                return None;
            }
        };

        while !self.peek_token_is(TokenKind::Semicolon) && prec < self.peek_prec() {
            let kind = self.peek.kind;
            match kind {
                TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Slash
                | TokenKind::Asterisk
                | TokenKind::Equal
                | TokenKind::NotEqual
                | TokenKind::Less
                | TokenKind::Greater
                | TokenKind::LessEqual
                | TokenKind::GreaterEqual
                | TokenKind::LParen
                | TokenKind::LBracket => {
                    self.next_token();
                    lhs = match kind {
                        TokenKind::LParen => self.parse_call_expression(lhs)?,
                        TokenKind::LBracket => self.parse_index_expression(lhs)?,
                        _ => self.parse_infix_expression(lhs)?,
                    };
                }
                _ => break,
            }
        }

        Some(lhs)
    }

    fn parse_prefix_expression(&mut self) -> Option<Expression> {
        let operator = self.current.literal.to_string();
        self.next_token();
        let operand = Box::new(self.parse_expression(PREFIX)?);
        Some(Expression::Prefix(PrefixExpr { operator, operand }))
    }

    fn parse_infix_expression(&mut self, lhs: Expression) -> Option<Expression> {
        let lhs = Box::new(lhs);
        let operator = self.current.literal.to_string();
        let prec = self.current_prec();
        self.next_token();
        let rhs = Box::new(self.parse_expression(prec)?);
        Some(Expression::Infix(InfixExpr { lhs, operator, rhs }))
    }

    fn parse_identifier(&mut self) -> Option<Expression> {
        Some(Expression::Identifier(self.current.literal.to_string()))
    }

    fn parse_boolean(&mut self) -> Option<Expression> {
        Some(Expression::Boolean(self.cur_token_is(TokenKind::True)))
    }

    fn parse_int(&mut self) -> Option<Expression> {
        macro_rules! strip_int_suffixes {
            ($lit:expr) => {{
                $lit.strip_suffix("_i8")
                    .or_else(|| $lit.strip_suffix("i8"))
                    .or_else(|| $lit.strip_suffix("_i16"))
                    .or_else(|| $lit.strip_suffix("i16"))
                    .or_else(|| $lit.strip_suffix("_i32"))
                    .or_else(|| $lit.strip_suffix("i32"))
                    .or_else(|| $lit.strip_suffix("_i64"))
                    .or_else(|| $lit.strip_suffix("i64"))
                    .or_else(|| $lit.strip_suffix("_i128"))
                    .or_else(|| $lit.strip_suffix("i128"))
                    .or_else(|| $lit.strip_suffix("_isize"))
                    .or_else(|| $lit.strip_suffix("isize"))
                    .or_else(|| $lit.strip_suffix("_u8"))
                    .or_else(|| $lit.strip_suffix("u8"))
                    .or_else(|| $lit.strip_suffix("_u16"))
                    .or_else(|| $lit.strip_suffix("u16"))
                    .or_else(|| $lit.strip_suffix("_u32"))
                    .or_else(|| $lit.strip_suffix("u32"))
                    .or_else(|| $lit.strip_suffix("_u64"))
                    .or_else(|| $lit.strip_suffix("u64"))
                    .or_else(|| $lit.strip_suffix("_u128"))
                    .or_else(|| $lit.strip_suffix("u128"))
                    .or_else(|| $lit.strip_suffix("_usize"))
                    .or_else(|| $lit.strip_suffix("usize"))
                    .unwrap_or($lit)
            }};
        }

        let literal = strip_int_suffixes!(self.current.literal);
        let token_kind = self.current.kind;

        macro_rules! parse_int_type {
            ($rust_ty:ty, $variant:ident) => {{
                let (radix, value) = if let Some(bin) = literal
                    .strip_prefix("0b")
                    .or_else(|| literal.strip_prefix("0B"))
                {
                    <$rust_ty>::from_str_radix(bin, 2)
                        .ok()
                        .map(|v| (Radix::Bin, v))
                } else if let Some(oct) = literal
                    .strip_prefix("0o")
                    .or_else(|| literal.strip_prefix("0O"))
                {
                    <$rust_ty>::from_str_radix(oct, 8)
                        .ok()
                        .map(|v| (Radix::Oct, v))
                } else if let Some(hex) = literal
                    .strip_prefix("0x")
                    .or_else(|| literal.strip_prefix("0X"))
                {
                    <$rust_ty>::from_str_radix(hex, 16)
                        .ok()
                        .map(|v| (Radix::Hex, v))
                } else {
                    literal.parse::<$rust_ty>().ok().map(|v| (Radix::Dec, v))
                }
                .or_else(|| {
                    self.errors.push(format!(
                        "could not parse {:?} as {}",
                        self.current.literal,
                        stringify!($rust_ty)
                    ));
                    None
                })?;

                Expression::$variant($variant { radix, value })
            }};
        }

        let expr = match token_kind {
            TokenKind::I8 => parse_int_type!(i8, I8),
            TokenKind::I16 => parse_int_type!(i16, I16),
            TokenKind::I32 => parse_int_type!(i32, I32),
            TokenKind::I64 => parse_int_type!(i64, I64),
            TokenKind::I128 => parse_int_type!(i128, I128),
            TokenKind::Isize => parse_int_type!(isize, Isize),
            TokenKind::U8 => parse_int_type!(u8, U8),
            TokenKind::U16 => parse_int_type!(u16, U16),
            TokenKind::U32 => parse_int_type!(u32, U32),
            TokenKind::U64 => parse_int_type!(u64, U64),
            TokenKind::U128 => parse_int_type!(u128, U128),
            TokenKind::Usize => parse_int_type!(usize, Usize),
            _ => unreachable!(),
        };

        Some(expr)
    }

    fn parse_float(&mut self) -> Option<Expression> {
        macro_rules! strip_float_suffixes {
            ($lit:expr) => {{
                $lit.strip_suffix("_f32")
                    .or_else(|| $lit.strip_suffix("f32"))
                    .or_else(|| $lit.strip_suffix("_f64"))
                    .or_else(|| $lit.strip_suffix("f64"))
                    .unwrap_or($lit)
            }};
        }

        let literal = strip_float_suffixes!(self.current.literal);

        let expr = match self.current.kind {
            TokenKind::F32 => {
                let value = literal.parse::<f32>().ok().or_else(|| {
                    self.errors
                        .push(format!("could not parse {:?} as f32", self.current.literal));
                    None
                })?;

                if !value.is_finite() && !literal.contains("inf") && !literal.contains("nan") {
                    self.errors.push(format!(
                        "literal out of range for f32: {}",
                        self.current.literal
                    ));
                    return None;
                }

                Expression::F32(F32::from(value))
            }
            TokenKind::F64 => {
                let value = literal.parse::<f64>().ok().or_else(|| {
                    self.errors
                        .push(format!("could not parse {:?} as f64", self.current.literal));
                    None
                })?;

                Expression::F64(F64::from(value))
            }
            _ => unreachable!(),
        };

        Some(expr)
    }

    fn parse_string_literal(&mut self) -> Option<Expression> {
        Some(Expression::String(self.current.literal.to_owned()))
    }

    fn parse_array_literal(&mut self) -> Option<Expression> {
        Some(Expression::Array(ArrayLiteral {
            elements: self.parse_expression_list(TokenKind::RBracket)?,
        }))
    }

    fn parse_index_expression(&mut self, expr: Expression) -> Option<Expression> {
        let expr = Box::new(expr);
        self.next_token();
        let index = Box::new(self.parse_expression(LOWEST)?);

        if !self.expect_peek(TokenKind::RBracket) {
            return None;
        }
        Some(Expression::Index(IndexExpr { expr, index }))
    }

    fn parse_hash_literal(&mut self) -> Option<Expression> {
        let mut pairs = Vec::new();

        while !self.peek_token_is(TokenKind::RBrace) {
            self.next_token();
            let key = self.parse_expression(LOWEST)?;

            if !self.expect_peek(TokenKind::Colon) {
                return None;
            }

            self.next_token();
            let value = self.parse_expression(LOWEST)?;
            pairs.push((key, value));

            if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }

        Some(Expression::Hash(HashLiteral { pairs }))
    }

    fn parse_grouped_expression(&mut self) -> Option<Expression> {
        self.next_token();
        let expr = self.parse_expression(LOWEST)?;
        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }
        Some(expr)
    }

    fn parse_conditional_expression(&mut self) -> Option<Expression> {
        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        self.next_token();
        let condition = Box::new(self.parse_expression(LOWEST)?);
        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }
        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let consequence = self.parse_block_statement()?;
        let alternative = if self.peek_token_is(TokenKind::Else) {
            self.next_token();
            if !self.expect_peek(TokenKind::LBrace) {
                return None;
            }
            Some(self.parse_block_statement()?)
        } else {
            None
        };

        Some(Expression::Conditional(ConditionalExpr {
            condition,
            consequence,
            alternative,
        }))
    }

    fn parse_block_statement(&mut self) -> Option<BlockStatement> {
        let mut statements = Vec::new();
        self.next_token();

        loop {
            if self.cur_token_is(TokenKind::RBrace) || self.cur_token_is(TokenKind::Eof) {
                break;
            }
            let stmt = self.parse_statement()?;
            statements.push(stmt);
            self.next_token();
        }

        Some(BlockStatement { statements })
    }

    fn parse_function(&mut self) -> Option<Expression> {
        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_function_params()?;
        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;

        Some(Expression::Function(Function { params, body }))
    }

    fn parse_function_params(&mut self) -> Option<Vec<String>> {
        let mut identifiers = Vec::new();

        if self.peek_token_is(TokenKind::RParen) {
            self.next_token();
            return Some(identifiers);
        }

        self.next_token();
        identifiers.push(self.current.literal.to_string());

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            identifiers.push(self.current.literal.to_string());
        }

        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }

        Some(identifiers)
    }

    fn parse_call_expression(&mut self, func: Expression) -> Option<Expression> {
        Some(Expression::Call(CallExpr {
            function: Box::new(func),
            arguments: self.parse_expression_list(TokenKind::RParen)?,
        }))
    }

    fn parse_expression_list(&mut self, end: TokenKind) -> Option<Vec<Expression>> {
        let mut arguments = Vec::new();

        if self.peek_token_is(end) {
            self.next_token();
            return Some(arguments);
        }

        self.next_token();
        arguments.push(self.parse_expression(LOWEST)?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            arguments.push(self.parse_expression(LOWEST)?);
        }

        if !self.expect_peek(end) {
            return None;
        }

        Some(arguments)
    }

    /// Advance the parser: shift `peek` into `current` token,
    /// and read a fresh `peek` token from the lexer.
    fn next_token(&mut self) {
        self.current = std::mem::replace(&mut self.peek, self.lexer.next_token());
    }

    fn cur_token_is(&self, kind: TokenKind) -> bool {
        self.current.kind == kind
    }

    fn peek_token_is(&self, kind: TokenKind) -> bool {
        self.peek.kind == kind
    }

    /// If the next token is `expected`, consume it and return true.
    /// Otherwise, register the error message and return false.
    fn expect_peek(&mut self, expected: TokenKind) -> bool {
        if self.peek.kind == expected {
            self.next_token();
            return true;
        }
        self.peek_error(expected);
        false
    }

    /// Returns the precedence of the `current` token or
    /// a default [`LOWEST`] if none is registered.
    fn current_prec(&self) -> u8 {
        Precedence.get(&self.current.kind).unwrap_or(LOWEST)
    }

    /// Returns the precedence of the `peek` token or
    /// a default [`LOWEST`] if none is registered.
    fn peek_prec(&self) -> u8 {
        Precedence.get(&self.peek.kind).unwrap_or(LOWEST)
    }

    fn peek_error(&mut self, kind: TokenKind) {
        let msg = format!(
            "expected next token to be `{:?}`, got `{:?}` instead",
            kind, self.peek.kind
        );
        self.errors.push(msg);
    }

    fn prefix_parse_error(&mut self, kind: TokenKind) {
        let msg = format!("no prefix parse function for `{:?}` found", kind);
        self.errors.push(msg);
    }
}
