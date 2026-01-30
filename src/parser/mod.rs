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
            TokenKind::Int64 => self.parse_int64()?,
            TokenKind::Float64 => self.parse_float64()?,
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

    fn parse_int64(&mut self) -> Option<Expression> {
        let literal = self.current.literal;

        let literal = literal
            .strip_suffix("_i64")
            .or_else(|| literal.strip_suffix("i64"))
            .unwrap_or(literal);

        let (radix, value) = if let Some(bin) = literal
            .strip_prefix("0b")
            .or_else(|| literal.strip_prefix("0B"))
        {
            match i64::from_str_radix(bin, 2) {
                Ok(v) => (Radix::Bin, v),
                Err(_) => {
                    self.errors.push(format!(
                        "could not parse {:?} as binary integer",
                        self.current.literal
                    ));
                    return None;
                }
            }
        } else if let Some(oct) = literal
            .strip_prefix("0o")
            .or_else(|| literal.strip_prefix("0O"))
        {
            match i64::from_str_radix(oct, 8) {
                Ok(v) => (Radix::Oct, v),
                Err(_) => {
                    self.errors.push(format!(
                        "could not parse {:?} as octal integer",
                        self.current.literal
                    ));
                    return None;
                }
            }
        } else if let Some(hex) = literal
            .strip_prefix("0x")
            .or_else(|| literal.strip_prefix("0X"))
        {
            match i64::from_str_radix(hex, 16) {
                Ok(v) => (Radix::Hex, v),
                Err(_) => {
                    self.errors.push(format!(
                        "could not parse {:?} as hexadecimal integer",
                        self.current.literal
                    ));
                    return None;
                }
            }
        } else {
            match literal.parse::<i64>() {
                Ok(v) => (Radix::Dec, v),
                Err(_) => {
                    self.errors.push(format!(
                        "could not parse {:?} as `Int64`",
                        self.current.literal
                    ));
                    return None;
                }
            }
        };

        Some(Expression::Int64(Int64 { radix, value }))
    }

    fn parse_float64(&mut self) -> Option<Expression> {
        let literal = self.current.literal;

        let literal = literal
            .strip_suffix("_f64")
            .or_else(|| literal.strip_suffix("f64"))
            .unwrap_or(literal);

        match literal.parse::<f64>() {
            Ok(value) => Some(Expression::Float64(Float64::from(value))),
            Err(_) => {
                let msg = format!("could not parse {:?} as `Float64`", self.current.literal);
                self.errors.push(msg);
                None
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Program {
        let mut parser = Parser::new(Lexer::new(input));
        let program = parser.parse_program();
        assert!(
            parser.errors().is_empty(),
            "parser errors: {:?}",
            parser.errors()
        );
        program
    }

    fn expect_single_stmt(program: &Program) -> &Statement {
        assert_eq!(program.statements.len(), 1);
        &program.statements[0]
    }

    #[test]
    fn parse_let_statements() {
        [
            ("let x = 5;", "x", "5"),
            ("let y = true;", "y", "true"),
            ("let foobar = y;", "foobar", "y"),
        ]
        .iter()
        .for_each(|(input, ident, value)| {
            let program = parse(input);
            let Statement::Let(let_stmt) = expect_single_stmt(&program) else {
                panic!("expected Let statement");
            };
            assert_eq!(let_stmt.ident, *ident);
            assert_eq!(let_stmt.value.to_string(), *value);
        });
    }

    #[test]
    fn parse_return_statements() {
        [
            ("return 5;", "5"),
            ("return true;", "true"),
            ("return foobar;", "foobar"),
        ]
        .iter()
        .for_each(|(input, value)| {
            let program = parse(input);
            let Statement::Return(ret) = expect_single_stmt(&program) else {
                panic!("expected Return statement");
            };
            assert_eq!(ret.value.to_string(), *value);
        });
    }

    #[test]
    fn parse_identifier_expression() {
        let program = parse("foobar;");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Identifier(ident),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected identifier expression");
        };
        assert_eq!(ident, "foobar");
    }

    #[test]
    fn parse_integer_literal_expression() {
        let program = parse("5;");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Int64(Int64 { value, .. }),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Int64 expression");
        };
        assert_eq!(*value, 5);
    }

    #[test]
    fn parse_boolean_expression() {
        [("true;", true), ("false;", false)]
            .iter()
            .for_each(|(input, expected)| {
                let program = parse(input);
                let Statement::Expression(ExpressionStatement {
                    value: Expression::Boolean(value),
                }) = expect_single_stmt(&program)
                else {
                    panic!("expected Boolean expression");
                };
                assert_eq!(*value, *expected);
            });
    }

    #[test]
    fn parse_prefix_expressions() {
        [
            ("!5;", "!", "5"),
            ("-15;", "-", "15"),
            ("!foobar;", "!", "foobar"),
            ("-foobar;", "-", "foobar"),
            ("!true;", "!", "true"),
            ("!false;", "!", "false"),
        ]
        .iter()
        .for_each(|(input, op, operand)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Prefix(prefix),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Prefix expression");
            };
            assert_eq!(prefix.operator, *op);
            assert_eq!(prefix.operand.to_string(), *operand);
        });
    }

    #[test]
    fn parse_infix_expressions() {
        [
            ("5 + 5;", "5", "+", "5"),
            ("5 - 5;", "5", "-", "5"),
            ("5 * 5;", "5", "*", "5"),
            ("5 / 5;", "5", "/", "5"),
            ("5 > 5;", "5", ">", "5"),
            ("5 < 5;", "5", "<", "5"),
            ("5 == 5;", "5", "==", "5"),
            ("5 != 5;", "5", "!=", "5"),
            ("true == true", "true", "==", "true"),
            ("true != false", "true", "!=", "false"),
            ("false == false", "false", "==", "false"),
        ]
        .iter()
        .for_each(|(input, lhs, op, rhs)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Infix(infix),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Infix expression");
            };
            assert_eq!(infix.lhs.to_string(), *lhs);
            assert_eq!(infix.operator, *op);
            assert_eq!(infix.rhs.to_string(), *rhs);
        });
    }

    #[test]
    fn parse_string_literal() {
        [
            (r#""hello world""#, "hello world"),
            (r#""foo bar""#, "foo bar"),
            (r#""""#, ""),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::String(s),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected string literal");
            };
            assert_eq!(s, *expected);
        });
    }

    #[test]
    fn parse_float_literal() {
        [
            ("3.14;", 3.14),
            ("0.5;", 0.5),
            ("123.456;", 123.456),
            ("1e10;", 1e10),
            ("1.5E-3;", 1.5E-3),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Float64(float),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected float literal");
            };
            let value: f64 = (*float).into();
            assert!((value - expected).abs() < 1e-10, "input: {}", input);
        });
    }

    #[test]
    fn parse_array_literal() {
        let program = parse("[1, 2 * 2, 3 + 3]");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Array(array),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected array literal");
        };
        assert_eq!(array.elements.len(), 3);
        assert_eq!(array.elements[0].to_string(), "1");
        assert_eq!(array.elements[1].to_string(), "(2 * 2)");
        assert_eq!(array.elements[2].to_string(), "(3 + 3)");
    }

    #[test]
    fn parse_empty_array() {
        let program = parse("[]");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Array(array),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected array literal");
        };
        assert_eq!(array.elements.len(), 0);
    }

    #[test]
    fn parse_index_expression() {
        let program = parse("myArray[1 + 1]");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Index(index),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected index expression");
        };
        assert!(matches!(&*index.expr, Expression::Identifier(id) if id == "myArray"));
        assert_eq!(index.index.to_string(), "(1 + 1)");
    }

    #[test]
    fn parse_hash_literal() {
        let program = parse(r#"{"one": 1, "two": 2, "three": 3}"#);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Hash(hash),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected hash literal");
        };
        assert_eq!(hash.pairs.len(), 3);

        let expected = [("one", "1"), ("two", "2"), ("three", "3")];
        for (key, value) in expected {
            let found = hash
                .pairs
                .iter()
                .any(|(k, v)| k.to_string() == key && v.to_string() == value);
            assert!(found, "expected key-value pair: {} => {}", key, value);
        }
    }

    #[test]
    fn parse_empty_hash() {
        let program = parse("{}");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Hash(hash),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected hash literal");
        };
        assert_eq!(hash.pairs.len(), 0);
    }

    #[test]
    fn parse_hash_with_expressions() {
        let program = parse(r#"{"one": 0 + 1, "two": 10 - 8}"#);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Hash(hash),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected hash literal");
        };
        assert_eq!(hash.pairs.len(), 2);
    }

    #[test]
    fn parse_binary_literals() {
        [("0b1010;", 10), ("0B1111;", 15), ("0b0;", 0)]
            .iter()
            .for_each(|(input, expected)| {
                let program = parse(input);
                let Statement::Expression(ExpressionStatement {
                    value: Expression::Int64(int64),
                }) = expect_single_stmt(&program)
                else {
                    panic!("expected Int64 expression");
                };
                assert_eq!(int64.radix, ast::Radix::Bin);
                assert_eq!(int64.value, *expected, "input: {}", input);
            });
    }

    #[test]
    fn parse_octal_literals() {
        [("0o755;", 493), ("0O644;", 420), ("0o0;", 0)]
            .iter()
            .for_each(|(input, expected)| {
                let program = parse(input);
                let Statement::Expression(ExpressionStatement {
                    value: Expression::Int64(int64),
                }) = expect_single_stmt(&program)
                else {
                    panic!("expected Int64 expression");
                };
                assert_eq!(int64.radix, ast::Radix::Oct);
                assert_eq!(int64.value, *expected, "input: {}", input);
            });
    }

    #[test]
    fn parse_hex_literals() {
        [("0xff;", 255), ("0xFF;", 255), ("0xDEAD;", 57005)]
            .iter()
            .for_each(|(input, expected)| {
                let program = parse(input);
                let Statement::Expression(ExpressionStatement {
                    value: Expression::Int64(int64),
                }) = expect_single_stmt(&program)
                else {
                    panic!("expected Int64 expression");
                };
                assert_eq!(int64.radix, ast::Radix::Hex);
                assert_eq!(int64.value, *expected, "input: {}", input);
            });
    }

    #[test]
    fn parse_rust_style_suffixes() {
        let program = parse("123i64;");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Int64(int64),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Int64 expression");
        };
        assert_eq!(int64.value, 123);

        let program = parse("3.14f64;");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Float64(float64),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Float64 expression");
        };
        let value: f64 = (*float64).into();
        assert!((value - 3.14).abs() < 1e-10);
    }

    #[test]
    fn parse_operator_precedence() {
        [
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
            ("a + add(b * c) + d", "((a + add((b * c))) + d)"),
            (
                "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
                "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))",
            ),
            (
                "add(a + b + c * d / f + g)",
                "add((((a + b) + ((c * d) / f)) + g))",
            ),
        ]
        .iter()
        .for_each(|(input, expected)| {
            assert_eq!(parse(input).to_string(), *expected);
        });
    }

    #[test]
    fn parse_if_expression() {
        let program = parse("if (x < y) { x }");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Conditional(cond),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Conditional expression");
        };

        let Expression::Infix(infix) = cond.condition.as_ref() else {
            panic!("expected Infix condition");
        };
        assert_eq!(infix.to_string(), "(x < y)");
        assert_eq!(cond.consequence.statements.len(), 1);
        assert_eq!(cond.consequence.statements[0].to_string(), "x");
        assert!(cond.alternative.is_none());
    }

    #[test]
    fn parse_if_else_expression() {
        let program = parse("if (x < y) { x } else { y }");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Conditional(cond),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Conditional expression");
        };

        assert_eq!(cond.condition.to_string(), "(x < y)");
        assert_eq!(cond.consequence.statements[0].to_string(), "x");
        assert_eq!(
            cond.alternative.as_ref().unwrap().statements[0].to_string(),
            "y"
        );
    }

    #[test]
    fn parse_function_literal() {
        let program = parse("fn(x, y) { x + y; }");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Function(func),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Function expression");
        };

        assert_eq!(func.params, vec!["x", "y"]);
        assert_eq!(func.body.statements.len(), 1);
        assert_eq!(func.body.statements[0].to_string(), "(x + y)");
    }

    #[test]
    fn parse_function_parameters() {
        [
            ("fn() {};", vec![]),
            ("fn(x) {};", vec!["x"]),
            ("fn(x, y, z) {};", vec!["x", "y", "z"]),
        ]
        .iter()
        .for_each(|(input, expected_params)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Function(func),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Function expression");
            };
            assert_eq!(func.params, *expected_params);
        });
    }

    #[test]
    fn parse_call_expression() {
        let program = parse("add(1, 2 * 3, 4 + 5);");
        let Statement::Expression(ExpressionStatement {
            value: Expression::Call(call),
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Call expression");
        };

        assert_eq!(call.function.to_string(), "add");
        assert_eq!(call.arguments.len(), 3);
        assert_eq!(call.arguments[0].to_string(), "1");
        assert_eq!(call.arguments[1].to_string(), "(2 * 3)");
        assert_eq!(call.arguments[2].to_string(), "(4 + 5)");
    }

    #[test]
    fn parse_call_arguments() {
        [
            ("add();", "add", vec![]),
            ("add(1);", "add", vec!["1"]),
            (
                "add(1, 2 * 3, 4 + 5);",
                "add",
                vec!["1", "(2 * 3)", "(4 + 5)"],
            ),
        ]
        .iter()
        .for_each(|(input, func_name, expected_args)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Call(call),
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Call expression");
            };
            assert_eq!(call.function.to_string(), *func_name);
            assert_eq!(
                call.arguments
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<_>>(),
                *expected_args
            );
        });
    }
}
