//! Conversion from parsed [`TypeExpr`] to internal [`Type`] representation.

use maat_ast::TypeExpr;

use crate::ty::{FnType, Type};

/// Resolves a parsed type expression into an internal type.
///
/// Maps named types to their corresponding [`Type`] variants:
/// - Numeric types (`i8`, `i64`, `u32`, etc.)
/// - `bool` -> [`Type::Bool`]
/// - `String` -> [`Type::String`]
/// - `null` -> [`Type::Null`]
/// - Unknown names are treated as generic type references.
///
/// Compound types ([`TypeExpr::Array`], [`TypeExpr::Map`], [`TypeExpr::Fn`],
/// [`TypeExpr::Generic`]) are resolved recursively.
pub fn resolve_type_expr(expr: &TypeExpr) -> Type {
    match expr {
        TypeExpr::Named(named) => resolve_named(&named.name),
        TypeExpr::Array(elem, _) => Type::Array(Box::new(resolve_type_expr(elem))),
        TypeExpr::Map(k, v, _) => Type::Hash(
            Box::new(resolve_type_expr(k)),
            Box::new(resolve_type_expr(v)),
        ),
        TypeExpr::Fn(params, ret, _) => Type::Function(FnType {
            params: params.iter().map(resolve_type_expr).collect(),
            ret: Box::new(resolve_type_expr(ret)),
        }),
        TypeExpr::Generic(name, args, _) => {
            let _ = args;
            Type::Generic(name.clone(), vec![])
        }
    }
}

/// Maps a type name string to its internal type representation.
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
        "bool" => Type::Bool,
        "str" | "String" => Type::String,
        "null" => Type::Null,
        other => Type::Generic(other.to_string(), vec![]),
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
            ("String", Type::String),
            ("null", Type::Null),
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
        let expr = TypeExpr::Array(
            Box::new(TypeExpr::Named(NamedType {
                name: "i64".to_string(),
                span: Span::ZERO,
            })),
            Span::ZERO,
        );
        assert_eq!(resolve_type_expr(&expr), Type::Array(Box::new(Type::I64)));
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
