pub mod ast;
mod prec;

use ast::*;
use prec::{LOWEST, PREFIX, Precedence};

use crate::{Lexer, Token, TokenKind};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token<'a>,
    peek: Token<'a>,
    errors: Vec<String>,
}

impl<'a> Parser<'a> {
    /// Initializes the parser with current/peek tokens set.
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        Self {
            current: lexer.next_token(),
            peek: lexer.next_token(),
            lexer,
            errors: vec![],
        }
    }

    pub fn errors(&self) -> &Vec<String> {
        &self.errors
    }

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
            TokenKind::Int => self.parse_integer()?,
            TokenKind::Bang | TokenKind::Minus => self.parse_prefix_expression()?,
            TokenKind::True | TokenKind::False => self.parse_boolean()?,
            TokenKind::LParen => self.parse_grouped_expression()?,
            TokenKind::If => self.parse_conditional_expression()?,
            TokenKind::Function => self.parse_function()?,
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
                | TokenKind::LParen => {
                    self.next_token();
                    lhs = match kind {
                        TokenKind::LParen => self.parse_call_expression(lhs)?,
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

    fn parse_integer(&mut self) -> Option<Expression> {
        if let Ok(value) = self.current.literal.parse::<i64>() {
            return Some(Expression::Int64(Int64 { value }));
        }
        None
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
            arguments: self.parse_call_args()?,
        }))
    }

    fn parse_call_args(&mut self) -> Option<Vec<Expression>> {
        let mut arguments = Vec::new();

        if self.peek_token_is(TokenKind::RParen) {
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

        if !self.expect_peek(TokenKind::RParen) {
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
            "expected next token to be {:?}, got {:?} instead",
            kind, self.peek.kind
        );
        self.errors.push(msg);
    }

    fn prefix_parse_error(&mut self, kind: TokenKind) {
        let msg = format!("no prefix parse function for {:?} found", kind);
        self.errors.push(msg);
    }
}
