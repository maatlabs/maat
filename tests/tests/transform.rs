use maat_ast::*;

fn one() -> Expression {
    Expression::I64(I64 {
        radix: Radix::Dec,
        value: 1,
    })
}

fn two() -> Expression {
    Expression::I64(I64 {
        radix: Radix::Dec,
        value: 2,
    })
}

fn turn_one_into_two(node: Node) -> Node {
    match node {
        Node::Expression(Expression::I64(i)) if i.value == 1 => {
            Node::Expression(Expression::I64(I64 {
                radix: i.radix,
                value: 2,
            }))
        }
        n => n,
    }
}

#[test]
fn transform_integers() {
    let input = one();
    let modified = transform(Node::Expression(input), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::I64(i)) => assert_eq!(i.value, 2),
        _ => panic!("Expected I64 expression"),
    }
}

#[test]
fn transform_program() {
    let program = Program {
        statements: vec![
            Statement::Expression(ExpressionStatement { value: one() }),
            Statement::Expression(ExpressionStatement { value: two() }),
        ],
    };

    let modified = transform(Node::Program(program), &mut turn_one_into_two);

    match modified {
        Node::Program(prog) => {
            assert_eq!(prog.statements.len(), 2);

            match &prog.statements[0] {
                Statement::Expression(ExpressionStatement {
                    value: Expression::I64(i),
                }) => {
                    assert_eq!(i.value, 2);
                }
                _ => panic!("Expected I64 expression in first statement"),
            }

            match &prog.statements[1] {
                Statement::Expression(ExpressionStatement {
                    value: Expression::I64(i),
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
    let input = Expression::Infix(InfixExpr {
        lhs: Box::new(one()),
        operator: "+".to_string(),
        rhs: Box::new(two()),
    });

    let modified = transform(Node::Expression(input), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Infix(infix)) => {
            match *infix.lhs {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for lhs"),
            }
            match *infix.rhs {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for rhs"),
            }
        }
        _ => panic!("Expected Infix expression"),
    }
}

#[test]
fn transform_prefix_expression() {
    let input = Expression::Prefix(PrefixExpr {
        operator: "-".to_string(),
        operand: Box::new(one()),
    });

    let modified = transform(Node::Expression(input), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Prefix(prefix)) => match *prefix.operand {
            Expression::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64 for operand"),
        },
        _ => panic!("Expected Prefix expression"),
    }
}

#[test]
fn transform_index_expression() {
    let input = Expression::Index(IndexExpr {
        expr: Box::new(one()),
        index: Box::new(one()),
    });

    let modified = transform(Node::Expression(input), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Index(index)) => {
            match *index.expr {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for expr"),
            }
            match *index.index {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for index"),
            }
        }
        _ => panic!("Expected Index expression"),
    }
}

#[test]
fn transform_conditional_expression() {
    let input = Expression::Conditional(ConditionalExpr {
        condition: Box::new(one()),
        consequence: BlockStatement {
            statements: vec![Statement::Expression(ExpressionStatement { value: one() })],
        },
        alternative: Some(BlockStatement {
            statements: vec![Statement::Expression(ExpressionStatement { value: one() })],
        }),
    });

    let modified = transform(Node::Expression(input), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Conditional(cond)) => {
            match *cond.condition {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for condition"),
            }

            match &cond.consequence.statements[0] {
                Statement::Expression(ExpressionStatement {
                    value: Expression::I64(i),
                }) => {
                    assert_eq!(i.value, 2);
                }
                _ => panic!("Expected I64 in consequence"),
            }

            if let Some(alt) = cond.alternative {
                match &alt.statements[0] {
                    Statement::Expression(ExpressionStatement {
                        value: Expression::I64(i),
                    }) => {
                        assert_eq!(i.value, 2);
                    }
                    _ => panic!("Expected I64 in alternative"),
                }
            } else {
                panic!("Expected alternative");
            }
        }
        _ => panic!("Expected Conditional expression"),
    }
}

#[test]
fn transform_return_statement() {
    let stmt = Statement::Return(ReturnStatement { value: one() });

    let modified = transform(Node::Statement(stmt), &mut turn_one_into_two);

    match modified {
        Node::Statement(Statement::Return(ret)) => match ret.value {
            Expression::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64"),
        },
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn transform_let_statement() {
    let stmt = Statement::Let(LetStatement {
        ident: "x".to_string(),
        value: one(),
    });

    let modified = transform(Node::Statement(stmt), &mut turn_one_into_two);

    match modified {
        Node::Statement(Statement::Let(let_stmt)) => match let_stmt.value {
            Expression::I64(i) => assert_eq!(i.value, 2),
            _ => panic!("Expected I64"),
        },
        _ => panic!("Expected Let statement"),
    }
}

#[test]
fn transform_function_literal() {
    let func = Expression::Function(Function {
        name: None,
        params: vec!["x".to_string()],
        body: BlockStatement {
            statements: vec![Statement::Expression(ExpressionStatement { value: one() })],
        },
    });

    let modified = transform(Node::Expression(func), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Function(f)) => match &f.body.statements[0] {
            Statement::Expression(ExpressionStatement {
                value: Expression::I64(i),
            }) => {
                assert_eq!(i.value, 2);
            }
            _ => panic!("Expected I64 in function body"),
        },
        _ => panic!("Expected Function expression"),
    }
}

#[test]
fn transform_function_call() {
    let call = Expression::Call(CallExpr {
        function: Box::new(Expression::Identifier("myFunc".to_string())),
        arguments: vec![one(), one()],
    });

    let modified = transform(Node::Expression(call), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Call(c)) => {
            assert_eq!(c.arguments.len(), 2);

            match &c.arguments[0] {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for first argument"),
            }

            match &c.arguments[1] {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for second argument"),
            }
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn transform_array_literal() {
    let array = Expression::Array(ArrayLiteral {
        elements: vec![one(), one()],
    });

    let modified = transform(Node::Expression(array), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Array(arr)) => {
            assert_eq!(arr.elements.len(), 2);

            match &arr.elements[0] {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for first element"),
            }

            match &arr.elements[1] {
                Expression::I64(i) => assert_eq!(i.value, 2),
                _ => panic!("Expected I64 for second element"),
            }
        }
        _ => panic!("Expected Array expression"),
    }
}

#[test]
fn transform_hash_literal() {
    let hash = Expression::Hash(HashLiteral {
        pairs: vec![(one(), one()), (one(), one())],
    });

    let modified = transform(Node::Expression(hash), &mut turn_one_into_two);

    match modified {
        Node::Expression(Expression::Hash(h)) => {
            assert_eq!(h.pairs.len(), 2);

            for (key, value) in &h.pairs {
                match key {
                    Expression::I64(i) => assert_eq!(i.value, 2),
                    _ => panic!("Expected I64 for key"),
                }
                match value {
                    Expression::I64(i) => assert_eq!(i.value, 2),
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
        statements: vec![Statement::Let(LetStatement {
            ident: "x".to_string(),
            value: Expression::Infix(InfixExpr {
                lhs: Box::new(Expression::Array(ArrayLiteral {
                    elements: vec![one(), two()],
                })),
                operator: "+".to_string(),
                rhs: Box::new(Expression::Hash(HashLiteral {
                    pairs: vec![(one(), two())],
                })),
            }),
        })],
    };

    let modified = transform(Node::Program(program), &mut turn_one_into_two);

    match modified {
        Node::Program(prog) => match &prog.statements[0] {
            Statement::Let(let_stmt) => match &let_stmt.value {
                Expression::Infix(infix) => {
                    match &*infix.lhs {
                        Expression::Array(arr) => match &arr.elements[0] {
                            Expression::I64(i) => assert_eq!(i.value, 2),
                            _ => panic!("Expected modified value in array"),
                        },
                        _ => panic!("Expected Array"),
                    }

                    match &*infix.rhs {
                        Expression::Hash(h) => {
                            let (key, _) = &h.pairs[0];
                            match key {
                                Expression::I64(i) => assert_eq!(i.value, 2),
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
