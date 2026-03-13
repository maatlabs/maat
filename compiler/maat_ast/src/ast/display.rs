//! Implements `std::fmt::Display` for all AST nodes.

use super::*;

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
            Self::ReAssign(assign_stmt) => assign_stmt.fmt(f)?,
            Self::Return(ret_stmt) => ret_stmt.fmt(f)?,
            Self::Expr(expr_stmt) => expr_stmt.fmt(f)?,
            Self::Block(block_stmt) => block_stmt.fmt(f)?,
            Self::FuncDef(fn_item) => fn_item.fmt(f)?,
            Self::Loop(loop_stmt) => loop_stmt.fmt(f)?,
            Self::While(while_stmt) => while_stmt.fmt(f)?,
            Self::For(for_stmt) => for_stmt.fmt(f)?,
            Self::StructDecl(s) => s.fmt(f)?,
            Self::EnumDecl(e) => e.fmt(f)?,
            Self::TraitDecl(t) => t.fmt(f)?,
            Self::ImplBlock(i) => i.fmt(f)?,
            Self::Use(u) => u.fmt(f)?,
            Self::Mod(m) => m.fmt(f)?,
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
            Self::StructLit(struct_lit) => struct_lit.fmt(f),
            Self::PathExpr(path_expr) => path_expr.fmt(f),
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

impl fmt::Display for FuncDef {
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
        let vis = visibility_modifier(self.is_public);
        write!(f, "{vis}{}: {}", self.name, self.ty)
    }
}

impl fmt::Display for EnumDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for StructLitExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{ ", self.name)?;
        let fields = self
            .fields
            .iter()
            .map(|(name, val)| format!("{name}: {val}"))
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "{fields} }}")
    }
}

impl fmt::Display for PathExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join("::"))
    }
}

impl fmt::Display for UseStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = visibility_modifier(self.is_public);
        let path = self.path.join("::");
        match &self.items {
            Some(items) => write!(f, "{vis}use {}::{{{}}};", path, items.join(", ")),
            None => write!(f, "{vis}use {};", path),
        }
    }
}

impl fmt::Display for ModStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for UnknownTypeAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown type annotation")
    }
}

/// Helper to check if an item's `is_public` field is set or not.
/// If set, returns "pub", else "".
#[inline]
fn visibility_modifier(vis: bool) -> &'static str {
    if vis { "pub " } else { "" }
}
