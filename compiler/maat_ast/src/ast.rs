//! AST definitions for Maat: statements, expressions, and program structure.

use std::fmt;

use maat_span::Span;

pub mod display;
pub mod fold;

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
    ReAssign(ReAssignStmt),
    Return(ReturnStmt),
    Expr(ExprStmt),
    Block(BlockStmt),
    FuncDef(FuncDef),
    Loop(LoopStmt),
    While(WhileStmt),
    For(ForStmt),
    StructDecl(StructDecl),
    EnumDecl(EnumDecl),
    TraitDecl(TraitDecl),
    ImplBlock(ImplBlock),
    Use(UseStmt),
    Mod(ModStmt),
}

/// A `let` binding: `let <ident>: <type> = <value>;` or
/// `let mut <ident>: <type> = <value>;`.
///
/// When `mutable` is `true`, the binding can be reassigned via
/// `ident = expr;` or compound assignment (`ident += expr;`).
/// When `false`, rebinding the same name requires a new `let`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LetStmt {
    pub ident: String,
    pub mutable: bool,
    pub type_annotation: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

/// A reassignment to an existing mutable binding: `<ident> = <value>;`.
///
/// Produced by plain assignment (`x = expr;`) and compound assignment
/// (`x += expr;`, desugared to `x = x + expr`). The compiler validates
/// that the target variable exists and was declared with `let mut`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReAssignStmt {
    pub ident: String,
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

/// A named function declaration: `fn foo<T>(x: T, y: i64) -> T { x }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<TypedParam>,
    pub generic_params: Vec<GenericParam>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockStmt,
    pub is_public: bool,
    pub span: Span,
}

impl FuncDef {
    /// Returns an iterator over the parameter names.
    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|p| p.name.as_str())
    }
}

/// An infinite loop: `loop { <body> }` or `'label: loop { <body> }`.
///
/// Exits only via `break`. The optional break value becomes
/// the loop expression's result. When labeled, `break 'label` and
/// `continue 'label` target this specific loop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopStmt {
    pub label: Option<String>,
    pub body: BlockStmt,
    pub span: Span,
}

/// A conditional loop: `while <condition> { <body> }` or
/// `'label: while <condition> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStmt {
    pub label: Option<String>,
    pub condition: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

/// An iterator loop: `for <ident> in <iterable> { <body> }` or
/// `'label: for <ident> in <iterable> { <body> }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForStmt {
    pub label: Option<String>,
    pub ident: String,
    pub iterable: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

/// All possible expression types in Maat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Ident(Ident),
    Number(Number),
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
    StructLit(StructLitExpr),
    PathExpr(PathExpr),
    Range(RangeExpr),
}

impl Expr {
    /// Returns the source span covering this expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Ident(v) => v.span,
            Self::Number(v) => v.span,
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
            Self::StructLit(v) => v.span,
            Self::PathExpr(v) => v.span,
            Self::Range(v) => v.span,
        }
    }

    /// Returns `true` if the expression is an integer literal (any width, signed or unsigned),
    /// including negated integer literals (e.g., `-100`).
    ///
    /// Used to determine whether a literal can coerce to a declared numeric type
    /// without requiring explicit suffixes or casts.
    pub fn is_integer_literal(&self) -> bool {
        match self {
            Self::Number(_) => true,
            Self::Prefix(prefix) if prefix.operator == "-" => prefix.operand.is_integer_literal(),
            _ => false,
        }
    }

    /// Extracts the compile-time integer value from a literal expression (including negated literals).
    ///
    /// Returns the value as `i128` (wide enough for all integer types).
    pub fn extract_integer_value(&self) -> Option<i128> {
        match self {
            Self::Number(lit) => Some(lit.value),
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

/// A numeric integer literal with type, value, radix, and span.
///
/// All integer literal types (i8..u128, isize, usize) are represented uniformly.
/// The value is stored as `i128`, which is wide enough for all signed types and
/// for unsigned values up to `i128::MAX`. The parser validates that the literal
/// value fits within the target type's range before constructing this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Number {
    pub kind: NumberKind,
    pub value: i128,
    pub radix: Radix,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Radix {
    Bin,
    Oct,
    Dec,
    Hex,
}

/// Target integer type of a numeric literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberKind {
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

impl NumberKind {
    /// Returns the type name as it appears in source code.
    pub fn as_str(self) -> &'static str {
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

    /// Returns `true` if this is a signed integer kind.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Isize
        )
    }

    /// Returns `true` if `value` fits within the range of `self`.
    pub fn fits(&self, value: i128) -> bool {
        match self {
            Self::I8 => i8::try_from(value).is_ok(),
            Self::I16 => i16::try_from(value).is_ok(),
            Self::I32 => i32::try_from(value).is_ok(),
            Self::I64 => i64::try_from(value).is_ok(),
            Self::I128 => true,
            Self::Isize => isize::try_from(value).is_ok(),
            Self::U8 => u8::try_from(value).is_ok(),
            Self::U16 => u16::try_from(value).is_ok(),
            Self::U32 => u32::try_from(value).is_ok(),
            Self::U64 => u64::try_from(value).is_ok(),
            Self::U128 => u128::try_from(value).is_ok(),
            Self::Usize => usize::try_from(value).is_ok(),
        }
    }
}

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

/// A break expression: `break`, `break <value>`, or `break 'label [<value>]`.
///
/// When used inside a `loop`, the optional value becomes the
/// loop's result. In `while` and `for` loops, break takes no value.
/// When a label is present, the break targets the loop with that label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakExpr {
    pub label: Option<String>,
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// A continue expression: `continue` or `continue 'label`.
///
/// Skips the remainder of the current loop iteration and jumps
/// to the loop's condition check (for `while`) or next iteration
/// (for `loop` and `for`). When a label is present, the continue
/// targets the loop with that label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContinueExpr {
    pub label: Option<String>,
    pub span: Span,
}

/// A `struct` declaration: `struct Name<T> { field: Type, ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub fields: Vec<StructField>,
    pub is_public: bool,
    pub span: Span,
}

/// A named field in a struct declaration:
/// `field_name: FieldType` or `pub field_name: FieldType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
    pub is_public: bool,
    pub span: Span,
}

/// An `enum` declaration: `enum Name<T> { Variant, ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub is_public: bool,
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
    pub is_public: bool,
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
    pub methods: Vec<FuncDef>,
    pub span: Span,
}

/// A `use` import statement: `use foo::bar;` or `use foo::bar::{baz, qux};`.
///
/// Imports items from other modules into the current scope. Glob imports
/// (`use foo::*`) are deliberately unsupported to preserve ZK auditability.
/// When `is_public` is `true`, the import is a re-export (`pub use`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UseStmt {
    /// The path segments leading to the imported item(s) (e.g., `["foo", "bar"]`).
    pub path: Vec<String>,
    /// When present, the specific items imported from the path (e.g., `{baz, qux}`).
    /// When `None`, the final segment itself is the imported item.
    pub items: Option<Vec<String>>,
    /// Whether this is a re-export (`pub use`).
    pub is_public: bool,
    pub span: Span,
}

/// A `mod` declaration: `mod foo;` (external file) or `mod foo { ... }` (inline).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModStmt {
    /// The module name.
    pub name: String,
    /// The inline body, if present. `None` means an external file module (`mod foo;`).
    pub body: Option<Vec<Stmt>>,
    pub is_public: bool,
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
    /// Receiver type name resolved by the type checker (e.g. `"Array"`, `"str"`, `"Set"`).
    pub receiver: Option<String>,
    pub span: Span,
}

/// Struct literal construction: `Point { x: 1, y: 2 }` or with functional
/// update syntax: `Point { x: 10, ..other }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructLitExpr {
    /// The struct type name.
    pub name: String,
    /// Field initializers: `(field_name, value_expr)`.
    pub fields: Vec<(String, Expr)>,
    /// Optional base expression for functional update (`..expr`).
    pub base: Option<Box<Expr>>,
    pub span: Span,
}

/// A qualified path expression: `Enum::Variant`.
///
/// Used for enum variant construction (e.g., `Option::Some`, `Color::Red`).
/// When followed by `(args)`, the parser produces a `Call` with a `PathExpr`
/// as the function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathExpr {
    /// Path segments (e.g., `["Option", "Some"]`).
    pub segments: Vec<String>,
    pub span: Span,
}

/// Range expression: `start..end` (exclusive) or `start..=end` (inclusive).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    /// Whether this is an inclusive range (`..=`).
    pub inclusive: bool,
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

/// Unescapes a string literal by processing escape sequences.
///
/// Supports standard escape sequences:
/// - `\\` -> backslash
/// - `\"` -> double quote
/// - `\n` -> newline
/// - `\r` -> carriage return
/// - `\t` -> tab
/// - `\0` -> null character
///
/// Invalid escape sequences are preserved as-is.
pub fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}
