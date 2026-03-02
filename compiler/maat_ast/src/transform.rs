//! Implements AST transformation via the visitor pattern.
//!
//! Provides the `transform` function for traversing and modifying
//! AST nodes. It's essential for macro expansion and other AST modifications.
use crate::ast::*;

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
/// use maat_ast::{ast::*, transform};
/// use maat_span::Span;
///
/// // Double all integer values
/// let mut program = Program {
///     statements: vec![Stmt::Expr(ExprStmt {
///         value: Expr::I64(I64 {
///             radix: Radix::Dec,
///             value: 5,
///             span: Span::ZERO,
///         }),
///         span: Span::ZERO,
///     })],
/// };
///
/// let result = transform(Node::Program(program), &mut |node| {
///     match node {
///         Node::Expr(Expr::I64(mut i)) => {
///             i.value *= 2;
///             Node::Expr(Expr::I64(i))
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
///     if let Node::Expr(Expr::Call(call)) = &node {
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
                .map(|stmt| match transform(Node::Stmt(stmt), transformer) {
                    Node::Stmt(s) => s,
                    _ => unreachable!("Stmt transformation returned non-statement"),
                })
                .collect();
            Node::Program(program)
        }

        Node::Stmt(stmt) => {
            let new_stmt = match stmt {
                Stmt::Let(mut let_stmt) => {
                    let_stmt.value = match transform(Node::Expr(let_stmt.value), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Let(let_stmt)
                }

                Stmt::Return(mut ret_stmt) => {
                    ret_stmt.value = match transform(Node::Expr(ret_stmt.value), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Return(ret_stmt)
                }

                Stmt::Expr(mut expr_stmt) => {
                    expr_stmt.value = match transform(Node::Expr(expr_stmt.value), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Expr(expr_stmt)
                }

                Stmt::Block(mut block) => {
                    block.statements = block
                        .statements
                        .into_iter()
                        .map(|stmt| match transform(Node::Stmt(stmt), transformer) {
                            Node::Stmt(s) => s,
                            _ => unreachable!("Stmt transformation returned non-statement"),
                        })
                        .collect();
                    Stmt::Block(block)
                }

                Stmt::FnItem(mut fn_item) => {
                    fn_item.body =
                        match transform(Node::Stmt(Stmt::Block(fn_item.body)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::FnItem(fn_item)
                }

                Stmt::Loop(mut loop_stmt) => {
                    loop_stmt.body =
                        match transform(Node::Stmt(Stmt::Block(loop_stmt.body)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::Loop(loop_stmt)
                }

                Stmt::While(mut while_stmt) => {
                    while_stmt.condition = Box::new(
                        match transform(Node::Expr(*while_stmt.condition), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    while_stmt.body =
                        match transform(Node::Stmt(Stmt::Block(while_stmt.body)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::While(while_stmt)
                }

                Stmt::For(mut for_stmt) => {
                    for_stmt.iterable = Box::new(
                        match transform(Node::Expr(*for_stmt.iterable), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    for_stmt.body =
                        match transform(Node::Stmt(Stmt::Block(for_stmt.body)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::For(for_stmt)
                }
            };
            Node::Stmt(new_stmt)
        }

        Node::Expr(expr) => {
            let new_expr = match expr {
                Expr::Array(mut array) => {
                    array.elements = array
                        .elements
                        .into_iter()
                        .map(|elem| match transform(Node::Expr(elem), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::Array(array)
                }

                Expr::Index(mut index) => {
                    index.expr = Box::new(match transform(Node::Expr(*index.expr), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    index.index =
                        Box::new(match transform(Node::Expr(*index.index), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    Expr::Index(index)
                }

                Expr::Map(mut map) => {
                    map.pairs = map
                        .pairs
                        .into_iter()
                        .map(|(key, val)| {
                            let new_key = match transform(Node::Expr(key), transformer) {
                                Node::Expr(e) => e,
                                _ => {
                                    unreachable!("Expr transformation returned non-expression")
                                }
                            };
                            let new_val = match transform(Node::Expr(val), transformer) {
                                Node::Expr(e) => e,
                                _ => {
                                    unreachable!("Expr transformation returned non-expression")
                                }
                            };
                            (new_key, new_val)
                        })
                        .collect();
                    Expr::Map(map)
                }

                Expr::Prefix(mut prefix) => {
                    prefix.operand =
                        Box::new(match transform(Node::Expr(*prefix.operand), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    Expr::Prefix(prefix)
                }

                Expr::Infix(mut infix) => {
                    infix.lhs = Box::new(match transform(Node::Expr(*infix.lhs), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    infix.rhs = Box::new(match transform(Node::Expr(*infix.rhs), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    Expr::Infix(infix)
                }

                Expr::Cond(mut cond) => {
                    cond.condition =
                        Box::new(match transform(Node::Expr(*cond.condition), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    cond.consequence =
                        match transform(Node::Stmt(Stmt::Block(cond.consequence)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    if let Some(alt) = cond.alternative {
                        cond.alternative =
                            Some(match transform(Node::Stmt(Stmt::Block(alt)), transformer) {
                                Node::Stmt(Stmt::Block(b)) => b,
                                _ => unreachable!("Block transformation returned non-block"),
                            });
                    }
                    Expr::Cond(cond)
                }

                Expr::Lambda(mut lambda) => {
                    lambda.body = match transform(Node::Stmt(Stmt::Block(lambda.body)), transformer)
                    {
                        Node::Stmt(Stmt::Block(b)) => b,
                        _ => unreachable!("Block transformation returned non-block"),
                    };
                    Expr::Lambda(lambda)
                }

                Expr::Macro(mut macro_lit) => {
                    macro_lit.body =
                        match transform(Node::Stmt(Stmt::Block(macro_lit.body)), transformer) {
                            Node::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Expr::Macro(macro_lit)
                }

                Expr::Call(mut call) => {
                    call.function =
                        Box::new(match transform(Node::Expr(*call.function), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    call.arguments = call
                        .arguments
                        .into_iter()
                        .map(|arg| match transform(Node::Expr(arg), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::Call(call)
                }

                Expr::Cast(mut cast) => {
                    cast.expr = Box::new(match transform(Node::Expr(*cast.expr), transformer) {
                        Node::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    Expr::Cast(cast)
                }

                Expr::Break(mut break_expr) => {
                    break_expr.value = break_expr.value.map(|v| {
                        Box::new(match transform(Node::Expr(*v), transformer) {
                            Node::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                    });
                    Expr::Break(break_expr)
                }

                // Leaf nodes (literals, identifiers, continue) don't need transformation
                expr => expr,
            };
            Node::Expr(new_expr)
        }
    };

    transformer(node)
}
