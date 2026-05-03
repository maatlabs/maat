//! Conversion from parsed [`TypeExpr`] to internal [`Type`] representation.

use std::rc::Rc;

use maat_ast::TypeExpr;

use crate::{FnType, Type};

pub fn resolve_type_expr(expr: &TypeExpr) -> Type {
    match expr {
        TypeExpr::Named(named) => resolve_named(&named.name),
        TypeExpr::Vector(elem, _) => Type::Vector(Box::new(resolve_type_expr(elem))),
        TypeExpr::Set(elem, _) => Type::Set(Box::new(resolve_type_expr(elem))),
        TypeExpr::Map(k, v, _) => Type::Map(
            Box::new(resolve_type_expr(k)),
            Box::new(resolve_type_expr(v)),
        ),
        TypeExpr::Fn(params, ret, _) => Type::Function(FnType {
            params: params.iter().map(resolve_type_expr).collect(),
            ret: Box::new(resolve_type_expr(ret)),
        }),
        TypeExpr::Generic(name, args, _) => {
            let _ = args;
            Type::Generic(Rc::from(name.as_str()), vec![])
        }
        TypeExpr::Tuple(elems, _) => Type::Tuple(elems.iter().map(resolve_type_expr).collect()),
        TypeExpr::Array(elem, n, _) => Type::Array(Box::new(resolve_type_expr(elem)), *n),
    }
}

fn resolve_named(name: &str) -> Type {
    match name {
        "i8" => Type::I8,
        "i16" => Type::I16,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "i128" => Type::I128,
        "isize" => Type::Isize,
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "u128" => Type::U128,
        "usize" => Type::Usize,
        "Felt" => Type::Felt,
        "bool" => Type::Bool,
        "char" => Type::Char,
        "str" => Type::Str,
        other => Type::Generic(Rc::from(other), vec![]),
    }
}

#[cfg(test)]
mod tests {
    use maat_ast::NamedType;
    use maat_span::Span;

    use super::*;

    #[test]
    fn resolve_primitives() {
        let cases = [
            ("i8", Type::I8),
            ("i64", Type::I64),
            ("u32", Type::U32),
            ("bool", Type::Bool),
            ("str", Type::Str),
        ];
        for (name, expected) in cases {
            let expr = TypeExpr::Named(NamedType {
                name: name.to_string(),
                span: Span::ZERO,
            });
            assert_eq!(resolve_type_expr(&expr), expected);
        }
    }

    #[test]
    fn resolve_array() {
        let expr = TypeExpr::Vector(
            Box::new(TypeExpr::Named(NamedType {
                name: "i64".to_string(),
                span: Span::ZERO,
            })),
            Span::ZERO,
        );
        assert_eq!(resolve_type_expr(&expr), Type::Vector(Box::new(Type::I64)));
    }

    #[test]
    fn resolve_function() {
        let expr = TypeExpr::Fn(
            vec![TypeExpr::Named(NamedType {
                name: "i64".to_string(),
                span: Span::ZERO,
            })],
            Box::new(TypeExpr::Named(NamedType {
                name: "bool".to_string(),
                span: Span::ZERO,
            })),
            Span::ZERO,
        );
        assert_eq!(
            resolve_type_expr(&expr),
            Type::Function(FnType {
                params: vec![Type::I64],
                ret: Box::new(Type::Bool),
            })
        );
    }
}
