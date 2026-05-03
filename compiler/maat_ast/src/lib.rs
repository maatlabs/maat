//! Abstract syntax tree (AST) for parsed Maat source.
//! Contains definitions for statement nodes, expression nodes, literals,
//! type annotations, and more.

#![forbid(unsafe_code)]

mod display;
mod fold;
mod format;
mod transform;

pub use fold::fold_constants;
pub use format::{FmtSegment, parse_format_string, unescape_char, unescape_string};
use maat_span::Span;
pub use transform::{TransformFn, transform};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaatAst {
    Program(Program),
    Stmt(Stmt),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LetStmt {
    pub ident: String,
    pub mutable: bool,
    pub type_annotation: Option<TypeExpr>,
    pub value: Expr,
    pub pattern: Option<Pattern>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReAssignStmt {
    pub ident: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStmt {
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprStmt {
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockStmt {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<TypedParam>,
    pub generic_params: Vec<GenericParam>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockStmt,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

impl FuncDef {
    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|p| p.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopStmt {
    pub label: Option<String>,
    pub bound: u64,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStmt {
    pub label: Option<String>,
    pub bound: u64,
    pub condition: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForStmt {
    pub label: Option<String>,
    pub ident: String,
    pub pattern: Option<Pattern>,
    pub iterable: Box<Expr>,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Ident(Ident),
    Number(Number),
    Bool(BoolLit),
    Str(StrLit),
    Char(CharLit),
    Vector(Vector),
    Index(IndexExpr),
    Map(MapLit),

    Prefix(PrefixExpr),
    Infix(InfixExpr),
    Cond(CondExpr),
    Lambda(Lambda),
    MacroLit(MacroLit),
    Call(CallExpr),
    MacroCall(MacroCallExpr),
    Cast(CastExpr),
    Break(BreakExpr),
    Continue(ContinueExpr),

    Match(MatchExpr),
    Try(TryExpr),
    Tuple(TupleExpr),
    FieldAccess(FieldAccessExpr),
    MethodCall(MethodCallExpr),
    StructLit(StructLitExpr),
    PathExpr(PathExpr),
    Range(RangeExpr),
    Array(ArrayLit),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Ident(v) => v.span,
            Self::Number(v) => v.span,
            Self::Bool(v) => v.span,
            Self::Str(v) => v.span,
            Self::Char(v) => v.span,
            Self::Vector(v) => v.span,
            Self::Index(v) => v.span,
            Self::Map(v) => v.span,
            Self::Prefix(v) => v.span,
            Self::Infix(v) => v.span,
            Self::Cond(v) => v.span,
            Self::Lambda(v) => v.span,
            Self::MacroLit(v) => v.span,
            Self::Call(v) => v.span,
            Self::MacroCall(v) => v.span,
            Self::Cast(v) => v.span,
            Self::Break(v) => v.span,
            Self::Continue(v) => v.span,
            Self::Match(v) => v.span,
            Self::Try(v) => v.span,
            Self::Tuple(v) => v.span,
            Self::FieldAccess(v) => v.span,
            Self::MethodCall(v) => v.span,
            Self::StructLit(v) => v.span,
            Self::PathExpr(v) => v.span,
            Self::Range(v) => v.span,
            Self::Array(v) => v.span,
        }
    }

    pub fn is_integer_literal(&self) -> bool {
        match self {
            Self::Number(_) => true,
            Self::Prefix(prefix) if prefix.operator == "-" => prefix.operand.is_integer_literal(),
            _ => false,
        }
    }

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoolLit {
    pub value: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrLit {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharLit {
    pub value: char,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Number {
    pub kind: NumKind,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumKind {
    /// An unsuffixed integer literal awaiting type inference.
    Int {
        type_var: u32,
    },
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
    /// A field-element literal over the Goldilocks base field (`Felt`).
    Fe,
}

impl NumKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Int { .. } => "{int}",
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
            Self::Fe => "Felt",
        }
    }

    pub fn from_suffix(s: &str) -> Option<Self> {
        match s {
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "isize" => Some(Self::Isize),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "usize" => Some(Self::Usize),
            "fe" | "Felt" => Some(Self::Fe),
            _ => None,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Isize
        )
    }

    pub fn is_felt(self) -> bool {
        matches!(self, Self::Fe)
    }

    /// Returns `true` if `value` fits within the range of `self`.
    pub fn fits(&self, value: i128) -> bool {
        match self {
            Self::Int { .. } => true,
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
            // Field-element literals fit iff their value is in `[0, 2^64)`.
            // Modular reduction against the Goldilocks prime happens at lowering time.
            Self::Fe => u64::try_from(value).is_ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Vector {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayLit {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExpr {
    pub expr: Box<Expr>,
    pub index: Box<Expr>,
    pub array_len: Option<usize>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapLit {
    pub pairs: Vec<(Expr, Expr)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixExpr {
    pub operator: String,
    pub operand: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOpClass {
    #[default]
    Default,
    Felt,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InfixExpr {
    pub lhs: Box<Expr>,
    pub operator: String,
    pub rhs: Box<Expr>,
    pub op_class: BinOpClass,
    pub array_eq_len: Option<usize>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CondExpr {
    pub condition: Box<Expr>,
    pub consequence: BlockStmt,
    pub alternative: Option<BlockStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub params: Vec<TypedParam>,
    pub generic_params: Vec<GenericParam>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockStmt,
    pub span: Span,
}

impl Lambda {
    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|p| p.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroLit {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub function: Box<Expr>,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroCallExpr {
    pub name: String,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target: CastTarget,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastTarget {
    Num(NumKind),
    Char,
}

impl CastTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Num(k) => k.as_str(),
            Self::Char => "char",
        }
    }

    pub fn from_type_name(s: &str) -> Option<Self> {
        if s == "char" {
            Some(Self::Char)
        } else {
            NumKind::from_suffix(s).map(Self::Num)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakExpr {
    pub label: Option<String>,
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContinueExpr {
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TryExpr {
    pub expr: Box<Expr>,
    pub kind: TryKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TryKind {
    #[default]
    Unknown,
    Option,
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub fields: Vec<StructField>,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariant {
    pub name: String,
    pub kind: EnumVariantKind,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnumVariantKind {
    Unit,
    Tuple(Vec<TypeExpr>),
    Struct(Vec<StructField>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitDecl {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub methods: Vec<TraitMethod>,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitMethod {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub params: Vec<TypedParam>,
    pub return_type: Option<TypeExpr>,
    pub default_body: Option<BlockStmt>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplBlock {
    pub trait_name: Option<TypeExpr>,
    pub self_type: TypeExpr,
    pub generic_params: Vec<GenericParam>,
    pub methods: Vec<FuncDef>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UseStmt {
    pub path: Vec<String>,
    pub items: Option<Vec<String>>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModStmt {
    pub name: String,
    pub body: Option<Vec<Stmt>>,
    pub is_public: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
    Wildcard(Span),
    Ident {
        name: String,
        mutable: bool,
        span: Span,
    },
    Literal(Box<Expr>),
    TupleStruct {
        path: String,
        fields: Vec<Pattern>,
        span: Span,
    },
    Struct {
        path: String,
        fields: Vec<PatternField>,
        span: Span,
    },
    Tuple(Vec<Pattern>, Span),
    Or(Vec<Pattern>, Span),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatternField {
    pub name: String,
    pub pattern: Option<Box<Pattern>>,
    pub span: Span,
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(s) => *s,
            Self::Ident { span, .. } => *span,
            Self::Literal(expr) => expr.span(),
            Self::TupleStruct { span, .. } => *span,
            Self::Struct { span, .. } => *span,
            Self::Tuple(_, s) => *s,
            Self::Or(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldAccessExpr {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: String,
    pub arguments: Vec<Expr>,
    pub receiver: Option<String>,
    pub array_len: Option<usize>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructLitExpr {
    pub name: String,
    pub fields: Vec<(String, Expr)>,
    pub base: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathExpr {
    pub segments: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub inclusive: bool,
    pub kind: Option<NumKind>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeExpr {
    Named(NamedType),
    Vector(Box<TypeExpr>, Span),
    Set(Box<TypeExpr>, Span),
    Map(Box<TypeExpr>, Box<TypeExpr>, Span),
    Fn(Vec<TypeExpr>, Box<TypeExpr>, Span),
    Generic(String, Vec<TypeExpr>, Span),
    Tuple(Vec<TypeExpr>, Span),
    Array(Box<TypeExpr>, usize, Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            Self::Named(n) => n.span,
            Self::Vector(_, s)
            | Self::Set(_, s)
            | Self::Map(_, _, s)
            | Self::Fn(_, _, s)
            | Self::Generic(_, _, s)
            | Self::Tuple(_, s)
            | Self::Array(_, _, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamedType {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedParam {
    pub name: String,
    pub type_expr: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TraitBound>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitBound {
    pub name: String,
    pub span: Span,
}
