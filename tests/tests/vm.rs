use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_runtime::{Hashable, Object};
use maat_vm::VM;

#[derive(Debug)]
enum TestValue {
    I64(i64),
    I32(i32),
    F64(f64),
    Usize(usize),
    Bool(bool),
    Str(String),
    IntArray(Vec<i64>),
    Hash(Vec<(i64, i64)>),
    Null,
}

fn run_vm_test(input: &str, expected: TestValue) {
    let bytecode = maat_tests::compile(input);
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");

    let stack_elem = vm
        .last_popped_stack_elem()
        .expect("no value on stack")
        .clone();

    match expected {
        TestValue::I64(expected_val) => match stack_elem {
            Object::I64(val) => {
                assert_eq!(val, expected_val, "wrong integer value for input: {input}")
            }
            Object::Usize(val) => {
                assert_eq!(
                    val as i64, expected_val,
                    "wrong integer value for input: {input}"
                )
            }
            _ => panic!("expected integer object, got: {:?}", stack_elem),
        },
        TestValue::I32(expected_val) => match stack_elem {
            Object::I32(val) => {
                assert_eq!(val, expected_val, "wrong I32 value for input: {input}")
            }
            _ => panic!("expected I32 object, got: {:?}", stack_elem),
        },
        TestValue::Usize(expected_val) => match stack_elem {
            Object::Usize(val) => {
                assert_eq!(val, expected_val, "wrong Usize value for input: {input}")
            }
            _ => panic!("expected Usize object, got: {:?}", stack_elem),
        },
        TestValue::F64(expected_val) => match stack_elem {
            Object::F64(val) => {
                assert!(
                    (val - expected_val).abs() < f64::EPSILON,
                    "wrong F64 value for input: {input}"
                )
            }
            _ => panic!("expected F64 object, got: {:?}", stack_elem),
        },
        TestValue::Bool(expected_val) => match stack_elem {
            Object::Boolean(val) => {
                assert_eq!(val, expected_val, "wrong boolean value for input: {input}")
            }
            _ => panic!("expected boolean object, got: {:?}", stack_elem),
        },
        TestValue::Str(expected_val) => match stack_elem {
            Object::String(val) => {
                assert_eq!(val, expected_val, "wrong string value for input: {input}")
            }
            _ => panic!("expected string object, got: {:?}", stack_elem),
        },
        TestValue::IntArray(expected_vals) => match stack_elem {
            Object::Array(elements) => {
                assert_eq!(
                    elements.len(),
                    expected_vals.len(),
                    "wrong array length for input: {input}"
                );
                for (i, expected_elem) in expected_vals.iter().enumerate() {
                    match &elements[i] {
                        Object::I64(val) => assert_eq!(
                            *val, *expected_elem,
                            "wrong array element at index {i} for input: {input}"
                        ),
                        other => {
                            panic!("expected integer in array at index {i}, got: {:?}", other)
                        }
                    }
                }
            }
            _ => panic!("expected array object, got: {:?}", stack_elem),
        },
        TestValue::Hash(expected_pairs) => match &stack_elem {
            Object::Hash(hash_obj) => {
                assert_eq!(
                    hash_obj.pairs.len(),
                    expected_pairs.len(),
                    "wrong hash size for input: {input}"
                );
                for (key, value) in &expected_pairs {
                    let hash_key = Hashable::I64(*key);
                    let actual = hash_obj
                        .pairs
                        .get(&hash_key)
                        .unwrap_or_else(|| panic!("missing key {key} in hash for input: {input}"));
                    match actual {
                        Object::I64(val) => assert_eq!(
                            *val, *value,
                            "wrong hash value for key {key} in input: {input}"
                        ),
                        other => {
                            panic!("expected integer value for key {key}, got: {:?}", other)
                        }
                    }
                }
            }
            _ => panic!("expected hash object, got: {:?}", stack_elem),
        },
        TestValue::Null => {
            assert_eq!(
                stack_elem,
                Object::Null,
                "expected null for input: {input}, got: {:?}",
                stack_elem
            );
        }
    }
}

fn run_vm_error_test(input: &str, expected_error: &str) {
    let bytecode = maat_tests::compile_raw(input);
    let mut vm = VM::new(bytecode);
    let result = vm.run();

    assert!(result.is_err(), "expected VM error for input: {input}");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains(expected_error),
        "wrong VM error for input: {input}\n  expected to contain: {expected_error}\n  got: {err_msg}"
    );
}

#[test]
fn integer_arithmetic() {
    let cases = vec![
        ("1", TestValue::I64(1)),
        ("2", TestValue::I64(2)),
        ("1 + 2", TestValue::I64(3)),
        ("1 - 2", TestValue::I64(-1)),
        ("1 * 2", TestValue::I64(2)),
        ("4 / 2", TestValue::I64(2)),
        ("50 / 2 * 2 + 10 - 5", TestValue::I64(55)),
        ("5 * (2 + 10)", TestValue::I64(60)),
        ("5 + 5 + 5 + 5 - 10", TestValue::I64(10)),
        ("2 * 2 * 2 * 2 * 2", TestValue::I64(32)),
        ("5 * 2 + 10", TestValue::I64(20)),
        ("5 + 2 * 10", TestValue::I64(25)),
        ("5 * (2 + 10)", TestValue::I64(60)),
        ("-5", TestValue::I64(-5)),
        ("-10", TestValue::I64(-10)),
        ("-50 + 100 + -50", TestValue::I64(0)),
        ("(5 + 10 * 2 + 15 / 3) * 2 + -10", TestValue::I64(50)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn boolean_expressions() {
    let cases = vec![
        ("true", TestValue::Bool(true)),
        ("false", TestValue::Bool(false)),
        ("1 < 2", TestValue::Bool(true)),
        ("1 > 2", TestValue::Bool(false)),
        ("1 < 1", TestValue::Bool(false)),
        ("1 > 1", TestValue::Bool(false)),
        ("1 == 1", TestValue::Bool(true)),
        ("1 != 1", TestValue::Bool(false)),
        ("1 == 2", TestValue::Bool(false)),
        ("1 != 2", TestValue::Bool(true)),
        ("true == true", TestValue::Bool(true)),
        ("false == false", TestValue::Bool(true)),
        ("true == false", TestValue::Bool(false)),
        ("true != false", TestValue::Bool(true)),
        ("false != true", TestValue::Bool(true)),
        ("(1 < 2) == true", TestValue::Bool(true)),
        ("(1 < 2) == false", TestValue::Bool(false)),
        ("(1 > 2) == true", TestValue::Bool(false)),
        ("(1 > 2) == false", TestValue::Bool(true)),
        ("!true", TestValue::Bool(false)),
        ("!false", TestValue::Bool(true)),
        ("!5", TestValue::Bool(false)),
        ("!!true", TestValue::Bool(true)),
        ("!!false", TestValue::Bool(false)),
        ("!!5", TestValue::Bool(true)),
        ("!(if (false) { 5; })", TestValue::Bool(true)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn conditionals() {
    let cases = vec![
        ("if (true) { 10 }", TestValue::I64(10)),
        ("if (true) { 10 } else { 20 }", TestValue::I64(10)),
        ("if (false) { 10 } else { 20 }", TestValue::I64(20)),
        ("if (1) { 10 }", TestValue::I64(10)),
        ("if (1 < 2) { 10 }", TestValue::I64(10)),
        ("if (1 < 2) { 10 } else { 20 }", TestValue::I64(10)),
        ("if (1 > 2) { 10 } else { 20 }", TestValue::I64(20)),
        ("if (1 > 2) { 10 }", TestValue::Null),
        ("if (false) { 10 }", TestValue::Null),
        (
            "if ((if (false) { 10 })) { 10 } else { 20 }",
            TestValue::I64(20),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn global_let_statements() {
    let cases = vec![
        ("let one = 1; one", TestValue::I64(1)),
        ("let one = 1; let two = 2; one + two", TestValue::I64(3)),
        (
            "let one = 1; let two = one + one; one + two",
            TestValue::I64(3),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn string_literals() {
    let cases = vec![
        (
            r#""zero knowledge""#,
            TestValue::Str("zero knowledge".to_string()),
        ),
        (
            r#""zero " + "knowledge""#,
            TestValue::Str("zero knowledge".to_string()),
        ),
        (
            r#""zero " + "knowledge" + " proofs""#,
            TestValue::Str("zero knowledge proofs".to_string()),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn array_literals() {
    let cases = vec![
        ("[]", TestValue::IntArray(vec![])),
        ("[1, 2, 3]", TestValue::IntArray(vec![1, 2, 3])),
        (
            "[1 + 2, 3 * 4, 5 + 6]",
            TestValue::IntArray(vec![3, 12, 11]),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn hash_literals() {
    let cases = vec![
        ("{}", TestValue::Hash(vec![])),
        ("{1: 2, 2: 3}", TestValue::Hash(vec![(1, 2), (2, 3)])),
        (
            "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
            TestValue::Hash(vec![(2, 4), (6, 16)]),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn index_expressions() {
    let cases = vec![
        ("[1, 2, 3][1]", TestValue::I64(2)),
        ("[1, 2, 3][0 + 2]", TestValue::I64(3)),
        ("[[1, 1, 1]][0][0]", TestValue::I64(1)),
        ("[][0]", TestValue::Null),
        ("[1, 2, 3][99]", TestValue::Null),
        ("[1][-1]", TestValue::Null),
        ("{1: 1, 2: 2}[1]", TestValue::I64(1)),
        ("{1: 1, 2: 2}[2]", TestValue::I64(2)),
        ("{1: 1}[0]", TestValue::Null),
        ("{}[0]", TestValue::Null),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn calling_functions_without_arguments() {
    let cases = vec![
        (
            "let fivePlusTen = fn() { 5 + 10; }; fivePlusTen();",
            TestValue::I64(15),
        ),
        (
            "let one = fn() { 1; }; let two = fn() { 2; }; one() + two()",
            TestValue::I64(3),
        ),
        (
            "let a = fn() { 1 }; let b = fn() { a() + 1 }; let c = fn() { b() + 1 }; c();",
            TestValue::I64(3),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn functions_with_return_statement() {
    let cases = vec![
        (
            "let earlyExit = fn() { return 99; 100; }; earlyExit();",
            TestValue::I64(99),
        ),
        (
            "let earlyExit = fn() { return 99; return 100; }; earlyExit();",
            TestValue::I64(99),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn functions_without_return_value() {
    let cases = vec![
        ("let noReturn = fn() { }; noReturn();", TestValue::Null),
        (
            "let noReturn = fn() { }; let noReturnTwo = fn() { noReturn(); }; noReturn(); noReturnTwo();",
            TestValue::Null,
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn first_class_functions() {
    let cases = vec![
        (
            "let returnsOne = fn() { 1; }; let returnsOneReturner = fn() { returnsOne; }; returnsOneReturner()();",
            TestValue::I64(1),
        ),
        (
            "let returnsOneReturner = fn() { let returnsOne = fn() { 1; }; returnsOne; }; returnsOneReturner()();",
            TestValue::I64(1),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn calling_functions_with_bindings() {
    let cases = vec![
        (
            "let one = fn() { let one = 1; one }; one();",
            TestValue::I64(1),
        ),
        (
            "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; }; oneAndTwo();",
            TestValue::I64(3),
        ),
        (
            "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; }; let threeAndFour = fn() { let three = 3; let four = 4; three + four; }; oneAndTwo() + threeAndFour();",
            TestValue::I64(10),
        ),
        (
            "let firstFoobar = fn() { let foobar = 50; foobar; }; let secondFoobar = fn() { let foobar = 100; foobar; }; firstFoobar() + secondFoobar();",
            TestValue::I64(150),
        ),
        (
            "let globalSeed = 50; let minusOne = fn() { let num = 1; globalSeed - num; }; let minusTwo = fn() { let num = 2; globalSeed - num; }; minusOne() + minusTwo();",
            TestValue::I64(97),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn calling_functions_with_arguments_and_bindings() {
    let cases = vec![
        (
            "let identity = fn(a) { a; }; identity(4);",
            TestValue::I64(4),
        ),
        (
            "let sum = fn(a, b) { a + b; }; sum(1, 2);",
            TestValue::I64(3),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; sum(1, 2);",
            TestValue::I64(3),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; sum(1, 2) + sum(3, 4);",
            TestValue::I64(10),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; let outer = fn() { sum(1, 2) + sum(3, 4); }; outer();",
            TestValue::I64(10),
        ),
        (
            "let globalNum = 10; let sum = fn(a, b) { let c = a + b; c + globalNum; }; let outer = fn() { sum(1, 2) + sum(3, 4) + globalNum; }; outer() + globalNum;",
            TestValue::I64(50),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn calling_functions_with_wrong_arguments() {
    let cases = vec![
        (
            "fn() { 1; }(1);",
            "wrong number of arguments: want=0, got=1",
        ),
        (
            "fn(a) { a; }();",
            "wrong number of arguments: want=1, got=0",
        ),
        (
            "fn(a, b) { a + b; }(1);",
            "wrong number of arguments: want=2, got=1",
        ),
    ];

    for (input, expected_error) in cases {
        run_vm_error_test(input, expected_error);
    }
}

#[test]
fn builtin_functions() {
    let cases = vec![
        (r#"len("")"#, TestValue::Usize(0)),
        (r#"len("four")"#, TestValue::Usize(4)),
        ("len([1, 2, 3])", TestValue::Usize(3)),
        ("len([])", TestValue::Usize(0)),
        (r#"print("hello", "world!")"#, TestValue::Null),
        ("first([1, 2, 3])", TestValue::I64(1)),
        ("first([])", TestValue::Null),
        ("last([1, 2, 3])", TestValue::I64(3)),
        ("last([])", TestValue::Null),
        ("rest([1, 2, 3])", TestValue::IntArray(vec![2, 3])),
        ("rest([])", TestValue::Null),
        ("push([], 1)", TestValue::IntArray(vec![1])),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn builtin_function_errors() {
    let cases = vec![
        ("len(1)", "argument to `len` not supported"),
        (r#"len("one", "two")"#, "wrong number of arguments"),
        ("first(1)", "argument to `first` must be an array"),
        ("last(1)", "argument to `last` must be an array"),
        ("push(1, 1)", "argument to `push` must be an array"),
    ];

    for (input, expected_error) in cases {
        run_vm_error_test(input, expected_error);
    }
}

#[test]
fn stack_underflow() {
    use maat_errors::Error;

    let mut instructions = Instructions::new();
    instructions.extend(&Instructions::from(encode(Opcode::Pop, &[])));

    let bytecode = Bytecode {
        instructions,
        constants: vec![],
        source_map: Default::default(),
    };

    let mut vm = VM::new(bytecode);
    let result = vm.run();

    assert!(result.is_err(), "should fail on stack underflow");

    match result.unwrap_err() {
        Error::Vm(err) => {
            assert!(
                err.message.contains("stack underflow"),
                "expected stack underflow error, got: {}",
                err.message
            );
        }
        other => panic!("expected VmError, got {:?}", other),
    }
}

#[test]
fn closures() {
    let cases = vec![
        (
            "let newClosure = fn(a) { fn() { a; }; }; let closure = newClosure(99); closure();",
            TestValue::I64(99),
        ),
        (
            "let newAdder = fn(a, b) { fn(c) { a + b + c }; }; let adder = newAdder(1, 2); adder(8);",
            TestValue::I64(11),
        ),
        (
            "let newAdder = fn(a, b) { let c = a + b; fn(d) { c + d }; }; let adder = newAdder(1, 2); adder(8);",
            TestValue::I64(11),
        ),
        (
            "let newAdderOuter = fn(a, b) { let c = a + b; fn(d) { let e = d + c; fn(f) { e + f; }; }; }; let newAdderInner = newAdderOuter(1, 2); let adder = newAdderInner(3); adder(8);",
            TestValue::I64(14),
        ),
        (
            "let a = 1; let newAdderOuter = fn(b) { fn(c) { fn(d) { a + b + c + d }; }; }; let newAdderInner = newAdderOuter(2); let adder = newAdderInner(3); adder(8);",
            TestValue::I64(14),
        ),
        (
            "let newClosure = fn(a, b) { let one = fn() { a; }; let two = fn() { b; }; fn() { one() + two(); }; }; let closure = newClosure(9, 90); closure();",
            TestValue::I64(99),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn recursive_functions() {
    let cases = vec![
        (
            "let countDown = fn(x) { if (x == 0) { return 0; } else { countDown(x - 1); } }; countDown(1);",
            TestValue::I64(0),
        ),
        (
            "let countDown = fn(x) { if (x == 0) { return 0; } else { countDown(x - 1); } }; let wrapper = fn() { countDown(1); }; wrapper();",
            TestValue::I64(0),
        ),
        (
            "let wrapper = fn() { let countDown = fn(x) { if (x == 0) { return 0; } else { countDown(x - 1); } }; countDown(1); }; wrapper();",
            TestValue::I64(0),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn closure_captures_array_with_builtins() {
    run_vm_test(
        r#"
        let sum = fn(arr) {
            let iter = fn(idx, acc) {
                if (idx == len(arr)) {
                    acc
                } else {
                    iter(idx + 1, acc + arr[idx]);
                }
            };
            iter(0, 0);
        };
        sum([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        "#,
        TestValue::I64(55),
    );
}

#[test]
fn recursive_fibonacci() {
    run_vm_test(
        r#"
        let fibonacci = fn(x) {
            if (x == 0) {
                return 0;
            } else {
                if (x == 1) {
                    return 1;
                } else {
                    fibonacci(x - 1) + fibonacci(x - 2);
                }
            }
        };
        fibonacci(15);
        "#,
        TestValue::I64(610),
    );
}

#[test]
fn typed_integer_arithmetic() {
    let cases = vec![
        ("10i32 + 20i32", TestValue::I32(30)),
        ("100i32 - 50i32", TestValue::I32(50)),
        ("3i32 * 7i32", TestValue::I32(21)),
        ("20i32 / 4i32", TestValue::I32(5)),
        ("5usize + 3usize", TestValue::Usize(8)),
        ("10usize - 2usize", TestValue::Usize(8)),
        ("4usize * 5usize", TestValue::Usize(20)),
        ("20usize / 4usize", TestValue::Usize(5)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn float_arithmetic() {
    let cases = vec![
        ("1.5f64 + 2.5f64", TestValue::F64(4.0)),
        ("10.0f64 - 3.5f64", TestValue::F64(6.5)),
        ("2.0f64 * 3.0f64", TestValue::F64(6.0)),
        ("10.0f64 / 4.0f64", TestValue::F64(2.5)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn signed_negation() {
    let cases = vec![
        ("-5i32", TestValue::I32(-5)),
        ("-1.5f64", TestValue::F64(-1.5)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn unsigned_negation_error() {
    run_vm_error_test("-(5usize)", "unsupported type for negation");
}

#[test]
fn cast_expressions() {
    let cases = vec![
        ("42 as i32", TestValue::I32(42)),
        ("255u8 as i32", TestValue::I32(255)),
        ("1000i32 as i64", TestValue::I64(1000)),
        ("10 as usize", TestValue::Usize(10)),
        ("42 as f64", TestValue::F64(42.0)),
        ("3.14f64 as i64", TestValue::I64(3)),
        ("5usize as i64", TestValue::I64(5)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn cast_expression_errors() {
    let cases = vec![
        ("256 as u8", "out of range for u8"),
        ("-1 as u8", "out of range for u8"),
        ("-1 as usize", "out of range for usize"),
    ];

    for (input, expected_error) in cases {
        run_vm_error_test(input, expected_error);
    }
}

#[test]
fn len_returns_usize() {
    run_vm_test("len([1, 2, 3])", TestValue::Usize(3));
    run_vm_test(r#"len("hello")"#, TestValue::Usize(5));
}

#[test]
fn len_with_cast() {
    run_vm_test("len([1, 2, 3]) as i64", TestValue::I64(3));
}

#[test]
fn cross_type_integer_comparison() {
    let cases = vec![
        ("5 == len([1, 2, 3, 4, 5])", TestValue::Bool(true)),
        ("5 != len([1, 2, 3, 4, 5])", TestValue::Bool(false)),
        ("10 > len([1, 2, 3])", TestValue::Bool(true)),
        ("1 < len([1, 2, 3])", TestValue::Bool(true)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn float_comparison_total_ordering() {
    let cases = vec![
        ("1.0f64 < 2.0f64", TestValue::Bool(true)),
        ("2.0f64 > 1.0f64", TestValue::Bool(true)),
        ("1.0f64 == 1.0f64", TestValue::Bool(true)),
        ("1.0f64 != 2.0f64", TestValue::Bool(true)),
        ("1.5f64 < 1.5f64", TestValue::Bool(false)),
        ("1.5f64 > 1.5f64", TestValue::Bool(false)),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn loop_with_break() {
    run_vm_test(
        r#"
        let x = 0;
        loop {
            let x = x + 1;
            if (x == 5) {
                break;
            }
        }
        x;
        "#,
        TestValue::I64(5),
    );
}

#[test]
fn loop_with_return_from_function() {
    run_vm_test(
        r#"
        let find_first = fn(arr, target) {
            let i = 0;
            loop {
                if (i == len(arr) as i64) {
                    return -1;
                }
                if (arr[i] == target) {
                    return i;
                }
                let i = i + 1;
            }
        };
        find_first([10, 20, 30, 40], 30);
        "#,
        TestValue::I64(2),
    );
}

#[test]
fn while_loop() {
    run_vm_test(
        r#"
        let x = 0;
        let sum = 0;
        while (x < 5) {
            let x = x + 1;
            let sum = sum + x;
        }
        sum;
        "#,
        TestValue::I64(15),
    );
}

#[test]
fn while_loop_false_condition() {
    run_vm_test(
        r#"
        let x = 10;
        while (x < 5) {
            let x = x + 1;
        }
        x;
        "#,
        TestValue::I64(10),
    );
}

#[test]
fn for_loop_over_array() {
    run_vm_test(
        r#"
        let sum = 0;
        for x in [1, 2, 3, 4, 5] {
            let sum = sum + x;
        }
        sum;
        "#,
        TestValue::I64(15),
    );
}

#[test]
fn for_loop_empty_array() {
    run_vm_test(
        r#"
        let sum = 0;
        for x in [] {
            let sum = sum + 1;
        }
        sum;
        "#,
        TestValue::I64(0),
    );
}

#[test]
fn nested_loops() {
    run_vm_test(
        r#"
        let total = 0;
        let i = 0;
        while (i < 3) {
            let j = 0;
            while (j < 3) {
                let total = total + 1;
                let j = j + 1;
            }
            let i = i + 1;
        }
        total;
        "#,
        TestValue::I64(9),
    );
}

#[test]
fn while_with_break() {
    run_vm_test(
        r#"
        let x = 0;
        while (x < 100) {
            let x = x + 1;
            if (x == 7) {
                break;
            }
        }
        x;
        "#,
        TestValue::I64(7),
    );
}

#[test]
fn for_with_break() {
    run_vm_test(
        r#"
        let result = 0;
        for x in [10, 20, 30, 40, 50] {
            if (x == 30) {
                break;
            }
            let result = result + x;
        }
        result;
        "#,
        TestValue::I64(30),
    );
}

#[test]
fn loop_in_function() {
    run_vm_test(
        r#"
        let countdown = fn(n) {
            let result = 0;
            let i = n;
            while (i > 0) {
                let result = result + i;
                let i = i - 1;
            }
            result;
        };
        countdown(10);
        "#,
        TestValue::I64(55),
    );
}

#[test]
fn nested_for_loops() {
    run_vm_test(
        r#"
        let sum = 0;
        for x in [1, 2, 3] {
            for y in [10, 20] {
                let sum = sum + x + y;
            }
        }
        sum;
        "#,
        TestValue::I64(102),
    );
}

#[test]
fn for_loop_with_function_calls() {
    run_vm_test(
        r#"
        let double = fn(x) { x * 2; };
        let sum = 0;
        for x in [1, 2, 3] {
            let sum = sum + double(x);
        }
        sum;
        "#,
        TestValue::I64(12),
    );
}
