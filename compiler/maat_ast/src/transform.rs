//! Implements AST transformation via the visitor pattern.
//!
//! Provides the `transform` function for traversing and
//! modifying AST nodes for macro expansion and other AST modifications.

use crate::*;

/// A function that takes a node and returns a potentially modified one.
pub type TransformFn<'a> = &'a mut dyn FnMut(MaatAst) -> MaatAst;

/// Recursively traverses and modifies an AST node using the provided transformer function.
///
/// The traversal follows a post-order pattern: first, all child nodes are recursively
/// transformed, then the transformer function is applied to the current node. This ensures
/// that changes are applied from the leaves up to the root.
pub fn transform(node: MaatAst, transformer: TransformFn) -> MaatAst {
    let node = match node {
        MaatAst::Program(mut program) => {
            program.statements = program
                .statements
                .into_iter()
                .map(|stmt| match transform(MaatAst::Stmt(stmt), transformer) {
                    MaatAst::Stmt(s) => s,
                    _ => unreachable!("Stmt transformation returned non-statement"),
                })
                .collect();
            MaatAst::Program(program)
        }

        MaatAst::Stmt(stmt) => {
            let new_stmt = match stmt {
                Stmt::Let(mut let_stmt) => {
                    let_stmt.value = match transform(MaatAst::Expr(let_stmt.value), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Let(let_stmt)
                }

                Stmt::ReAssign(mut assign_stmt) => {
                    assign_stmt.value =
                        match transform(MaatAst::Expr(assign_stmt.value), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        };
                    Stmt::ReAssign(assign_stmt)
                }

                Stmt::Return(mut ret_stmt) => {
                    ret_stmt.value = match transform(MaatAst::Expr(ret_stmt.value), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Return(ret_stmt)
                }

                Stmt::Expr(mut expr_stmt) => {
                    expr_stmt.value = match transform(MaatAst::Expr(expr_stmt.value), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    };
                    Stmt::Expr(expr_stmt)
                }

                Stmt::Block(mut block) => {
                    block.statements = block
                        .statements
                        .into_iter()
                        .map(|stmt| match transform(MaatAst::Stmt(stmt), transformer) {
                            MaatAst::Stmt(s) => s,
                            _ => unreachable!("Stmt transformation returned non-statement"),
                        })
                        .collect();
                    Stmt::Block(block)
                }

                Stmt::FuncDef(mut fn_item) => {
                    fn_item.body =
                        match transform(MaatAst::Stmt(Stmt::Block(fn_item.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::FuncDef(fn_item)
                }

                Stmt::Loop(mut loop_stmt) => {
                    loop_stmt.body =
                        match transform(MaatAst::Stmt(Stmt::Block(loop_stmt.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::Loop(loop_stmt)
                }

                Stmt::While(mut while_stmt) => {
                    while_stmt.condition = Box::new(
                        match transform(MaatAst::Expr(*while_stmt.condition), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    while_stmt.body =
                        match transform(MaatAst::Stmt(Stmt::Block(while_stmt.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::While(while_stmt)
                }

                Stmt::For(mut for_stmt) => {
                    for_stmt.iterable = Box::new(
                        match transform(MaatAst::Expr(*for_stmt.iterable), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    for_stmt.body =
                        match transform(MaatAst::Stmt(Stmt::Block(for_stmt.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Stmt::For(for_stmt)
                }

                Stmt::Mod(mut mod_stmt) => {
                    if let Some(body) = mod_stmt.body {
                        mod_stmt.body = Some(
                            body.into_iter()
                                .map(|stmt| match transform(MaatAst::Stmt(stmt), transformer) {
                                    MaatAst::Stmt(s) => s,
                                    _ => {
                                        unreachable!("Stmt transformation returned non-statement")
                                    }
                                })
                                .collect(),
                        );
                    }
                    Stmt::Mod(mod_stmt)
                }

                // Type declaration and import statements are treated as leaves.
                // Their internal structure is not traversed by the general AST
                // transformer. Macro expansion targets expressions and bindings,
                // not type definitions or imports.
                stmt @ (Stmt::StructDecl(_)
                | Stmt::EnumDecl(_)
                | Stmt::TraitDecl(_)
                | Stmt::ImplBlock(_)
                | Stmt::Use(_)) => stmt,
            };
            MaatAst::Stmt(new_stmt)
        }

        MaatAst::Expr(expr) => {
            let new_expr = match expr {
                Expr::Vector(mut vector) => {
                    vector.elements = vector
                        .elements
                        .into_iter()
                        .map(|elem| match transform(MaatAst::Expr(elem), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::Vector(vector)
                }

                Expr::Index(mut index) => {
                    index.expr =
                        Box::new(match transform(MaatAst::Expr(*index.expr), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    index.index =
                        Box::new(match transform(MaatAst::Expr(*index.index), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    Expr::Index(index)
                }

                Expr::Map(mut map) => {
                    map.pairs = map
                        .pairs
                        .into_iter()
                        .map(|(key, val)| {
                            let new_key = match transform(MaatAst::Expr(key), transformer) {
                                MaatAst::Expr(e) => e,
                                _ => {
                                    unreachable!("Expr transformation returned non-expression")
                                }
                            };
                            let new_val = match transform(MaatAst::Expr(val), transformer) {
                                MaatAst::Expr(e) => e,
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
                    prefix.operand = Box::new(
                        match transform(MaatAst::Expr(*prefix.operand), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    Expr::Prefix(prefix)
                }

                Expr::Infix(mut infix) => {
                    infix.lhs = Box::new(match transform(MaatAst::Expr(*infix.lhs), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    infix.rhs = Box::new(match transform(MaatAst::Expr(*infix.rhs), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    Expr::Infix(infix)
                }

                Expr::Cond(mut cond) => {
                    cond.condition = Box::new(
                        match transform(MaatAst::Expr(*cond.condition), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    cond.consequence = match transform(
                        MaatAst::Stmt(Stmt::Block(cond.consequence)),
                        transformer,
                    ) {
                        MaatAst::Stmt(Stmt::Block(b)) => b,
                        _ => unreachable!("Block transformation returned non-block"),
                    };
                    if let Some(alt) = cond.alternative {
                        cond.alternative = Some(
                            match transform(MaatAst::Stmt(Stmt::Block(alt)), transformer) {
                                MaatAst::Stmt(Stmt::Block(b)) => b,
                                _ => unreachable!("Block transformation returned non-block"),
                            },
                        );
                    }
                    Expr::Cond(cond)
                }

                Expr::Lambda(mut lambda) => {
                    lambda.body =
                        match transform(MaatAst::Stmt(Stmt::Block(lambda.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Expr::Lambda(lambda)
                }

                Expr::MacroLit(mut macro_lit) => {
                    macro_lit.body =
                        match transform(MaatAst::Stmt(Stmt::Block(macro_lit.body)), transformer) {
                            MaatAst::Stmt(Stmt::Block(b)) => b,
                            _ => unreachable!("Block transformation returned non-block"),
                        };
                    Expr::MacroLit(macro_lit)
                }

                Expr::Call(mut call) => {
                    call.function = Box::new(
                        match transform(MaatAst::Expr(*call.function), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    call.arguments = call
                        .arguments
                        .into_iter()
                        .map(|arg| match transform(MaatAst::Expr(arg), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::Call(call)
                }

                Expr::MacroCall(mut mc) => {
                    mc.arguments = mc
                        .arguments
                        .into_iter()
                        .map(|arg| match transform(MaatAst::Expr(arg), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::MacroCall(mc)
                }

                Expr::Cast(mut cast) => {
                    cast.expr = Box::new(match transform(MaatAst::Expr(*cast.expr), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    Expr::Cast(cast)
                }

                Expr::Break(mut break_expr) => {
                    break_expr.value = break_expr.value.map(|v| {
                        Box::new(match transform(MaatAst::Expr(*v), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                    });
                    Expr::Break(break_expr)
                }

                Expr::Try(mut try_expr) => {
                    try_expr.expr = Box::new(
                        match transform(MaatAst::Expr(*try_expr.expr), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    Expr::Try(try_expr)
                }

                Expr::Match(mut match_expr) => {
                    match_expr.scrutinee = Box::new(
                        match transform(MaatAst::Expr(*match_expr.scrutinee), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    match_expr.arms = match_expr
                        .arms
                        .into_iter()
                        .map(|mut arm| {
                            arm.body = match transform(MaatAst::Expr(arm.body), transformer) {
                                MaatAst::Expr(e) => e,
                                _ => unreachable!("Expr transformation returned non-expression"),
                            };
                            if let Some(guard) = arm.guard {
                                arm.guard = Some(Box::new(
                                    match transform(MaatAst::Expr(*guard), transformer) {
                                        MaatAst::Expr(e) => e,
                                        _ => unreachable!(
                                            "Expr transformation returned non-expression"
                                        ),
                                    },
                                ));
                            }
                            arm
                        })
                        .collect();
                    Expr::Match(match_expr)
                }

                Expr::FieldAccess(mut field_access) => {
                    field_access.object = Box::new(
                        match transform(MaatAst::Expr(*field_access.object), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    Expr::FieldAccess(field_access)
                }

                Expr::MethodCall(mut method_call) => {
                    method_call.object = Box::new(
                        match transform(MaatAst::Expr(*method_call.object), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        },
                    );
                    method_call.arguments = method_call
                        .arguments
                        .into_iter()
                        .map(|arg| match transform(MaatAst::Expr(arg), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::MethodCall(method_call)
                }

                Expr::StructLit(mut struct_lit) => {
                    struct_lit.fields = struct_lit
                        .fields
                        .into_iter()
                        .map(|(name, val)| {
                            let new_val = match transform(MaatAst::Expr(val), transformer) {
                                MaatAst::Expr(e) => e,
                                _ => unreachable!("Expr transformation returned non-expression"),
                            };
                            (name, new_val)
                        })
                        .collect();
                    struct_lit.base = struct_lit.base.map(|base| {
                        Box::new(match transform(MaatAst::Expr(*base), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                    });
                    Expr::StructLit(struct_lit)
                }

                Expr::Tuple(mut tuple) => {
                    tuple.elements = tuple
                        .elements
                        .into_iter()
                        .map(|elem| match transform(MaatAst::Expr(elem), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        })
                        .collect();
                    Expr::Tuple(tuple)
                }

                Expr::Range(mut range) => {
                    range.start =
                        Box::new(match transform(MaatAst::Expr(*range.start), transformer) {
                            MaatAst::Expr(e) => e,
                            _ => unreachable!("Expr transformation returned non-expression"),
                        });
                    range.end = Box::new(match transform(MaatAst::Expr(*range.end), transformer) {
                        MaatAst::Expr(e) => e,
                        _ => unreachable!("Expr transformation returned non-expression"),
                    });
                    Expr::Range(range)
                }

                // Leaf nodes (literals, identifiers, continue, paths) need no recursive traversal
                expr => expr,
            };
            MaatAst::Expr(new_expr)
        }
    };

    transformer(node)
}
