use maat_ast::*;
use maat_span::Span;

fn one() -> Expr {
    Expr::Number(Number {
        kind: NumberKind::I64,
        value: 1,
        radix: Radix::Dec,
        span: Span::ZERO,
    })
}

fn two() -> Expr {
    Expr::Number(Number {
        kind: NumberKind::I64,
        value: 2,
        radix: Radix::Dec,
        span: Span::ZERO,
    })
}

fn turn_one_into_two(node: Node) -> Node {
    match node {
        Node::Expr(Expr::Number(n)) if n.kind == NumberKind::I64 && n.value == 1 => {
            Node::Expr(Expr::Number(Number {
                kind: n.kind,
                value: 2,
                radix: n.radix,
                span: n.span,
            }))
        }
        n => n,
    }
}

#[test]
fn transform_leaf_nodes() {
    let modified = transform(Node::Expr(one()), &mut turn_one_into_two);
    match modified {
        Node::Expr(Expr::Number(n)) => assert_eq!(n.value, 2),
        _ => panic!("expected Number expression"),
    }
}

#[test]
fn transform_statements() {
    let program = Program {
        statements: vec![
            Stmt::Let(LetStmt {
                mutable: false,
                ident: "x".to_string(),
                type_annotation: None,
                value: one(),
                span: Span::ZERO,
            }),
            Stmt::Return(ReturnStmt {
                value: one(),
                span: Span::ZERO,
            }),
            Stmt::Expr(ExprStmt {
                value: two(),
                span: Span::ZERO,
            }),
        ],
    };
    let Node::Program(prog) = transform(Node::Program(program), &mut turn_one_into_two) else {
        panic!("expected Program");
    };
    let Stmt::Let(ref ls) = prog.statements[0] else {
        panic!("expected Let");
    };
    assert!(matches!(&ls.value, Expr::Number(n) if n.value == 2));
    let Stmt::Return(ref rs) = prog.statements[1] else {
        panic!("expected Return");
    };
    assert!(matches!(&rs.value, Expr::Number(n) if n.value == 2));
    let Stmt::Expr(ref es) = prog.statements[2] else {
        panic!("expected Expr");
    };
    assert!(matches!(&es.value, Expr::Number(n) if n.value == 2));
}

#[test]
fn transform_compound_expressions() {
    let input = Expr::Infix(InfixExpr {
        lhs: Box::new(one()),
        operator: "+".to_string(),
        rhs: Box::new(Expr::Prefix(PrefixExpr {
            operator: "-".to_string(),
            operand: Box::new(one()),
            span: Span::ZERO,
        })),
        span: Span::ZERO,
    });
    let Node::Expr(Expr::Infix(infix)) = transform(Node::Expr(input), &mut turn_one_into_two)
    else {
        panic!("expected Infix");
    };
    assert!(matches!(*infix.lhs, Expr::Number(n) if n.value == 2));
    let Expr::Prefix(prefix) = *infix.rhs else {
        panic!("expected Prefix");
    };
    assert!(matches!(*prefix.operand, Expr::Number(n) if n.value == 2));
}

#[test]
fn transform_collections() {
    let array = Expr::Array(Array {
        elements: vec![one(), one()],
        span: Span::ZERO,
    });
    let Node::Expr(Expr::Array(arr)) = transform(Node::Expr(array), &mut turn_one_into_two) else {
        panic!("expected Array");
    };
    assert!(
        arr.elements
            .iter()
            .all(|e| matches!(e, Expr::Number(n) if n.value == 2))
    );
    let hash = Expr::Map(Map {
        pairs: vec![(one(), one())],
        span: Span::ZERO,
    });
    let Node::Expr(Expr::Map(h)) = transform(Node::Expr(hash), &mut turn_one_into_two) else {
        panic!("expected Map");
    };
    let (ref k, ref v) = h.pairs[0];
    assert!(matches!(k, Expr::Number(n) if n.value == 2));
    assert!(matches!(v, Expr::Number(n) if n.value == 2));
}

#[test]
fn transform_nested_structures() {
    let program = Program {
        statements: vec![Stmt::Let(LetStmt {
            mutable: false,
            ident: "x".to_string(),
            type_annotation: None,
            value: Expr::Call(CallExpr {
                function: Box::new(Expr::Lambda(Lambda {
                    params: vec![],
                    generic_params: vec![],
                    return_type: None,
                    body: BlockStmt {
                        statements: vec![Stmt::Expr(ExprStmt {
                            value: Expr::Cond(CondExpr {
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
                            }),
                            span: Span::ZERO,
                        })],
                        span: Span::ZERO,
                    },
                    span: Span::ZERO,
                })),
                arguments: vec![
                    one(),
                    Expr::Index(IndexExpr {
                        expr: Box::new(one()),
                        index: Box::new(one()),
                        span: Span::ZERO,
                    }),
                ],
                span: Span::ZERO,
            }),
            span: Span::ZERO,
        })],
    };

    let Node::Program(prog) = transform(Node::Program(program), &mut turn_one_into_two) else {
        panic!("expected Program");
    };

    // Every `1` in the tree should now be `2`.
    let display = prog.to_string();
    assert!(
        !display.contains(" 1"),
        "transform missed a node; display: {display}"
    );
}
