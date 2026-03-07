//! Recursive descent parser using Pratt parsing for operator precedence.
//!
//! # Example
//!
//! ```
//! use maat_lexer::Lexer;
//! use maat_parser::Parser;
//!
//! let input = "let x = 5 + 10;";
//! let lexer = Lexer::new(input);
//! let mut parser = Parser::new(lexer);
//! let program = parser.parse();
//!
//! assert_eq!(parser.errors().len(), 0);
//! assert_eq!(program.statements.len(), 1);
//! ```

use maat_ast::*;
use maat_errors::ParseError;
use maat_lexer::{Lexer, Span, Token, TokenKind};

use crate::prec::{LOWEST, PREFIX, Precedence};

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
    errors: Vec<ParseError>,
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
    /// use maat_lexer::Lexer;
    /// use maat_parser::Parser;
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

    /// Returns the precedence of the `current` token or
    /// a default [`LOWEST`] if none is registered.
    fn current_prec(&self) -> u8 {
        Precedence::get(&self.current.kind).unwrap_or(LOWEST)
    }

    /// Returns the precedence of the `peek` token or
    /// a default [`LOWEST`] if none is registered.
    fn peek_prec(&self) -> u8 {
        Precedence::get(&self.peek.kind).unwrap_or(LOWEST)
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
        if self.peek_token_is(expected) {
            self.next_token();
            return true;
        }

        let err = format!(
            "expected next token to be `{:?}`, got `{:?}` instead",
            expected, self.peek.kind
        );
        self.push_error(err);
        false
    }

    /// Advance the parser: shift `peek` into `current` token,
    /// and read a fresh `peek` token from the lexer.
    fn next_token(&mut self) {
        self.current = std::mem::replace(&mut self.peek, self.lexer.next_token());
    }

    /// Pushes an error with the current token's span.
    fn push_error(&mut self, message: impl Into<String>) {
        self.errors
            .push(ParseError::new(message, self.current.span));
    }

    /// Returns a reference to the errors encountered during parsing.
    ///
    /// Each error includes the error message and the source position where
    /// it occurred, enabling precise error reporting with line and column numbers.
    ///
    /// # Example
    ///
    /// ```
    /// use maat_lexer::Lexer;
    /// use maat_parser::Parser;
    ///
    /// let lexer = Lexer::new("let = 5;");
    /// let mut parser = Parser::new(lexer);
    /// let _program = parser.parse();
    ///
    /// assert!(!parser.errors().is_empty());
    /// ```
    pub fn errors(&self) -> &Vec<ParseError> {
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
    /// use maat_lexer::Lexer;
    /// use maat_parser::Parser;
    ///
    /// let input = r#"
    ///     let x = 5;
    ///     let y = 10;
    ///     return x + y;
    /// "#;
    /// let lexer = Lexer::new(input);
    /// let mut parser = Parser::new(lexer);
    /// let program = parser.parse();
    ///
    /// assert_eq!(parser.errors().len(), 0);
    /// assert_eq!(program.statements.len(), 3);
    /// ```
    pub fn parse(&mut self) -> Program {
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

    fn parse_statement(&mut self) -> Option<Stmt> {
        match self.current.kind {
            TokenKind::Let => self.parse_let_statement().map(Stmt::Let),
            TokenKind::Return => self.parse_return_statement().map(Stmt::Return),
            TokenKind::Fn if self.peek_token_is(TokenKind::Ident) => {
                self.parse_fn_declaration().map(Stmt::FuncDef)
            }
            TokenKind::Loop => self.parse_loop_statement().map(Stmt::Loop),
            TokenKind::While => self.parse_while_statement().map(Stmt::While),
            TokenKind::For => self.parse_for_statement().map(Stmt::For),
            TokenKind::Struct => self.parse_struct_decl().map(Stmt::StructDecl),
            TokenKind::Enum => self.parse_enum_decl().map(Stmt::EnumDecl),
            TokenKind::Trait => self.parse_trait_decl().map(Stmt::TraitDecl),
            TokenKind::Impl => self.parse_impl_block().map(Stmt::ImplBlock),
            _ => self.parse_expression_statement().map(Stmt::Expr),
        }
    }

    fn parse_let_statement(&mut self) -> Option<LetStmt> {
        let start = self.current.span;
        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let ident = self.current.literal.to_string();

        let type_annotation = if self.peek_token_is(TokenKind::Colon) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        if !self.expect_peek(TokenKind::Assign) {
            return None;
        }
        self.next_token();
        let value = self.parse_expression(LOWEST)?;
        let end = if self.peek_token_is(TokenKind::Semicolon) {
            self.next_token();
            self.current.span
        } else {
            value.span()
        };
        Some(LetStmt {
            ident,
            type_annotation,
            value,
            span: start.merge(end),
        })
    }

    fn parse_return_statement(&mut self) -> Option<ReturnStmt> {
        let start = self.current.span;
        self.next_token();
        let value = self.parse_expression(LOWEST)?;
        let end = if self.peek_token_is(TokenKind::Semicolon) {
            self.next_token();
            self.current.span
        } else {
            value.span()
        };
        Some(ReturnStmt {
            value,
            span: start.merge(end),
        })
    }

    fn parse_expression_statement(&mut self) -> Option<ExprStmt> {
        let value = self.parse_expression(LOWEST)?;
        let span = if self.peek_token_is(TokenKind::Semicolon) {
            let s = value.span().merge(self.peek.span);
            self.next_token();
            s
        } else {
            value.span()
        };
        Some(ExprStmt { value, span })
    }

    /// Parse an expression using Pratt-parsing:
    /// 1. Parse a prefix subexpression.
    /// 2. While the next token's precedence is higher than `prec`,
    ///    consume it and parse an infix operation.
    fn parse_expression(&mut self, prec: u8) -> Option<Expr> {
        let mut expr = match self.current.kind {
            TokenKind::Ident => self.parse_identifier()?,

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
            TokenKind::String => self.parse_string_literal()?,
            TokenKind::Bang | TokenKind::Minus => self.parse_prefix_expression()?,
            TokenKind::True | TokenKind::False => self.parse_boolean()?,
            TokenKind::LParen => self.parse_grouped_expression()?,
            TokenKind::If => self.parse_conditional_expression()?,
            TokenKind::Fn => self.parse_lambda()?,
            TokenKind::Macro => self.parse_macro()?,
            TokenKind::LBracket => self.parse_array_literal()?,
            TokenKind::LBrace => self.parse_hash_literal()?,
            TokenKind::Break => self.parse_break_expression()?,
            TokenKind::Continue => self.parse_continue_expression()?,
            TokenKind::Match => self.parse_match_expression()?,
            TokenKind::SelfValue => {
                let span = self.current.span;
                Expr::Ident(Ident {
                    value: "self".to_string(),
                    span,
                })
            }
            kind => {
                let msg = format!("no prefix parse function for `{:?}` found", kind);
                self.push_error(msg);
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
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::As
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::Dot => {
                    self.next_token();
                    expr = match kind {
                        TokenKind::LParen => self.parse_call_expression(expr)?,
                        TokenKind::LBracket => self.parse_index_expression(expr)?,
                        TokenKind::As => self.parse_cast_expression(expr)?,
                        TokenKind::Dot => self.parse_field_or_method_call(expr)?,
                        _ => self.parse_infix_expression(expr)?,
                    };
                }
                _ => break,
            }
        }

        Some(expr)
    }

    fn parse_prefix_expression(&mut self) -> Option<Expr> {
        let start = self.current.span;
        let operator = self.current.literal.to_string();
        self.next_token();
        let operand = Box::new(self.parse_expression(PREFIX)?);
        let span = start.merge(operand.span());
        Some(Expr::Prefix(PrefixExpr {
            operator,
            operand,
            span,
        }))
    }

    fn parse_infix_expression(&mut self, lhs: Expr) -> Option<Expr> {
        let start = lhs.span();
        let lhs = Box::new(lhs);
        let operator = self.current.literal.to_string();
        let prec = self.current_prec();
        self.next_token();
        let rhs = Box::new(self.parse_expression(prec)?);
        let span = start.merge(rhs.span());
        Some(Expr::Infix(InfixExpr {
            lhs,
            operator,
            rhs,
            span,
        }))
    }

    fn parse_cast_expression(&mut self, lhs: Expr) -> Option<Expr> {
        let start = lhs.span();
        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }

        let end = self.current.span;
        let target: TypeAnnotation = self.current.literal.parse().ok().or_else(|| {
            self.push_error(format!(
                "unknown type annotation `{}`",
                self.current.literal
            ));
            None
        })?;

        Some(Expr::Cast(CastExpr {
            expr: Box::new(lhs),
            target,
            span: start.merge(end),
        }))
    }

    fn parse_identifier(&mut self) -> Option<Expr> {
        let name = self.current.literal.to_string();
        let start = self.current.span;

        // Path expression: `Enum::Variant`
        if self.peek_token_is(TokenKind::PathSep) {
            return self.parse_path_expression(name, start);
        }

        // Struct literal: `Name { field: value, ... }`
        // Disambiguate from hash literal by requiring an uppercase first character.
        if self.peek_token_is(TokenKind::LBrace) && name.starts_with(char::is_uppercase) {
            return self.parse_struct_literal(name, start);
        }

        Some(Expr::Ident(Ident {
            value: name,
            span: start,
        }))
    }

    fn parse_path_expression(&mut self, first: String, start: Span) -> Option<Expr> {
        let mut segments = vec![first];

        while self.peek_token_is(TokenKind::PathSep) {
            self.next_token();
            if !self.expect_peek(TokenKind::Ident) {
                return None;
            }
            segments.push(self.current.literal.to_string());
        }

        let end = self.current.span;
        Some(Expr::PathExpr(PathExpr {
            segments,
            span: start.merge(end),
        }))
    }

    fn parse_struct_literal(&mut self, name: String, start: Span) -> Option<Expr> {
        self.next_token();
        let mut fields = Vec::new();

        while !self.peek_token_is(TokenKind::RBrace) {
            if !self.expect_peek(TokenKind::Ident) {
                return None;
            }
            let field_name = self.current.literal.to_string();

            if !self.expect_peek(TokenKind::Colon) {
                return None;
            }

            self.next_token();
            let value = self.parse_expression(LOWEST)?;
            fields.push((field_name, value));

            if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }

        let end = self.current.span;
        Some(Expr::StructLit(StructLitExpr {
            name,
            fields,
            span: start.merge(end),
        }))
    }

    fn parse_boolean(&mut self) -> Option<Expr> {
        Some(Expr::Bool(Bool {
            value: self.cur_token_is(TokenKind::True),
            span: self.current.span,
        }))
    }

    fn parse_int(&mut self) -> Option<Expr> {
        let literal = self.current.literal;
        let token_kind = self.current.kind;
        let span = self.current.span;

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
                    self.push_error(format!(
                        "could not parse {:?} as {}",
                        self.current.literal,
                        stringify!($rust_ty)
                    ));
                    None
                })?;

                Expr::$variant($variant { radix, value, span })
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

    fn parse_string_literal(&mut self) -> Option<Expr> {
        Some(Expr::Str(Str {
            value: self.current.literal.to_owned(),
            span: self.current.span,
        }))
    }

    fn parse_array_literal(&mut self) -> Option<Expr> {
        let start = self.current.span;
        let elements = self.parse_expression_list(TokenKind::RBracket)?;
        let end = self.current.span;
        Some(Expr::Array(Array {
            elements,
            span: start.merge(end),
        }))
    }

    fn parse_index_expression(&mut self, expr: Expr) -> Option<Expr> {
        let start = expr.span();
        let expr = Box::new(expr);
        self.next_token();
        let index = Box::new(self.parse_expression(LOWEST)?);

        if !self.expect_peek(TokenKind::RBracket) {
            return None;
        }
        let end = self.current.span;
        Some(Expr::Index(IndexExpr {
            expr,
            index,
            span: start.merge(end),
        }))
    }

    fn parse_hash_literal(&mut self) -> Option<Expr> {
        let start = self.current.span;
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

        let end = self.current.span;
        Some(Expr::Map(Map {
            pairs,
            span: start.merge(end),
        }))
    }

    fn parse_grouped_expression(&mut self) -> Option<Expr> {
        self.next_token();
        let expr = self.parse_expression(LOWEST)?;
        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }
        Some(expr)
    }

    fn parse_conditional_expression(&mut self) -> Option<Expr> {
        let start = self.current.span;
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

        let end = alternative
            .as_ref()
            .map_or(consequence.span, |alt| alt.span);
        Some(Expr::Cond(CondExpr {
            condition,
            consequence,
            alternative,
            span: start.merge(end),
        }))
    }

    fn parse_loop_statement(&mut self) -> Option<LoopStmt> {
        let start = self.current.span;
        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;
        Some(LoopStmt {
            body,
            span: start.merge(end),
        })
    }

    fn parse_while_statement(&mut self) -> Option<WhileStmt> {
        let start = self.current.span;
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
        let body = self.parse_block_statement()?;
        let end = self.current.span;
        Some(WhileStmt {
            condition,
            body,
            span: start.merge(end),
        })
    }

    fn parse_for_statement(&mut self) -> Option<ForStmt> {
        let start = self.current.span;
        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let ident = self.current.literal.to_string();
        if !self.expect_peek(TokenKind::In) {
            return None;
        }
        self.next_token();
        let iterable = Box::new(self.parse_expression(LOWEST)?);
        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;
        Some(ForStmt {
            ident,
            iterable,
            body,
            span: start.merge(end),
        })
    }

    fn parse_break_expression(&mut self) -> Option<Expr> {
        let start = self.current.span;
        let value = if !self.peek_token_is(TokenKind::Semicolon)
            && !self.peek_token_is(TokenKind::RBrace)
            && !self.peek_token_is(TokenKind::Eof)
        {
            self.next_token();
            Some(Box::new(self.parse_expression(LOWEST)?))
        } else {
            None
        };
        let end = value.as_ref().map_or(start, |v| v.span());
        Some(Expr::Break(BreakExpr {
            value,
            span: start.merge(end),
        }))
    }

    fn parse_continue_expression(&mut self) -> Option<Expr> {
        Some(Expr::Continue(ContinueExpr {
            span: self.current.span,
        }))
    }

    fn parse_match_expression(&mut self) -> Option<Expr> {
        let start = self.current.span;
        self.next_token();
        let scrutinee = Box::new(self.parse_expression(LOWEST)?);

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }

        let mut arms = Vec::new();
        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            if self.peek_token_is(TokenKind::Comma) {
                self.next_token();
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }
        let end = self.current.span;

        Some(Expr::Match(MatchExpr {
            scrutinee,
            arms,
            span: start.merge(end),
        }))
    }

    fn parse_match_arm(&mut self) -> Option<MatchArm> {
        let start = self.current.span;
        let pattern = self.parse_pattern()?;

        let guard = if self.peek_token_is(TokenKind::If) {
            self.next_token();
            self.next_token();
            Some(Box::new(self.parse_expression(LOWEST)?))
        } else {
            None
        };

        if !self.expect_peek(TokenKind::FatArrow) {
            return None;
        }
        self.next_token();

        let body = if self.cur_token_is(TokenKind::LBrace) {
            // Block body: `=> { stmts... }`
            let block = self.parse_block_statement()?;
            let span = block.span;
            // Represent the block as a unit-returning conditional placeholder.
            // If the block has a final expression statement, unwrap it; otherwise
            // the arm evaluates to null. For now, we lower it to a Cond expression
            // so the existing evaluator handles it naturally.
            let body_span = span;
            // Wrap the block as an anonymous conditional that always executes.
            Expr::Cond(CondExpr {
                condition: Box::new(Expr::Bool(Bool {
                    value: true,
                    span: body_span,
                })),
                consequence: block,
                alternative: None,
                span: body_span,
            })
        } else {
            self.parse_expression(LOWEST)?
        };

        let end = body.span();
        Some(MatchArm {
            pattern,
            guard,
            body,
            span: start.merge(end),
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        let base = self.parse_single_pattern()?;

        // Or-pattern: `A | B | C`
        if self.peek_token_is(TokenKind::Or) {
            let start = base.span();
            let mut alternatives = vec![base];
            while self.peek_token_is(TokenKind::Or) {
                self.next_token();
                self.next_token();
                alternatives.push(self.parse_single_pattern()?);
            }
            let end = alternatives.last().map_or(start, |p| p.span());
            Some(Pattern::Or(alternatives, start.merge(end)))
        } else {
            Some(base)
        }
    }

    /// Parses a single (non-or) pattern.
    fn parse_single_pattern(&mut self) -> Option<Pattern> {
        let span = self.current.span;

        match self.current.kind {
            TokenKind::Ident if self.current.literal == "_" => Some(Pattern::Wildcard(span)),

            TokenKind::True => Some(Pattern::Literal(Box::new(Expr::Bool(Bool {
                value: true,
                span,
            })))),
            TokenKind::False => Some(Pattern::Literal(Box::new(Expr::Bool(Bool {
                value: false,
                span,
            })))),

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
            | TokenKind::Usize => {
                let int = self.parse_int()?;
                Some(Pattern::Literal(Box::new(int)))
            }

            TokenKind::Minus => {
                let prefix = self.parse_prefix_expression()?;
                Some(Pattern::Literal(Box::new(prefix)))
            }

            TokenKind::String => {
                let s = self.parse_string_literal()?;
                Some(Pattern::Literal(Box::new(s)))
            }

            // Identifier or path-based patterns: `x`, `Some(x)`, `Point { x }`
            TokenKind::Ident | TokenKind::SelfType => {
                let name = self.current.literal.to_string();
                match self.peek.kind {
                    // Tuple-struct pattern: `Name(pat, pat, ...)`
                    TokenKind::LParen => {
                        self.next_token();
                        let fields = self.parse_pattern_list(TokenKind::RParen)?;
                        let end = self.current.span;
                        Some(Pattern::TupleStruct {
                            path: name,
                            fields,
                            span: span.merge(end),
                        })
                    }
                    // Struct pattern: `Name { field, field: pat, ... }`
                    TokenKind::LBrace => {
                        self.next_token();
                        let fields = self.parse_pattern_fields()?;
                        let end = self.current.span;
                        Some(Pattern::Struct {
                            path: name,
                            fields,
                            span: span.merge(end),
                        })
                    }
                    // Plain identifier binding (lowercase) or unit variant (uppercase)
                    _ => Some(Pattern::Ident(name, span)),
                }
            }

            kind => {
                self.push_error(format!("expected pattern, got `{kind:?}`"));
                None
            }
        }
    }

    /// Parses a comma-separated list of patterns terminated by `end_token`.
    ///
    /// Leaves the current token on the closing delimiter.
    fn parse_pattern_list(&mut self, end_token: TokenKind) -> Option<Vec<Pattern>> {
        let mut patterns = Vec::new();

        if self.peek_token_is(end_token) {
            self.next_token();
            return Some(patterns);
        }

        self.next_token();
        patterns.push(self.parse_pattern()?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            if self.peek_token_is(end_token) {
                // trailing comma
                break;
            }
            self.next_token();
            patterns.push(self.parse_pattern()?);
        }

        if !self.expect_peek(end_token) {
            return None;
        }

        Some(patterns)
    }

    /// Parses the field list inside a struct pattern: `{ field, field: pat, .. }`.
    ///
    /// Leaves the current token on `}`.
    fn parse_pattern_fields(&mut self) -> Option<Vec<PatternField>> {
        let mut fields = Vec::new();

        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            let field_span = self.current.span;

            if !self.cur_token_is(TokenKind::Ident) {
                self.push_error(format!(
                    "expected field name in struct pattern, got `{:?}`",
                    self.current.kind
                ));
                return None;
            }

            let name = self.current.literal.to_string();

            let pattern = if self.peek_token_is(TokenKind::Colon) {
                self.next_token();
                self.next_token();
                Some(Box::new(self.parse_pattern()?))
            } else {
                None
            };

            fields.push(PatternField {
                name,
                pattern,
                span: field_span,
            });

            if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }

        Some(fields)
    }

    /// Parses a field access or method call after a `.` token.
    ///
    /// Current token is `.` on entry. The next token must be an identifier.
    fn parse_field_or_method_call(&mut self, object: Expr) -> Option<Expr> {
        let start = object.span();

        if !self.peek_token_is(TokenKind::Ident)
            && !self.peek_token_is(TokenKind::SelfValue)
            && !self.peek_token_is(TokenKind::SelfType)
        {
            self.push_error(format!(
                "expected field or method name after `.`, got `{:?}`",
                self.peek.kind
            ));
            return None;
        }
        self.next_token();

        let member = self.current.literal.to_string();

        if self.peek_token_is(TokenKind::LParen) {
            // Method call: `object.method(args)`
            self.next_token();
            let arguments = self.parse_expression_list(TokenKind::RParen)?;
            let end = self.current.span;
            Some(Expr::MethodCall(MethodCallExpr {
                object: Box::new(object),
                method: member,
                arguments,
                span: start.merge(end),
            }))
        } else {
            // Field access: `object.field`
            let end = self.current.span;
            Some(Expr::FieldAccess(FieldAccessExpr {
                object: Box::new(object),
                field: member,
                span: start.merge(end),
            }))
        }
    }

    fn parse_struct_decl(&mut self) -> Option<StructDecl> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }

        let mut fields = Vec::new();
        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            let field = self.parse_struct_field()?;
            fields.push(field);
            if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }
        let end = self.current.span;

        Some(StructDecl {
            name,
            generic_params,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_struct_field(&mut self) -> Option<StructField> {
        let start = self.current.span;
        if !self.cur_token_is(TokenKind::Ident) {
            self.push_error(format!(
                "expected field name, got `{:?}`",
                self.current.kind
            ));
            return None;
        }
        let name = self.current.literal.to_string();

        if !self.expect_peek(TokenKind::Colon) {
            return None;
        }
        self.next_token();
        let ty = self.parse_type_expr()?;
        let end = ty.span();

        Some(StructField {
            name,
            ty,
            span: start.merge(end),
        })
    }

    fn parse_enum_decl(&mut self) -> Option<EnumDecl> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }

        let mut variants = Vec::new();
        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            let variant = self.parse_enum_variant()?;
            variants.push(variant);
            if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }
        let end = self.current.span;

        Some(EnumDecl {
            name,
            generic_params,
            variants,
            span: start.merge(end),
        })
    }

    /// Parses a single enum variant.
    ///
    /// - Unit: `None`
    /// - Tuple: `Some(T)`
    /// - Struct: `Point { x: i64, y: i64 }`
    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let start = self.current.span;
        if !self.cur_token_is(TokenKind::Ident) {
            self.push_error(format!(
                "expected variant name, got `{:?}`",
                self.current.kind
            ));
            return None;
        }
        let name = self.current.literal.to_string();

        let (kind, end) = if self.peek_token_is(TokenKind::LParen) {
            // Tuple variant: `Name(T1, T2, ...)`
            self.next_token();
            let mut types = Vec::new();
            if !self.peek_token_is(TokenKind::RParen) {
                self.next_token();
                types.push(self.parse_type_expr()?);
                while self.peek_token_is(TokenKind::Comma) {
                    self.next_token();
                    if self.peek_token_is(TokenKind::RParen) {
                        break;
                    }
                    self.next_token();
                    types.push(self.parse_type_expr()?);
                }
            }
            if !self.expect_peek(TokenKind::RParen) {
                return None;
            }
            (EnumVariantKind::Tuple(types), self.current.span)
        } else if self.peek_token_is(TokenKind::LBrace) {
            // Struct variant: `Name { field: Type, ... }`
            self.next_token();
            let mut fields = Vec::new();
            while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
                self.next_token();
                let field = self.parse_struct_field()?;
                fields.push(field);
                if !self.peek_token_is(TokenKind::RBrace) && !self.expect_peek(TokenKind::Comma) {
                    return None;
                }
            }
            if !self.expect_peek(TokenKind::RBrace) {
                return None;
            }
            (EnumVariantKind::Struct(fields), self.current.span)
        } else {
            // Unit variant: `Name`
            (EnumVariantKind::Unit, start)
        };

        Some(EnumVariant {
            name,
            kind,
            span: start.merge(end),
        })
    }

    fn parse_trait_decl(&mut self) -> Option<TraitDecl> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }

        let mut methods = Vec::new();
        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            if self.cur_token_is(TokenKind::Fn) {
                let method = self.parse_trait_method()?;
                methods.push(method);
            } else {
                self.push_error(format!(
                    "expected `fn` in trait body, got `{:?}`",
                    self.current.kind
                ));
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }
        let end = self.current.span;

        Some(TraitDecl {
            name,
            generic_params,
            methods,
            span: start.merge(end),
        })
    }

    fn parse_trait_method(&mut self) -> Option<TraitMethod> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_method_parameters()?;

        let return_type = if self.peek_token_is(TokenKind::Arrow) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let (default_body, end) = if self.peek_token_is(TokenKind::LBrace) {
            self.next_token();
            let body = self.parse_block_statement()?;
            let end = body.span;
            (Some(body), end)
        } else {
            let end = if self.peek_token_is(TokenKind::Semicolon) {
                self.next_token();
                self.current.span
            } else {
                return_type.as_ref().map_or(start, |t| t.span())
            };
            (None, end)
        };

        Some(TraitMethod {
            name,
            generic_params,
            params,
            return_type,
            default_body,
            span: start.merge(end),
        })
    }

    fn parse_impl_block(&mut self) -> Option<ImplBlock> {
        let start = self.current.span;

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        self.next_token();
        let first_type = self.parse_type_expr()?;

        let (trait_name, self_type) = if self.peek_token_is(TokenKind::For) {
            self.next_token();
            self.next_token();
            let self_ty = self.parse_type_expr()?;
            (Some(first_type), self_ty)
        } else {
            (None, first_type)
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }

        let mut methods = Vec::new();
        while !self.peek_token_is(TokenKind::RBrace) && !self.peek_token_is(TokenKind::Eof) {
            self.next_token();
            if self.cur_token_is(TokenKind::Fn) {
                if self.peek_token_is(TokenKind::Ident) {
                    let method = self.parse_impl_method()?;
                    methods.push(method);
                } else {
                    self.push_error(format!(
                        "expected method name after `fn` in impl block, got `{:?}`",
                        self.peek.kind
                    ));
                    return None;
                }
            } else {
                self.push_error(format!(
                    "expected `fn` in impl block, got `{:?}`",
                    self.current.kind
                ));
                return None;
            }
        }

        if !self.expect_peek(TokenKind::RBrace) {
            return None;
        }
        let end = self.current.span;

        Some(ImplBlock {
            trait_name,
            self_type,
            generic_params,
            methods,
            span: start.merge(end),
        })
    }

    /// Parses a method definition inside an `impl` block.
    ///
    /// This is identical to a named function declaration but accepts `self`
    /// as the first parameter name.
    fn parse_impl_method(&mut self) -> Option<FuncDef> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_method_parameters()?;

        let return_type = if self.peek_token_is(TokenKind::Arrow) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;

        Some(FuncDef {
            name,
            params,
            generic_params,
            return_type,
            body,
            span: start.merge(end),
        })
    }

    /// Parses method parameters, accepting `self` as the first parameter name.
    ///
    /// Differs from `parse_typed_parameters` by also accepting `TokenKind::SelfValue`
    /// as a valid parameter identifier.
    fn parse_method_parameters(&mut self) -> Option<Vec<TypedParam>> {
        let mut params = Vec::new();

        if self.peek_token_is(TokenKind::RParen) {
            self.next_token();
            return Some(params);
        }

        self.next_token();
        params.push(self.parse_method_param()?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            params.push(self.parse_method_param()?);
        }

        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }

        Some(params)
    }

    /// Parses a single method parameter, treating `self` as a valid name.
    fn parse_method_param(&mut self) -> Option<TypedParam> {
        let start = self.current.span;

        let name = match self.current.kind {
            TokenKind::SelfValue => "self".to_string(),
            TokenKind::SelfType => "Self".to_string(),
            TokenKind::Ident => self.current.literal.to_string(),
            _ => {
                self.push_error(format!(
                    "expected parameter name, got `{:?}`",
                    self.current.kind
                ));
                return None;
            }
        };

        let type_expr = if self.peek_token_is(TokenKind::Colon) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let end = type_expr.as_ref().map_or(start, |t| t.span());

        Some(TypedParam {
            name,
            type_expr,
            span: start.merge(end),
        })
    }

    fn parse_block_statement(&mut self) -> Option<BlockStmt> {
        let start = self.current.span;
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

        let end = self.current.span;
        Some(BlockStmt {
            statements,
            span: start.merge(end),
        })
    }

    /// Parses a named function declaration: `fn name<T>(params) -> ret { body }`.
    ///
    /// Called when the current token is `fn` and the peek token is an identifier.
    fn parse_fn_declaration(&mut self) -> Option<FuncDef> {
        let start = self.current.span;

        if !self.expect_peek(TokenKind::Ident) {
            return None;
        }
        let name = self.current.literal.to_string();

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_typed_parameters()?;

        let return_type = if self.peek_token_is(TokenKind::Arrow) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;

        Some(FuncDef {
            name,
            params,
            generic_params,
            return_type,
            body,
            span: start.merge(end),
        })
    }

    /// Parses an anonymous function expression: `fn<T>(params) -> ret { body }`.
    fn parse_lambda(&mut self) -> Option<Expr> {
        let start = self.current.span;

        let generic_params = if self.peek_token_is(TokenKind::Less) {
            self.next_token();
            self.parse_generic_params()?
        } else {
            vec![]
        };

        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_typed_parameters()?;

        let return_type = if self.peek_token_is(TokenKind::Arrow) {
            self.next_token();
            self.next_token();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;

        Some(Expr::Lambda(Lambda {
            params,
            generic_params,
            return_type,
            body,
            span: start.merge(end),
        }))
    }

    fn parse_macro(&mut self) -> Option<Expr> {
        let start = self.current.span;
        if !self.expect_peek(TokenKind::LParen) {
            return None;
        }
        let params = self.parse_parameters()?;
        if !self.expect_peek(TokenKind::LBrace) {
            return None;
        }
        let body = self.parse_block_statement()?;
        let end = self.current.span;

        Some(Expr::Macro(Macro {
            params,
            body,
            span: start.merge(end),
        }))
    }

    fn parse_parameters(&mut self) -> Option<Vec<String>> {
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

    fn parse_typed_parameters(&mut self) -> Option<Vec<TypedParam>> {
        let mut params = Vec::new();

        if self.peek_token_is(TokenKind::RParen) {
            self.next_token();
            return Some(params);
        }

        self.next_token();
        params.push(self.parse_typed_param()?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            params.push(self.parse_typed_param()?);
        }

        if !self.expect_peek(TokenKind::RParen) {
            return None;
        }

        Some(params)
    }

    fn parse_typed_param(&mut self) -> Option<TypedParam> {
        let start = self.current.span;
        let name = self.current.literal.to_string();

        let type_expr = if self.peek_token_is(TokenKind::Colon) {
            self.next_token();
            self.next_token();
            let ty = self.parse_type_expr()?;
            Some(ty)
        } else {
            None
        };

        let end = type_expr.as_ref().map_or(start, |t| t.span());

        Some(TypedParam {
            name,
            type_expr,
            span: start.merge(end),
        })
    }

    /// Parses a type expression.
    ///
    /// Handles: `i64`, `bool`, `String`, `[T]`, `{K: V}`, `fn(A) -> B`,
    /// `Foo<T, U>`, and other named types.
    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        match self.current.kind {
            TokenKind::LBracket => {
                let start = self.current.span;
                self.next_token();
                let elem = self.parse_type_expr()?;
                if !self.expect_peek(TokenKind::RBracket) {
                    return None;
                }
                let end = self.current.span;
                Some(TypeExpr::Array(Box::new(elem), start.merge(end)))
            }
            TokenKind::LBrace => {
                let start = self.current.span;
                self.next_token();
                let key = self.parse_type_expr()?;
                if !self.expect_peek(TokenKind::Colon) {
                    return None;
                }
                self.next_token();
                let value = self.parse_type_expr()?;
                if !self.expect_peek(TokenKind::RBrace) {
                    return None;
                }
                let end = self.current.span;
                Some(TypeExpr::Map(
                    Box::new(key),
                    Box::new(value),
                    start.merge(end),
                ))
            }
            TokenKind::Fn => {
                let start = self.current.span;
                if !self.expect_peek(TokenKind::LParen) {
                    return None;
                }
                let mut param_types = Vec::new();
                if !self.peek_token_is(TokenKind::RParen) {
                    self.next_token();
                    param_types.push(self.parse_type_expr()?);
                    while self.peek_token_is(TokenKind::Comma) {
                        self.next_token();
                        self.next_token();
                        param_types.push(self.parse_type_expr()?);
                    }
                }
                if !self.expect_peek(TokenKind::RParen) {
                    return None;
                }
                if !self.expect_peek(TokenKind::Arrow) {
                    return None;
                }
                self.next_token();
                let ret = self.parse_type_expr()?;
                let end = ret.span();
                Some(TypeExpr::Fn(param_types, Box::new(ret), start.merge(end)))
            }
            TokenKind::Ident | TokenKind::SelfType => {
                let start = self.current.span;
                let name = self.current.literal.to_string();

                if self.peek_token_is(TokenKind::Less) {
                    self.next_token();
                    self.next_token();
                    let mut args = vec![self.parse_type_expr()?];
                    while self.peek_token_is(TokenKind::Comma) {
                        self.next_token();
                        self.next_token();
                        args.push(self.parse_type_expr()?);
                    }
                    if !self.expect_peek(TokenKind::Greater) {
                        return None;
                    }
                    let end = self.current.span;
                    Some(TypeExpr::Generic(name, args, start.merge(end)))
                } else {
                    Some(TypeExpr::Named(NamedType { name, span: start }))
                }
            }
            _ => {
                self.push_error(format!(
                    "expected type expression, got `{:?}`",
                    self.current.kind
                ));
                None
            }
        }
    }

    /// Parses generic type parameters: `<T>`, `<T, U>`, `<T: Bound + Bound>`.
    ///
    /// Called when current token is `<`.
    fn parse_generic_params(&mut self) -> Option<Vec<GenericParam>> {
        let mut params = Vec::new();

        if self.peek_token_is(TokenKind::Greater) {
            self.next_token();
            return Some(params);
        }

        self.next_token();
        params.push(self.parse_generic_param()?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            params.push(self.parse_generic_param()?);
        }

        if !self.expect_peek(TokenKind::Greater) {
            return None;
        }

        Some(params)
    }

    fn parse_generic_param(&mut self) -> Option<GenericParam> {
        let start = self.current.span;
        let name = self.current.literal.to_string();

        let mut bounds = Vec::new();
        if self.peek_token_is(TokenKind::Colon) {
            self.next_token();
            self.next_token();
            let bound_start = self.current.span;
            bounds.push(TraitBound {
                name: self.current.literal.to_string(),
                span: bound_start,
            });
            while self.peek_token_is(TokenKind::Plus) {
                self.next_token();
                self.next_token();
                let bound_start = self.current.span;
                bounds.push(TraitBound {
                    name: self.current.literal.to_string(),
                    span: bound_start,
                });
            }
        }

        let end = bounds.last().map_or(start, |b| b.span);

        Some(GenericParam {
            name,
            bounds,
            span: start.merge(end),
        })
    }

    fn parse_call_expression(&mut self, func: Expr) -> Option<Expr> {
        let start = func.span();
        let arguments = self.parse_expression_list(TokenKind::RParen)?;
        let end = self.current.span;
        Some(Expr::Call(CallExpr {
            function: Box::new(func),
            arguments,
            span: start.merge(end),
        }))
    }

    fn parse_expression_list(&mut self, end: TokenKind) -> Option<Vec<Expr>> {
        let mut list = Vec::new();

        if self.peek_token_is(end) {
            self.next_token();
            return Some(list);
        }

        self.next_token();
        list.push(self.parse_expression(LOWEST)?);

        while self.peek_token_is(TokenKind::Comma) {
            self.next_token();
            self.next_token();
            list.push(self.parse_expression(LOWEST)?);
        }

        if !self.expect_peek(end) {
            return None;
        }

        Some(list)
    }
}
