//! Combinator-based parser powered by [`winnow`], with Pratt-style operator
//! precedence for expressions.
//!
//! Tokens produced by the [`Lexer`] are collected into a flat slice, then parsed
//! via [`winnow`] stream operations. Statement dispatch uses a two-token
//! lookahead match; expression parsing uses a manual Pratt loop that delegates
//! to winnow for individual token consumption.
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

use std::cell::Cell;

use maat_ast::*;
use maat_errors::ParseError;
use maat_lexer::{Lexer, Token, TokenKind};
use maat_span::Span;
use winnow::error::{ContextError, ErrMode};
use winnow::token::any;
use winnow::{ModalResult, Parser as _};

use crate::prec::{LOWEST, PREFIX, Precedence};

type ParseResult<T> = ModalResult<T, ContextError>;

/// Maximum nesting depth for expressions. Prevents stack overflow on
/// deeply nested input like `(((((((...)))))))`  or `1+1+1+1+...`.
const MAX_NESTING_DEPTH: usize = 256;

/// Returns the [`TokenKind`] of the first unconsumed token, or
/// [`TokenKind::Eof`] if the stream is exhausted.
#[inline]
fn peek_kind(input: &[Token<'_>]) -> TokenKind {
    input.first().map_or(TokenKind::Eof, |t| t.kind)
}

/// Returns the [`TokenKind`] at offset `n` (0-indexed) without consuming.
#[inline]
fn peek_at(input: &[Token<'_>], n: usize) -> TokenKind {
    input.get(n).map_or(TokenKind::Eof, |t| t.kind)
}

/// Consumes the next token if its kind matches `expected`, otherwise returns
/// an error.
fn expect<'a>(input: &mut &'a [Token<'a>], expected: TokenKind) -> ParseResult<Token<'a>> {
    any.verify(move |t: &Token<'_>| t.kind == expected)
        .parse_next(input)
}

/// Optionally consumes the next token if its kind matches `expected`.
fn eat<'a>(input: &mut &'a [Token<'a>], expected: TokenKind) -> Option<Token<'a>> {
    if peek_kind(input) == expected {
        any::<_, ContextError>.parse_next(input).ok()
    } else {
        None
    }
}

/// Returns `true` if `kind` is a compound-assignment operator.
fn is_compound_assign(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::AddAssign
            | TokenKind::SubAssign
            | TokenKind::MulAssign
            | TokenKind::DivAssign
            | TokenKind::RemAssign
    )
}

/// A combinator-based parser that builds an AST from a token stream.
///
/// Tokens are eagerly collected from the [`Lexer`] into a contiguous slice,
/// then parsed via [`winnow`] stream operations. Errors encountered during
/// parsing are collected rather than immediately halting execution, allowing
/// multiple errors to be reported at once.
pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    /// Creates a new parser from a lexer.
    ///
    /// All tokens--including the trailing [`TokenKind::Eof`]--are eagerly
    /// collected into a contiguous slice for parsing.
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
        let mut tokens = Vec::new();
        loop {
            let tok = lexer.next_token();
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Self {
            tokens,
            errors: Vec::new(),
        }
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
        let input = &mut self.tokens.as_slice();
        let depth = Cell::new(0usize);
        let mut statements = Vec::new();

        while peek_kind(input) != TokenKind::Eof {
            match parse_statement(input, &depth) {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    let span = input.first().map_or(Span::ZERO, |t| t.span);
                    let msg = match e {
                        ErrMode::Backtrack(ref ctx) | ErrMode::Cut(ref ctx) => {
                            format!("{ctx:?}")
                        }
                        ErrMode::Incomplete(_) => "incomplete input".into(),
                    };
                    self.errors.push(ParseError::new(msg, span));
                    if peek_kind(input) != TokenKind::Eof {
                        *input = &input[1..];
                    }
                }
            }
        }
        Program { statements }
    }
}

/// Parses a single top-level or block-level statement.
fn parse_statement<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Stmt> {
    match peek_kind(input) {
        TokenKind::Pub => parse_pub_item(input, depth),
        TokenKind::Use => parse_use_stmt(input, false).map(Stmt::Use),
        TokenKind::Mod => parse_mod_stmt(input, depth, false).map(Stmt::Mod),
        TokenKind::Let => parse_let_stmt(input, depth).map(Stmt::Let),
        TokenKind::Return => parse_return_stmt(input, depth).map(Stmt::Return),
        TokenKind::Fn if peek_at(input, 1) == TokenKind::Ident => {
            parse_fn_declaration(input, depth, false).map(Stmt::FuncDef)
        }
        TokenKind::Label
            if matches!(peek_at(input, 1), TokenKind::Colon)
                && matches!(
                    peek_at(input, 2),
                    TokenKind::Loop | TokenKind::While | TokenKind::For
                ) =>
        {
            parse_labeled_loop(input, depth)
        }
        TokenKind::Loop => parse_loop_stmt(input, depth, None).map(Stmt::Loop),
        TokenKind::While => parse_while_stmt(input, depth, None).map(Stmt::While),
        TokenKind::For => parse_for_stmt(input, depth, None).map(Stmt::For),
        TokenKind::Struct => parse_struct_decl(input, depth, false).map(Stmt::StructDecl),
        TokenKind::Enum => parse_enum_decl(input, depth, false).map(Stmt::EnumDecl),
        TokenKind::Trait => parse_trait_decl(input, depth, false).map(Stmt::TraitDecl),
        TokenKind::Impl => parse_impl_block(input, depth).map(Stmt::ImplBlock),
        TokenKind::Ident if is_compound_assign(peek_at(input, 1)) => {
            parse_compound_assignment(input, depth).map(Stmt::ReAssign)
        }
        TokenKind::Ident if peek_at(input, 1) == TokenKind::Assign => {
            parse_assignment(input, depth).map(Stmt::ReAssign)
        }
        _ => parse_expression_stmt(input, depth).map(Stmt::Expr),
    }
}

/// Parses a `pub`-prefixed item.
fn parse_pub_item<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Stmt> {
    any.parse_next(input)?; // consume `pub`
    match peek_kind(input) {
        TokenKind::Fn if peek_at(input, 1) == TokenKind::Ident => {
            parse_fn_declaration(input, depth, true).map(Stmt::FuncDef)
        }
        TokenKind::Struct => parse_struct_decl(input, depth, true).map(Stmt::StructDecl),
        TokenKind::Enum => parse_enum_decl(input, depth, true).map(Stmt::EnumDecl),
        TokenKind::Trait => parse_trait_decl(input, depth, true).map(Stmt::TraitDecl),
        TokenKind::Mod => parse_mod_stmt(input, depth, true).map(Stmt::Mod),
        TokenKind::Use => parse_use_stmt(input, true).map(Stmt::Use),
        _ => Err(ErrMode::Backtrack(ContextError::new())),
    }
}

/// Parses a `use` statement:
/// `use foo::bar;`, `use foo::bar::{baz, qux};`, or `use foo;`.
fn parse_use_stmt<'a>(input: &mut &'a [Token<'a>], is_public: bool) -> ParseResult<UseStmt> {
    let start = expect(input, TokenKind::Use)?.span;
    let first = expect(input, TokenKind::Ident)?;
    let mut path = vec![first.literal.to_string()];
    let mut end = first.span;

    while peek_kind(input) == TokenKind::PathSep {
        any.parse_next(input)?; // consume `::`

        if peek_kind(input) == TokenKind::LBrace {
            any.parse_next(input)?; // consume `{`
            let items = parse_use_item_list(input)?;
            end = expect(input, TokenKind::RBrace)?.span;
            eat(input, TokenKind::Semicolon);
            return Ok(UseStmt {
                path,
                items: Some(items),
                is_public,
                span: start.merge(end),
            });
        }

        let seg = expect(input, TokenKind::Ident)?;
        end = seg.span;
        path.push(seg.literal.to_string());
    }

    eat(input, TokenKind::Semicolon);
    Ok(UseStmt {
        path,
        items: None,
        is_public,
        span: start.merge(end),
    })
}

/// Parses the item list in a grouped `use` import: `{foo, bar, baz}`.
/// Called after `{` has been consumed.
fn parse_use_item_list<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Vec<String>> {
    let mut items = Vec::new();

    if peek_kind(input) == TokenKind::RBrace {
        return Ok(items);
    }
    items.push(expect(input, TokenKind::Ident)?.literal.to_string());

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?; // consume `,`
        if peek_kind(input) == TokenKind::RBrace {
            break;
        }
        items.push(expect(input, TokenKind::Ident)?.literal.to_string());
    }
    Ok(items)
}

/// Parses a `mod` declaration: `mod foo;` (external) or `mod foo { ... }` (inline).
fn parse_mod_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<ModStmt> {
    let start = expect(input, TokenKind::Mod)?.span;
    let name_tok = expect(input, TokenKind::Ident)?;
    let name = name_tok.literal.to_string();

    if peek_kind(input) == TokenKind::LBrace {
        any.parse_next(input)?; // consume `{`
        let mut body = Vec::new();
        while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
            body.push(parse_statement(input, depth)?);
        }
        let end = expect(input, TokenKind::RBrace)?.span;
        Ok(ModStmt {
            name,
            body: Some(body),
            is_public,
            span: start.merge(end),
        })
    } else {
        let end = name_tok.span;
        eat(input, TokenKind::Semicolon);
        Ok(ModStmt {
            name,
            body: None,
            is_public,
            span: start.merge(end),
        })
    }
}

/// Parses a `let` binding: `let [mut] <ident>[: <type>] = <expr>;`.
fn parse_let_stmt<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<LetStmt> {
    let start = expect(input, TokenKind::Let)?.span;
    let mutable = eat(input, TokenKind::Mut).is_some();
    let ident_tok = expect(input, TokenKind::Ident)?;
    let ident = ident_tok.literal.to_string();

    let type_annotation = if peek_kind(input) == TokenKind::Colon {
        any.parse_next(input)?; // consume `:`
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    expect(input, TokenKind::Assign)?;
    let value = parse_expression(input, LOWEST, depth)?;
    let end = eat(input, TokenKind::Semicolon).map_or_else(|| value.span(), |t| t.span);

    Ok(LetStmt {
        ident,
        mutable,
        type_annotation,
        value,
        span: start.merge(end),
    })
}

/// Parses a `return` statement: `return <expr>;`.
fn parse_return_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<ReturnStmt> {
    let start = expect(input, TokenKind::Return)?.span;
    let value = parse_expression(input, LOWEST, depth)?;
    let end = eat(input, TokenKind::Semicolon).map_or_else(|| value.span(), |t| t.span);

    Ok(ReturnStmt {
        value,
        span: start.merge(end),
    })
}

/// Parses an expression used as a statement.
fn parse_expression_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<ExprStmt> {
    let value = parse_expression(input, LOWEST, depth)?;
    let span = if peek_kind(input) == TokenKind::Semicolon {
        let s = value.span().merge(input[0].span);
        any.parse_next(input)?;
        s
    } else {
        value.span()
    };
    Ok(ExprStmt { value, span })
}

/// Desugars `x op= expr` into `x = x op expr` as a [`ReAssignStmt`].
fn parse_compound_assignment<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<ReAssignStmt> {
    let ident_tok: Token<'a> = any.parse_next(input)?;
    let start = ident_tok.span;
    let ident = ident_tok.literal.to_string();

    let op_tok: Token<'a> = any.parse_next(input)?;
    let operator = match op_tok.kind {
        TokenKind::AddAssign => "+",
        TokenKind::SubAssign => "-",
        TokenKind::MulAssign => "*",
        TokenKind::DivAssign => "/",
        TokenKind::RemAssign => "%",
        _ => unreachable!(),
    };

    let rhs = parse_expression(input, LOWEST, depth)?;
    let end = rhs.span();

    let lhs = Box::new(Expr::Ident(Ident {
        value: ident.clone(),
        span: start,
    }));
    let value = Expr::Infix(InfixExpr {
        lhs,
        operator: operator.to_string(),
        rhs: Box::new(rhs),
        span: start.merge(end),
    });

    eat(input, TokenKind::Semicolon);
    Ok(ReAssignStmt {
        ident,
        value,
        span: start.merge(end),
    })
}

/// Parses a plain reassignment: `x = expr;`.
fn parse_assignment<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<ReAssignStmt> {
    let ident_tok: Token<'a> = any.parse_next(input)?;
    let start = ident_tok.span;
    let ident = ident_tok.literal.to_string();

    expect(input, TokenKind::Assign)?;
    let value = parse_expression(input, LOWEST, depth)?;
    let end = eat(input, TokenKind::Semicolon).map_or_else(|| value.span(), |t| t.span);

    Ok(ReAssignStmt {
        ident,
        value,
        span: start.merge(end),
    })
}

/// Parses an expression with a minimum binding power of `min_bp`.
///
/// Uses Pratt parsing: first parse a prefix subexpression, then while the
/// next token's precedence exceeds `min_bp`, consume it as an infix operator.
fn parse_expression<'a>(
    input: &mut &'a [Token<'a>],
    min_bp: u8,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    depth.set(depth.get() + 1);
    if depth.get() > MAX_NESTING_DEPTH {
        depth.set(depth.get() - 1);
        return Err(ErrMode::Backtrack(ContextError::new()));
    }
    let result = parse_expression_inner(input, min_bp, depth);
    depth.set(depth.get() - 1);
    result
}

fn parse_expression_inner<'a>(
    input: &mut &'a [Token<'a>],
    min_bp: u8,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let mut expr = match peek_kind(input) {
        TokenKind::Ident => parse_identifier(input, depth)?,
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
        | TokenKind::Usize => parse_int(input)?,
        TokenKind::String => parse_string_literal(input)?,
        TokenKind::Bang | TokenKind::Minus => parse_prefix_expression(input, depth)?,
        TokenKind::True | TokenKind::False => parse_boolean(input)?,
        TokenKind::LParen => parse_grouped_expression(input, depth)?,
        TokenKind::If => parse_conditional_expression(input, depth)?,
        TokenKind::Fn => parse_lambda(input, depth)?,
        TokenKind::Macro => parse_macro(input, depth)?,
        TokenKind::LBracket => parse_array_literal(input, depth)?,
        TokenKind::LBrace => parse_hash_literal(input, depth)?,
        TokenKind::Break => parse_break_expression(input, depth)?,
        TokenKind::Continue => parse_continue_expression(input)?,
        TokenKind::Match => parse_match_expression(input, depth)?,
        TokenKind::SelfValue => {
            let tok: Token<'a> = any.parse_next(input)?;
            Expr::Ident(Ident {
                value: "self".to_string(),
                span: tok.span,
            })
        }
        _ => return Err(ErrMode::Backtrack(ContextError::new())),
    };

    loop {
        let next = peek_kind(input);
        if next == TokenKind::Semicolon {
            break;
        }
        let Some(bp) = Precedence::get(&next) else {
            break;
        };
        if min_bp >= bp {
            break;
        }

        let op_tok: Token<'a> = any.parse_next(input)?;
        expr = match next {
            TokenKind::LParen => parse_call_expression(input, expr, depth)?,
            TokenKind::LBracket => parse_index_expression(input, expr, depth)?,
            TokenKind::As => parse_cast_expression(input, expr)?,
            TokenKind::Dot => parse_field_or_method_call(input, expr, depth)?,
            TokenKind::DotDot => parse_range_expression(input, expr, false, depth)?,
            TokenKind::DotDotEqual => parse_range_expression(input, expr, true, depth)?,
            _ => {
                let operator = op_tok.literal.to_string();
                let rhs = Box::new(parse_expression(input, bp, depth)?);
                let span = expr.span().merge(rhs.span());
                Expr::Infix(InfixExpr {
                    lhs: Box::new(expr),
                    operator,
                    rhs,
                    span,
                })
            }
        };
    }

    Ok(expr)
}

/// Parses an identifier, path expression (`Enum::Variant`), or struct literal (`Name { ... }`).
fn parse_identifier<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Expr> {
    let tok: Token<'a> = any.parse_next(input)?;
    let name = tok.literal.to_string();
    let start = tok.span;

    if peek_kind(input) == TokenKind::PathSep {
        return parse_path_expression(input, name, start);
    }

    if peek_kind(input) == TokenKind::LBrace && name.starts_with(char::is_uppercase) {
        return parse_struct_literal(input, name, start, depth);
    }

    Ok(Expr::Ident(Ident {
        value: name,
        span: start,
    }))
}

/// Parses a path expression: `Enum::Variant`, `Mod::Item::Sub`.
/// Called after the first identifier has been consumed. `start` is its span.
fn parse_path_expression<'a>(
    input: &mut &'a [Token<'a>],
    first: String,
    start: Span,
) -> ParseResult<Expr> {
    let mut segments = vec![first];
    let mut end = start;

    while peek_kind(input) == TokenKind::PathSep {
        any.parse_next(input)?; // consume `::`
        let seg = expect(input, TokenKind::Ident)?;
        end = seg.span;
        segments.push(seg.literal.to_string());
    }

    Ok(Expr::PathExpr(PathExpr {
        segments,
        span: start.merge(end),
    }))
}

/// Parses a struct literal: `Name { field: value, ... }` or with functional
/// update syntax: `Name { field: value, ..base }`.
fn parse_struct_literal<'a>(
    input: &mut &'a [Token<'a>],
    name: String,
    start: Span,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    any.parse_next(input)?; // consume `{`
    let mut fields = Vec::new();
    let mut base = None;

    while peek_kind(input) != TokenKind::RBrace {
        if peek_kind(input) == TokenKind::DotDot {
            any.parse_next(input)?; // consume `..`
            base = Some(Box::new(parse_expression(input, LOWEST, depth)?));
            if peek_kind(input) == TokenKind::Comma {
                any.parse_next(input)?;
            }
            break;
        }

        let field_name = expect(input, TokenKind::Ident)?.literal.to_string();
        expect(input, TokenKind::Colon)?;
        let value = parse_expression(input, LOWEST, depth)?;
        fields.push((field_name, value));

        if peek_kind(input) != TokenKind::RBrace {
            expect(input, TokenKind::Comma)?;
        }
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(Expr::StructLit(StructLitExpr {
        name,
        fields,
        base,
        span: start.merge(end),
    }))
}

/// Parses a numeric integer literal with radix detection and range validation.
fn parse_int<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Expr> {
    let tok: Token<'a> = any.parse_next(input)?;
    let literal = tok.literal;
    let span = tok.span;

    macro_rules! parse_int_type {
        ($rust_ty:ty, $kind:expr) => {{
            let (radix, value) = if let Some(bin) = literal
                .strip_prefix("0b")
                .or_else(|| literal.strip_prefix("0B"))
            {
                <$rust_ty>::from_str_radix(bin, 2)
                    .ok()
                    .map(|v| (Radix::Bin, v as i128))
            } else if let Some(oct) = literal
                .strip_prefix("0o")
                .or_else(|| literal.strip_prefix("0O"))
            {
                <$rust_ty>::from_str_radix(oct, 8)
                    .ok()
                    .map(|v| (Radix::Oct, v as i128))
            } else if let Some(hex) = literal
                .strip_prefix("0x")
                .or_else(|| literal.strip_prefix("0X"))
            {
                <$rust_ty>::from_str_radix(hex, 16)
                    .ok()
                    .map(|v| (Radix::Hex, v as i128))
            } else {
                literal
                    .parse::<$rust_ty>()
                    .ok()
                    .map(|v| (Radix::Dec, v as i128))
            }
            .ok_or_else(|| ErrMode::Backtrack(ContextError::new()))?;

            Expr::Number(Number {
                kind: $kind,
                value,
                radix,
                span,
            })
        }};
    }

    let expr = match tok.kind {
        TokenKind::I8 => parse_int_type!(i8, NumberKind::I8),
        TokenKind::I16 => parse_int_type!(i16, NumberKind::I16),
        TokenKind::I32 => parse_int_type!(i32, NumberKind::I32),
        TokenKind::I64 => parse_int_type!(i64, NumberKind::I64),
        TokenKind::I128 => parse_int_type!(i128, NumberKind::I128),
        TokenKind::Isize => parse_int_type!(isize, NumberKind::Isize),
        TokenKind::U8 => parse_int_type!(u8, NumberKind::U8),
        TokenKind::U16 => parse_int_type!(u16, NumberKind::U16),
        TokenKind::U32 => parse_int_type!(u32, NumberKind::U32),
        TokenKind::U64 => parse_int_type!(u64, NumberKind::U64),
        TokenKind::U128 => parse_int_type!(u128, NumberKind::U128),
        TokenKind::Usize => parse_int_type!(usize, NumberKind::Usize),
        _ => unreachable!(),
    };

    Ok(expr)
}

/// Parses a string literal.
fn parse_string_literal<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Expr> {
    let tok: Token<'a> = any.parse_next(input)?;
    Ok(Expr::Str(Str {
        value: tok.literal.to_owned(),
        span: tok.span,
    }))
}

/// Parses a prefix expression: `!expr` or `-expr`.
fn parse_prefix_expression<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let tok: Token<'a> = any.parse_next(input)?;
    let start = tok.span;
    let operator = tok.literal.to_string();
    let operand = Box::new(parse_expression(input, PREFIX, depth)?);
    let span = start.merge(operand.span());
    Ok(Expr::Prefix(PrefixExpr {
        operator,
        operand,
        span,
    }))
}

/// Parses a boolean literal (`true` or `false`).
fn parse_boolean<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Expr> {
    let tok: Token<'a> = any.parse_next(input)?;
    Ok(Expr::Bool(Bool {
        value: tok.kind == TokenKind::True,
        span: tok.span,
    }))
}

/// Parses a parenthesized expression: `(expr)`.
fn parse_grouped_expression<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    any.parse_next(input)?; // consume `(`
    let expr = parse_expression(input, LOWEST, depth)?;
    expect(input, TokenKind::RParen)?;
    Ok(expr)
}

/// Parses an `if` expression with optional `else` / `else if` chains.
fn parse_conditional_expression<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::If)?.span;
    expect(input, TokenKind::LParen)?;
    let condition = Box::new(parse_expression(input, LOWEST, depth)?);
    expect(input, TokenKind::RParen)?;
    expect(input, TokenKind::LBrace)?;
    let consequence = parse_block(input, depth)?;

    let alternative = if peek_kind(input) == TokenKind::Else {
        any.parse_next(input)?; // consume `else`
        if peek_kind(input) == TokenKind::If {
            let nested_cond = parse_conditional_expression(input, depth)?;
            let nested_span = nested_cond.span();
            Some(BlockStmt {
                statements: vec![Stmt::Expr(ExprStmt {
                    value: nested_cond,
                    span: nested_span,
                })],
                span: nested_span,
            })
        } else {
            expect(input, TokenKind::LBrace)?;
            Some(parse_block(input, depth)?)
        }
    } else {
        None
    };

    let end = alternative
        .as_ref()
        .map_or(consequence.span, |alt| alt.span);
    Ok(Expr::Cond(CondExpr {
        condition,
        consequence,
        alternative,
        span: start.merge(end),
    }))
}

/// Parses an anonymous function expression: `fn<T>(params) -> ret { body }`.
fn parse_lambda<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::Fn)?.span;

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?; // consume `<`
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LParen)?;
    let params = parse_typed_parameters(input)?;

    let return_type = if peek_kind(input) == TokenKind::Arrow {
        any.parse_next(input)?; // consume `->`
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;

    Ok(Expr::Lambda(Lambda {
        params,
        generic_params,
        return_type,
        body,
        span: start.merge(end),
    }))
}

/// Parses a macro literal: `macro(params) { body }`.
fn parse_macro<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::Macro)?.span;
    expect(input, TokenKind::LParen)?;
    let params = parse_parameters(input)?;
    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;

    Ok(Expr::Macro(Macro {
        params,
        body,
        span: start.merge(end),
    }))
}

/// Parses an array literal: `[expr, expr, ...]`.
fn parse_array_literal<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::LBracket)?.span;
    let (elements, end) = parse_delimited_exprs(input, TokenKind::RBracket, depth)?;
    Ok(Expr::Array(Array {
        elements,
        span: start.merge(end),
    }))
}

/// Parses a hash/map literal: `{ key: value, ... }`.
fn parse_hash_literal<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::LBrace)?.span;
    let mut pairs = Vec::new();

    while peek_kind(input) != TokenKind::RBrace {
        let key = parse_expression(input, LOWEST, depth)?;
        expect(input, TokenKind::Colon)?;
        let value = parse_expression(input, LOWEST, depth)?;
        pairs.push((key, value));

        if peek_kind(input) != TokenKind::RBrace {
            expect(input, TokenKind::Comma)?;
        }
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(Expr::Map(Map {
        pairs,
        span: start.merge(end),
    }))
}

/// Parses a `break` expression: `break`, `break <value>`, or `break 'label [<value>]`.
fn parse_break_expression<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::Break)?.span;

    let label = if peek_kind(input) == TokenKind::Label {
        let tok: Token<'a> = any.parse_next(input)?;
        Some(tok.literal.to_string())
    } else {
        None
    };

    let value = if peek_kind(input) != TokenKind::Semicolon
        && peek_kind(input) != TokenKind::RBrace
        && peek_kind(input) != TokenKind::Eof
    {
        Some(Box::new(parse_expression(input, LOWEST, depth)?))
    } else {
        None
    };
    let end = value.as_ref().map_or(start, |v| v.span());
    Ok(Expr::Break(BreakExpr {
        label,
        value,
        span: start.merge(end),
    }))
}

/// Parses a `continue` expression: `continue` or `continue 'label`.
fn parse_continue_expression<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::Continue)?.span;

    let label = if peek_kind(input) == TokenKind::Label {
        let tok: Token<'a> = any.parse_next(input)?;
        Some(tok.literal.to_string())
    } else {
        None
    };

    Ok(Expr::Continue(ContinueExpr { label, span: start }))
}

/// Parses a `match` expression: `match scrutinee { arms }`.
fn parse_match_expression<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = expect(input, TokenKind::Match)?.span;
    let scrutinee = Box::new(parse_expression(input, LOWEST, depth)?);
    expect(input, TokenKind::LBrace)?;

    let mut arms = Vec::new();
    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        arms.push(parse_match_arm(input, depth)?);
        eat(input, TokenKind::Comma);
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(Expr::Match(MatchExpr {
        scrutinee,
        arms,
        span: start.merge(end),
    }))
}

/// Parses a single `match` arm: `pattern [if guard] => body`.
fn parse_match_arm<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<MatchArm> {
    let pattern = parse_pattern(input, depth)?;
    let start = pattern.span();

    let guard = if peek_kind(input) == TokenKind::If {
        any.parse_next(input)?; // consume `if`
        Some(Box::new(parse_expression(input, LOWEST, depth)?))
    } else {
        None
    };

    expect(input, TokenKind::FatArrow)?;

    let body = if peek_kind(input) == TokenKind::LBrace {
        any.parse_next(input)?; // consume `{`
        let block = parse_block(input, depth)?;
        let body_span = block.span;
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
        parse_expression(input, LOWEST, depth)?
    };

    let end = body.span();
    Ok(MatchArm {
        pattern,
        guard,
        body,
        span: start.merge(end),
    })
}

/// Parses the arguments of a function call after `(` has been consumed.
fn parse_call_expression<'a>(
    input: &mut &'a [Token<'a>],
    func: Expr,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = func.span();
    let arguments = parse_delimited_exprs(input, TokenKind::RParen, depth)?;
    let end = arguments.1;
    Ok(Expr::Call(CallExpr {
        function: Box::new(func),
        arguments: arguments.0,
        span: start.merge(end),
    }))
}

/// Parses an index expression after `[` has been consumed.
fn parse_index_expression<'a>(
    input: &mut &'a [Token<'a>],
    expr: Expr,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = expr.span();
    let index = Box::new(parse_expression(input, LOWEST, depth)?);
    let end = expect(input, TokenKind::RBracket)?.span;
    Ok(Expr::Index(IndexExpr {
        expr: Box::new(expr),
        index,
        span: start.merge(end),
    }))
}

/// Parses a type cast after `as` has been consumed.
fn parse_cast_expression<'a>(input: &mut &'a [Token<'a>], lhs: Expr) -> ParseResult<Expr> {
    let start = lhs.span();
    let type_tok = expect(input, TokenKind::Ident)?;
    let end = type_tok.span;
    let target: TypeAnnotation = type_tok
        .literal
        .parse()
        .map_err(|_| ErrMode::Backtrack(ContextError::new()))?;

    Ok(Expr::Cast(CastExpr {
        expr: Box::new(lhs),
        target,
        span: start.merge(end),
    }))
}

/// Parses a field access or method call after `.` has been consumed.
fn parse_field_or_method_call<'a>(
    input: &mut &'a [Token<'a>],
    object: Expr,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start = object.span();
    let member_tok = any
        .verify(|t: &Token<'_>| {
            matches!(
                t.kind,
                TokenKind::Ident | TokenKind::SelfValue | TokenKind::SelfType
            )
        })
        .parse_next(input)?;
    let member = member_tok.literal.to_string();

    if peek_kind(input) == TokenKind::LParen {
        any.parse_next(input)?; // consume `(`
        let (arguments, end) = parse_delimited_exprs(input, TokenKind::RParen, depth)?;
        Ok(Expr::MethodCall(MethodCallExpr {
            object: Box::new(object),
            method: member,
            arguments,
            receiver: None,
            span: start.merge(end),
        }))
    } else {
        let end = member_tok.span;
        Ok(Expr::FieldAccess(FieldAccessExpr {
            object: Box::new(object),
            field: member,
            span: start.merge(end),
        }))
    }
}

/// Parses a range expression after `..` or `..=` has been consumed.
fn parse_range_expression<'a>(
    input: &mut &'a [Token<'a>],
    start_expr: Expr,
    inclusive: bool,
    depth: &Cell<usize>,
) -> ParseResult<Expr> {
    let start_span = start_expr.span();
    let bp = if inclusive {
        Precedence::get(&TokenKind::DotDotEqual).unwrap_or(LOWEST)
    } else {
        Precedence::get(&TokenKind::DotDot).unwrap_or(LOWEST)
    };
    let end_expr = parse_expression(input, bp, depth)?;
    let span = start_span.merge(end_expr.span());
    Ok(Expr::Range(RangeExpr {
        start: Box::new(start_expr),
        end: Box::new(end_expr),
        inclusive,
        span,
    }))
}

/// Parses a pattern, including or-patterns (`A | B`).
fn parse_pattern<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Pattern> {
    let base = parse_single_pattern(input, depth)?;

    if peek_kind(input) != TokenKind::Or {
        return Ok(base);
    }

    let start = base.span();
    let mut alternatives = vec![base];
    while peek_kind(input) == TokenKind::Or {
        any.parse_next(input)?; // consume `|`
        alternatives.push(parse_single_pattern(input, depth)?);
    }
    let end = alternatives.last().map_or(start, |p| p.span());
    Ok(Pattern::Or(alternatives, start.merge(end)))
}

/// Parses a single (non-or) pattern.
fn parse_single_pattern<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<Pattern> {
    match peek_kind(input) {
        TokenKind::Ident if input.first().is_some_and(|t| t.literal == "_") => {
            let tok: Token<'a> = any.parse_next(input)?;
            Ok(Pattern::Wildcard(tok.span))
        }

        TokenKind::True => {
            let tok: Token<'a> = any.parse_next(input)?;
            Ok(Pattern::Literal(Box::new(Expr::Bool(Bool {
                value: true,
                span: tok.span,
            }))))
        }
        TokenKind::False => {
            let tok: Token<'a> = any.parse_next(input)?;
            Ok(Pattern::Literal(Box::new(Expr::Bool(Bool {
                value: false,
                span: tok.span,
            }))))
        }

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
            let int = parse_int(input)?;
            Ok(Pattern::Literal(Box::new(int)))
        }

        TokenKind::Minus => {
            let prefix = parse_prefix_expression(input, depth)?;
            Ok(Pattern::Literal(Box::new(prefix)))
        }

        TokenKind::String => {
            let s = parse_string_literal(input)?;
            Ok(Pattern::Literal(Box::new(s)))
        }

        TokenKind::Ident | TokenKind::SelfType => {
            let tok: Token<'a> = any.parse_next(input)?;
            let name = tok.literal.to_string();
            let span = tok.span;

            match peek_kind(input) {
                TokenKind::LParen => {
                    any.parse_next(input)?; // consume `(`
                    let fields = parse_pattern_list(input, TokenKind::RParen, depth)?;
                    let end = fields.1;
                    Ok(Pattern::TupleStruct {
                        path: name,
                        fields: fields.0,
                        span: span.merge(end),
                    })
                }
                TokenKind::LBrace => {
                    any.parse_next(input)?; // consume `{`
                    let (fields, end) = parse_pattern_fields(input, depth)?;
                    Ok(Pattern::Struct {
                        path: name,
                        fields,
                        span: span.merge(end),
                    })
                }
                _ => Ok(Pattern::Ident(name, span)),
            }
        }

        _ => Err(ErrMode::Backtrack(ContextError::new())),
    }
}

/// Parses a comma-separated list of patterns terminated by `end_token`.
/// Returns patterns and the closing delimiter's span.
fn parse_pattern_list<'a>(
    input: &mut &'a [Token<'a>],
    end_token: TokenKind,
    depth: &Cell<usize>,
) -> ParseResult<(Vec<Pattern>, Span)> {
    let mut patterns = Vec::new();

    if peek_kind(input) == end_token {
        let end = expect(input, end_token)?.span;
        return Ok((patterns, end));
    }

    patterns.push(parse_pattern(input, depth)?);

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?; // consume `,`
        if peek_kind(input) == end_token {
            break;
        }
        patterns.push(parse_pattern(input, depth)?);
    }

    let end = expect(input, end_token)?.span;
    Ok((patterns, end))
}

/// Parses the field list inside a struct pattern: `{ field, field: pat, ... }`.
/// Called after `{` has been consumed. Returns fields and closing `}` span.
fn parse_pattern_fields<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<(Vec<PatternField>, Span)> {
    let mut fields = Vec::new();

    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        let field_tok = expect(input, TokenKind::Ident)?;
        let name = field_tok.literal.to_string();

        let pattern = if peek_kind(input) == TokenKind::Colon {
            any.parse_next(input)?; // consume `:`
            Some(Box::new(parse_pattern(input, depth)?))
        } else {
            None
        };

        fields.push(PatternField {
            name,
            pattern,
            span: field_tok.span,
        });

        if peek_kind(input) != TokenKind::RBrace {
            expect(input, TokenKind::Comma)?;
        }
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok((fields, end))
}

/// Parses a labeled loop: `'label: loop/while/for { ... }`.
fn parse_labeled_loop<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<Stmt> {
    let label_tok: Token<'a> = any.parse_next(input)?;
    let label = label_tok.literal.to_string();
    expect(input, TokenKind::Colon)?;
    match peek_kind(input) {
        TokenKind::Loop => parse_loop_stmt(input, depth, Some(label)).map(Stmt::Loop),
        TokenKind::While => parse_while_stmt(input, depth, Some(label)).map(Stmt::While),
        TokenKind::For => parse_for_stmt(input, depth, Some(label)).map(Stmt::For),
        _ => Err(ErrMode::Backtrack(ContextError::new())),
    }
}

/// Parses an infinite loop: `loop { body }` or `'label: loop { body }`.
fn parse_loop_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    label: Option<String>,
) -> ParseResult<LoopStmt> {
    let start = expect(input, TokenKind::Loop)?.span;
    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;
    Ok(LoopStmt {
        label,
        body,
        span: start.merge(end),
    })
}

/// Parses a `while` loop: `while (condition) { body }` or
/// `'label: while (condition) { body }`.
fn parse_while_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    label: Option<String>,
) -> ParseResult<WhileStmt> {
    let start = expect(input, TokenKind::While)?.span;
    expect(input, TokenKind::LParen)?;
    let condition = Box::new(parse_expression(input, LOWEST, depth)?);
    expect(input, TokenKind::RParen)?;
    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;
    Ok(WhileStmt {
        label,
        condition,
        body,
        span: start.merge(end),
    })
}

/// Parses a `for` loop: `for ident in iterable { body }` or
/// `'label: for ident in iterable { body }`.
fn parse_for_stmt<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    label: Option<String>,
) -> ParseResult<ForStmt> {
    let start = expect(input, TokenKind::For)?.span;
    let ident = expect(input, TokenKind::Ident)?.literal.to_string();
    expect(input, TokenKind::In)?;
    let iterable = Box::new(parse_expression(input, LOWEST, depth)?);
    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;
    Ok(ForStmt {
        label,
        ident,
        iterable,
        body,
        span: start.merge(end),
    })
}

/// Parses a struct declaration: `struct Name<T> { field: Type, ... }`.
fn parse_struct_decl<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<StructDecl> {
    let _ = depth;
    let start = expect(input, TokenKind::Struct)?.span;
    let name = expect(input, TokenKind::Ident)?.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?; // consume `<`
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LBrace)?;
    let mut fields = Vec::new();
    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        fields.push(parse_struct_field(input)?);
        if peek_kind(input) != TokenKind::RBrace {
            expect(input, TokenKind::Comma)?;
        }
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(StructDecl {
        name,
        generic_params,
        fields,
        is_public,
        span: start.merge(end),
    })
}

/// Parses a single struct field: `[pub] name: Type`.
fn parse_struct_field<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<StructField> {
    let start_tok = input
        .first()
        .ok_or(ErrMode::Backtrack(ContextError::new()))?;
    let start = start_tok.span;
    let is_public = eat(input, TokenKind::Pub).is_some();
    let name = expect(input, TokenKind::Ident)?.literal.to_string();
    expect(input, TokenKind::Colon)?;
    let ty = parse_type_expr(input)?;
    let end = ty.span();

    Ok(StructField {
        name,
        ty,
        is_public,
        span: start.merge(end),
    })
}

/// Parses an enum declaration: `enum Name<T> { Variant, ... }`.
fn parse_enum_decl<'a>(
    input: &mut &'a [Token<'a>],
    _depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<EnumDecl> {
    let start = expect(input, TokenKind::Enum)?.span;
    let name = expect(input, TokenKind::Ident)?.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LBrace)?;
    let mut variants = Vec::new();
    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        variants.push(parse_enum_variant(input)?);
        if peek_kind(input) != TokenKind::RBrace {
            expect(input, TokenKind::Comma)?;
        }
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(EnumDecl {
        name,
        generic_params,
        variants,
        is_public,
        span: start.merge(end),
    })
}

/// Parses a single enum variant: `Name`, `Name(T)`, or `Name { field: T }`.
fn parse_enum_variant<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<EnumVariant> {
    let name_tok = expect(input, TokenKind::Ident)?;
    let start = name_tok.span;
    let name = name_tok.literal.to_string();

    let (kind, end) = if peek_kind(input) == TokenKind::LParen {
        any.parse_next(input)?; // consume `(`
        let mut types = Vec::new();
        if peek_kind(input) != TokenKind::RParen {
            types.push(parse_type_expr(input)?);
            while peek_kind(input) == TokenKind::Comma {
                any.parse_next(input)?;
                if peek_kind(input) == TokenKind::RParen {
                    break;
                }
                types.push(parse_type_expr(input)?);
            }
        }
        let end = expect(input, TokenKind::RParen)?.span;
        (EnumVariantKind::Tuple(types), end)
    } else if peek_kind(input) == TokenKind::LBrace {
        any.parse_next(input)?; // consume `{`
        let mut fields = Vec::new();
        while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
            fields.push(parse_struct_field(input)?);
            if peek_kind(input) != TokenKind::RBrace {
                expect(input, TokenKind::Comma)?;
            }
        }
        let end = expect(input, TokenKind::RBrace)?.span;
        (EnumVariantKind::Struct(fields), end)
    } else {
        (EnumVariantKind::Unit, start)
    };

    Ok(EnumVariant {
        name,
        kind,
        span: start.merge(end),
    })
}

/// Parses a trait declaration: `trait Name<T> { fn method(...); }`.
fn parse_trait_decl<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<TraitDecl> {
    let start = expect(input, TokenKind::Trait)?.span;
    let name = expect(input, TokenKind::Ident)?.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LBrace)?;
    let mut methods = Vec::new();
    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        expect(input, TokenKind::Fn)?;
        methods.push(parse_trait_method(input, depth)?);
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(TraitDecl {
        name,
        generic_params,
        methods,
        is_public,
        span: start.merge(end),
    })
}

/// Parses a trait method signature (with optional default body).
/// Called after `fn` has been consumed.
fn parse_trait_method<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<TraitMethod> {
    let name_tok = expect(input, TokenKind::Ident)?;
    let start = name_tok.span;
    let name = name_tok.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LParen)?;
    let params = parse_method_parameters(input)?;

    let return_type = if peek_kind(input) == TokenKind::Arrow {
        any.parse_next(input)?;
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    let (default_body, end) = if peek_kind(input) == TokenKind::LBrace {
        any.parse_next(input)?; // consume `{`
        let body = parse_block(input, depth)?;
        let end = body.span;
        (Some(body), end)
    } else {
        let end = eat(input, TokenKind::Semicolon)
            .map(|t| t.span)
            .unwrap_or_else(|| return_type.as_ref().map_or(start, |t| t.span()));
        (None, end)
    };

    Ok(TraitMethod {
        name,
        generic_params,
        params,
        return_type,
        default_body,
        span: start.merge(end),
    })
}

/// Parses an `impl` block: `impl [Trait for] Type { methods }`.
fn parse_impl_block<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
) -> ParseResult<ImplBlock> {
    let start = expect(input, TokenKind::Impl)?.span;

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    let first_type = parse_type_expr(input)?;

    let (trait_name, self_type) = if peek_kind(input) == TokenKind::For {
        any.parse_next(input)?; // consume `for`
        let self_ty = parse_type_expr(input)?;
        (Some(first_type), self_ty)
    } else {
        (None, first_type)
    };

    expect(input, TokenKind::LBrace)?;
    let mut methods = Vec::new();
    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        let is_public = eat(input, TokenKind::Pub).is_some();
        expect(input, TokenKind::Fn)?;
        methods.push(parse_impl_method(input, depth, is_public)?);
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(ImplBlock {
        trait_name,
        self_type,
        generic_params,
        methods,
        span: start.merge(end),
    })
}

/// Parses a method definition inside an `impl` block.
/// Called after `fn` has been consumed.
fn parse_impl_method<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<FuncDef> {
    let name_tok = expect(input, TokenKind::Ident)?;
    let start = name_tok.span;
    let name = name_tok.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LParen)?;
    let params = parse_method_parameters(input)?;

    let return_type = if peek_kind(input) == TokenKind::Arrow {
        any.parse_next(input)?;
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;

    Ok(FuncDef {
        name,
        params,
        generic_params,
        return_type,
        body,
        is_public,
        span: start.merge(end),
    })
}

/// Parses a named function declaration: `fn name<T>(params) -> ret { body }`.
fn parse_fn_declaration<'a>(
    input: &mut &'a [Token<'a>],
    depth: &Cell<usize>,
    is_public: bool,
) -> ParseResult<FuncDef> {
    let start = expect(input, TokenKind::Fn)?.span;
    let name = expect(input, TokenKind::Ident)?.literal.to_string();

    let generic_params = if peek_kind(input) == TokenKind::Less {
        any.parse_next(input)?;
        parse_generic_params(input)?
    } else {
        vec![]
    };

    expect(input, TokenKind::LParen)?;
    let params = parse_typed_parameters(input)?;

    let return_type = if peek_kind(input) == TokenKind::Arrow {
        any.parse_next(input)?;
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    expect(input, TokenKind::LBrace)?;
    let body = parse_block(input, depth)?;
    let end = body.span;

    Ok(FuncDef {
        name,
        params,
        generic_params,
        return_type,
        body,
        is_public,
        span: start.merge(end),
    })
}

/// Parses a block of statements: `stmt; stmt; ... }`.
/// Called after `{` has been consumed.
fn parse_block<'a>(input: &mut &'a [Token<'a>], depth: &Cell<usize>) -> ParseResult<BlockStmt> {
    let start = input.first().map_or(Span::ZERO, |t| t.span);
    let mut statements = Vec::new();

    while peek_kind(input) != TokenKind::RBrace && peek_kind(input) != TokenKind::Eof {
        statements.push(parse_statement(input, depth)?);
    }

    let end = expect(input, TokenKind::RBrace)?.span;
    Ok(BlockStmt {
        statements,
        span: start.merge(end),
    })
}

/// Parses untyped parameters: `(a, b, c)`.
/// Called after `(` has been consumed.
fn parse_parameters<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Vec<String>> {
    let mut identifiers = Vec::new();

    if peek_kind(input) == TokenKind::RParen {
        any.parse_next(input)?;
        return Ok(identifiers);
    }

    identifiers.push(expect(input, TokenKind::Ident)?.literal.to_string());

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?; // consume `,`
        if peek_kind(input) == TokenKind::RParen {
            break;
        }
        identifiers.push(expect(input, TokenKind::Ident)?.literal.to_string());
    }

    expect(input, TokenKind::RParen)?;
    Ok(identifiers)
}

/// Parses typed parameters: `(x: i64, y: bool)`.
/// Called after `(` has been consumed.
fn parse_typed_parameters<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Vec<TypedParam>> {
    let mut params = Vec::new();

    if peek_kind(input) == TokenKind::RParen {
        any.parse_next(input)?;
        return Ok(params);
    }

    params.push(parse_typed_param(input)?);

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?;
        if peek_kind(input) == TokenKind::RParen {
            break;
        }
        params.push(parse_typed_param(input)?);
    }

    expect(input, TokenKind::RParen)?;
    Ok(params)
}

/// Parses a single typed parameter: `name[: Type]`.
fn parse_typed_param<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<TypedParam> {
    let tok = expect(input, TokenKind::Ident)?;
    let start = tok.span;
    let name = tok.literal.to_string();

    let type_expr = if peek_kind(input) == TokenKind::Colon {
        any.parse_next(input)?;
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    let end = type_expr.as_ref().map_or(start, |t| t.span());
    Ok(TypedParam {
        name,
        type_expr,
        span: start.merge(end),
    })
}

/// Parses method parameters, accepting `self` as a valid parameter name.
/// Called after `(` has been consumed.
fn parse_method_parameters<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Vec<TypedParam>> {
    let mut params = Vec::new();

    if peek_kind(input) == TokenKind::RParen {
        any.parse_next(input)?;
        return Ok(params);
    }

    params.push(parse_method_param(input)?);

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?;
        if peek_kind(input) == TokenKind::RParen {
            break;
        }
        params.push(parse_method_param(input)?);
    }

    expect(input, TokenKind::RParen)?;
    Ok(params)
}

/// Parses a single method parameter, treating `self` and `Self` as valid names.
fn parse_method_param<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<TypedParam> {
    let tok: Token<'a> = any.parse_next(input)?;
    let start = tok.span;

    let name = match tok.kind {
        TokenKind::SelfValue => "self".to_string(),
        TokenKind::SelfType => "Self".to_string(),
        TokenKind::Ident => tok.literal.to_string(),
        _ => return Err(ErrMode::Backtrack(ContextError::new())),
    };

    let type_expr = if peek_kind(input) == TokenKind::Colon {
        any.parse_next(input)?;
        Some(parse_type_expr(input)?)
    } else {
        None
    };

    let end = type_expr.as_ref().map_or(start, |t| t.span());
    Ok(TypedParam {
        name,
        type_expr,
        span: start.merge(end),
    })
}

/// Parses a type expression: `i64`, `[T]`, `{K: V}`, `fn(A) -> B`, `Foo<T>`.
fn parse_type_expr<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<TypeExpr> {
    match peek_kind(input) {
        TokenKind::LBracket => {
            let start = expect(input, TokenKind::LBracket)?.span;
            let elem = parse_type_expr(input)?;
            let end = expect(input, TokenKind::RBracket)?.span;
            Ok(TypeExpr::Array(Box::new(elem), start.merge(end)))
        }
        TokenKind::LBrace => {
            let start = expect(input, TokenKind::LBrace)?.span;
            let key = parse_type_expr(input)?;
            expect(input, TokenKind::Colon)?;
            let value = parse_type_expr(input)?;
            let end = expect(input, TokenKind::RBrace)?.span;
            Ok(TypeExpr::Map(
                Box::new(key),
                Box::new(value),
                start.merge(end),
            ))
        }
        TokenKind::Fn => {
            let start = expect(input, TokenKind::Fn)?.span;
            expect(input, TokenKind::LParen)?;
            let mut param_types = Vec::new();
            if peek_kind(input) != TokenKind::RParen {
                param_types.push(parse_type_expr(input)?);
                while peek_kind(input) == TokenKind::Comma {
                    any.parse_next(input)?;
                    if peek_kind(input) == TokenKind::RParen {
                        break;
                    }
                    param_types.push(parse_type_expr(input)?);
                }
            }
            expect(input, TokenKind::RParen)?;
            expect(input, TokenKind::Arrow)?;
            let ret = parse_type_expr(input)?;
            let end = ret.span();
            Ok(TypeExpr::Fn(param_types, Box::new(ret), start.merge(end)))
        }
        TokenKind::Ident | TokenKind::SelfType => {
            let tok: Token<'a> = any.parse_next(input)?;
            let start = tok.span;
            let name = tok.literal.to_string();

            if peek_kind(input) == TokenKind::Less {
                any.parse_next(input)?; // consume `<`
                let mut args = vec![parse_type_expr(input)?];
                while peek_kind(input) == TokenKind::Comma {
                    any.parse_next(input)?;
                    if peek_kind(input) == TokenKind::Greater {
                        break;
                    }
                    args.push(parse_type_expr(input)?);
                }
                let end = expect(input, TokenKind::Greater)?.span;
                Ok(TypeExpr::Generic(name, args, start.merge(end)))
            } else {
                Ok(TypeExpr::Named(NamedType { name, span: start }))
            }
        }
        _ => Err(ErrMode::Backtrack(ContextError::new())),
    }
}

/// Parses generic type parameters: `T>`, `T, U>`, `T: Bound + Bound>`.
/// Called after `<` has been consumed.
fn parse_generic_params<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<Vec<GenericParam>> {
    let mut params = Vec::new();

    if peek_kind(input) == TokenKind::Greater {
        any.parse_next(input)?;
        return Ok(params);
    }

    params.push(parse_generic_param(input)?);

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?;
        if peek_kind(input) == TokenKind::Greater {
            break;
        }
        params.push(parse_generic_param(input)?);
    }

    expect(input, TokenKind::Greater)?;
    Ok(params)
}

/// Parses a single generic parameter: `T` or `T: Bound + Bound`.
fn parse_generic_param<'a>(input: &mut &'a [Token<'a>]) -> ParseResult<GenericParam> {
    let tok = expect(input, TokenKind::Ident)?;
    let start = tok.span;
    let name = tok.literal.to_string();

    let mut bounds = Vec::new();
    if peek_kind(input) == TokenKind::Colon {
        any.parse_next(input)?; // consume `:`
        let bound_tok = expect(input, TokenKind::Ident)?;
        bounds.push(TraitBound {
            name: bound_tok.literal.to_string(),
            span: bound_tok.span,
        });
        while peek_kind(input) == TokenKind::Plus {
            any.parse_next(input)?;
            let bound_tok = expect(input, TokenKind::Ident)?;
            bounds.push(TraitBound {
                name: bound_tok.literal.to_string(),
                span: bound_tok.span,
            });
        }
    }

    let end = bounds.last().map_or(start, |b| b.span);
    Ok(GenericParam {
        name,
        bounds,
        span: start.merge(end),
    })
}

/// Parses a comma-separated list of expressions terminated by `end_token`.
/// Consumes the closing delimiter. Returns expressions and closing span.
fn parse_delimited_exprs<'a>(
    input: &mut &'a [Token<'a>],
    end_token: TokenKind,
    depth: &Cell<usize>,
) -> ParseResult<(Vec<Expr>, Span)> {
    let mut list = Vec::new();

    if peek_kind(input) == end_token {
        let end = expect(input, end_token)?.span;
        return Ok((list, end));
    }

    list.push(parse_expression(input, LOWEST, depth)?);

    while peek_kind(input) == TokenKind::Comma {
        any.parse_next(input)?;
        if peek_kind(input) == end_token {
            break;
        }
        list.push(parse_expression(input, LOWEST, depth)?);
    }

    let end = expect(input, end_token)?.span;
    Ok((list, end))
}
