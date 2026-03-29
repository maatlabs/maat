//! Implements [`core::fmt::Display`] for all AST nodes.

use core::fmt;

use crate::format::*;
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
            Self::Let(s) => s.fmt(f),
            Self::ReAssign(s) => s.fmt(f),
            Self::Return(s) => s.fmt(f),
            Self::Expr(s) => s.fmt(f),
            Self::Block(s) => s.fmt(f),
            Self::FuncDef(s) => s.fmt(f),
            Self::Loop(s) => s.fmt(f),
            Self::While(s) => s.fmt(f),
            Self::For(s) => s.fmt(f),
            Self::StructDecl(s) => s.fmt(f),
            Self::EnumDecl(s) => s.fmt(f),
            Self::TraitDecl(s) => s.fmt(f),
            Self::ImplBlock(s) => s.fmt(f),
            Self::Use(s) => s.fmt(f),
            Self::Mod(s) => s.fmt(f),
        }
    }
}

impl fmt::Display for LetStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (m, binding) = match &self.pattern {
            Some(pat) => ("", pat.to_string()),
            None => {
                let m = if self.mutable { "mut " } else { "" };
                (m, self.ident.clone())
            }
        };
        match &self.type_annotation {
            Some(ty) => write!(f, "let {m}{binding}: {ty} = {};", self.value),
            None => write!(f, "let {m}{binding} = {};", self.value),
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
            Self::Char(e) => e.value.fmt(f),
            Self::Vector(e) => e.fmt(f),
            Self::Index(e) => e.fmt(f),
            Self::Map(e) => e.fmt(f),
            Self::Prefix(e) => e.fmt(f),
            Self::Infix(e) => e.fmt(f),
            Self::Cond(e) => e.fmt(f),
            Self::Lambda(e) => e.fmt(f),
            Self::MacroLit(e) => e.fmt(f),
            Self::Call(e) => e.fmt(f),
            Self::MacroCall(e) => e.fmt(f),
            Self::Cast(e) => e.fmt(f),
            Self::Break(e) => e.fmt(f),
            Self::Continue(e) => e.fmt(f),
            Self::Match(e) => e.fmt(f),
            Self::Try(e) => write!(f, "{}?", e.expr),
            Self::Tuple(e) => e.fmt(f),
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
        f.write_str("[")?;
        write_comma_separated(f, &self.elements)?;
        f.write_str("]")
    }
}

impl fmt::Display for IndexExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}[{}])", self.expr, self.index)
    }
}

impl fmt::Display for MapLit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        let mut iter = self.pairs.iter();
        if let Some((k, v)) = iter.next() {
            write!(f, "{k}: {v}")?;
            for (k, v) in iter {
                write!(f, ", {k}: {v}")?;
            }
        }
        f.write_str("}")
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
        let vis = visibility_modifier(self.is_public);
        write!(f, "{vis}fn {}", self.name)?;
        write_generic_params(f, &self.generic_params)?;
        f.write_str("(")?;
        write_params(f, &self.params)?;
        f.write_str(")")?;
        write_return_type(f, &self.return_type)?;
        write!(f, " {}", self.body)
    }
}

impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fn")?;
        write_generic_params(f, &self.generic_params)?;
        f.write_str("(")?;
        write_params(f, &self.params)?;
        f.write_str(")")?;
        write_return_type(f, &self.return_type)?;
        write!(f, " {}", self.body)
    }
}

impl fmt::Display for MacroLit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("macro(")?;
        write_comma_separated(f, &self.params)?;
        write!(f, ") {}", self.body)
    }
}

impl fmt::Display for CallExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.function)?;
        write_comma_separated(f, &self.arguments)?;
        f.write_str(")")
    }
}

impl fmt::Display for MacroCallExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}!(", self.name)?;
        write_comma_separated(f, &self.arguments)?;
        f.write_str(")")
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
        f.write_str("break")?;
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
        f.write_str("continue")?;
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
        write!(f, "{vis}struct {}", self.name)?;
        write_generic_params(f, &self.generic_params)?;
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
        write!(f, "{vis}enum {}", self.name)?;
        write_generic_params(f, &self.generic_params)?;
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
                f.write_str("(")?;
                write_comma_separated(f, types)?;
                f.write_str(")")
            }
            Self::Struct(fields) => {
                f.write_str(" { ")?;
                write_comma_separated(f, fields)?;
                f.write_str(" }")
            }
        }
    }
}

impl fmt::Display for TraitDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
        let vis = visibility_modifier(self.is_public);
        write!(f, "{vis}trait {}", self.name)?;
        write_generic_params(f, &self.generic_params)?;
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
        write!(f, "fn {}", self.name)?;
        write_generic_params(f, &self.generic_params)?;
        f.write_str("(")?;
        write_params(f, &self.params)?;
        f.write_str(")")?;
        write_return_type(f, &self.return_type)?;
        match &self.default_body {
            Some(body) => write!(f, " {body}"),
            None => f.write_str(";"),
        }
    }
}

impl fmt::Display for ImplBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_doc_comment(f, &self.doc)?;
        f.write_str("impl")?;
        write_generic_params(f, &self.generic_params)?;
        match &self.trait_name {
            Some(t) => write!(f, " {t} for {}", self.self_type)?,
            None => write!(f, " {}", self.self_type)?,
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

impl fmt::Display for TupleExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        write_comma_separated(f, &self.elements)?;
        if self.elements.len() == 1 {
            f.write_str(",")?;
        }
        f.write_str(")")
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wildcard(_) => f.write_str("_"),
            Self::Ident { name, mutable, .. } => {
                if *mutable {
                    write!(f, "mut {name}")
                } else {
                    f.write_str(name)
                }
            }
            Self::Literal(expr) => write!(f, "{expr}"),
            Self::TupleStruct { path, fields, .. } => {
                write!(f, "{path}(")?;
                write_comma_separated(f, fields)?;
                f.write_str(")")
            }
            Self::Struct { path, fields, .. } => {
                write!(f, "{path} {{ ")?;
                write_comma_separated(f, fields)?;
                f.write_str(" }")
            }
            Self::Tuple(fields, _) => {
                f.write_str("(")?;
                write_comma_separated(f, fields)?;
                if fields.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Self::Or(patterns, _) => write_separated_with(f, patterns, " | "),
        }
    }
}

impl fmt::Display for PatternField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.pattern {
            Some(pat) => write!(f, "{}: {pat}", self.name),
            None => f.write_str(&self.name),
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
        write!(f, "{}.{}(", self.object, self.method)?;
        write_comma_separated(f, &self.arguments)?;
        f.write_str(")")
    }
}

impl fmt::Display for StructLitExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{ ", self.name)?;
        let mut first = true;
        for (name, val) in &self.fields {
            if !first {
                f.write_str(", ")?;
            }
            write!(f, "{name}: {val}")?;
            first = false;
        }
        if let Some(base) = &self.base {
            if !first {
                f.write_str(", ")?;
            }
            write!(f, "..{base}")?;
        }
        f.write_str(" }")
    }
}

impl fmt::Display for PathExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_separated_with(f, &self.segments, "::")
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
        write!(f, "{vis}use ")?;
        write_separated_with(f, &self.path, "::")?;
        match &self.items {
            Some(items) => {
                f.write_str("::{")?;
                write_comma_separated(f, items)?;
                f.write_str("};")
            }
            None => f.write_str(";"),
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
                f.write_str("fn(")?;
                write_comma_separated(f, params)?;
                write!(f, ") -> {ret}")
            }
            Self::Generic(name, args, _) => {
                write!(f, "{name}<")?;
                write_comma_separated(f, args)?;
                f.write_str(">")
            }
            Self::Tuple(elems, _) => {
                f.write_str("(")?;
                write_comma_separated(f, elems)?;
                if elems.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
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
        f.write_str(&self.name)?;
        if !self.bounds.is_empty() {
            f.write_str(": ")?;
            write_separated_with(f, self.bounds.iter().map(|b| b.name.as_str()), " + ")?;
        }
        Ok(())
    }
}

impl fmt::Display for TraitBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
