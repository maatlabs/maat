use maat_ast::*;
use maat_span::Span;

fn span() -> Span {
    Span::ZERO
}

#[test]
fn statements_display() {
    let let_stmt = LetStmt {
        mutable: false,
        ident: "x".to_string(),
        type_annotation: None,
        value: Expr::Number(Number {
            kind: NumberKind::I64,
            value: 100,
            radix: Radix::Dec,
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(let_stmt.to_string(), "let x = 100;");

    let type_ann = TypeExpr::Named(NamedType {
        name: "i32".to_string(),
        span: span(),
    });
    let let_typed = LetStmt {
        mutable: false,
        ident: "y".to_string(),
        type_annotation: Some(type_ann),
        value: Expr::Number(Number {
            kind: NumberKind::U8,
            value: 0xff,
            radix: Radix::Hex,
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(let_typed.to_string(), "let y: i32 = 0xff;");

    let ret = ReturnStmt {
        value: Expr::Ident(Ident {
            value: "result".to_string(),
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(ret.to_string(), "return result;");

    let block = BlockStmt {
        statements: vec![
            Stmt::Return(ReturnStmt {
                value: Expr::Number(Number {
                    kind: NumberKind::I32,
                    value: 10,
                    radix: Radix::Dec,
                    span: span(),
                }),
                span: span(),
            }),
            Stmt::Expr(ExprStmt {
                value: Expr::Ident(Ident {
                    value: "b".to_string(),
                    span: span(),
                }),
                span: span(),
            }),
        ],
        span: span(),
    };
    assert_eq!(block.to_string(), "{\nreturn 10;\nb;\n}");

    let prog = Program {
        statements: vec![Stmt::Let(LetStmt {
            mutable: false,
            ident: "x".to_string(),
            type_annotation: None,
            value: Expr::Number(Number {
                kind: NumberKind::I32,
                value: 42,
                radix: Radix::Dec,
                span: span(),
            }),
            span: span(),
        })],
    };
    assert_eq!(prog.to_string(), "let x = 42;");
}

#[test]
fn expressions_display() {
    let neg = PrefixExpr {
        operator: "-".to_string(),
        operand: Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 42,
            radix: Radix::Dec,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(neg.to_string(), "(-42)");

    let add = InfixExpr {
        lhs: Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 1,
            radix: Radix::Dec,
            span: span(),
        })),
        operator: "+".to_string(),
        rhs: Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 2,
            radix: Radix::Dec,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(add.to_string(), "(1 + 2)");

    let cast = CastExpr {
        expr: Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 42,
            radix: Radix::Dec,
            span: span(),
        })),
        target: TypeAnnotation::I64,
        span: span(),
    };
    assert_eq!(cast.to_string(), "(42 as i64)");

    let index = IndexExpr {
        expr: Box::new(Expr::Ident(Ident {
            value: "arr".to_string(),
            span: span(),
        })),
        index: Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 5,
            radix: Radix::Dec,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(index.to_string(), "(arr[5])");
}

#[test]
fn collections_display() {
    let empty_arr = Vector {
        elements: vec![],
        span: span(),
    };
    assert_eq!(empty_arr.to_string(), "[]");

    let arr = Vector {
        elements: vec![
            Expr::Number(Number {
                kind: NumberKind::I32,
                value: 1,
                radix: Radix::Dec,
                span: span(),
            }),
            Expr::Number(Number {
                kind: NumberKind::I32,
                value: 2,
                radix: Radix::Dec,
                span: span(),
            }),
        ],
        span: span(),
    };
    assert_eq!(arr.to_string(), "[1, 2]");

    let map = Map {
        pairs: vec![(
            Expr::Str(Str {
                value: "x".to_string(),
                span: span(),
            }),
            Expr::Number(Number {
                kind: NumberKind::I32,
                value: 10,
                radix: Radix::Dec,
                span: span(),
            }),
        )],
        span: span(),
    };
    assert_eq!(map.to_string(), "{x: 10}");
}

#[test]
fn control_flow_display() {
    let cond = CondExpr {
        condition: Box::new(Expr::Ident(Ident {
            value: "cond".to_string(),
            span: span(),
        })),
        consequence: BlockStmt {
            statements: vec![Stmt::Let(LetStmt {
                mutable: false,
                ident: "x".to_string(),
                type_annotation: None,
                value: Expr::Number(Number {
                    kind: NumberKind::I32,
                    value: 1,
                    radix: Radix::Dec,
                    span: span(),
                }),
                span: span(),
            })],
            span: span(),
        },
        alternative: Some(BlockStmt {
            statements: vec![Stmt::Let(LetStmt {
                mutable: false,
                ident: "x".to_string(),
                type_annotation: None,
                value: Expr::Number(Number {
                    kind: NumberKind::I32,
                    value: 2,
                    radix: Radix::Dec,
                    span: span(),
                }),
                span: span(),
            })],
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(
        cond.to_string(),
        "if cond {\nlet x = 1;\n} else {\nlet x = 2;\n}"
    );

    let while_stmt = WhileStmt {
        label: None,
        condition: Box::new(Expr::Ident(Ident {
            value: "cond".to_string(),
            span: span(),
        })),
        body: BlockStmt {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(while_stmt.to_string(), "while cond {}");

    let for_stmt = ForStmt {
        label: None,
        ident: "i".to_string(),
        iterable: Box::new(Expr::Ident(Ident {
            value: "0..10".to_string(),
            span: span(),
        })),
        body: BlockStmt {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(for_stmt.to_string(), "for i in 0..10 {}");

    let loop_stmt = LoopStmt {
        label: None,
        body: BlockStmt {
            statements: vec![Stmt::Expr(ExprStmt {
                value: Expr::Break(BreakExpr {
                    label: None,
                    value: None,
                    span: span(),
                }),
                span: span(),
            })],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(loop_stmt.to_string(), "loop {\nbreak;\n}");

    let break_val = BreakExpr {
        label: None,
        value: Some(Box::new(Expr::Number(Number {
            kind: NumberKind::I32,
            value: 42,
            radix: Radix::Dec,
            span: span(),
        }))),
        span: span(),
    };
    assert_eq!(break_val.to_string(), "break 42");
    assert_eq!(
        ContinueExpr {
            label: None,
            span: span()
        }
        .to_string(),
        "continue"
    );
}

#[test]
fn function_display() {
    let lambda = Lambda {
        params: vec![],
        generic_params: vec![],
        return_type: None,
        body: BlockStmt {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(lambda.to_string(), "fn() {}");

    let func = FuncDef {
        name: "identity".to_string(),
        params: vec![
            TypedParam {
                name: "a".to_string(),
                type_expr: Some(TypeExpr::Named(NamedType {
                    name: "T".to_string(),
                    span: span(),
                })),
                span: span(),
            },
            TypedParam {
                name: "b".to_string(),
                type_expr: Some(TypeExpr::Named(NamedType {
                    name: "i64".to_string(),
                    span: span(),
                })),
                span: span(),
            },
        ],
        generic_params: vec![GenericParam {
            name: "T".to_string(),
            bounds: vec![TraitBound {
                name: "Copy".to_string(),
                span: span(),
            }],
            span: span(),
        }],
        return_type: Some(TypeExpr::Named(NamedType {
            name: "T".to_string(),
            span: span(),
        })),
        body: BlockStmt {
            statements: vec![Stmt::Return(ReturnStmt {
                value: Expr::Ident(Ident {
                    value: "a".to_string(),
                    span: span(),
                }),
                span: span(),
            })],
            span: span(),
        },
        is_public: false,
        span: span(),
    };
    assert_eq!(
        func.to_string(),
        "fn identity<T: Copy>(a: T, b: i64) -> T {\nreturn a;\n}"
    );

    let call = CallExpr {
        function: Box::new(Expr::Ident(Ident {
            value: "add".to_string(),
            span: span(),
        })),
        arguments: vec![
            Expr::Number(Number {
                kind: NumberKind::I32,
                value: 1,
                radix: Radix::Dec,
                span: span(),
            }),
            Expr::Number(Number {
                kind: NumberKind::I32,
                value: 2,
                radix: Radix::Dec,
                span: span(),
            }),
        ],
        span: span(),
    };
    assert_eq!(call.to_string(), "add(1, 2)");

    let macro_lit = Macro {
        params: vec!["$a".to_string(), "$b".to_string()],
        body: BlockStmt {
            statements: vec![Stmt::Expr(ExprStmt {
                value: Expr::Ident(Ident {
                    value: "x".to_string(),
                    span: span(),
                }),
                span: span(),
            })],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(macro_lit.to_string(), "macro($a, $b) {\nx;\n}");
}
