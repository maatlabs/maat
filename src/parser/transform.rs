//! Implements AST transformation via the visitor pattern.
//!
//! Provides the `transform` function for traversing and modifying
//! AST nodes. It's essential for macro expansion and other AST modifications.
use super::ast::*;

/// A function that takes a node and returns a potentially modified one.
pub type TransformFn<'a> = &'a mut dyn FnMut(Node) -> Node;

/// Recursively traverses and modifies an AST node using the provided transformer function.
///
/// The traversal follows a post-order pattern: first, all child nodes are recursively
/// transformed, then the transformer function is applied to the current node. This ensures
/// that changes are applied from the leaves up to the root.
///
/// # Examples
///
/// ```
/// use maat::parser::ast::*;
/// use maat::transform;
///
/// // Double all integer values
/// let mut program = Program {
///     statements: vec![Statement::Expression(ExpressionStatement {
///         value: Expression::I64(I64 {
///             radix: Radix::Dec,
///             value: 5,
///         }),
///     })],
/// };
///
/// let result = transform(Node::Program(program), &mut |node| {
///     match node {
///         Node::Expression(Expression::I64(mut i)) => {
///             i.value *= 2;
///             Node::Expression(Expression::I64(i))
///         }
///         n => n,
///     }
/// });
/// ```
///
/// # Macro Expansion
///
/// This function is crucial for macro expansion, where macro calls need to be
/// identified and replaced with their expanded AST nodes:
///
/// ```ignore
/// transform(program, &mut |node| {
///     if let Node::Expression(Expression::Call(call)) = &node {
///         if is_macro_call(call) {
///             return expand_macro(call);
///         }
///     }
///     node
/// });
/// ```
pub fn transform(node: Node, transformer: TransformFn) -> Node {
    let node = match node {
        Node::Program(mut program) => {
            program.statements = program
                .statements
                .into_iter()
                .map(|stmt| match transform(Node::Statement(stmt), transformer) {
                    Node::Statement(s) => s,
                    _ => unreachable!("Statement transformation returned non-statement"),
                })
                .collect();
            Node::Program(program)
        }

        Node::Statement(stmt) => {
            let new_stmt = match stmt {
                Statement::Let(mut let_stmt) => {
                    let_stmt.value = match transform(Node::Expression(let_stmt.value), transformer)
                    {
                        Node::Expression(e) => e,
                        _ => unreachable!("Expression transformation returned non-expression"),
                    };
                    Statement::Let(let_stmt)
                }

                Statement::Return(mut ret_stmt) => {
                    ret_stmt.value = match transform(Node::Expression(ret_stmt.value), transformer)
                    {
                        Node::Expression(e) => e,
                        _ => unreachable!("Expression transformation returned non-expression"),
                    };
                    Statement::Return(ret_stmt)
                }

                Statement::Expression(mut expr_stmt) => {
                    expr_stmt.value =
                        match transform(Node::Expression(expr_stmt.value), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        };
                    Statement::Expression(expr_stmt)
                }

                Statement::Block(mut block) => {
                    block.statements = block
                        .statements
                        .into_iter()
                        .map(|stmt| match transform(Node::Statement(stmt), transformer) {
                            Node::Statement(s) => s,
                            _ => unreachable!("Statement transformation returned non-statement"),
                        })
                        .collect();
                    Statement::Block(block)
                }
            };
            Node::Statement(new_stmt)
        }

        Node::Expression(expr) => {
            let new_expr = match expr {
                Expression::Array(mut array) => {
                    array.elements = array
                        .elements
                        .into_iter()
                        .map(
                            |elem| match transform(Node::Expression(elem), transformer) {
                                Node::Expression(e) => e,
                                _ => unreachable!(
                                    "Expression transformation returned non-expression"
                                ),
                            },
                        )
                        .collect();
                    Expression::Array(array)
                }

                Expression::Index(mut index) => {
                    index.expr = Box::new(
                        match transform(Node::Expression(*index.expr), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        },
                    );
                    index.index = Box::new(
                        match transform(Node::Expression(*index.index), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        },
                    );
                    Expression::Index(index)
                }

                Expression::Hash(mut hash) => {
                    hash.pairs = hash
                        .pairs
                        .into_iter()
                        .map(|(key, val)| {
                            let new_key = match transform(Node::Expression(key), transformer) {
                                Node::Expression(e) => e,
                                _ => {
                                    unreachable!(
                                        "Expression transformation returned non-expression"
                                    )
                                }
                            };
                            let new_val = match transform(Node::Expression(val), transformer) {
                                Node::Expression(e) => e,
                                _ => {
                                    unreachable!(
                                        "Expression transformation returned non-expression"
                                    )
                                }
                            };
                            (new_key, new_val)
                        })
                        .collect();
                    Expression::Hash(hash)
                }

                Expression::Prefix(mut prefix) => {
                    prefix.operand = Box::new(
                        match transform(Node::Expression(*prefix.operand), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        },
                    );
                    Expression::Prefix(prefix)
                }

                Expression::Infix(mut infix) => {
                    infix.lhs =
                        Box::new(match transform(Node::Expression(*infix.lhs), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        });
                    infix.rhs =
                        Box::new(match transform(Node::Expression(*infix.rhs), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        });
                    Expression::Infix(infix)
                }

                Expression::Conditional(mut cond) => {
                    cond.condition = Box::new(
                        match transform(Node::Expression(*cond.condition), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        },
                    );
                    cond.consequence = match transform(
                        Node::Statement(Statement::Block(cond.consequence)),
                        transformer,
                    ) {
                        Node::Statement(Statement::Block(b)) => b,
                        _ => unreachable!("Block transformation returned non-block"),
                    };
                    if let Some(alt) = cond.alternative {
                        cond.alternative = Some(
                            match transform(Node::Statement(Statement::Block(alt)), transformer) {
                                Node::Statement(Statement::Block(b)) => b,
                                _ => unreachable!("Block transformation returned non-block"),
                            },
                        );
                    }
                    Expression::Conditional(cond)
                }

                Expression::Function(mut func) => {
                    func.body = match transform(
                        Node::Statement(Statement::Block(func.body)),
                        transformer,
                    ) {
                        Node::Statement(Statement::Block(b)) => b,
                        _ => unreachable!("Block transformation returned non-block"),
                    };
                    Expression::Function(func)
                }

                Expression::Call(mut call) => {
                    call.function = Box::new(
                        match transform(Node::Expression(*call.function), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        },
                    );
                    call.arguments = call
                        .arguments
                        .into_iter()
                        .map(|arg| match transform(Node::Expression(arg), transformer) {
                            Node::Expression(e) => e,
                            _ => unreachable!("Expression transformation returned non-expression"),
                        })
                        .collect();
                    Expression::Call(call)
                }

                // Leaf nodes (literals and identifiers) don't need transformation
                expr => expr,
            };
            Node::Expression(new_expr)
        }
    };

    transformer(node)
}
