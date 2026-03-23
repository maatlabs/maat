//! Implements [`core::fmt::Display`] for all AST nodes.

use core::fmt;

use crate::*;

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
        for s in &self.statements {
            s.fmt(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Let(s) => s.fmt(f)?,
            Self::ReAssign(s) => s.fmt(f)?,
            Self::Return(s) => s.fmt(f)?,
            Self::Expr(s) => s.fmt(f)?,
            Self::Block(s) => s.fmt(f)?,
            Self::FuncDef(s) => s.fmt(f)?,
            Self::Loop(s) => s.fmt(f)?,
            Self::While(s) => s.fmt(f)?,
            Self::For(s) => s.fmt(f)?,
            Self::StructDecl(s) => s.fmt(f)?,
            Self::EnumDecl(s) => s.fmt(f)?,
            Self::TraitDecl(s) => s.fmt(f)?,
            Self::ImplBlock(s) => s.fmt(f)?,
            Self::Use(s) => s.fmt(f)?,
            Self::Mod(s) => s.fmt(f)?,
        }
        Ok(())
    }
}

impl fmt::Display for LetStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = if self.mutable { "mut " } else { "" };
        match &self.type_annotation {
            Some(ty) => write!(f, "let {m}{}: {} = {};", self.ident, ty, self.value),
            None => write!(f, "let {m}{} = {};", self.ident, self.value),
        }
    }
}

impl fmt::Display for ReAssignStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {};", self.ident, self.value)
    }
}

impl fmt::Display for ReturnStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "return {};", self.value)
    }
}

impl fmt::Display for ExprStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{};", self.value)
    }
}

impl fmt::Display for BlockStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.statements.is_empty() {
            write!(f, "{{}}")
        } else {
            writeln!(f, "{{")?;
            for s in &self.statements {
                s.fmt(f)?;
                writeln!(f)?;
            }
            write!(f, "}}")
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident(e) => e.value.fmt(f),

            Self::Number(e) => {
                if e.kind.is_signed() {
                    match e.radix {
                        Radix::Bin => write!(f, "0b{:b}", e.value),
                        Radix::Oct => write!(f, "0o{:o}", e.value),
                        Radix::Dec => write!(f, "{}", e.value),
                        Radix::Hex => write!(f, "0x{:x}", e.value),
                    }
                } else {
                    let uval = e.value as u128;
                    match e.radix {
                        Radix::Bin => write!(f, "0b{:b}", uval),
                        Radix::Oct => write!(f, "0o{:o}", uval),
                        Radix::Dec => write!(f, "{}", uval),
                        Radix::Hex => write!(f, "0x{:x}", uval),
                    }
                }
            }

            Self::Bool(e) => e.value.fmt(f),
            Self::Str(e) => e.value.fmt(f),
            Self::Vector(e) => e.fmt(f),
            Self::Index(e) => e.fmt(f),
            Self::Map(e) => e.fmt(f),
            Self::Prefix(e) => e.fmt(f),
            Self::Infix(e) => e.fmt(f),
            Self::Cond(e) => e.fmt(f),
            Self::Lambda(e) => e.fmt(f),
            Self::Macro(e) => e.fmt(f),
            Self::Call(e) => e.fmt(f),
            Self::Cast(e) => e.fmt(f),
            Self::Break(e) => e.fmt(f),
            Self::Continue(e) => e.fmt(f),
            Self::Match(e) => e.fmt(f),
            Self::FieldAccess(e) => e.fmt(f),
            Self::MethodCall(e) => e.fmt(f),
            Self::StructLit(e) => e.fmt(f),
            Self::PathExpr(e) => e.fmt(f),
            Self::Range(e) => e.fmt(f),
        }
    }
}

impl fmt::Display for Vector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.elements
                .iter()
                .map(|arr| format!("{arr}"))
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
        if let Some(alt) = &self.alternative {
            write!(f, " else {alt}")?;
        }
        Ok(())
    }
}

impl fmt::Display for FuncDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
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

        let vis = visibility_modifier(self.is_public);
        write!(
            f,
            "{vis}fn {}{generics}({params}){ret} {}",
            self.name, self.body
        )
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
                .map(|call| format!("{call}"))
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
        if let Some(label) = &self.label {
            write!(f, "'{label}: ")?;
        }
        write!(f, "loop {}", self.body)
    }
}

impl fmt::Display for WhileStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            write!(f, "'{label}: ")?;
        }
        write!(f, "while {} {}", self.condition, self.body)
    }
}

impl fmt::Display for ForStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            write!(f, "'{label}: ")?;
        }
        write!(f, "for {} in {} {}", self.ident, self.iterable, self.body)
    }
}

impl fmt::Display for BreakExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "break")?;
        if let Some(label) = &self.label {
            write!(f, " '{label}")?;
        }
        if let Some(val) = &self.value {
            write!(f, " {val}")?;
        }
        Ok(())
    }
}

impl fmt::Display for ContinueExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "continue")?;
        if let Some(label) = &self.label {
            write!(f, " '{label}")?;
        }
        Ok(())
    }
}

impl fmt::Display for StructDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "{vis}struct {}{generics}", self.name)?;
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
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        write!(f, "{vis}{}: {}", self.name, self.ty)
    }
}

impl fmt::Display for EnumDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "{vis}enum {}{generics}", self.name)?;
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
        fmt_doc_comment(f, &self.doc)?;
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
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        let generics = fmt_generic_params(&self.generic_params);
        write!(f, "{vis}trait {}{generics}", self.name)?;
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
        fmt_doc_comment(f, &self.doc)?;
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
        fmt_doc_comment(f, &self.doc)?;
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

impl fmt::Display for StructLitExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{ ", self.name)?;
        let mut parts = self
            .fields
            .iter()
            .map(|(name, val)| format!("{name}: {val}"))
            .collect::<Vec<String>>();
        if let Some(base) = &self.base {
            parts.push(format!("..{base}"));
        }
        write!(f, "{} }}", parts.join(", "))
    }
}

impl fmt::Display for PathExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join("::"))
    }
}

impl fmt::Display for RangeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = if self.inclusive { "..=" } else { ".." };
        write!(f, "{}{op}{}", self.start, self.end)
    }
}

impl fmt::Display for UseStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = visibility_modifier(self.is_public);
        let path = self.path.join("::");
        match &self.items {
            Some(items) => write!(f, "{vis}use {path}::{{{}}};", items.join(", ")),
            None => write!(f, "{vis}use {path};"),
        }
    }
}

impl fmt::Display for ModStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        match &self.body {
            Some(body) => {
                writeln!(f, "{vis}mod {} {{", self.name)?;
                for stmt in body {
                    writeln!(f, "    {stmt}")?;
                }
                write!(f, "}}")
            }
            None => write!(f, "{vis}mod {};", self.name),
        }
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(n) => f.write_str(&n.name),
            Self::Vector(elem, _) => write!(f, "[{elem}]"),
            Self::Set(elem, _) => write!(f, "Set<{elem}>"),
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

/// Helper to check if an item's `is_public` field is set or not.
/// If set, returns "pub", else "".
#[inline]
fn visibility_modifier(vis: bool) -> &'static str {
    if vis { "pub " } else { "" }
}

/// Renders documentation comment lines (`///`) above the item.
///
/// Each line of the stored doc string is emitted as a separate `///` line,
/// reconstructing the original source form.
fn fmt_doc_comment(f: &mut fmt::Formatter<'_>, doc: &Option<String>) -> fmt::Result {
    if let Some(text) = doc {
        for line in text.lines() {
            writeln!(f, "///{line}")?;
        }
    }
    Ok(())
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
