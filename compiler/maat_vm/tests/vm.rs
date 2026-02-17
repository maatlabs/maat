use maat_ast::{Node, Program};
use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_codegen::Compiler;
use maat_lexer::Lexer;
use maat_parse::Parser;
use maat_runtime::{Hashable, Object};
use maat_vm::VM;

#[derive(Debug)]
enum TestValue {
    Int(i64),
    Bool(bool),
    Str(String),
    IntArray(Vec<i64>),
    Hash(Vec<(i64, i64)>),
    Null,
}

fn parse_program(input: &str) -> Program {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    parser.parse_program()
}

fn run_vm_test(input: &str, expected: TestValue) {
    let program = parse_program(input);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");

    let bytecode = compiler.bytecode().expect("bytecode extraction failed");
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");

    let stack_elem = vm
        .last_popped_stack_elem()
        .expect("no value on stack")
        .clone();

    match expected {
        TestValue::Int(expected_val) => match stack_elem {
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
    let program = parse_program(input);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");

    let bytecode = compiler.bytecode().expect("bytecode extraction failed");
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
        ("1", TestValue::Int(1)),
        ("2", TestValue::Int(2)),
        ("1 + 2", TestValue::Int(3)),
        ("1 - 2", TestValue::Int(-1)),
        ("1 * 2", TestValue::Int(2)),
        ("4 / 2", TestValue::Int(2)),
        ("50 / 2 * 2 + 10 - 5", TestValue::Int(55)),
        ("5 * (2 + 10)", TestValue::Int(60)),
        ("5 + 5 + 5 + 5 - 10", TestValue::Int(10)),
        ("2 * 2 * 2 * 2 * 2", TestValue::Int(32)),
        ("5 * 2 + 10", TestValue::Int(20)),
        ("5 + 2 * 10", TestValue::Int(25)),
        ("5 * (2 + 10)", TestValue::Int(60)),
        ("-5", TestValue::Int(-5)),
        ("-10", TestValue::Int(-10)),
        ("-50 + 100 + -50", TestValue::Int(0)),
        ("(5 + 10 * 2 + 15 / 3) * 2 + -10", TestValue::Int(50)),
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
        ("if (true) { 10 }", TestValue::Int(10)),
        ("if (true) { 10 } else { 20 }", TestValue::Int(10)),
        ("if (false) { 10 } else { 20 }", TestValue::Int(20)),
        ("if (1) { 10 }", TestValue::Int(10)),
        ("if (1 < 2) { 10 }", TestValue::Int(10)),
        ("if (1 < 2) { 10 } else { 20 }", TestValue::Int(10)),
        ("if (1 > 2) { 10 } else { 20 }", TestValue::Int(20)),
        ("if (1 > 2) { 10 }", TestValue::Null),
        ("if (false) { 10 }", TestValue::Null),
        (
            "if ((if (false) { 10 })) { 10 } else { 20 }",
            TestValue::Int(20),
        ),
    ];

    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn global_let_statements() {
    let cases = vec![
        ("let one = 1; one", TestValue::Int(1)),
        ("let one = 1; let two = 2; one + two", TestValue::Int(3)),
        (
            "let one = 1; let two = one + one; one + two",
            TestValue::Int(3),
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
        ("[1, 2, 3][1]", TestValue::Int(2)),
        ("[1, 2, 3][0 + 2]", TestValue::Int(3)),
        ("[[1, 1, 1]][0][0]", TestValue::Int(1)),
        ("[][0]", TestValue::Null),
        ("[1, 2, 3][99]", TestValue::Null),
        ("[1][-1]", TestValue::Null),
        ("{1: 1, 2: 2}[1]", TestValue::Int(1)),
        ("{1: 1, 2: 2}[2]", TestValue::Int(2)),
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
            TestValue::Int(15),
        ),
        (
            "let one = fn() { 1; }; let two = fn() { 2; }; one() + two()",
            TestValue::Int(3),
        ),
        (
            "let a = fn() { 1 }; let b = fn() { a() + 1 }; let c = fn() { b() + 1 }; c();",
            TestValue::Int(3),
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
            TestValue::Int(99),
        ),
        (
            "let earlyExit = fn() { return 99; return 100; }; earlyExit();",
            TestValue::Int(99),
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
            TestValue::Int(1),
        ),
        (
            "let returnsOneReturner = fn() { let returnsOne = fn() { 1; }; returnsOne; }; returnsOneReturner()();",
            TestValue::Int(1),
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
            TestValue::Int(1),
        ),
        (
            "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; }; oneAndTwo();",
            TestValue::Int(3),
        ),
        (
            "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; }; let threeAndFour = fn() { let three = 3; let four = 4; three + four; }; oneAndTwo() + threeAndFour();",
            TestValue::Int(10),
        ),
        (
            "let firstFoobar = fn() { let foobar = 50; foobar; }; let secondFoobar = fn() { let foobar = 100; foobar; }; firstFoobar() + secondFoobar();",
            TestValue::Int(150),
        ),
        (
            "let globalSeed = 50; let minusOne = fn() { let num = 1; globalSeed - num; }; let minusTwo = fn() { let num = 2; globalSeed - num; }; minusOne() + minusTwo();",
            TestValue::Int(97),
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
            TestValue::Int(4),
        ),
        (
            "let sum = fn(a, b) { a + b; }; sum(1, 2);",
            TestValue::Int(3),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; sum(1, 2);",
            TestValue::Int(3),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; sum(1, 2) + sum(3, 4);",
            TestValue::Int(10),
        ),
        (
            "let sum = fn(a, b) { let c = a + b; c; }; let outer = fn() { sum(1, 2) + sum(3, 4); }; outer();",
            TestValue::Int(10),
        ),
        (
            "let globalNum = 10; let sum = fn(a, b) { let c = a + b; c + globalNum; }; let outer = fn() { sum(1, 2) + sum(3, 4) + globalNum; }; outer() + globalNum;",
            TestValue::Int(50),
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
        (r#"len("")"#, TestValue::Int(0)),
        (r#"len("four")"#, TestValue::Int(4)),
        ("len([1, 2, 3])", TestValue::Int(3)),
        ("len([])", TestValue::Int(0)),
        (r#"puts("hello", "world!")"#, TestValue::Null),
        ("first([1, 2, 3])", TestValue::Int(1)),
        ("first([])", TestValue::Null),
        ("last([1, 2, 3])", TestValue::Int(3)),
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
