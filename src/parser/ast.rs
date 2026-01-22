//! AST definitions for Maat: statements, expressions, and program structure.

use std::fmt;

/// A complete program in Maat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// Statements: `let` bindings, `return` statements, expression
/// statements, or nested blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Let(LetStatement),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
    Block(BlockStatement),
}

/// A `let` binding: `let <ident> = <value>;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetStatement {
    pub ident: String,
    pub value: Expression,
}

/// A `return` statement: `return <value>;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStatement {
    pub value: Expression,
}

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionStatement {
    pub value: Expression,
}

/// A block of statements: `{ ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

/// All possible expression types in Maat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Identifier(String),
    Int64(Int64),
    Boolean(bool),
    Prefix(PrefixExpr),
    Infix(InfixExpr),
    Conditional(ConditionalExpr),
    Function(Function),
    Call(CallExpr),
}

/// Signed 64-bit integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int64 {
    pub value: i64,
}

/// Prefix expression: `!<expr>`, `-<expr>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixExpr {
    pub operator: String,
    pub operand: Box<Expression>,
}

/// Binary/infix expression: `<lhs> <operator> <rhs>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfixExpr {
    pub lhs: Box<Expression>,
    pub operator: String,
    pub rhs: Box<Expression>,
}

/// Conditional (if/else) expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalExpr {
    pub condition: Box<Expression>,
    pub consequence: BlockStatement,
    pub alternative: Option<BlockStatement>,
}

/// Function literal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStatement,
}

/// Function call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpr {
    pub function: Box<Expression>,
    pub arguments: Vec<Expression>,
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stmt in &self.statements {
            stmt.fmt(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Let(let_stmt) => let_stmt.fmt(f)?,
            Self::Return(ret_stmt) => ret_stmt.fmt(f)?,
            Self::Expression(expr_stmt) => expr_stmt.fmt(f)?,
            Self::Block(block_stmt) => block_stmt.fmt(f)?,
        }
        Ok(())
    }
}

impl fmt::Display for LetStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "let {} = {};", self.ident, self.value)
    }
}

impl fmt::Display for ReturnStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "return {};", self.value)
    }
}

impl fmt::Display for ExpressionStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stmt in &self.statements {
            stmt.fmt(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(ident) => ident.fmt(f),
            Self::Int64(int64) => int64.fmt(f),
            Self::Boolean(boolean) => boolean.fmt(f),
            Self::Prefix(prefix_expr) => prefix_expr.fmt(f),
            Self::Infix(infix_expr) => infix_expr.fmt(f),
            Self::Conditional(cond_expr) => cond_expr.fmt(f),
            Self::Function(func_lit) => func_lit.fmt(f),
            Self::Call(call_expr) => call_expr.fmt(f),
        }
    }
}

impl fmt::Display for Int64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl fmt::Display for PrefixExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}{})", self.operator, self.operand)
    }
}

impl fmt::Display for InfixExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} {} {})", self.lhs, self.operator, self.rhs)
    }
}

impl fmt::Display for ConditionalExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "if {} {{ {} }}", self.condition, self.consequence)?;

        if let Some(alternative) = &self.alternative {
            write!(f, " else {{ {alternative} }}")?;
        }

        Ok(())
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn({}) {{ {} }}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for CallExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({})",
            self.function,
            self.arguments
                .iter()
                .map(|expr| format!("{expr}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}
