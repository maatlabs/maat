//! AST definitions for Maat: statements, expressions, and program structure.

use std::fmt;

use maat_span::Span;

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
    Loop(LoopStatement),
    While(WhileStatement),
    For(ForStatement),
}

impl Statement {
    /// Returns the source span covering this statement.
    pub fn span(&self) -> Span {
        match self {
            Self::Let(s) => s.span,
            Self::Return(s) => s.span,
            Self::Expression(s) => s.span,
            Self::Block(s) => s.span,
            Self::Loop(s) => s.span,
            Self::While(s) => s.span,
            Self::For(s) => s.span,
        }
    }
}

/// A `let` binding: `let <ident> = <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LetStatement {
    pub ident: String,
    pub value: Expression,
    pub span: Span,
}

/// A `return` statement: `return <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStatement {
    pub value: Expression,
    pub span: Span,
}

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpressionStatement {
    pub value: Expression,
    pub span: Span,
}

/// A block of statements: `{ ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// All possible expression types in Maat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expression {
    Identifier(Ident),

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

    Boolean(BooleanLiteral),
    String(StringLiteral),
    Array(ArrayLiteral),
    Index(IndexExpr),
    Hash(HashLiteral),
    Prefix(PrefixExpr),
    Infix(InfixExpr),
    Conditional(ConditionalExpr),
    Function(Function),
    Macro(MacroLiteral),
    Call(CallExpr),
    Cast(CastExpr),
    Break(BreakExpr),
    Continue(ContinueExpr),
}

impl Expression {
    /// Returns the source span covering this expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Identifier(v) => v.span,
            Self::I8(v) => v.span,
            Self::I16(v) => v.span,
            Self::I32(v) => v.span,
            Self::I64(v) => v.span,
            Self::I128(v) => v.span,
            Self::Isize(v) => v.span,
            Self::U8(v) => v.span,
            Self::U16(v) => v.span,
            Self::U32(v) => v.span,
            Self::U64(v) => v.span,
            Self::U128(v) => v.span,
            Self::Usize(v) => v.span,
            Self::F32(v) => v.span,
            Self::F64(v) => v.span,
            Self::Boolean(v) => v.span,
            Self::String(v) => v.span,
            Self::Array(v) => v.span,
            Self::Index(v) => v.span,
            Self::Hash(v) => v.span,
            Self::Prefix(v) => v.span,
            Self::Infix(v) => v.span,
            Self::Conditional(v) => v.span,
            Self::Function(v) => v.span,
            Self::Macro(v) => v.span,
            Self::Call(v) => v.span,
            Self::Cast(v) => v.span,
            Self::Break(v) => v.span,
            Self::Continue(v) => v.span,
        }
    }

    /// Returns a human-readable name for this expression type.
    ///
    /// Used primarily for error reporting.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Identifier(_) => "identifier",
            Self::I8(_) => "i8 literal",
            Self::I16(_) => "i16 literal",
            Self::I32(_) => "i32 literal",
            Self::I64(_) => "i64 literal",
            Self::I128(_) => "i128 literal",
            Self::Isize(_) => "isize literal",
            Self::U8(_) => "u8 literal",
            Self::U16(_) => "u16 literal",
            Self::U32(_) => "u32 literal",
            Self::U64(_) => "u64 literal",
            Self::U128(_) => "u128 literal",
            Self::Usize(_) => "usize literal",
            Self::F32(_) => "f32 literal",
            Self::F64(_) => "f64 literal",
            Self::Boolean(_) => "boolean literal",
            Self::String(_) => "string literal",
            Self::Array(_) => "array literal",
            Self::Index(_) => "index expression",
            Self::Hash(_) => "hash literal",
            Self::Prefix(_) => "prefix expression",
            Self::Infix(_) => "infix expression",
            Self::Conditional(_) => "conditional expression",
            Self::Function(_) => "function literal",
            Self::Macro(_) => "macro literal",
            Self::Call(_) => "function call",
            Self::Cast(_) => "cast expression",
            Self::Break(_) => "break expression",
            Self::Continue(_) => "continue expression",
        }
    }
}

/// An identifier reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    pub value: String,
    pub span: Span,
}

/// A boolean literal (`true` or `false`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BooleanLiteral {
    pub value: bool,
    pub span: Span,
}

/// A string literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
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
            pub span: Span,
        }
    };
}

/// Macro to generate floating-point type structs with native storage (as raw bits).
macro_rules! define_float_type {
    ($name:ident, $native:ty, $bits:ty, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            pub bits: $bits,
            pub span: Span,
        }

        impl From<$native> for $name {
            fn from(value: $native) -> Self {
                Self {
                    bits: <$native>::to_bits(value),
                    span: Span::ZERO,
                }
            }
        }

        impl From<$name> for $native {
            fn from(value: $name) -> Self {
                <$native>::from_bits(value.bits)
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
    pub span: Span,
}

/// Indexing operation: `<lhs>[<index>]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExpr {
    pub expr: Box<Expression>,
    pub index: Box<Expression>,
    pub span: Span,
}

/// Hash literal: `{ key: value, ... }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HashLiteral {
    pub pairs: Vec<(Expression, Expression)>,
    pub span: Span,
}

/// Prefix expression: `!<expr>`, `-<expr>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixExpr {
    pub operator: String,
    pub operand: Box<Expression>,
    pub span: Span,
}

/// Binary/infix expression: `<lhs> <operator> <rhs>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InfixExpr {
    pub lhs: Box<Expression>,
    pub operator: String,
    pub rhs: Box<Expression>,
    pub span: Span,
}

/// Conditional (if/else) expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConditionalExpr {
    pub condition: Box<Expression>,
    pub consequence: BlockStatement,
    pub alternative: Option<BlockStatement>,
    pub span: Span,
}

/// Function literal with optional name for recursive self-reference.
///
/// Named functions are created when a function literal is assigned via a
/// `let` binding (e.g., `let foo = fn(x) { ... }`). The name enables
/// recursive closures to reference themselves without capturing an
/// outer binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub span: Span,
}

/// Macro literal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroLiteral {
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub span: Span,
}

/// Function call
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub function: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

/// Explicit type cast: `expression as type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CastExpr {
    pub expr: Box<Expression>,
    pub target: TypeAnnotation,
    pub span: Span,
}

/// An infinite loop: `loop { <body> }`.
///
/// Exits only via `break`. The optional break value becomes
/// the loop expression's result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopStatement {
    pub body: BlockStatement,
    pub span: Span,
}

/// A conditional loop: `while <condition> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStatement {
    pub condition: Box<Expression>,
    pub body: BlockStatement,
    pub span: Span,
}

/// An iterator loop: `for <ident> in <iterable> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForStatement {
    pub ident: String,
    pub iterable: Box<Expression>,
    pub body: BlockStatement,
    pub span: Span,
}

/// A break expression: `break` or `break <value>`.
///
/// When used inside a `loop`, the optional value becomes the
/// loop's result. In `while` and `for` loops, break takes no value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakExpr {
    pub value: Option<Box<Expression>>,
    pub span: Span,
}

/// A continue expression: `continue`.
///
/// Skips the remainder of the current loop iteration and jumps
/// to the loop's condition check (for `while`) or next iteration
/// (for `loop` and `for`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContinueExpr {
    pub span: Span,
}

/// Target type for cast expressions.
///
/// Represents the set of numeric types that a value can be explicitly
/// converted to via the `as` operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeAnnotation {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
}

impl TypeAnnotation {
    /// Returns the canonical string name of this type annotation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

/// Parsing error for [`TypeAnnotation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownTypeAnnotation;

impl fmt::Display for UnknownTypeAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown type annotation")
    }
}

impl std::str::FromStr for TypeAnnotation {
    type Err = UnknownTypeAnnotation;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "i8" => Ok(Self::I8),
            "i16" => Ok(Self::I16),
            "i32" => Ok(Self::I32),
            "i64" => Ok(Self::I64),
            "i128" => Ok(Self::I128),
            "isize" => Ok(Self::Isize),
            "u8" => Ok(Self::U8),
            "u16" => Ok(Self::U16),
            "u32" => Ok(Self::U32),
            "u64" => Ok(Self::U64),
            "u128" => Ok(Self::U128),
            "usize" => Ok(Self::Usize),
            "f32" => Ok(Self::F32),
            "f64" => Ok(Self::F64),
            _ => Err(UnknownTypeAnnotation),
        }
    }
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
            Self::Loop(loop_stmt) => loop_stmt.fmt(f)?,
            Self::While(while_stmt) => while_stmt.fmt(f)?,
            Self::For(for_stmt) => for_stmt.fmt(f)?,
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
            Self::Identifier(ident) => ident.value.fmt(f),

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

            Self::Boolean(b) => b.value.fmt(f),
            Self::String(s) => s.value.fmt(f),
            Self::Array(array_lit) => array_lit.fmt(f),
            Self::Index(index_expr) => index_expr.fmt(f),
            Self::Hash(hash_lit) => hash_lit.fmt(f),
            Self::Prefix(prefix_expr) => prefix_expr.fmt(f),
            Self::Infix(infix_expr) => infix_expr.fmt(f),
            Self::Conditional(cond_expr) => cond_expr.fmt(f),
            Self::Function(func_lit) => func_lit.fmt(f),
            Self::Macro(macro_lit) => macro_lit.fmt(f),
            Self::Call(call_expr) => call_expr.fmt(f),
            Self::Cast(cast_expr) => cast_expr.fmt(f),
            Self::Break(break_expr) => break_expr.fmt(f),
            Self::Continue(cont_expr) => cont_expr.fmt(f),
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
        match &self.name {
            Some(name) => write!(
                f,
                "fn<{name}>({}) {{\n{}\n}}",
                self.params.join(", "),
                self.body
            ),
            None => write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body),
        }
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

impl fmt::Display for CastExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} as {})", self.expr, self.target.as_str())
    }
}

impl fmt::Display for LoopStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "loop {{ {} }}", self.body)
    }
}

impl fmt::Display for WhileStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "while {} {{ {} }}", self.condition, self.body)
    }
}

impl fmt::Display for ForStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "for {} in {} {{ {} }}",
            self.ident, self.iterable, self.body
        )
    }
}

impl fmt::Display for BreakExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Some(val) => write!(f, "break {val}"),
            None => write!(f, "break"),
        }
    }
}

impl fmt::Display for ContinueExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "continue")
    }
}

impl fmt::Display for TypeAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
