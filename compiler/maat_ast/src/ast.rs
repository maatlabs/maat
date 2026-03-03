//! AST definitions for Maat: statements, expressions, and program structure.

use std::fmt;

use maat_span::Span;

/// Top-level AST node wrapper for all language items.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Program(Program),
    Stmt(Stmt),
    Expr(Expr),
}

/// A complete compilation unit (crate).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

/// Statements: `let` bindings, `return` statements, function declarations,
/// expression statements, or nested blocks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Stmt {
    Let(LetStmt),
    Return(ReturnStmt),
    Expr(ExprStmt),
    Block(BlockStmt),
    FnItem(FnItem),
    Loop(LoopStmt),
    While(WhileStmt),
    For(ForStmt),
    StructDecl(StructDecl),
    EnumDecl(EnumDecl),
    TraitDecl(TraitDecl),
    ImplBlock(ImplBlock),
}

/// A `let` binding: `let <ident> = <value>;` or
/// `let <ident>: <type> = <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LetStmt {
    pub ident: String,
    pub type_annotation: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

/// A `return` statement: `return <value>;`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStmt {
    pub value: Expr,
    pub span: Span,
}

/// An expression used as a statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprStmt {
    pub value: Expr,
    pub span: Span,
}

/// A block of statements: `{ ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockStmt {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// All possible expression types in Maat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Ident(Ident),

    I8(I8),
    I16(I16),
    I32(I32),
    I64(I64),
    I128(I128),
    Isize(Isize),

    U8(U8),
    U16(U16),
    U32(U32),
    U64(U64),
    U128(U128),
    Usize(Usize),

    Bool(Bool),
    Str(Str),
    Array(Array),
    Index(IndexExpr),
    Map(Map),

    Prefix(PrefixExpr),
    Infix(InfixExpr),
    Cond(CondExpr),
    Lambda(Lambda),
    Macro(Macro),
    Call(CallExpr),
    Cast(CastExpr),
    Break(BreakExpr),
    Continue(ContinueExpr),
    Match(MatchExpr),
    FieldAccess(FieldAccessExpr),
    MethodCall(MethodCallExpr),
}

impl Expr {
    /// Returns the source span covering this expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Ident(v) => v.span,
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
            Self::Bool(v) => v.span,
            Self::Str(v) => v.span,
            Self::Array(v) => v.span,
            Self::Index(v) => v.span,
            Self::Map(v) => v.span,
            Self::Prefix(v) => v.span,
            Self::Infix(v) => v.span,
            Self::Cond(v) => v.span,
            Self::Lambda(v) => v.span,
            Self::Macro(v) => v.span,
            Self::Call(v) => v.span,
            Self::Cast(v) => v.span,
            Self::Break(v) => v.span,
            Self::Continue(v) => v.span,
            Self::Match(v) => v.span,
            Self::FieldAccess(v) => v.span,
            Self::MethodCall(v) => v.span,
        }
    }

    /// Returns `true` if the expression is an integer literal (any width, signed or unsigned),
    /// including negated integer literals (e.g., `-100`).
    ///
    /// Used to determine whether a literal can coerce to a declared numeric type
    /// without requiring explicit suffixes or casts.
    pub fn is_integer_literal(&self) -> bool {
        match self {
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::I128(_)
            | Self::Isize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::Usize(_) => true,
            // Negated literals: `-100` is `Prefix("-", I64(100))`
            Self::Prefix(prefix) if prefix.operator == "-" => prefix.operand.is_integer_literal(),
            _ => false,
        }
    }

    /// Extracts the compile-time integer value from a literal expression (including negated literals).
    ///
    /// Returns the value as `i128` (wide enough for all integer types).
    pub fn extract_integer_value(&self) -> Option<i128> {
        match self {
            Self::I8(lit) => Some(lit.value as i128),
            Self::I16(lit) => Some(lit.value as i128),
            Self::I32(lit) => Some(lit.value as i128),
            Self::I64(lit) => Some(lit.value as i128),
            Self::I128(lit) => Some(lit.value),
            Self::Isize(lit) => Some(lit.value as i128),
            Self::U8(lit) => Some(lit.value as i128),
            Self::U16(lit) => Some(lit.value as i128),
            Self::U32(lit) => Some(lit.value as i128),
            Self::U64(lit) => Some(lit.value as i128),
            Self::U128(lit) => Some(lit.value as i128),
            Self::Usize(lit) => Some(lit.value as i128),
            Self::Prefix(prefix) if prefix.operator == "-" => {
                prefix.operand.extract_integer_value().map(|v| -v)
            }
            _ => None,
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
pub struct Bool {
    pub value: bool,
    pub span: Span,
}

/// A string literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Str {
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

/// Arrays: `[expr, expr, ...]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Array {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// Indexing operation: `<lhs>[<index>]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExpr {
    pub expr: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}

/// Key-value hash literal: `{ key: value, ... }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Map {
    pub pairs: Vec<(Expr, Expr)>,
    pub span: Span,
}

/// Prefix expression: `!<expr>`, `-<expr>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixExpr {
    pub operator: String,
    pub operand: Box<Expr>,
    pub span: Span,
}

/// Binary/infix expression: `<lhs> <operator> <rhs>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InfixExpr {
    pub lhs: Box<Expr>,
    pub operator: String,
    pub rhs: Box<Expr>,
    pub span: Span,
}

/// Cond (if/else) expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CondExpr {
    pub condition: Box<Expr>,
    pub consequence: BlockStmt,
    pub alternative: Option<BlockStmt>,
    pub span: Span,
}

/// A named function declaration: `fn foo<T>(x: T, y: i64) -> T { x }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnItem {
    pub name: String,
    pub params: Vec<TypedParam>,
    pub generic_params: Vec<GenericParam>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockStmt,
    pub span: Span,
}

impl FnItem {
    /// Returns an iterator over the parameter names.
    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|p| p.name.as_str())
    }
}

/// An anonymous function/closure expression: `fn(x, y) { x + y }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub params: Vec<TypedParam>,
    pub generic_params: Vec<GenericParam>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockStmt,
    pub span: Span,
}

impl Lambda {
    /// Returns an iterator over the parameter names.
    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|p| p.name.as_str())
    }
}

/// Macro literal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Macro {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub span: Span,
}

/// Function call expression: `<callee>(<args>)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub function: Box<Expr>,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

/// Explicit type cast: `expression as type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target: TypeAnnotation,
    pub span: Span,
}

/// An infinite loop: `loop { <body> }`.
///
/// Exits only via `break`. The optional break value becomes
/// the loop expression's result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopStmt {
    pub body: BlockStmt,
    pub span: Span,
}

/// A conditional loop: `while <condition> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStmt {
    pub condition: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

/// An iterator loop: `for <ident> in <iterable> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForStmt {
    pub ident: String,
    pub iterable: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

/// A break expression: `break` or `break <value>`.
///
/// When used inside a `loop`, the optional value becomes the
/// loop's result. In `while` and `for` loops, break takes no value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakExpr {
    pub value: Option<Box<Expr>>,
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

/// A `struct` declaration: `struct Name<T> { field: Type, ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

/// A named field in a struct declaration: `field_name: FieldType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// An `enum` declaration: `enum Name<T> { Variant, ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// A single variant in an enum declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariant {
    pub name: String,
    pub kind: EnumVariantKind,
    pub span: Span,
}

/// The payload of a single enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnumVariantKind {
    /// A unit variant: `None`.
    Unit,
    /// A tuple variant: `Some(T)`.
    Tuple(Vec<TypeExpr>),
    /// A struct variant: `Point { x: i64, y: i64 }`.
    Struct(Vec<StructField>),
}

/// A `trait` declaration: `trait Name<T> { fn method(...); }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub methods: Vec<TraitMethod>,
    pub span: Span,
}

/// A single method signature (with optional default body) in a trait declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitMethod {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub params: Vec<TypedParam>,
    pub return_type: Option<TypeExpr>,
    pub default_body: Option<BlockStmt>,
    pub span: Span,
}

/// An `impl` block: either inherent (`impl Type { ... }`) or
/// trait (`impl Trait for Type { ... }`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplBlock {
    pub trait_name: Option<TypeExpr>,
    pub self_type: TypeExpr,
    pub generic_params: Vec<GenericParam>,
    pub methods: Vec<FnItem>,
    pub span: Span,
}

/// A `match` expression: `match scrutinee { arms }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// A single arm in a `match` expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Expr,
    pub span: Span,
}

/// A pattern used in `match` arms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// `_` — matches any value without binding.
    Wildcard(Span),
    /// `x` — binds the matched value to a new variable.
    Ident(String, Span),
    /// `42`, `true`, `"hello"` — matches a specific literal value.
    Literal(Box<Expr>),
    /// `Some(x)`, `Err(e)`, `Point(a, b)` — matches a tuple-struct or tuple-variant.
    TupleStruct {
        path: String,
        fields: Vec<Pattern>,
        span: Span,
    },
    /// `Point { x, y }`, `Point { x: px }` — matches a struct or struct-variant.
    Struct {
        path: String,
        fields: Vec<PatternField>,
        span: Span,
    },
    /// `PatA | PatB` — matches if either alternative matches.
    Or(Vec<Pattern>, Span),
}

/// A named field binding inside a struct pattern.
///
/// Represents a single `field: pattern` binding (or shorthand `field`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatternField {
    pub name: String,
    /// The sub-pattern to bind the field value to.
    /// When `None`, the field name itself is used as the binding (`field` shorthand).
    pub pattern: Option<Box<Pattern>>,
    pub span: Span,
}

impl Pattern {
    /// Returns the source span of this pattern.
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(s) => *s,
            Self::Ident(_, s) => *s,
            Self::Literal(expr) => expr.span(),
            Self::TupleStruct { span, .. } => *span,
            Self::Struct { span, .. } => *span,
            Self::Or(_, s) => *s,
        }
    }
}

/// Field access: `expr.field`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldAccessExpr {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

/// Method call: `expr.method(args)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: String,
    /// Arguments passed to the method (excluding the receiver).
    pub arguments: Vec<Expr>,
    pub span: Span,
}

/// A parsed type expression used in type annotations.
///
/// Appears in `let` bindings (`let x: TypeExpr = ...`), function
/// parameters (`fn(x: TypeExpr)`), and return types (`-> TypeExpr`).
///
/// # Variants
///
/// - `Named` — a simple named type like `i64`, `bool`, or a user-defined
///   type like `Point`.
/// - `Array` — `[T]`, an array of element type `T`.
/// - `Hash` — `{K: V}`, a hash map from key type `K` to value type `V`.
/// - `Fn` — `fn(A, B) -> R`, a function type.
/// - `Generic` — a parameterized type like `Option<T>` or `Result<T, E>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeExpr {
    Named(NamedType),
    Array(Box<TypeExpr>, Span),
    Map(Box<TypeExpr>, Box<TypeExpr>, Span),
    Fn(Vec<TypeExpr>, Box<TypeExpr>, Span),
    Generic(String, Vec<TypeExpr>, Span),
}

impl TypeExpr {
    /// Returns the source span covering this type expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Named(n) => n.span,
            Self::Array(_, s) | Self::Map(_, _, s) | Self::Fn(_, _, s) | Self::Generic(_, _, s) => {
                *s
            }
        }
    }
}

/// A simple named type reference (e.g., `i64`, `bool`, `String`, `Point`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamedType {
    pub name: String,
    pub span: Span,
}

/// A function parameter with an optional type annotation.
///
/// ```text
/// fn(x: i64, y)  // x has type annotation, y does not
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedParam {
    pub name: String,
    pub type_expr: Option<TypeExpr>,
    pub span: Span,
}

/// A generic type parameter with optional trait bounds.
///
/// ```text
/// fn<T, U: Display + Clone>(...)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TraitBound>,
    pub span: Span,
}

/// A trait bound on a generic type parameter.
///
/// ```text
/// T: Display + Clone
///    ^^^^^^^   ^^^^^
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitBound {
    pub name: String,
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
            _ => Err(UnknownTypeAnnotation),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(p) => p.fmt(f),
            Self::Stmt(s) => s.fmt(f),
            Self::Expr(e) => e.fmt(f),
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

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Let(let_stmt) => let_stmt.fmt(f)?,
            Self::Return(ret_stmt) => ret_stmt.fmt(f)?,
            Self::Expr(expr_stmt) => expr_stmt.fmt(f)?,
            Self::Block(block_stmt) => block_stmt.fmt(f)?,
            Self::FnItem(fn_item) => fn_item.fmt(f)?,
            Self::Loop(loop_stmt) => loop_stmt.fmt(f)?,
            Self::While(while_stmt) => while_stmt.fmt(f)?,
            Self::For(for_stmt) => for_stmt.fmt(f)?,
            Self::StructDecl(s) => s.fmt(f)?,
            Self::EnumDecl(e) => e.fmt(f)?,
            Self::TraitDecl(t) => t.fmt(f)?,
            Self::ImplBlock(i) => i.fmt(f)?,
        }
        Ok(())
    }
}

impl fmt::Display for LetStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.type_annotation {
            Some(ty) => write!(f, "let {}: {} = {};", self.ident, ty, self.value),
            None => write!(f, "let {} = {};", self.ident, self.value),
        }
    }
}

impl fmt::Display for ReturnStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "return {};", self.value)
    }
}

impl fmt::Display for ExprStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl fmt::Display for BlockStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.statements.is_empty() {
            write!(f, "{{}}")
        } else {
            writeln!(f, "{{")?;
            for stmt in &self.statements {
                stmt.fmt(f)?;
                writeln!(f)?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for Expr {
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
            Self::Ident(ident) => ident.value.fmt(f),

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

            Self::Bool(b) => b.value.fmt(f),
            Self::Str(s) => s.value.fmt(f),
            Self::Array(array_lit) => array_lit.fmt(f),
            Self::Index(index_expr) => index_expr.fmt(f),
            Self::Map(map) => map.fmt(f),
            Self::Prefix(prefix_expr) => prefix_expr.fmt(f),
            Self::Infix(infix_expr) => infix_expr.fmt(f),
            Self::Cond(cond_expr) => cond_expr.fmt(f),
            Self::Lambda(lambda) => lambda.fmt(f),
            Self::Macro(macro_lit) => macro_lit.fmt(f),
            Self::Call(call_expr) => call_expr.fmt(f),
            Self::Cast(cast_expr) => cast_expr.fmt(f),
            Self::Break(break_expr) => break_expr.fmt(f),
            Self::Continue(cont_expr) => cont_expr.fmt(f),
            Self::Match(match_expr) => match_expr.fmt(f),
            Self::FieldAccess(field_access) => field_access.fmt(f),
            Self::MethodCall(method_call) => method_call.fmt(f),
        }
    }
}

impl fmt::Display for Array {
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

impl fmt::Display for Map {
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

impl fmt::Display for CondExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "if {} {}", self.condition, self.consequence)?;
        if let Some(alternative) = &self.alternative {
            write!(f, " else {}", alternative)?;
        }
        Ok(())
    }
}

impl fmt::Display for FnItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let generics = if self.generic_params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                self.generic_params
                    .iter()
                    .map(|g| g.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let ret = self
            .return_type
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {t}"));

        write!(f, "fn {}{generics}({params}){ret} {}", self.name, self.body)
    }
}

impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let generics = if self.generic_params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                self.generic_params
                    .iter()
                    .map(|g| g.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let ret = self
            .return_type
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {t}"));

        write!(f, "fn{generics}({params}){ret} {}", self.body)
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macro({}) {}", self.params.join(", "), self.body)
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

impl fmt::Display for LoopStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "loop {}", self.body)
    }
}

impl fmt::Display for WhileStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "while {} {}", self.condition, self.body)
    }
}

impl fmt::Display for ForStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "for {} in {} {}", self.ident, self.iterable, self.body)
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

impl fmt::Display for StructDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "struct {}{generics}", self.name)?;
        if self.fields.is_empty() {
            write!(f, " {{}}")
        } else {
            writeln!(f, " {{")?;
            for field in &self.fields {
                writeln!(f, "    {field},")?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for StructField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.ty)
    }
}

impl fmt::Display for EnumDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "enum {}{generics}", self.name)?;
        if self.variants.is_empty() {
            write!(f, " {{}}")
        } else {
            writeln!(f, " {{")?;
            for variant in &self.variants {
                writeln!(f, "    {variant},")?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for EnumVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.name, self.kind)
    }
}

impl fmt::Display for EnumVariantKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => Ok(()),
            Self::Tuple(types) => {
                let inner = types
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({inner})")
            }
            Self::Struct(fields) => {
                write!(f, " {{ ")?;
                let inner = fields
                    .iter()
                    .map(|field| field.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{inner} }}")
            }
        }
    }
}

impl fmt::Display for TraitDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "trait {}{generics}", self.name)?;
        if self.methods.is_empty() {
            write!(f, " {{}}")
        } else {
            writeln!(f, " {{")?;
            for method in &self.methods {
                writeln!(f, "    {method}")?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for TraitMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generics = fmt_generic_params(&self.generic_params);
        let params = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let ret = self
            .return_type
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {t}"));
        write!(f, "fn {}{generics}({params}){ret}", self.name)?;
        match &self.default_body {
            Some(body) => write!(f, " {body}"),
            None => write!(f, ";"),
        }
    }
}

impl fmt::Display for ImplBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generics = fmt_generic_params(&self.generic_params);
        match &self.trait_name {
            Some(t) => write!(f, "impl{generics} {t} for {}", self.self_type)?,
            None => write!(f, "impl{generics} {}", self.self_type)?,
        }
        if self.methods.is_empty() {
            write!(f, " {{}}")
        } else {
            writeln!(f, " {{")?;
            for method in &self.methods {
                writeln!(f, "    {method}")?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for MatchExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "match {} {{", self.scrutinee)?;
        for arm in &self.arms {
            writeln!(f, "    {arm},")?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for MatchArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} => {}", self.pattern, self.body)?;
        if let Some(guard) = &self.guard {
            write!(f, " if {guard}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wildcard(_) => write!(f, "_"),
            Self::Ident(name, _) => write!(f, "{name}"),
            Self::Literal(expr) => write!(f, "{expr}"),
            Self::TupleStruct { path, fields, .. } => {
                let inner = fields
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{path}({inner})")
            }
            Self::Struct { path, fields, .. } => {
                let inner = fields
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{path} {{ {inner} }}")
            }
            Self::Or(patterns, _) => {
                let inner = patterns
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(" | ");
                write!(f, "{inner}")
            }
        }
    }
}

impl fmt::Display for PatternField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.pattern {
            Some(pat) => write!(f, "{}: {pat}", self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

impl fmt::Display for FieldAccessExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.object, self.field)
    }
}

impl fmt::Display for MethodCallExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args = self
            .arguments
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "{}.{}({args})", self.object, self.method)
    }
}

/// Formats a generic parameter list as `<T, U: Bound>`, or an empty string if empty.
fn fmt_generic_params(params: &[GenericParam]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            params
                .iter()
                .map(|g| g.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(n) => f.write_str(&n.name),
            Self::Array(elem, _) => write!(f, "[{elem}]"),
            Self::Map(k, v, _) => write!(f, "{{{k}: {v}}}"),
            Self::Fn(params, ret, _) => {
                let params = params
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fn({params}) -> {ret}")
            }
            Self::Generic(name, args, _) => {
                let args = args
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{name}<{args}>")
            }
        }
    }
}

impl fmt::Display for TypedParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.type_expr {
            Some(ty) => write!(f, "{}: {ty}", self.name),
            None => f.write_str(&self.name),
        }
    }
}

impl fmt::Display for GenericParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.bounds.is_empty() {
            f.write_str(&self.name)
        } else {
            let bounds = self
                .bounds
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>()
                .join(" + ");
            write!(f, "{}: {bounds}", self.name)
        }
    }
}

impl fmt::Display for TraitBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

impl fmt::Display for TypeAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
