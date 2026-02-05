//! AST definitions for Maat: statements, expressions, and program structure.

use std::fmt;

/// Top-level AST node wrapper for all language items.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression),
}

/// A complete program in Maat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// Statements: `let` bindings, `return` statements, expression
/// statements, or nested blocks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Statement {
    Let(LetStatement),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
    Block(BlockStatement),
}

/// A `let` binding: `let <ident> = <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LetStatement {
    pub ident: String,
    pub value: Expression,
}

/// A `return` statement: `return <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStatement {
    pub value: Expression,
}

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpressionStatement {
    pub value: Expression,
}

/// A block of statements: `{ ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

/// All possible expression types in Maat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expression {
    Identifier(String),

    // Signed integer types
    I8(I8),
    I16(I16),
    I32(I32),
    I64(I64),
    I128(I128),
    Isize(Isize),

    // Unsigned integer types
    U8(U8),
    U16(U16),
    U32(U32),
    U64(U64),
    U128(U128),
    Usize(Usize),

    // Floating-point types
    F32(F32),
    F64(F64),

    Boolean(bool),
    String(String),
    Array(ArrayLiteral),
    Index(IndexExpr),
    Hash(HashLiteral),
    Prefix(PrefixExpr),
    Infix(InfixExpr),
    Conditional(ConditionalExpr),
    Function(Function),
    Macro(MacroLiteral),
    Call(CallExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Radix {
    Bin,
    Oct,
    Dec,
    Hex,
}

/// Macro to generate integer type structs with radix support and native storage.
macro_rules! define_int_type {
    ($name:ident, $native:ty, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            pub radix: Radix,
            pub value: $native,
        }
    };
}

/// Macro to generate floating-point type structs with native storage (as raw bits).
macro_rules! define_float_type {
    ($name:ident, $native:ty, $bits:ty, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub $bits);

        impl From<$native> for $name {
            fn from(value: $native) -> Self {
                Self(<$native>::to_bits(value))
            }
        }

        impl From<$name> for $native {
            fn from(value: $name) -> Self {
                <$native>::from_bits(value.0)
            }
        }
    };
}

// Signed integer types
define_int_type!(I8, i8, "8-bit signed integer literal.");
define_int_type!(I16, i16, "16-bit signed integer literal.");
define_int_type!(I32, i32, "32-bit signed integer literal.");
define_int_type!(I64, i64, "64-bit signed integer literal.");
define_int_type!(I128, i128, "128-bit signed integer literal.");
define_int_type!(Isize, isize, "Pointer-sized signed integer literal.");

// Unsigned integer types
define_int_type!(U8, u8, "8-bit unsigned integer literal.");
define_int_type!(U16, u16, "16-bit unsigned integer literal.");
define_int_type!(U32, u32, "32-bit unsigned integer literal.");
define_int_type!(U64, u64, "64-bit unsigned integer literal.");
define_int_type!(U128, u128, "128-bit unsigned integer literal.");
define_int_type!(Usize, usize, "Pointer-sized unsigned integer literal.");

// Floating-point types
define_float_type!(F32, f32, u32, "32-bit floating-point literal.");
define_float_type!(F64, f64, u64, "64-bit floating-point literal.");

/// Arrays: `[expr, expr, ...]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayLiteral {
    pub elements: Vec<Expression>,
}

/// Indexing operation: `<lhs>[<index>]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExpr {
    pub expr: Box<Expression>,
    pub index: Box<Expression>,
}

/// Hash literal: `{ key: value, ... }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HashLiteral {
    pub pairs: Vec<(Expression, Expression)>,
}

/// Prefix expression: `!<expr>`, `-<expr>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixExpr {
    pub operator: String,
    pub operand: Box<Expression>,
}

/// Binary/infix expression: `<lhs> <operator> <rhs>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InfixExpr {
    pub lhs: Box<Expression>,
    pub operator: String,
    pub rhs: Box<Expression>,
}

/// Conditional (if/else) expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConditionalExpr {
    pub condition: Box<Expression>,
    pub consequence: BlockStatement,
    pub alternative: Option<BlockStatement>,
}

/// Function literal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStatement,
}

/// Macro literal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroLiteral {
    pub params: Vec<String>,
    pub body: BlockStatement,
}

/// Function call
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub function: Box<Expression>,
    pub arguments: Vec<Expression>,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(p) => p.fmt(f),
            Self::Statement(s) => s.fmt(f),
            Self::Expression(e) => e.fmt(f),
        }
    }
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
        macro_rules! fmt_int {
            ($v:expr) => {
                match $v.radix {
                    Radix::Bin => write!(f, "0b{:b}", $v.value),
                    Radix::Oct => write!(f, "0o{:o}", $v.value),
                    Radix::Dec => write!(f, "{}", $v.value),
                    Radix::Hex => write!(f, "0x{:x}", $v.value),
                }
            };
        }

        match self {
            Self::Identifier(ident) => ident.fmt(f),

            // Integer types
            Self::I8(v) => fmt_int!(v),
            Self::I16(v) => fmt_int!(v),
            Self::I32(v) => fmt_int!(v),
            Self::I64(v) => fmt_int!(v),
            Self::I128(v) => fmt_int!(v),
            Self::Isize(v) => fmt_int!(v),
            Self::U8(v) => fmt_int!(v),
            Self::U16(v) => fmt_int!(v),
            Self::U32(v) => fmt_int!(v),
            Self::U64(v) => fmt_int!(v),
            Self::U128(v) => fmt_int!(v),
            Self::Usize(v) => fmt_int!(v),

            // Float types
            Self::F32(v) => {
                let val: f32 = (*v).into();
                write!(f, "{val}")
            }
            Self::F64(v) => {
                let val: f64 = (*v).into();
                write!(f, "{val}")
            }

            Self::Boolean(boolean) => boolean.fmt(f),
            Self::String(string) => string.fmt(f),
            Self::Array(array_lit) => array_lit.fmt(f),
            Self::Index(index_expr) => index_expr.fmt(f),
            Self::Hash(hash_lit) => hash_lit.fmt(f),
            Self::Prefix(prefix_expr) => prefix_expr.fmt(f),
            Self::Infix(infix_expr) => infix_expr.fmt(f),
            Self::Conditional(cond_expr) => cond_expr.fmt(f),
            Self::Function(func_lit) => func_lit.fmt(f),
            Self::Macro(macro_lit) => macro_lit.fmt(f),
            Self::Call(call_expr) => call_expr.fmt(f),
        }
    }
}

impl fmt::Display for ArrayLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.elements
                .iter()
                .map(|expr| format!("{expr}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl fmt::Display for IndexExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}[{}])", self.expr, self.index)
    }
}

impl fmt::Display for HashLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}",
            self.pairs
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
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
        write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for MacroLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macro({}) {{\n{}\n}}", self.params.join(", "), self.body)
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
