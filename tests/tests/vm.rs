use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_runtime::{Hashable, Integer, Value};
use maat_vm::VM;

#[derive(Debug)]
enum TestValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    Bool(bool),
    Str(String),
    IntVector(Vec<i64>),
    Map(Vec<(i64, i64)>),
    Range(i64, i64),
    RangeInclusive(i64, i64),
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
        TestValue::I8(exp) => match stack_elem {
            Value::Integer(Integer::I8(val)) => {
                assert_eq!(val, exp, "wrong I8 value for input: {input}")
            }
            _ => panic!("expected I8, got: {stack_elem:?}"),
        },
        TestValue::I16(exp) => match stack_elem {
            Value::Integer(Integer::I16(val)) => {
                assert_eq!(val, exp, "wrong I16 value for input: {input}")
            }
            _ => panic!("expected I16, got: {stack_elem:?}"),
        },
        TestValue::I32(exp) => match stack_elem {
            Value::Integer(Integer::I32(val)) => {
                assert_eq!(val, exp, "wrong I32 value for input: {input}")
            }
            _ => panic!("expected I32, got: {stack_elem:?}"),
        },
        TestValue::I64(exp) => match stack_elem {
            Value::Integer(Integer::I64(val)) => {
                assert_eq!(val, exp, "wrong integer value for input: {input}")
            }
            Value::Integer(Integer::Usize(val)) => {
                assert_eq!(val as i64, exp, "wrong integer value for input: {input}")
            }
            _ => panic!("expected integer, got: {stack_elem:?}"),
        },
        TestValue::I128(exp) => match stack_elem {
            Value::Integer(Integer::I128(val)) => {
                assert_eq!(val, exp, "wrong I128 value for input: {input}")
            }
            _ => panic!("expected I128, got: {stack_elem:?}"),
        },
        TestValue::U8(exp) => match stack_elem {
            Value::Integer(Integer::U8(val)) => {
                assert_eq!(val, exp, "wrong U8 value for input: {input}")
            }
            _ => panic!("expected U8, got: {stack_elem:?}"),
        },
        TestValue::U16(exp) => match stack_elem {
            Value::Integer(Integer::U16(val)) => {
                assert_eq!(val, exp, "wrong U16 value for input: {input}")
            }
            _ => panic!("expected U16, got: {stack_elem:?}"),
        },
        TestValue::U32(exp) => match stack_elem {
            Value::Integer(Integer::U32(val)) => {
                assert_eq!(val, exp, "wrong U32 value for input: {input}")
            }
            _ => panic!("expected U32, got: {stack_elem:?}"),
        },
        TestValue::U64(exp) => match stack_elem {
            Value::Integer(Integer::U64(val)) => {
                assert_eq!(val, exp, "wrong U64 value for input: {input}")
            }
            _ => panic!("expected U64, got: {stack_elem:?}"),
        },
        TestValue::U128(exp) => match stack_elem {
            Value::Integer(Integer::U128(val)) => {
                assert_eq!(val, exp, "wrong U128 value for input: {input}")
            }
            _ => panic!("expected U128, got: {stack_elem:?}"),
        },
        TestValue::Usize(exp) => match stack_elem {
            Value::Integer(Integer::Usize(val)) => {
                assert_eq!(val, exp, "wrong Usize value for input: {input}")
            }
            _ => panic!("expected Usize, got: {stack_elem:?}"),
        },
        TestValue::Bool(exp) => match stack_elem {
            Value::Bool(val) => {
                assert_eq!(val, exp, "wrong boolean value for input: {input}")
            }
            _ => panic!("expected boolean, got: {:?}", stack_elem),
        },
        TestValue::Str(exp) => match stack_elem {
            Value::Str(val) => {
                assert_eq!(val, exp, "wrong string value for input: {input}")
            }
            _ => panic!("expected string, got: {:?}", stack_elem),
        },
        TestValue::IntVector(expected_vals) => match stack_elem {
            Value::Vector(elements) => {
                assert_eq!(
                    elements.len(),
                    expected_vals.len(),
                    "wrong vector length for input: {input}"
                );
                for (i, expected_elem) in expected_vals.iter().enumerate() {
                    match &elements[i] {
                        Value::Integer(Integer::I64(val)) => assert_eq!(
                            *val, *expected_elem,
                            "wrong vector element at index {i} for input: {input}"
                        ),
                        other => {
                            panic!("expected integer in vector at index {i}, got: {:?}", other)
                        }
                    }
                }
            }
            _ => panic!("expected vector, got: {:?}", stack_elem),
        },
        TestValue::Map(expected_pairs) => match &stack_elem {
            Value::Map(map_obj) => {
                assert_eq!(
                    map_obj.pairs.len(),
                    expected_pairs.len(),
                    "wrong map size for input: {input}"
                );
                for (key, value) in &expected_pairs {
                    let map_key = Hashable::Integer(Integer::I64(*key));
                    let actual = map_obj
                        .pairs
                        .get(&map_key)
                        .unwrap_or_else(|| panic!("missing key {key} in map for input: {input}"));
                    match actual {
                        Value::Integer(Integer::I64(val)) => assert_eq!(
                            *val, *value,
                            "wrong map value for key {key} in input: {input}"
                        ),
                        other => {
                            panic!("expected integer value for key {key}, got: {:?}", other)
                        }
                    }
                }
            }
            _ => panic!("expected map , got: {:?}", stack_elem),
        },
        TestValue::Range(s, e) => {
            assert_eq!(
                stack_elem,
                Value::Range(s, e),
                "expected Range({s}..{e}) for input: {input}, got: {:?}",
                stack_elem
            );
        }
        TestValue::RangeInclusive(s, e) => {
            assert_eq!(
                stack_elem,
                Value::RangeInclusive(s, e),
                "expected RangeInclusive({s}..={e}) for input: {input}, got: {:?}",
                stack_elem
            );
        }
        TestValue::Null => {
            assert_eq!(
                stack_elem,
                Value::Null,
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
        ("!!true", TestValue::Bool(true)),
        ("!!false", TestValue::Bool(false)),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn conditionals() {
    let cases = vec![
        ("if true { 10 }", TestValue::I64(10)),
        ("if true { 10 } else { 20 }", TestValue::I64(10)),
        ("if false { 10 } else { 20 }", TestValue::I64(20)),
        ("if 1 < 2 { 10 }", TestValue::I64(10)),
        ("if (1 < 2) { 10 } else { 20 }", TestValue::I64(10)),
        ("if (1 > 2) { 10 } else { 20 }", TestValue::I64(20)),
        ("if 1 > 2 { 10 }", TestValue::Null),
        ("if false { 10 }", TestValue::Null),
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
fn vectors() {
    let cases = vec![
        ("[]", TestValue::IntVector(vec![])),
        ("[1, 2, 3]", TestValue::IntVector(vec![1, 2, 3])),
        (
            "[1 + 2, 3 * 4, 5 + 6]",
            TestValue::IntVector(vec![3, 12, 11]),
        ),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn map_literals() {
    let cases = vec![
        ("{}", TestValue::Map(vec![])),
        ("{1: 2, 2: 3}", TestValue::Map(vec![(1, 2), (2, 3)])),
        (
            "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
            TestValue::Map(vec![(2, 4), (6, 16)]),
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
fn builtin_methods() {
    let cases = vec![
        // Vector methods
        ("[1, 2, 3].len()", TestValue::Usize(3)),
        ("[].len()", TestValue::Usize(0)),
        ("[1, 2, 3].first().unwrap()", TestValue::I64(1)),
        ("[1, 2, 3].first().is_some()", TestValue::Bool(true)),
        ("[].first().is_none()", TestValue::Bool(true)),
        ("[1, 2, 3].last().unwrap()", TestValue::I64(3)),
        ("[].last().is_none()", TestValue::Bool(true)),
        ("[1, 2, 3].split_first()", TestValue::IntVector(vec![2, 3])),
        ("[].split_first()", TestValue::IntVector(vec![])),
        ("[].push(1)", TestValue::IntVector(vec![1])),
        // str methods
        (r#""".len()"#, TestValue::Usize(0)),
        (r#""four".len()"#, TestValue::Usize(4)),
        (r#""hello".len()"#, TestValue::Usize(5)),
        // len with cast
        ("[1, 2, 3].len() as i64", TestValue::I64(3)),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn builtin_method_chaining() {
    // `push` returns a new vector, so chaining is possible
    run_vm_test("[1, 2].push(3).len()", TestValue::Usize(3));
    // `split_first` returns the tail as a new vector
    run_vm_test(
        "[1, 2, 3].split_first().first().unwrap()",
        TestValue::I64(2),
    );
    run_vm_test("[1, 2, 3].split_first().last().unwrap()", TestValue::I64(3));
    run_vm_test(
        "let vector = [10, 20, 30]; vector.len()",
        TestValue::Usize(3),
    );
    run_vm_test(
        "let vector = [10, 20, 30]; vector.first().unwrap()",
        TestValue::I64(10),
    );
    run_vm_test(
        "let vector = [10, 20, 30]; vector.last().unwrap()",
        TestValue::I64(30),
    );
    run_vm_test(r#"let s = "hello world"; s.len()"#, TestValue::Usize(11));
    run_vm_test(
        "let vector = [1, 2, 3]; vector.len() as i64 + 1",
        TestValue::I64(4),
    );
    run_vm_test(
        "let vector = [1, 2, 3]; vector.split_first().len()",
        TestValue::Usize(2),
    );
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
        type_registry: vec![],
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
fn closure_captures_vector_with_builtins() {
    run_vm_test(
        r#"
        let sum = fn(vector) {
            let iter = fn(idx, acc) {
                if (idx == vector.len()) {
                    acc
                } else {
                    iter(idx + 1, acc + vector[idx]);
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
        // Signed negation
        ("-5i32", TestValue::I32(-5)),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn unsigned_negation_error() {
    run_vm_error_test("-(5usize)", "integer negation overflow");
}

#[test]
fn cast_expressions() {
    let cases = vec![
        ("42 as i32", TestValue::I32(42)),
        ("255u8 as i32", TestValue::I32(255)),
        ("1000i32 as i64", TestValue::I64(1000)),
        ("10 as usize", TestValue::Usize(10)),
        ("5usize as i64", TestValue::I64(5)),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
    // Cast errors
    let error_cases = vec![
        ("256 as u8", "out of range for u8"),
        ("-1 as u8", "out of range for u8"),
        ("-1 as usize", "out of range for usize"),
    ];
    for (input, expected_error) in error_cases {
        run_vm_error_test(input, expected_error);
    }
}

#[test]
fn cross_type_integer_comparison() {
    let cases = vec![
        ("5usize == [1, 2, 3, 4, 5].len()", TestValue::Bool(true)),
        ("5usize != [1, 2, 3, 4, 5].len()", TestValue::Bool(false)),
        ("10usize > [1, 2, 3].len()", TestValue::Bool(true)),
        ("1usize < [1, 2, 3].len()", TestValue::Bool(true)),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn loop_control_flow() {
    // break exits loop
    run_vm_test(
        r#"
        let mut x = 0;
        loop {
            x = x + 1;
            if (x == 5) {
                break;
            }
        }
        x;
        "#,
        TestValue::I64(5),
    );

    // return from function inside loop
    run_vm_test(
        r#"
        let find_first = fn(vector, target) {
            let mut i = 0;
            loop {
                if (i == vector.len() as i64) {
                    return -1;
                }
                if (vector[i] == target) {
                    return i;
                }
                i = i + 1;
            }
        };
        find_first([10, 20, 30, 40], 30);
        "#,
        TestValue::I64(2),
    );

    // loop inside function with accumulator
    run_vm_test(
        r#"
        let countdown = fn(n) {
            let mut result = 0;
            let mut i = n;
            while (i > 0) {
                result = result + i;
                i = i - 1;
            }
            result;
        };
        countdown(10);
        "#,
        TestValue::I64(55),
    );
}

#[test]
fn while_loops() {
    // Basic while loop
    run_vm_test(
        r#"
        let mut x = 0;
        let mut sum = 0;
        while (x < 5) {
            x = x + 1;
            sum = sum + x;
        }
        sum;
        "#,
        TestValue::I64(15),
    );
    // False condition (body never executes)
    run_vm_test(
        r#"
        let mut x = 10;
        while (x < 5) {
            x = x + 1;
        }
        x;
        "#,
        TestValue::I64(10),
    );
    // While with break
    run_vm_test(
        r#"
        let mut x = 0;
        while (x < 100) {
            x = x + 1;
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
fn for_loops() {
    // Basic for loop
    run_vm_test(
        r#"
        let mut sum = 0;
        for x in [1, 2, 3, 4, 5] {
            sum = sum + x;
        }
        sum;
        "#,
        TestValue::I64(15),
    );
    // Empty vector
    run_vm_test(
        r#"
        let mut sum = 0;
        for x in [] {
            sum = sum + 1;
        }
        sum;
        "#,
        TestValue::I64(0),
    );
    // Nested while loops
    run_vm_test(
        r#"
        let mut total = 0;
        let mut i = 0;
        while (i < 3) {
            let mut j = 0;
            while (j < 3) {
                total = total + 1;
                j = j + 1;
            }
            i = i + 1;
        }
        total;
        "#,
        TestValue::I64(9),
    );
    // For with break
    run_vm_test(
        r#"
        let mut result = 0;
        for x in [10, 20, 30, 40, 50] {
            if (x == 30) {
                break;
            }
            result = result + x;
        }
        result;
        "#,
        TestValue::I64(30),
    );
    // Nested for loops
    run_vm_test(
        r#"
        let mut sum = 0;
        for x in [1, 2, 3] {
            for y in [10, 20] {
                sum = sum + x + y;
            }
        }
        sum;
        "#,
        TestValue::I64(102),
    );
    // For loop with function calls
    run_vm_test(
        r#"
        let double = fn(x) { x * 2; };
        let mut sum = 0;
        for x in [1, 2, 3] {
            sum = sum + double(x);
        }
        sum;
        "#,
        TestValue::I64(12),
    );
}

#[test]
fn comments() {
    run_vm_test("let x = 5; // this is a comment\nx", TestValue::I64(5));
    run_vm_test("let x = /* inline */ 10; x", TestValue::I64(10));
    run_vm_test(
        "let x = /* outer /* inner */ still comment */ 42; x",
        TestValue::I64(42),
    );
}

#[test]
fn modulo_ops() {
    run_vm_test("10 % 3", TestValue::I64(1));
    run_vm_test("-7 % 3", TestValue::I64(2));
    run_vm_test("12 % 4", TestValue::I64(0));
    run_vm_test("7 % -3", TestValue::I64(1));
}

#[test]
fn bitwise_ops() {
    run_vm_test("0xFF & 0x0F", TestValue::I64(15));
    run_vm_test("0xF0 | 0x0F", TestValue::I64(255));
    run_vm_test("0xFF ^ 0x0F", TestValue::I64(240));
    run_vm_test("1 | 2 & 3", TestValue::I64(3));
}

#[test]
fn shift_ops() {
    run_vm_test("1 << 8", TestValue::I64(256));
    run_vm_test("256 >> 4", TestValue::I64(16));
}

#[test]
fn compound_assign() {
    run_vm_test("let mut x = 10; x += 5; x", TestValue::I64(15));
    run_vm_test("let mut x = 10; x -= 3; x", TestValue::I64(7));
    run_vm_test("let mut x = 4; x *= 3; x", TestValue::I64(12));
    run_vm_test("let mut x = 20; x /= 4; x", TestValue::I64(5));
    run_vm_test("let mut x = 17; x %= 5; x", TestValue::I64(2));
}

#[test]
fn plain_assignment() {
    run_vm_test("let mut x = 10; x = 20; x", TestValue::I64(20));
    run_vm_test(
        r#"let mut x = 1; let mut y = 2; x = y; y = 10; x"#,
        TestValue::I64(2),
    );
}

#[test]
fn immutable_assignment_error() {
    let input = "let x = 10; x = 20; x";
    let lexer = maat_lexer::MaatLexer::new(input);
    let mut parser = maat_parser::MaatParser::new(lexer);
    let mut program = parser.parse();
    maat_ast::fold_constants(&mut program);
    let mut compiler = maat_codegen::Compiler::new();
    let result = compiler.compile(&maat_ast::Node::Program(program));
    assert!(
        result.is_err(),
        "expected compile error for immutable assignment"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cannot re-assign to immutable variable"),
        "wrong error message: {err}"
    );
}

#[test]
fn forward_function_reference() {
    run_vm_test(
        r#"
        fn foo() -> i64 { bar() }
        fn bar() -> i64 { 42 }
        foo()
        "#,
        TestValue::I64(42),
    );
    // mutual recursion
    run_vm_test(
        r#"
        fn is_even(n: i64) -> bool {
            if (n == 0) { true } else { is_odd(n - 1) }
        }
        fn is_odd(n: i64) -> bool {
            if (n == 0) { false } else { is_even(n - 1) }
        }
        is_even(10)
        "#,
        TestValue::Bool(true),
    );
}

#[test]
fn block_scoping() {
    run_vm_test(
        r#"
        fn test() -> i64 {
            let x = 1;
            if (true) {
                let x = 2;
            }
            x
        }
        test()
        "#,
        TestValue::I64(1),
    );
    run_vm_test(
        r#"
        fn test() -> i64 {
            let mut sum = 0;
            let mut i = 0;
            while (i < 5) {
                sum = sum + i;
                i = i + 1;
            }
            sum
        }
        test()
        "#,
        TestValue::I64(10),
    );
    run_vm_test(
        r#"
        fn test() -> i64 {
            let mut x = 0;
            let mut i = 0;
            while (i < 3) {
                let y = i * 10;
                x = x + y;
                i = i + 1;
            }
            x
        }
        test()
        "#,
        TestValue::I64(30),
    );
}

#[test]
fn range_for_loop() {
    // Half-open range: 0..5 sums to 0+1+2+3+4 = 10
    run_vm_test(
        r#"
        fn sum_range() -> i64 {
            let mut total = 0;
            for i in 0..5 {
                total = total + i;
            }
            total
        }
        sum_range()
        "#,
        TestValue::I64(10),
    );
    // Inclusive range: 1..=5 sums to 1+2+3+4+5 = 15
    run_vm_test(
        r#"
        fn sum_inclusive() -> i64 {
            let mut total = 0;
            for i in 1..=5 {
                total = total + i;
            }
            total
        }
        sum_inclusive()
        "#,
        TestValue::I64(15),
    );
    // Empty range: 5..5 should execute zero iterations
    run_vm_test(
        r#"
        fn empty_range() -> i64 {
            let mut total = 0;
            for i in 5..5 {
                total = total + i;
            }
            total
        }
        empty_range()
        "#,
        TestValue::I64(0),
    );
    // Range with break
    run_vm_test(
        r#"
        fn range_break() -> i64 {
            let mut total = 0;
            for i in 0..100 {
                if (i == 5) {
                    break;
                }
                total = total + i;
            }
            total
        }
        range_break()
        "#,
        TestValue::I64(10),
    );
    // Range with continue
    run_vm_test(
        r#"
        fn range_continue() -> i64 {
            let mut total = 0;
            for i in 0..10 {
                if (i % 2 == 0) {
                    continue;
                }
                total = total + i;
            }
            total
        }
        range_continue()
        "#,
        TestValue::I64(25),
    );
    // Nested range for-loops
    run_vm_test(
        r#"
        fn nested_ranges() -> i64 {
            let mut total = 0;
            for i in 0..3 {
                for j in 0..3 {
                    total = total + 1;
                }
            }
            total
        }
        nested_ranges()
        "#,
        TestValue::I64(9),
    );
}

#[test]
fn range_expression_standalone() {
    run_vm_test("0..10", TestValue::Range(0, 10));
    run_vm_test("1..=5", TestValue::RangeInclusive(1, 5));
}

#[test]
fn arithmetic_overflow() {
    // i64::MAX + 1 should produce an overflow error
    run_vm_error_test("9223372036854775807 + 1", "arithmetic overflow");
    // i64::MIN - 1 should produce an overflow error
    run_vm_error_test("-9223372036854775807 - 1 - 1", "arithmetic overflow");
    // i64::MIN * -1 overflows
    run_vm_error_test(
        "let x = -9223372036854775807 - 1; x * -1",
        "arithmetic overflow",
    );
    // i8 overflow
    run_vm_error_test("127i8 + 1i8", "arithmetic overflow");
    // u8 overflow
    run_vm_error_test("255u8 + 1u8", "arithmetic overflow");
    // Negation overflow: i64::MIN cannot be negated
    run_vm_error_test("let x = -9223372036854775807 - 1; -x", "negation overflow");
}

#[test]
fn division_by_zero() {
    run_vm_error_test("1 / 0", "division by zero");
    run_vm_error_test("10 % 0", "modulo by zero");
    run_vm_error_test("1i32 / 0i32", "division by zero");
}

#[test]
fn euclidean_modulo_edge_case() {
    // i64::MIN % -1 is a known edge case (would overflow in C)
    // Rust's checked_rem_euclid returns None for this case.
    run_vm_error_test(
        "let x = -9223372036854775807 - 1; x % -1",
        "modulo by zero or overflow",
    );
}

#[test]
fn shift_overflow() {
    // Shifting by more bits than the type width should produce an error
    run_vm_error_test("1 << 64", "shift value exceeds type bit width");
    run_vm_error_test("1 >> 64", "shift value exceeds type bit width");
    // Negative shift value
    run_vm_error_test(
        "1 << -1",
        "shift amount must be a non-negative integer <= u32::MAX",
    );
}

#[test]
fn deeply_nested_expressions() {
    // 50 levels of nested addition: ((((1 + 1) + 1) + 1) ... + 1) = 50
    let mut expr = "1".to_string();
    for _ in 1..50 {
        expr = format!("({expr} + 1)");
    }
    run_vm_test(&expr, TestValue::I64(50));
}

#[test]
fn empty_range_iterations() {
    // Negative range: start > end for half-open should execute zero iterations
    run_vm_test(
        r#"
        fn test() -> i64 {
            let mut total = 0;
            for i in 10..5 {
                total += i;
            }
            total
        }
        test()
        "#,
        TestValue::I64(0),
    );
    // Inclusive range where start > end
    run_vm_test(
        r#"
        fn test() -> i64 {
            let mut total = 0;
            for i in 1..=0 {
                total += i;
            }
            total
        }
        test()
        "#,
        TestValue::I64(0),
    );
}

#[test]
fn string_operations() {
    // Empty string length
    run_vm_test(r#""".len()"#, TestValue::Usize(0));
    // String concatenation of empties
    run_vm_test(r#""" + "" + "hello""#, TestValue::Str("hello".to_string()));
    // Escape sequences
    run_vm_test(r#"let s = "line1\nline2"; s.len()"#, TestValue::Usize(11));
}

#[test]
fn nested_option_matching() {
    run_vm_test(
        r#"
        fn unwrap_nested(opt: Option<i64>) -> i64 {
            match opt {
                Some(x) => x,
                None => -1,
            }
        }
        let a = Some(42);
        let b: Option<i64> = None;
        unwrap_nested(a) + unwrap_nested(b)
        "#,
        TestValue::I64(41),
    );
}

#[test]
fn result_error_propagation() {
    run_vm_test(
        r#"
        fn try_divide(a: i64, b: i64) -> Result<i64, i64> {
            if (b == 0) {
                Err(-1)
            } else {
                Ok(a / b)
            }
        }
        let r = try_divide(10, 2);
        match r {
            Ok(v) => v,
            Err(e) => e,
        }
        "#,
        TestValue::I64(5),
    );
    run_vm_test(
        r#"
        fn try_divide(a: i64, b: i64) -> Result<i64, i64> {
            if (b == 0) {
                Err(-1)
            } else {
                Ok(a / b)
            }
        }
        let r = try_divide(10, 0);
        match r {
            Ok(v) => v,
            Err(e) => e,
        }
        "#,
        TestValue::I64(-1),
    );
}

#[test]
fn break_with_value() {
    run_vm_test(
        r#"
        fn find_first_even(vector: [i64]) -> i64 {
            let mut result = -1;
            for x in vector {
                if (x % 2 == 0) {
                    result = x;
                    break;
                }
            }
            result
        }
        find_first_even([1, 3, 5, 8, 10])
        "#,
        TestValue::I64(8),
    );
}

#[test]
fn continue_in_loops() {
    // Skip even numbers, sum odd numbers 1..10
    run_vm_test(
        r#"
        fn sum_odd() -> i64 {
            let mut total = 0;
            let mut i = 0;
            while (i < 10) {
                i += 1;
                if (i % 2 == 0) {
                    continue;
                }
                total += i;
            }
            total
        }
        sum_odd()
        "#,
        TestValue::I64(25),
    );
}

#[test]
fn labeled_loops() {
    // break 'outer from nested loop
    run_vm_test(
        r#"
        fn find_pair() -> i64 {
            let mut result = 0;
            'outer: for i in 0..5 {
                for j in 0..5 {
                    if (i + j == 6) {
                        result = i * 10 + j;
                        break 'outer;
                    }
                }
            }
            result
        }
        find_pair()
        "#,
        TestValue::I64(24),
    );

    // continue 'outer from nested loop
    run_vm_test(
        r#"
        fn sum_diag() -> i64 {
            let mut total = 0;
            'outer: for i in 0..5 {
                for j in 0..5 {
                    if (j > i) {
                        continue 'outer;
                    }
                    total += 1;
                }
            }
            total
        }
        sum_diag()
        "#,
        TestValue::I64(15),
    );

    // labeled while loop
    run_vm_test(
        r#"
        fn labeled_while() -> i64 {
            let mut x = 0;
            let mut y = 0;
            'outer: while (x < 10) {
                x += 1;
                while (y < 10) {
                    y += 1;
                    if (y == 3) {
                        continue 'outer;
                    }
                }
            }
            x * 100 + y
        }
        labeled_while()
        "#,
        TestValue::I64(1010),
    );
}

#[test]
fn variable_shadowing_across_blocks() {
    run_vm_test(
        r#"
        fn test() -> i64 {
            let x = 1;
            let result = if (true) {
                let x = 100;
                x
            } else {
                0
            };
            x + result
        }
        test()
        "#,
        TestValue::I64(101),
    );
}

#[test]
fn deeply_nested_function_calls() {
    run_vm_test(
        r#"
        fn a(x: i64) -> i64 { b(x + 1) }
        fn b(x: i64) -> i64 { c(x + 1) }
        fn c(x: i64) -> i64 { d(x + 1) }
        fn d(x: i64) -> i64 { e(x + 1) }
        fn e(x: i64) -> i64 { x + 1 }
        a(0)
        "#,
        TestValue::I64(5),
    );
}

#[test]
fn conditional_reassignment_in_loop() {
    run_vm_test(
        r#"
        fn test() -> i64 {
            let vector = [10, 20, 30];
            let target: i64 = 1;
            let mut result: i64 = 0;
            for i in 0..3 {
                if (i == target) {
                    result = vector[i];
                } else {
                    result += 1;
                }
            }
            result
        }
        test()
        "#,
        TestValue::I64(21),
    );

    run_vm_test(
        r#"
        fn test() -> i64 {
            let mut sum: i64 = 0;
            for i in 0..5 {
                if (i % 2 == 0) {
                    sum += i;
                }
            }
            sum
        }
        test()
        "#,
        TestValue::I64(6),
    );
}

#[test]
fn struct_update_syntax() {
    // Basic struct update: override one field, inherit the rest
    run_vm_test(
        r#"
        struct Point { x: i64, y: i64 }
        fn test() -> i64 {
            let p1 = Point { x: 1, y: 2 };
            let p2 = Point { x: 10, ..p1 };
            p2.x + p2.y
        }
        test()
        "#,
        TestValue::I64(12),
    );

    // Update with no explicit fields (clone via update syntax)
    run_vm_test(
        r#"
        struct Pair { a: i64, b: i64 }
        fn test() -> i64 {
            let p1 = Pair { a: 3, b: 7 };
            let p2 = Pair { ..p1 };
            p2.a + p2.b
        }
        test()
        "#,
        TestValue::I64(10),
    );

    // Update with all fields overridden (base is unused but valid)
    run_vm_test(
        r#"
        struct Vec2 { x: i64, y: i64 }
        fn test() -> i64 {
            let v1 = Vec2 { x: 1, y: 2 };
            let v2 = Vec2 { x: 100, y: 200, ..v1 };
            v2.x + v2.y
        }
        test()
        "#,
        TestValue::I64(300),
    );

    // Struct update with three fields
    run_vm_test(
        r#"
        struct Config { width: i64, height: i64, depth: i64 }
        fn test() -> i64 {
            let base = Config { width: 10, height: 20, depth: 30 };
            let updated = Config { height: 99, ..base };
            updated.width + updated.height + updated.depth
        }
        test()
        "#,
        TestValue::I64(139),
    );

    // Struct update with a function returning the base
    run_vm_test(
        r#"
        struct Rect { w: i64, h: i64 }
        fn default_rect() -> Rect {
            Rect { w: 5, h: 10 }
        }
        fn test() -> i64 {
            let r = Rect { h: 42, ..default_rect() };
            r.w + r.h
        }
        test()
        "#,
        TestValue::I64(47),
    );
}

#[test]
fn map_type() {
    // Map::new, insert, get
    run_vm_test(
        r#"
        fn test() -> i64 {
            let m = Map::new();
            let m = m.insert(1, 100);
            let m = m.insert(2, 200);
            m.get(1).unwrap() + m.get(2).unwrap()
        }
        test()
        "#,
        TestValue::I64(300),
    );

    // Map::contains_key
    run_vm_test(
        r#"
        fn test() -> bool {
            let m = Map::new();
            let m = m.insert(42, 0);
            m.contains_key(42)
        }
        test()
        "#,
        TestValue::Bool(true),
    );

    // Map::contains_key returns false for missing key
    run_vm_test(
        r#"
        fn test() -> bool {
            let m = Map::new();
            let m = m.insert(1, 10);
            m.contains_key(99)
        }
        test()
        "#,
        TestValue::Bool(false),
    );

    // Map::remove
    run_vm_test(
        r#"
        fn test() -> usize {
            let m = Map::new();
            let m = m.insert(1, 10);
            let m = m.insert(2, 20);
            let m = m.remove(1);
            m.len()
        }
        test()
        "#,
        TestValue::Usize(1),
    );

    // Map::len
    run_vm_test(
        r#"
        fn test() -> usize {
            let m = Map::new();
            let m = m.insert(1, 10);
            let m = m.insert(2, 20);
            let m = m.insert(3, 30);
            m.len()
        }
        test()
        "#,
        TestValue::Usize(3),
    );

    // Map::keys
    run_vm_test(
        r#"
        fn test() -> i64 {
            let m = Map::new();
            let m = m.insert(10, 100);
            let m = m.insert(20, 200);
            let ks = m.keys();
            ks[0] + ks[1]
        }
        test()
        "#,
        TestValue::I64(30),
    );

    // Map::values
    run_vm_test(
        r#"
        fn test() -> i64 {
            let m = Map::new();
            let m = m.insert(1, 100);
            let m = m.insert(2, 200);
            let vs = m.values();
            vs[0] + vs[1]
        }
        test()
        "#,
        TestValue::I64(300),
    );

    // Map with string keys
    run_vm_test(
        r#"
        fn test() -> i64 {
            let m = Map::new();
            let m = m.insert("x", 10);
            let m = m.insert("y", 20);
            m.get("x").unwrap() + m.get("y").unwrap()
        }
        test()
        "#,
        TestValue::I64(30),
    );

    // Map indexing with [] operator
    run_vm_test(
        r#"
        fn test() -> i64 {
            let m = Map::new();
            let m = m.insert(1, 42);
            m[1]
        }
        test()
        "#,
        TestValue::I64(42),
    );
}

#[test]
fn generic_set() {
    // Set<str>: insert and contains with string elements
    run_vm_test(
        r#"
        fn test() -> bool {
            let s = Set::new();
            let s = s.insert("a");
            let s = s.insert("b");
            s.contains("a")
        }
        test()
        "#,
        TestValue::Bool(true),
    );
    // Set<str>: contains returns false for missing element
    run_vm_test(
        r#"
        fn test() -> bool {
            let s = Set::new();
            let s = s.insert("hello");
            s.contains("world")
        }
        test()
        "#,
        TestValue::Bool(false),
    );
    // Set<bool>: insert and len
    run_vm_test(
        r#"
        fn test() -> usize {
            let s = Set::new();
            let s = s.insert(true);
            let s = s.insert(false);
            let s = s.insert(true);
            s.len()
        }
        test()
        "#,
        TestValue::Usize(2),
    );
    // Set<str>: remove
    run_vm_test(
        r#"
        fn test() -> usize {
            let s = Set::new();
            let s = s.insert("x");
            let s = s.insert("y");
            let s = s.insert("z");
            let s = s.remove("y");
            s.len()
        }
        test()
        "#,
        TestValue::Usize(2),
    );

    // Set<i64>: to_vector preserves elements
    run_vm_test(
        r#"
        fn test() -> i64 {
            let s = Set::new();
            let s = s.insert(10);
            let s = s.insert(20);
            let vector = s.to_vector();
            vector[0] + vector[1]
        }
        test()
        "#,
        TestValue::I64(30),
    );
    // Set used in struct field with generic element type
    run_vm_test(
        r#"
        struct State {
            visited: Set,
        }
        fn test() -> bool {
            let st = State { visited: Set::new().insert("node_a") };
            st.visited.contains("node_a")
        }
        test()
        "#,
        TestValue::Bool(true),
    );
    // Set<str>: method chaining
    run_vm_test(
        r#"
        fn test() -> usize {
            Set::new().insert("a").insert("b").insert("c").len()
        }
        test()
        "#,
        TestValue::Usize(3),
    );
}

#[test]
fn numeric_from_conversions() {
    // Signed widening chain
    run_vm_test("let x: i8 = 42; i16::from(x)", TestValue::I16(42));
    run_vm_test("let x: i16 = 1000; i32::from(x)", TestValue::I32(1000));
    run_vm_test("let x: i32 = 100000; i64::from(x)", TestValue::I64(100000));
    run_vm_test(
        "let x: i64 = 9223372036854775807; i128::from(x)",
        TestValue::I128(9223372036854775807),
    );

    // Unsigned widening chain
    run_vm_test("let x: u8 = 200; u16::from(x)", TestValue::U16(200));
    run_vm_test("let x: u16 = 50000; u32::from(x)", TestValue::U32(50000));
    run_vm_test(
        "let x: u32 = 3000000000; u64::from(x)",
        TestValue::U64(3000000000),
    );
    run_vm_test(
        "let x: u64 = 10000000000; u128::from(x)",
        TestValue::U128(10000000000),
    );

    // Cross-sign widening (unsigned --> larger signed)
    run_vm_test("let x: u8 = 255; i16::from(x)", TestValue::I16(255));
    run_vm_test("let x: u16 = 65535; i32::from(x)", TestValue::I32(65535));
    run_vm_test(
        "let x: u32 = 4294967295; i64::from(x)",
        TestValue::I64(4294967295),
    );
}

#[test]
fn default_values() {
    run_vm_test("i8::default()", TestValue::I8(0));
    run_vm_test("i16::default()", TestValue::I16(0));
    run_vm_test("i32::default()", TestValue::I32(0));
    run_vm_test("i64::default()", TestValue::I64(0));
    run_vm_test("u8::default()", TestValue::U8(0));
    run_vm_test("u16::default()", TestValue::U16(0));
    run_vm_test("u32::default()", TestValue::U32(0));
    run_vm_test("u64::default()", TestValue::U64(0));
    run_vm_test("bool::default()", TestValue::Bool(false));
    run_vm_test("str::default()", TestValue::Str(String::new()));
}

#[test]
fn cmp_min_max_clamp() {
    run_vm_test("cmp::min(10, 20)", TestValue::I64(10));
    run_vm_test("cmp::min(20, 10)", TestValue::I64(10));
    run_vm_test("cmp::max(10, 20)", TestValue::I64(20));
    run_vm_test("cmp::max(20, 10)", TestValue::I64(20));
    run_vm_test("cmp::clamp(25, 0, 20)", TestValue::I64(20));
    run_vm_test("cmp::clamp(5, 0, 20)", TestValue::I64(5));
    run_vm_test("cmp::clamp(-5, 0, 20)", TestValue::I64(0));

    // Typed variants
    run_vm_test("cmp::min(10i32, 20i32)", TestValue::I32(10));
    run_vm_test("cmp::max(10i32, 20i32)", TestValue::I32(20));
}
