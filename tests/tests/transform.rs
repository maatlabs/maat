use maat_ast::*;
use maat_span::Span;

fn one() -> Expr {
    Expr::I64(I64 {
        radix: Radix::Dec,
        value: 1,
        span: Span::ZERO,
    })
}

fn two() -> Expr {
    Expr::I64(I64 {
        radix: Radix::Dec,
        value: 2,
        span: Span::ZERO,
    })
}

fn turn_one_into_two(node: Node) -> Node {
    match node {
        Node::Expr(Expr::I64(i)) if i.value == 1 => Node::Expr(Expr::I64(I64 {
            radix: i.radix,
            value: 2,
            span: i.span,
        })),
        n => n,
    }
}

#[test]
fn transform_integers() {
    let input = one();
    let modified = transform(Node::Expr(input), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::I64(i)) => assert_eq!(i.value, 2),
        _ => panic!("Expected I64 expression"),
    }
}

#[test]
fn transform_program() {
    let program = Program {
        statements: vec![
            Stmt::Expr(ExprStmt {
                value: one(),
                span: Span::ZERO,
            }),
            Stmt::Expr(ExprStmt {
                value: two(),
                span: Span::ZERO,
            }),
        ],
    };

    let modified = transform(Node::Program(program), &mut turn_one_into_two);

    match modified {
        Node::Program(prog) => {
            assert_eq!(prog.statements.len(), 2);

            match &prog.statements[0] {
                Stmt::Expr(ExprStmt {
                    value: Expr::I64(i),
                    ..
                }) => {
                    assert_eq!(i.value, 2);
                }
                _ => panic!("Expected I64 expression in first statement"),
            }

            match &prog.statements[1] {
                Stmt::Expr(ExprStmt {
                    value: Expr::I64(i),
                    ..
                }) => {
                    assert_eq!(i.value, 2);
                }
                _ => panic!("Expected I64 expression in second statement"),
            }
        }
        _ => panic!("Expected Program node"),
    }
}

#[test]
fn transform_infix_expression() {
    let input = Expr::Infix(InfixExpr {
        lhs: Box::new(one()),
        operator: "+".to_string(),
        rhs: Box::new(two()),
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(input), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Infix(infix)) => {
            match *infix.lhs {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for lhs"),
            }
            match *infix.rhs {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for rhs"),
            }
        }
        _ => panic!("Expected Infix expression"),
    }
}

#[test]
fn transform_prefix_expression() {
    let input = Expr::Prefix(PrefixExpr {
        operator: "-".to_string(),
        operand: Box::new(one()),
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(input), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Prefix(prefix)) => match *prefix.operand {
            Expr::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64 for operand"),
        },
        _ => panic!("Expected Prefix expression"),
    }
}

#[test]
fn transform_index_expression() {
    let input = Expr::Index(IndexExpr {
        expr: Box::new(one()),
        index: Box::new(one()),
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(input), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Index(index)) => {
            match *index.expr {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for expr"),
            }
            match *index.index {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for index"),
            }
        }
        _ => panic!("Expected Index expression"),
    }
}

#[test]
fn transform_conditional_expression() {
    let input = Expr::Cond(CondExpr {
        condition: Box::new(one()),
        consequence: BlockStmt {
            statements: vec![Stmt::Expr(ExprStmt {
                value: one(),
                span: Span::ZERO,
            })],
            span: Span::ZERO,
        },
        alternative: Some(BlockStmt {
            statements: vec![Stmt::Expr(ExprStmt {
                value: one(),
                span: Span::ZERO,
            })],
            span: Span::ZERO,
        }),
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(input), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Cond(cond)) => {
            match *cond.condition {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for condition"),
            }

            match &cond.consequence.statements[0] {
                Stmt::Expr(ExprStmt {
                    value: Expr::I64(i),
                    ..
                }) => {
                    assert_eq!(i.value, 2);
                }
                _ => panic!("Expected I64 in consequence"),
            }

            if let Some(alt) = cond.alternative {
                match &alt.statements[0] {
                    Stmt::Expr(ExprStmt {
                        value: Expr::I64(i),
                        ..
                    }) => {
                        assert_eq!(i.value, 2);
                    }
                    _ => panic!("Expected I64 in alternative"),
                }
            } else {
                panic!("Expected alternative");
            }
        }
        _ => panic!("Expected Cond expression"),
    }
}

#[test]
fn transform_return_statement() {
    let stmt = Stmt::Return(ReturnStmt {
        value: one(),
        span: Span::ZERO,
    });

    let modified = transform(Node::Stmt(stmt), &mut turn_one_into_two);

    match modified {
        Node::Stmt(Stmt::Return(ret)) => match ret.value {
            Expr::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64"),
        },
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn transform_let_statement() {
    let stmt = Stmt::Let(LetStmt {
        mutable: false,
        ident: "x".to_string(),
        type_annotation: None,
        value: one(),
        span: Span::ZERO,
    });

    let modified = transform(Node::Stmt(stmt), &mut turn_one_into_two);

    match modified {
        Node::Stmt(Stmt::Let(let_stmt)) => match let_stmt.value {
            Expr::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64"),
        },
        _ => panic!("Expected Let statement"),
    }
}

#[test]
fn transform_function_literal() {
    let func = Expr::Lambda(Lambda {
        params: vec![TypedParam {
            name: "x".to_string(),
            type_expr: None,
            span: Span::ZERO,
        }],
        generic_params: vec![],
        return_type: None,
        body: BlockStmt {
            statements: vec![Stmt::Expr(ExprStmt {
                value: one(),
                span: Span::ZERO,
            })],
            span: Span::ZERO,
        },
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(func), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Lambda(f)) => match &f.body.statements[0] {
            Stmt::Expr(ExprStmt {
                value: Expr::I64(i),
                ..
            }) => {
                assert_eq!(i.value, 2);
            }
            _ => panic!("Expected I64 in function body"),
        },
        _ => panic!("Expected Lambda expression"),
    }
}

#[test]
fn transform_function_call() {
    let call = Expr::Call(CallExpr {
        function: Box::new(Expr::Ident(Ident {
            value: "myFunc".to_string(),
            span: Span::ZERO,
        })),
        arguments: vec![one(), one()],
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(call), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Call(c)) => {
            assert_eq!(c.arguments.len(), 2);

            match &c.arguments[0] {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for first argument"),
            }

            match &c.arguments[1] {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for second argument"),
            }
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn transform_array_literal() {
    let array = Expr::Array(Array {
        elements: vec![one(), one()],
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(array), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Array(arr)) => {
            assert_eq!(arr.elements.len(), 2);

            match &arr.elements[0] {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for first element"),
            }

            match &arr.elements[1] {
                Expr::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for second element"),
            }
        }
        _ => panic!("Expected Array expression"),
    }
}

#[test]
fn transform_hash_literal() {
    let hash = Expr::Map(Map {
        pairs: vec![(one(), one()), (one(), one())],
        span: Span::ZERO,
    });

    let modified = transform(Node::Expr(hash), &mut turn_one_into_two);

    match modified {
        Node::Expr(Expr::Map(h)) => {
            assert_eq!(h.pairs.len(), 2);

            for (key, value) in &h.pairs {
                match key {
                    Expr::I64(i) => assert_eq!(i.value, 2),
                    _ => panic!("Expected I64 for key"),
                }
                match value {
                    Expr::I64(i) => assert_eq!(i.value, 2),
                    _ => panic!("Expected I64 for value"),
                }
            }
        }
        _ => panic!("Expected Hash expression"),
    }
}

#[test]
fn transform_nested_structures() {
    let program = Program {
        statements: vec![Stmt::Let(LetStmt {
            mutable: false,
            ident: "x".to_string(),
            type_annotation: None,
            value: Expr::Infix(InfixExpr {
                lhs: Box::new(Expr::Array(Array {
                    elements: vec![one(), two()],
                    span: Span::ZERO,
                })),
                operator: "+".to_string(),
                rhs: Box::new(Expr::Map(Map {
                    pairs: vec![(one(), two())],
                    span: Span::ZERO,
                })),
                span: Span::ZERO,
            }),
            span: Span::ZERO,
        })],
    };

    let modified = transform(Node::Program(program), &mut turn_one_into_two);

    match modified {
        Node::Program(prog) => match &prog.statements[0] {
            Stmt::Let(let_stmt) => match &let_stmt.value {
                Expr::Infix(infix) => {
                    match &*infix.lhs {
                        Expr::Array(arr) => match &arr.elements[0] {
                            Expr::I64(i) => assert_eq!(i.value, 2),
                            _ => panic!("Expected modified value in array"),
                        },
                        _ => panic!("Expected Array"),
                    }

                    match &*infix.rhs {
                        Expr::Map(h) => {
                            let (key, _) = &h.pairs[0];
                            match key {
                                Expr::I64(i) => assert_eq!(i.value, 2),
                                _ => panic!("Expected modified key in hash"),
                            }
                        }
                        _ => panic!("Expected Hash"),
                    }
                }
                _ => panic!("Expected Infix expression"),
            },
            _ => panic!("Expected Let statement"),
        },
        _ => panic!("Expected Program"),
    }
}
