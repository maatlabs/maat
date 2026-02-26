use maat_ast::*;

fn parse(input: &str) -> Program {
    maat_tests::parse(input)
}

fn expect_single_stmt(program: &Program) -> &Statement {
    assert_eq!(program.statements.len(), 1);
    &program.statements[0]
}

#[test]
fn parse_let_statements() {
    [
        ("let x = 5;", "x", "5"),
        ("let y = true;", "y", "true"),
        ("let foobar = y;", "foobar", "y"),
    ]
    .iter()
    .for_each(|(input, ident, value)| {
        let program = parse(input);
        let Statement::Let(let_stmt) = expect_single_stmt(&program) else {
            panic!("expected Let statement");
        };
        assert_eq!(let_stmt.ident, *ident);
        assert_eq!(let_stmt.value.to_string(), *value);
    });
}

#[test]
fn parse_return_statements() {
    [
        ("return 5;", "5"),
        ("return true;", "true"),
        ("return foobar;", "foobar"),
    ]
    .iter()
    .for_each(|(input, value)| {
        let program = parse(input);
        let Statement::Return(ret) = expect_single_stmt(&program) else {
            panic!("expected Return statement");
        };
        assert_eq!(ret.value.to_string(), *value);
    });
}

#[test]
fn parse_identifier_expression() {
    let program = parse("foobar;");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Identifier(ident),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected identifier expression");
    };
    assert_eq!(ident.value, "foobar");
}

#[test]
fn parse_integer_literal_expression() {
    let program = parse("5;");
    let Statement::Expression(ExpressionStatement {
        value: Expression::I64(I64 { value, .. }),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected I64 expression");
    };
    assert_eq!(*value, 5);
}

#[test]
fn parse_boolean_expression() {
    [("true;", true), ("false;", false)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::Boolean(value),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Boolean expression");
            };
            assert_eq!(value.value, *expected);
        });
}

#[test]
fn parse_prefix_expressions() {
    [
        ("!5;", "!", "5"),
        ("-15;", "-", "15"),
        ("!foobar;", "!", "foobar"),
        ("-foobar;", "-", "foobar"),
        ("!true;", "!", "true"),
        ("!false;", "!", "false"),
    ]
    .iter()
    .for_each(|(input, op, operand)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Prefix(prefix),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Prefix expression");
        };
        assert_eq!(prefix.operator, *op);
        assert_eq!(prefix.operand.to_string(), *operand);
    });
}

#[test]
fn parse_infix_expressions() {
    [
        ("5 + 5;", "5", "+", "5"),
        ("5 - 5;", "5", "-", "5"),
        ("5 * 5;", "5", "*", "5"),
        ("5 / 5;", "5", "/", "5"),
        ("5 > 5;", "5", ">", "5"),
        ("5 < 5;", "5", "<", "5"),
        ("5 >= 5;", "5", ">=", "5"),
        ("5 <= 5;", "5", "<=", "5"),
        ("5 == 5;", "5", "==", "5"),
        ("5 != 5;", "5", "!=", "5"),
        ("true == true", "true", "==", "true"),
        ("true != false", "true", "!=", "false"),
        ("false == false", "false", "==", "false"),
    ]
    .iter()
    .for_each(|(input, lhs, op, rhs)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Infix(infix),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Infix expression");
        };
        assert_eq!(infix.lhs.to_string(), *lhs);
        assert_eq!(infix.operator, *op);
        assert_eq!(infix.rhs.to_string(), *rhs);
    });
}

#[test]
fn parse_string_literal() {
    [
        (r#""hello world""#, "hello world"),
        (r#""foo bar""#, "foo bar"),
        (r#""""#, ""),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::String(s),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected string literal");
        };
        assert_eq!(s.value, *expected);
    });
}

#[test]
fn parse_float_literal() {
    [
        ("3.15;", 3.15),
        ("0.5;", 0.5),
        ("123.456;", 123.456),
        ("1e10;", 1e10),
        ("1.5E-3;", 1.5E-3),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::F64(float),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected float literal");
        };
        let value: f64 = (*float).into();
        assert!((value - expected).abs() < 1e-10, "input: {}", input);
    });
}

#[test]
fn parse_array_literal() {
    let program = parse("[1, 2 * 2, 3 + 3]");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Array(array),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 3);
    assert_eq!(array.elements[0].to_string(), "1");
    assert_eq!(array.elements[1].to_string(), "(2 * 2)");
    assert_eq!(array.elements[2].to_string(), "(3 + 3)");
}

#[test]
fn parse_empty_array() {
    let program = parse("[]");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Array(array),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 0);
}

#[test]
fn parse_index_expression() {
    let program = parse("myArray[1 + 1]");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Index(index),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected index expression");
    };
    assert!(matches!(&*index.expr, Expression::Identifier(id) if id.value == "myArray"));
    assert_eq!(index.index.to_string(), "(1 + 1)");
}

#[test]
fn parse_hash_literal() {
    let program = parse(r#"{"one": 1, "two": 2, "three": 3}"#);
    let Statement::Expression(ExpressionStatement {
        value: Expression::Hash(hash),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(hash.pairs.len(), 3);

    let expected = [("one", "1"), ("two", "2"), ("three", "3")];
    for (key, value) in expected {
        let found = hash
            .pairs
            .iter()
            .any(|(k, v)| k.to_string() == key && v.to_string() == value);
        assert!(found, "expected key-value pair: {} => {}", key, value);
    }
}

#[test]
fn parse_empty_hash() {
    let program = parse("{}");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Hash(hash),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(hash.pairs.len(), 0);
}

#[test]
fn parse_hash_with_expressions() {
    let program = parse(r#"{"one": 0 + 1, "two": 10 - 8}"#);
    let Statement::Expression(ExpressionStatement {
        value: Expression::Hash(hash),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(hash.pairs.len(), 2);
}

#[test]
fn parse_binary_literals() {
    [("0b1010;", 10), ("0B1111;", 15), ("0b0;", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::I64(int64),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected I64 expression");
            };
            assert_eq!(int64.radix, Radix::Bin);
            assert_eq!(int64.value, *expected, "input: {}", input);
        });
}

#[test]
fn parse_octal_literals() {
    [("0o755;", 493), ("0O644;", 420), ("0o0;", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::I64(int64),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected I64 expression");
            };
            assert_eq!(int64.radix, Radix::Oct);
            assert_eq!(int64.value, *expected, "input: {}", input);
        });
}

#[test]
fn parse_hex_literals() {
    [("0xff;", 255), ("0xFF;", 255), ("0xDEAD;", 57005)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Statement::Expression(ExpressionStatement {
                value: Expression::I64(int64),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected I64 expression");
            };
            assert_eq!(int64.radix, Radix::Hex);
            assert_eq!(int64.value, *expected, "input: {}", input);
        });
}

#[test]
fn parse_rust_style_suffixes() {
    let program = parse("123i64;");
    let Statement::Expression(ExpressionStatement {
        value: Expression::I64(i64_lit),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected I64 expression");
    };
    assert_eq!(i64_lit.value, 123);

    let program = parse("3.15f64;");
    let Statement::Expression(ExpressionStatement {
        value: Expression::F64(f64_lit),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected F64 expression");
    };
    let value: f64 = (*f64_lit).into();
    assert!((value - 3.15).abs() < 1e-10);
}

#[test]
fn parse_operator_precedence() {
    [
        ("-a * b", "((-a) * b)"),
        ("!-a", "(!(-a))"),
        ("a + b + c", "((a + b) + c)"),
        ("a + b - c", "((a + b) - c)"),
        ("a * b * c", "((a * b) * c)"),
        ("a * b / c", "((a * b) / c)"),
        ("a + b / c", "(a + (b / c))"),
        ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
        ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
        ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
        ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
        (
            "3 + 4 * 5 == 3 * 1 + 4 * 5",
            "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
        ),
        ("true", "true"),
        ("false", "false"),
        ("3 > 5 == false", "((3 > 5) == false)"),
        ("3 < 5 == true", "((3 < 5) == true)"),
        ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
        ("(5 + 5) * 2", "((5 + 5) * 2)"),
        ("2 / (5 + 5)", "(2 / (5 + 5))"),
        ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5))"),
        ("-(5 + 5)", "(-(5 + 5))"),
        ("!(true == true)", "(!(true == true))"),
        ("a + add(b * c) + d", "((a + add((b * c))) + d)"),
        (
            "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
            "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))",
        ),
        (
            "add(a + b + c * d / f + g)",
            "add((((a + b) + ((c * d) / f)) + g))",
        ),
    ]
    .iter()
    .for_each(|(input, expected)| {
        assert_eq!(parse(input).to_string(), *expected);
    });
}

#[test]
fn parse_if_expression() {
    let program = parse("if (x < y) { x }");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Conditional(cond),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Conditional expression");
    };

    let Expression::Infix(infix) = cond.condition.as_ref() else {
        panic!("expected Infix condition");
    };
    assert_eq!(infix.to_string(), "(x < y)");
    assert_eq!(cond.consequence.statements.len(), 1);
    assert_eq!(cond.consequence.statements[0].to_string(), "x");
    assert!(cond.alternative.is_none());
}

#[test]
fn parse_if_else_expression() {
    let program = parse("if (x < y) { x } else { y }");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Conditional(cond),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Conditional expression");
    };

    assert_eq!(cond.condition.to_string(), "(x < y)");
    assert_eq!(cond.consequence.statements[0].to_string(), "x");
    assert_eq!(
        cond.alternative.as_ref().unwrap().statements[0].to_string(),
        "y"
    );
}

#[test]
fn parse_function_literal() {
    let program = parse("fn(x, y) { x + y; }");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Function(func),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Function expression");
    };

    assert_eq!(func.param_names().collect::<Vec<_>>(), vec!["x", "y"]);
    assert_eq!(func.body.statements.len(), 1);
    assert_eq!(func.body.statements[0].to_string(), "(x + y)");
}

#[test]
fn parse_function_parameters() {
    [
        ("fn() {};", vec![]),
        ("fn(x) {};", vec!["x"]),
        ("fn(x, y, z) {};", vec!["x", "y", "z"]),
    ]
    .iter()
    .for_each(|(input, expected_params)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Function(func),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Function expression");
        };
        let names: Vec<&str> = func.param_names().collect();
        assert_eq!(names, *expected_params);
    });
}

#[test]
fn parse_call_expression() {
    let program = parse("add(1, 2 * 3, 4 + 5);");
    let Statement::Expression(ExpressionStatement {
        value: Expression::Call(call),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Call expression");
    };

    assert_eq!(call.function.to_string(), "add");
    assert_eq!(call.arguments.len(), 3);
    assert_eq!(call.arguments[0].to_string(), "1");
    assert_eq!(call.arguments[1].to_string(), "(2 * 3)");
    assert_eq!(call.arguments[2].to_string(), "(4 + 5)");
}

#[test]
fn parse_call_arguments() {
    [
        ("add();", "add", vec![]),
        ("add(1);", "add", vec!["1"]),
        (
            "add(1, 2 * 3, 4 + 5);",
            "add",
            vec!["1", "(2 * 3)", "(4 + 5)"],
        ),
    ]
    .iter()
    .for_each(|(input, func_name, expected_args)| {
        let program = parse(input);
        let Statement::Expression(ExpressionStatement {
            value: Expression::Call(call),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Call expression");
        };
        assert_eq!(call.function.to_string(), *func_name);
        assert_eq!(
            call.arguments
                .iter()
                .map(|arg| arg.to_string())
                .collect::<Vec<_>>(),
            *expected_args
        );
    });
}

#[test]
fn parse_loop_statement() {
    let program = parse("loop { 1; }");
    let Statement::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    assert_eq!(loop_stmt.body.statements.len(), 1);
    assert_eq!(loop_stmt.body.statements[0].to_string(), "1");
}

#[test]
fn parse_while_statement() {
    let program = parse("while (x < 10) { x; }");
    let Statement::While(while_stmt) = expect_single_stmt(&program) else {
        panic!("expected While statement");
    };
    assert_eq!(while_stmt.condition.to_string(), "(x < 10)");
    assert_eq!(while_stmt.body.statements.len(), 1);
    assert_eq!(while_stmt.body.statements[0].to_string(), "x");
}

#[test]
fn parse_for_statement() {
    let program = parse("for x in [1, 2, 3] { x; }");
    let Statement::For(for_stmt) = expect_single_stmt(&program) else {
        panic!("expected For statement");
    };
    assert_eq!(for_stmt.ident, "x");
    assert_eq!(for_stmt.iterable.to_string(), "[1, 2, 3]");
    assert_eq!(for_stmt.body.statements.len(), 1);
    assert_eq!(for_stmt.body.statements[0].to_string(), "x");
}

#[test]
fn parse_break_expression() {
    let program = parse("loop { break; }");
    let Statement::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Statement::Expression(ExpressionStatement {
        value: Expression::Break(break_expr),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert!(break_expr.value.is_none());
}

#[test]
fn parse_break_with_value() {
    let program = parse("loop { break 42; }");
    let Statement::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Statement::Expression(ExpressionStatement {
        value: Expression::Break(break_expr),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert_eq!(break_expr.value.as_ref().unwrap().to_string(), "42");
}

#[test]
fn parse_continue_expression() {
    let program = parse("loop { continue; }");
    let Statement::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Statement::Expression(ExpressionStatement {
        value: Expression::Continue(_),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Continue expression");
    };
}
