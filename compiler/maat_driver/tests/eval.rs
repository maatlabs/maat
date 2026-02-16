use maat_driver::{Env, Hashable, Lexer, NULL, Object, Parser, Result, eval, *};

fn test_eval(input: &str) -> Result<Object> {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(
        parser.errors().is_empty(),
        "parser errors: {:?}",
        parser.errors()
    );
    let env = Env::default();
    eval(Node::Program(program), &env)
}

#[test]
fn eval_int64_expression() {
    [
        ("5", 5),
        ("10", 10),
        ("-5", -5),
        ("-10", -10),
        ("5 + 5 + 5 + 5 - 10", 10),
        ("2 * 2 * 2 * 2 * 2", 32),
        ("-50 + 100 + -50", 0),
        ("5 * 2 + 10", 20),
        ("5 + 2 * 10", 25),
        ("20 + 2 * -10", 0),
        ("50 / 2 * 2 + 10", 60),
        ("2 * (5 + 10)", 30),
        ("3 * 3 * 3 + 10", 37),
        ("3 * (3 * 3) + 10", 37),
        ("(5 + 10 * 2 + 15 / 3) * 2 + -10", 50),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn eval_boolean_expression() {
    [
        ("true", true),
        ("false", false),
        ("1 < 2", true),
        ("1 > 2", false),
        ("1 < 1", false),
        ("1 > 1", false),
        ("1 <= 2", true),
        ("1 >= 2", false),
        ("1 <= 1", true),
        ("1 >= 1", true),
        ("2 <= 1", false),
        ("2 >= 1", true),
        ("1 == 1", true),
        ("1 != 1", false),
        ("1 == 2", false),
        ("1 != 2", true),
        ("true == true", true),
        ("false == false", true),
        ("true == false", false),
        ("true != false", true),
        ("false != true", true),
        ("(1 < 2) == true", true),
        ("(1 < 2) == false", false),
        ("(1 > 2) == true", false),
        ("(1 > 2) == false", true),
        ("(1 <= 1) == true", true),
        ("(2 >= 1) == true", true),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::Boolean(*expected), "input: {}", input);
    });
}

#[test]
fn eval_bang_operator() {
    [
        ("!true", false),
        ("!false", true),
        ("!5", false),
        ("!!true", true),
        ("!!false", false),
        ("!!5", true),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::Boolean(*expected), "input: {}", input);
    });
}

#[test]
fn eval_if_else_expressions() {
    [
        ("if (true) { 10 }", Some(10)),
        ("if (false) { 10 }", None),
        ("if (1) { 10 }", Some(10)),
        ("if (1 < 2) { 10 }", Some(10)),
        ("if (1 > 2) { 10 }", None),
        ("if (1 > 2) { 10 } else { 20 }", Some(20)),
        ("if (1 < 2) { 10 } else { 20 }", Some(10)),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, Object::Null, "input: {}", input),
        }
    });
}

#[test]
fn eval_return_statements() {
    [
        ("return 10;", 10),
        ("return 10; 9;", 10),
        ("return 2 * 5; 9;", 10),
        ("9; return 2 * 5; 9;", 10),
        ("if (10 > 1) { return 10; }", 10),
        ("if (10 > 1) { if (10 > 1) { return 10; } return 1; }", 10),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn eval_let_statements() {
    [
        ("let a = 5; a;", 5),
        ("let a = 5 * 5; a;", 25),
        ("let a = 5; let b = a; b;", 5),
        ("let a = 5; let b = a; let c = a + b + 5; c;", 15),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn eval_function_object() {
    let input = "fn(x) { x + 2; };";
    let result = test_eval(input).unwrap();
    match result {
        Object::Function(func) => {
            assert_eq!(func.params.len(), 1);
            assert_eq!(func.params[0], "x");
            assert_eq!(func.body.statements.len(), 1);
        }
        _ => panic!("expected Function object, got {:?}", result),
    }
}

#[test]
fn eval_function_application() {
    [
        ("let identity = fn(x) { x; }; identity(5);", 5),
        ("let identity = fn(x) { return x; }; identity(5);", 5),
        ("let double = fn(x) { x * 2; }; double(5);", 10),
        ("let add = fn(x, y) { x + y; }; add(5, 5);", 10),
        ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", 20),
        ("fn(x) { x; }(5)", 5),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn eval_closures() {
    let input = "
            let newAdder = fn(x) {
                fn(y) { x + y };
            };
            let addTwo = newAdder(2);
            addTwo(2);
        ";
    let result = test_eval(input).unwrap();
    assert_eq!(result, Object::I64(4));
}

#[test]
fn eval_string_literals() {
    [
        (r#""Hello World!""#, "Hello World!"),
        (r#""foo bar""#, "foo bar"),
        (r#""""#, ""),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(
            result,
            Object::String(expected.to_string()),
            "input: {}",
            input
        );
    });
}

#[test]
fn eval_string_concatenation() {
    [
        (r#""Hello" + " " + "World!""#, "Hello World!"),
        (r#""foo" + "bar""#, "foobar"),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(
            result,
            Object::String(expected.to_string()),
            "input: {}",
            input
        );
    });
}

#[test]
fn eval_string_escape_sequences() {
    [
        (r#""hello \"world\"""#, "hello \"world\""),
        (r#""line1\nline2""#, "line1\nline2"),
        (r#""tab\there""#, "tab\there"),
        (r#""backslash\\""#, "backslash\\"),
        (r#""quote\"""#, "quote\""),
        (r#""null\0char""#, "null\0char"),
        (r#""mixed\t\n\r\\""#, "mixed\t\n\r\\"),
        (r#""invalid\xescape""#, "invalid\\xescape"), // Invalid escape preserved
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(
            result,
            Object::String(expected.to_string()),
            "input: {}",
            input
        );
    });
}

#[test]
fn eval_array_literals() {
    let result = test_eval("[1, 2 * 2, 3 + 3]").unwrap();
    match result {
        Object::Array(arr) => {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Object::I64(1));
            assert_eq!(arr[1], Object::I64(4));
            assert_eq!(arr[2], Object::I64(6));
        }
        _ => panic!("expected array"),
    }
}

#[test]
fn eval_array_index_expressions() {
    [
        ("[1, 2, 3][0usize]", Some(1)),
        ("[1, 2, 3][1usize]", Some(2)),
        ("[1, 2, 3][2usize]", Some(3)),
        ("let i = 0usize; [1][i];", Some(1)),
        ("[1, 2, 3][1usize + 1usize];", Some(3)),
        ("let myArray = [1, 2, 3]; myArray[2usize];", Some(3)),
        (
            "let myArray = [1, 2, 3]; myArray[0usize] + myArray[1usize] + myArray[2usize];",
            Some(6),
        ),
        ("[1, 2, 3][3usize]", None),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_array_index_with_integer_types() {
    [
        // Default I64 type (no suffix)
        ("[1, 2, 3][0]", Some(1)),
        ("[1, 2, 3][1]", Some(2)),
        ("[1, 2, 3][2]", Some(3)),
        ("let i = 1; [10, 20, 30][i];", Some(20)),
        // Explicit integer types
        ("[1, 2, 3][0_i8]", Some(1)),
        ("[1, 2, 3][1_i16]", Some(2)),
        ("[1, 2, 3][2_i32]", Some(3)),
        ("[1, 2, 3][0_i64]", Some(1)),
        ("[1, 2, 3][1_u8]", Some(2)),
        ("[1, 2, 3][2_u16]", Some(3)),
        ("[1, 2, 3][0_u32]", Some(1)),
        ("[1, 2, 3][1_u64]", Some(2)),
        // Out of bounds
        ("[1, 2, 3][10]", None),
        ("[1, 2, 3][100_u8]", None),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_array_index_negative_returns_null() {
    let result = test_eval("[1, 2, 3][-1]").unwrap();
    assert_eq!(result, Object::Null);
}

#[test]
fn eval_hash_literals() {
    let input = r#"
            let two = "two";
            {
                "one": 10 - 9,
                two: 1 + 1,
                "thr" + "ee": 6 / 2,
                4: 4,
                true: 5,
                false: 6
            }
        "#;
    let result = test_eval(input).unwrap();
    match result {
        Object::Hash(hash) => {
            assert_eq!(hash.pairs.len(), 6);

            let expected = [
                (Hashable::String("one".to_string()), Object::I64(1)),
                (Hashable::String("two".to_string()), Object::I64(2)),
                (Hashable::String("three".to_string()), Object::I64(3)),
                (Hashable::I64(4), Object::I64(4)),
                (Hashable::Boolean(true), Object::I64(5)),
                (Hashable::Boolean(false), Object::I64(6)),
            ];

            for (key, value) in expected {
                assert_eq!(hash.pairs.get(&key), Some(&value));
            }
        }
        _ => panic!("expected hash"),
    }
}

#[test]
fn eval_hash_index_expressions() {
    [
        (r#"{"foo": 5}["foo"]"#, Some(5)),
        (r#"{"foo": 5}["bar"]"#, None),
        (r#"let key = "foo"; {"foo": 5}[key]"#, Some(5)),
        (r#"{}["foo"]"#, None),
        (r#"{5: 5}[5]"#, Some(5)),
        (r#"{true: 5}[true]"#, Some(5)),
        (r#"{false: 5}[false]"#, Some(5)),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_builtin_len() {
    [
        (r#"len("")"#, Some(0)),
        (r#"len("four")"#, Some(4)),
        (r#"len("hello world")"#, Some(11)),
        ("len([1, 2, 3])", Some(3)),
        ("len([])", Some(0)),
        ("len(1)", None),
    ]
    .iter()
    .for_each(|(input, expected)| match expected {
        Some(val) => {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Usize(*val), "input: {}", input);
        }
        None => {
            assert!(test_eval(input).is_err(), "expected error for: {}", input);
        }
    });
}

#[test]
fn eval_builtin_first() {
    [
        ("first([1, 2, 3])", Some(1)),
        ("first([10, 20])", Some(10)),
        ("first([])", None),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_builtin_last() {
    [
        ("last([1, 2, 3])", Some(3)),
        ("last([10, 20])", Some(20)),
        ("last([])", None),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(val) => assert_eq!(result, Object::I64(*val), "input: {}", input),
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_builtin_rest() {
    [
        ("rest([1, 2, 3])", Some(vec![2, 3])),
        ("rest([10, 20])", Some(vec![20])),
        ("rest([])", None),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match expected {
            Some(vals) => match result {
                Object::Array(arr) => {
                    assert_eq!(arr.len(), vals.len());
                    for (i, val) in vals.iter().enumerate() {
                        assert_eq!(arr[i], Object::I64(*val));
                    }
                }
                _ => panic!("expected array for: {}", input),
            },
            None => assert_eq!(result, NULL, "input: {}", input),
        }
    });
}

#[test]
fn eval_builtin_push() {
    let result = test_eval("push([1, 2], 3)").unwrap();
    match result {
        Object::Array(arr) => {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Object::I64(1));
            assert_eq!(arr[1], Object::I64(2));
            assert_eq!(arr[2], Object::I64(3));
        }
        _ => panic!("expected array"),
    }

    let result = test_eval("push([], 1)").unwrap();
    match result {
        Object::Array(arr) => {
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0], Object::I64(1));
        }
        _ => panic!("expected array"),
    }
}

#[test]
fn eval_builtin_print_or_puts() {
    // print/puts returns null
    let result = test_eval(r#"print("hello", "world")"#).unwrap();
    assert_eq!(result, NULL);
    let result = test_eval(r#"puts("test")"#).unwrap();
    assert_eq!(result, NULL);

    // print/puts with no arguments
    let result = test_eval("puts()").unwrap();
    assert_eq!(result, NULL);

    // print/puts with mixed types
    let result = test_eval(r#"print("value:", 42, true)"#).unwrap();
    assert_eq!(result, NULL);
}

#[test]
fn eval_float_arithmetic() {
    [
        ("3.5 + 2.5", 6.0),
        ("10.5 - 5.5", 5.0),
        ("2.5 * 4.0", 10.0),
        ("10.0 / 2.0", 5.0),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        match result {
            Object::F64(val) => {
                assert!((val - expected).abs() < 1e-10, "input: {}", input);
            }
            _ => panic!("expected float for: {}", input),
        }
    });
}

#[test]
fn eval_binary_literals() {
    [
        ("0b1010", 10),
        ("0B1111", 15),
        ("0b0", 0),
        ("0b11111111", 255),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn eval_octal_literals() {
    [("0o755", 493), ("0O644", 420), ("0o0", 0), ("0o10", 8)]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::I64(*expected), "input: {}", input);
        });
}

#[test]
fn eval_hex_literals() {
    [("0xff", 255), ("0xFF", 255), ("0xDEAD", 57005), ("0x0", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::I64(*expected), "input: {}", input);
        });
}

#[test]
fn eval_rust_style_suffixes() {
    [("123i64", 123), ("456i64", 456), ("0i64", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::I64(*expected), "input: {}", input);
        });

    [("3.15f64", 3.15), ("0.5f64", 0.5)]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match result {
                Object::F64(val) => {
                    assert!((val - expected).abs() < 1e-10, "input: {}", input);
                }
                _ => panic!("expected float for: {}", input),
            }
        });
}

#[test]
fn eval_radix_arithmetic() {
    [
        ("0b1010 + 0o12", 20),    // 10 + 10 = 20
        ("0xff - 0b11111111", 0), // 255 - 255 = 0
        ("0x10 * 2", 32),         // 16 * 2 = 32
    ]
    .iter()
    .for_each(|(input, expected)| {
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::I64(*expected), "input: {}", input);
    });
}

#[test]
fn error_handling() {
    [
        ("5 + true;", "invalid infix expression"),
        ("5 + true; 5;", "invalid infix expression"),
        ("-true", "cannot be negated"),
        ("true + false;", "invalid boolean operation"),
        ("5; true + false; 5", "invalid boolean operation"),
        ("if (10 > 1) { true + false; }", "invalid boolean operation"),
        (
            "if (10 > 1) { if (10 > 1) { return true + false; } return 1; }",
            "invalid boolean operation",
        ),
        ("foobar", "unknown identifier"),
    ]
    .iter()
    .for_each(|(input, expected_msg)| {
        let result = test_eval(input);
        assert!(result.is_err(), "expected error for input: {}", input);
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains(expected_msg),
            "expected error containing '{}', got '{}'",
            expected_msg,
            err
        );
    });
}
