use maat_ast::*;
use maat_span::Span;

fn span() -> Span {
    Span::ZERO
}

#[test]
fn program() {
    // Empty program
    let prog = Program { statements: vec![] };
    assert_eq!(prog.to_string(), "");

    // Program with a single let statement
    let stmt = Statement::Let(LetStatement {
        ident: "x".to_string(),
        type_annotation: None,
        value: Expression::I32(I32 {
            radix: Radix::Dec,
            value: 42,
            span: span(),
        }),
        span: span(),
    });
    let prog = Program {
        statements: vec![stmt],
    };
    assert_eq!(prog.to_string(), "let x = 42;");

    // Program with multiple statements
    let stmt1 = Statement::Return(ReturnStatement {
        value: Expression::Boolean(BooleanLiteral {
            value: true,
            span: span(),
        }),
        span: span(),
    });
    let stmt2 = Statement::Expression(ExpressionStatement {
        value: Expression::Identifier(Ident {
            value: "foo".to_string(),
            span: span(),
        }),
        span: span(),
    });
    let prog = Program {
        statements: vec![stmt1, stmt2],
    };
    assert_eq!(prog.to_string(), "return true;foo");
}

#[test]
fn let_statement() {
    // Without type annotation
    let let_stmt = LetStatement {
        ident: "x".to_string(),
        type_annotation: None,
        value: Expression::I64(I64 {
            radix: Radix::Dec,
            value: 100,
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(let_stmt.to_string(), "let x = 100;");

    // With type annotation
    let type_ann = TypeExpr::Named(NamedType {
        name: "i32".to_string(),
        span: span(),
    });
    let let_stmt = LetStatement {
        ident: "y".to_string(),
        type_annotation: Some(type_ann),
        value: Expression::U8(U8 {
            radix: Radix::Hex,
            value: 0xff,
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(let_stmt.to_string(), "let y: i32 = 0xff;");
}

#[test]
fn return_statement() {
    let ret = ReturnStatement {
        value: Expression::Identifier(Ident {
            value: "result".to_string(),
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(ret.to_string(), "return result;");
}

#[test]
fn expression_statement() {
    let expr_stmt = ExpressionStatement {
        value: Expression::Call(CallExpr {
            function: Box::new(Expression::Identifier(Ident {
                value: "foo".to_string(),
                span: span(),
            })),
            arguments: vec![
                Expression::I32(I32 {
                    radix: Radix::Dec,
                    value: 1,
                    span: span(),
                }),
                Expression::I32(I32 {
                    radix: Radix::Dec,
                    value: 2,
                    span: span(),
                }),
            ],
            span: span(),
        }),
        span: span(),
    };
    assert_eq!(expr_stmt.to_string(), "foo(1, 2)");
}

#[test]
fn block_statement() {
    // Empty block
    let block = BlockStatement {
        statements: vec![],
        span: span(),
    };
    assert_eq!(block.to_string(), "{}");

    // Block with one statement
    let stmt = Statement::Let(LetStatement {
        ident: "a".to_string(),
        type_annotation: None,
        value: Expression::Boolean(BooleanLiteral {
            value: false,
            span: span(),
        }),
        span: span(),
    });
    let block = BlockStatement {
        statements: vec![stmt],
        span: span(),
    };
    assert_eq!(block.to_string(), "{\nlet a = false;\n}");

    // Block with multiple statements
    let stmt1 = Statement::Return(ReturnStatement {
        value: Expression::I32(I32 {
            radix: Radix::Dec,
            value: 10,
            span: span(),
        }),
        span: span(),
    });
    let stmt2 = Statement::Expression(ExpressionStatement {
        value: Expression::Identifier(Ident {
            value: "b".to_string(),
            span: span(),
        }),
        span: span(),
    });
    let block = BlockStatement {
        statements: vec![stmt1, stmt2],
        span: span(),
    };
    assert_eq!(block.to_string(), "{\nreturn 10;\nb\n}");
}

#[test]
fn array_literal() {
    let empty = ArrayLiteral {
        elements: vec![],
        span: span(),
    };
    assert_eq!(empty.to_string(), "[]");

    let single = ArrayLiteral {
        elements: vec![Expression::Boolean(BooleanLiteral {
            value: true,
            span: span(),
        })],
        span: span(),
    };
    assert_eq!(single.to_string(), "[true]");

    let multiple = ArrayLiteral {
        elements: vec![
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 1,
                span: span(),
            }),
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 2,
                span: span(),
            }),
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 3,
                span: span(),
            }),
        ],
        span: span(),
    };
    assert_eq!(multiple.to_string(), "[1, 2, 3]");
}

#[test]
fn index_expression() {
    let index = IndexExpr {
        expr: Box::new(Expression::Identifier(Ident {
            value: "arr".to_string(),
            span: span(),
        })),
        index: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 5,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(index.to_string(), "(arr[5])");
}

#[test]
fn hash_literal() {
    let empty = HashLiteral {
        pairs: vec![],
        span: span(),
    };
    assert_eq!(empty.to_string(), "{}");

    let single = HashLiteral {
        pairs: vec![(
            Expression::String(StringLiteral {
                value: "a".to_string(),
                span: span(),
            }),
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 1,
                span: span(),
            }),
        )],
        span: span(),
    };
    assert_eq!(single.to_string(), "{a: 1}");

    let multiple = HashLiteral {
        pairs: vec![
            (
                Expression::String(StringLiteral {
                    value: "x".to_string(),
                    span: span(),
                }),
                Expression::I32(I32 {
                    radix: Radix::Dec,
                    value: 10,
                    span: span(),
                }),
            ),
            (
                Expression::String(StringLiteral {
                    value: "y".to_string(),
                    span: span(),
                }),
                Expression::I32(I32 {
                    radix: Radix::Dec,
                    value: 20,
                    span: span(),
                }),
            ),
        ],
        span: span(),
    };
    assert_eq!(multiple.to_string(), "{x: 10, y: 20}");
}

#[test]
fn prefix_expression() {
    let neg = PrefixExpr {
        operator: "-".to_string(),
        operand: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 42,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(neg.to_string(), "(-42)");

    let not = PrefixExpr {
        operator: "!".to_string(),
        operand: Box::new(Expression::Boolean(BooleanLiteral {
            value: true,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(not.to_string(), "(!true)");
}

#[test]
fn infix_expression() {
    let add = InfixExpr {
        lhs: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 1,
            span: span(),
        })),
        operator: "+".to_string(),
        rhs: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 2,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(add.to_string(), "(1 + 2)");

    let eq = InfixExpr {
        lhs: Box::new(Expression::Identifier(Ident {
            value: "x".to_string(),
            span: span(),
        })),
        operator: "==".to_string(),
        rhs: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 0,
            span: span(),
        })),
        span: span(),
    };
    assert_eq!(eq.to_string(), "(x == 0)");
}

#[test]
fn conditional_expression() {
    let condition = Expression::Identifier(Ident {
        value: "cond".to_string(),
        span: span(),
    });
    let consequence = BlockStatement {
        statements: vec![Statement::Let(LetStatement {
            ident: "x".to_string(),
            type_annotation: None,
            value: Expression::I32(I32 {
                radix: Radix::Dec,
                value: 1,
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };
    let alternative = BlockStatement {
        statements: vec![Statement::Let(LetStatement {
            ident: "x".to_string(),
            type_annotation: None,
            value: Expression::I32(I32 {
                radix: Radix::Dec,
                value: 2,
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };

    // If without else
    let cond_if = ConditionalExpr {
        condition: Box::new(condition.clone()),
        consequence: consequence.clone(),
        alternative: None,
        span: span(),
    };
    assert_eq!(cond_if.to_string(), "if cond {\nlet x = 1;\n}");

    // If with else
    let cond_if_else = ConditionalExpr {
        condition: Box::new(condition),
        consequence,
        alternative: Some(alternative),
        span: span(),
    };
    assert_eq!(
        cond_if_else.to_string(),
        "if cond {\nlet x = 1;\n} else {\nlet x = 2;\n}"
    );
}

#[test]
fn function() {
    // Anonymous function with no generics, no return type, empty body
    let func_anon = Function {
        name: None,
        params: vec![],
        generic_params: vec![],
        return_type: None,
        body: BlockStatement {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(func_anon.to_string(), "fn() {}");

    // Named function with parameters, generics, return type, non-empty body
    let param1 = TypedParam {
        name: "a".to_string(),
        type_expr: Some(TypeExpr::Named(NamedType {
            name: "T".to_string(),
            span: span(),
        })),
        span: span(),
    };
    let param2 = TypedParam {
        name: "b".to_string(),
        type_expr: Some(TypeExpr::Named(NamedType {
            name: "i64".to_string(),
            span: span(),
        })),
        span: span(),
    };
    let generic = GenericParam {
        name: "T".to_string(),
        bounds: vec![TraitBound {
            name: "Copy".to_string(),
            span: span(),
        }],
        span: span(),
    };
    let return_ty = TypeExpr::Named(NamedType {
        name: "T".to_string(),
        span: span(),
    });
    let body = BlockStatement {
        statements: vec![Statement::Return(ReturnStatement {
            value: Expression::Identifier(Ident {
                value: "a".to_string(),
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };

    let func_named = Function {
        name: Some("identity".to_string()),
        params: vec![param1, param2],
        generic_params: vec![generic],
        return_type: Some(return_ty),
        body: body.clone(),
        span: span(),
    };
    assert_eq!(
        func_named.to_string(),
        "fn identity<T: Copy>(a: T, b: i64) -> T {\nreturn a;\n}"
    );

    let generic_no_bounds = GenericParam {
        name: "U".to_string(),
        bounds: vec![],
        span: span(),
    };
    let func_simple = Function {
        name: Some("identity".to_string()),
        params: vec![],
        generic_params: vec![generic_no_bounds],
        return_type: None,
        body,
        span: span(),
    };
    assert_eq!(func_simple.to_string(), "fn identity<U>() {\nreturn a;\n}");
}

#[test]
fn macro_literal() {
    // No parameters, empty body
    let macro_empty = MacroLiteral {
        params: vec![],
        body: BlockStatement {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(macro_empty.to_string(), "macro() {}");

    // Parameters and body
    let body = BlockStatement {
        statements: vec![Statement::Expression(ExpressionStatement {
            value: Expression::Identifier(Ident {
                value: "x".to_string(),
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };
    let macro_with_params = MacroLiteral {
        params: vec!["$a".to_string(), "$b".to_string()],
        body,
        span: span(),
    };
    assert_eq!(macro_with_params.to_string(), "macro($a, $b) {\nx\n}");
}

#[test]
fn call_expression() {
    let call_empty = CallExpr {
        function: Box::new(Expression::Identifier(Ident {
            value: "f".to_string(),
            span: span(),
        })),
        arguments: vec![],
        span: span(),
    };
    assert_eq!(call_empty.to_string(), "f()");

    let call_one = CallExpr {
        function: Box::new(Expression::Identifier(Ident {
            value: "f".to_string(),
            span: span(),
        })),
        arguments: vec![Expression::I32(I32 {
            radix: Radix::Dec,
            value: 42,
            span: span(),
        })],
        span: span(),
    };
    assert_eq!(call_one.to_string(), "f(42)");

    let call_multi = CallExpr {
        function: Box::new(Expression::Identifier(Ident {
            value: "add".to_string(),
            span: span(),
        })),
        arguments: vec![
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 1,
                span: span(),
            }),
            Expression::I32(I32 {
                radix: Radix::Dec,
                value: 2,
                span: span(),
            }),
        ],
        span: span(),
    };
    assert_eq!(call_multi.to_string(), "add(1, 2)");
}

#[test]
fn type_cast() {
    let i32_to_i64 = CastExpr {
        expr: Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 42,
            span: span(),
        })),
        target: TypeAnnotation::I64,
        span: span(),
    };
    assert_eq!(i32_to_i64.to_string(), "(42 as i64)");

    let f64_to_u32 = CastExpr {
        expr: Box::new(Expression::F64(F64::from(2.56f64))),
        target: TypeAnnotation::U32,
        span: span(),
    };
    assert_eq!(f64_to_u32.to_string(), "(2.56 as u32)");
}

#[test]
fn infinite_loop() {
    let empty_loop = LoopStatement {
        body: BlockStatement {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(empty_loop.to_string(), "loop {}");

    // A loop with one `break;` statement
    let body = BlockStatement {
        statements: vec![Statement::Expression(ExpressionStatement {
            value: Expression::Break(BreakExpr {
                value: None,
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };
    let loop_with_break = LoopStatement { body, span: span() };
    assert_eq!(loop_with_break.to_string(), "loop {\nbreak\n}");
}

#[test]
fn while_loop() {
    let condition = Expression::Identifier(Ident {
        value: "cond".to_string(),
        span: span(),
    });
    let empty_body = WhileStatement {
        condition: Box::new(condition.clone()),
        body: BlockStatement {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(empty_body.to_string(), "while cond {}");

    let body = BlockStatement {
        statements: vec![Statement::Expression(ExpressionStatement {
            value: Expression::Call(CallExpr {
                function: Box::new(Expression::Identifier(Ident {
                    value: "work".to_string(),
                    span: span(),
                })),
                arguments: vec![],
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };
    let while_stmt = WhileStatement {
        condition: Box::new(condition),
        body,
        span: span(),
    };
    assert_eq!(while_stmt.to_string(), "while cond {\nwork()\n}");
}

#[test]
fn for_loop() {
    let iterable = Expression::Identifier(Ident {
        value: "0..10".to_string(),
        span: span(),
    });
    let empty_body = ForStatement {
        ident: "i".to_string(),
        iterable: Box::new(iterable.clone()),
        body: BlockStatement {
            statements: vec![],
            span: span(),
        },
        span: span(),
    };
    assert_eq!(empty_body.to_string(), "for i in 0..10 {}");

    let body = BlockStatement {
        statements: vec![Statement::Expression(ExpressionStatement {
            value: Expression::Call(CallExpr {
                function: Box::new(Expression::Identifier(Ident {
                    value: "println".to_string(),
                    span: span(),
                })),
                arguments: vec![Expression::Identifier(Ident {
                    value: "i".to_string(),
                    span: span(),
                })],
                span: span(),
            }),
            span: span(),
        })],
        span: span(),
    };
    let for_stmt = ForStatement {
        ident: "i".to_string(),
        iterable: Box::new(iterable),
        body,
        span: span(),
    };
    assert_eq!(for_stmt.to_string(), "for i in 0..10 {\nprintln(i)\n}");
}

#[test]
fn break_expression() {
    let break_no_val = BreakExpr {
        value: None,
        span: span(),
    };
    assert_eq!(break_no_val.to_string(), "break");

    let break_with_val = BreakExpr {
        value: Some(Box::new(Expression::I32(I32 {
            radix: Radix::Dec,
            value: 42,
            span: span(),
        }))),
        span: span(),
    };
    assert_eq!(break_with_val.to_string(), "break 42");
}

#[test]
fn continue_expression() {
    let cont = ContinueExpr { span: span() };
    assert_eq!(cont.to_string(), "continue");
}
