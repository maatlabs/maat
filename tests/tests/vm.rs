use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_runtime::{Hashable, Object};
use maat_vm::VM;

#[derive(Debug)]
enum TestValue {
    I64(i64),
    I32(i32),
    Usize(usize),
    Bool(bool),
    Str(String),
    IntArray(Vec<i64>),
    Hash(Vec<(i64, i64)>),
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
        TestValue::I64(exp) => match stack_elem {
            Object::I64(val) => {
                assert_eq!(val, exp, "wrong integer value for input: {input}")
            }
            Object::Usize(val) => {
                assert_eq!(val as i64, exp, "wrong integer value for input: {input}")
            }
            _ => panic!("expected integer object, got: {:?}", stack_elem),
        },
        TestValue::I32(exp) => match stack_elem {
            Object::I32(val) => {
                assert_eq!(val, exp, "wrong I32 value for input: {input}")
            }
            _ => panic!("expected I32 object, got: {:?}", stack_elem),
        },
        TestValue::Usize(exp) => match stack_elem {
            Object::Usize(val) => {
                assert_eq!(val, exp, "wrong Usize value for input: {input}")
            }
            _ => panic!("expected Usize object, got: {:?}", stack_elem),
        },
        TestValue::Bool(exp) => match stack_elem {
            Object::Bool(val) => {
                assert_eq!(val, exp, "wrong boolean value for input: {input}")
            }
            _ => panic!("expected boolean object, got: {:?}", stack_elem),
        },
        TestValue::Str(exp) => match stack_elem {
            Object::Str(val) => {
                assert_eq!(val, exp, "wrong string value for input: {input}")
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
        TestValue::Range(s, e) => {
            assert_eq!(
                stack_elem,
                Object::Range(s, e),
                "expected Range({s}..{e}) for input: {input}, got: {:?}",
                stack_elem
            );
        }
        TestValue::RangeInclusive(s, e) => {
            assert_eq!(
                stack_elem,
                Object::RangeInclusive(s, e),
                "expected RangeInclusive({s}..={e}) for input: {input}, got: {:?}",
                stack_elem
            );
        }
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
        ("if (true) { 10 }", TestValue::I64(10)),
        ("if (true) { 10 } else { 20 }", TestValue::I64(10)),
        ("if (false) { 10 } else { 20 }", TestValue::I64(20)),
        ("if (1 < 2) { 10 }", TestValue::I64(10)),
        ("if (1 < 2) { 10 } else { 20 }", TestValue::I64(10)),
        ("if (1 > 2) { 10 } else { 20 }", TestValue::I64(20)),
        ("if (1 > 2) { 10 }", TestValue::Null),
        ("if (false) { 10 }", TestValue::Null),
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
fn builtin_methods() {
    let cases = vec![
        // Array methods
        ("[1, 2, 3].len()", TestValue::Usize(3)),
        ("[].len()", TestValue::Usize(0)),
        ("[1, 2, 3].first()", TestValue::I64(1)),
        ("[].first()", TestValue::Null),
        ("[1, 2, 3].last()", TestValue::I64(3)),
        ("[].last()", TestValue::Null),
        ("[1, 2, 3].rest()", TestValue::IntArray(vec![2, 3])),
        ("[].rest()", TestValue::Null),
        ("[].push(1)", TestValue::IntArray(vec![1])),
        // str methods
        (r#""".len()"#, TestValue::Usize(0)),
        (r#""four".len()"#, TestValue::Usize(4)),
        (r#""hello".len()"#, TestValue::Usize(5)),
        // len with cast
        ("[1, 2, 3].len() as i64", TestValue::I64(3)),
        // print remains a free function
        (r#"print("hello", "world!")"#, TestValue::Null),
    ];
    for (input, expected) in cases {
        run_vm_test(input, expected);
    }
}

#[test]
fn builtin_method_chaining() {
    // `push` returns a new array, so chaining is possible
    run_vm_test("[1, 2].push(3).len()", TestValue::Usize(3));
    // `rest` returns a new array
    run_vm_test("[1, 2, 3].rest().first()", TestValue::I64(2));
    run_vm_test("[1, 2, 3].rest().last()", TestValue::I64(3));
    run_vm_test("let arr = [10, 20, 30]; arr.len()", TestValue::Usize(3));
    run_vm_test("let arr = [10, 20, 30]; arr.first()", TestValue::I64(10));
    run_vm_test("let arr = [10, 20, 30]; arr.last()", TestValue::I64(30));
    run_vm_test(r#"let s = "hello world"; s.len()"#, TestValue::Usize(11));
    run_vm_test(
        "let arr = [1, 2, 3]; arr.len() as i64 + 1",
        TestValue::I64(4),
    );
    run_vm_test("let arr = [1, 2, 3]; arr.rest().len()", TestValue::Usize(2));
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
fn closure_captures_array_with_builtins() {
    run_vm_test(
        r#"
        let sum = fn(arr) {
            let iter = fn(idx, acc) {
                if (idx == arr.len()) {
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
        // Signed negation
        ("-5i32", TestValue::I32(-5)),
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
        ("5 == [1, 2, 3, 4, 5].len()", TestValue::Bool(true)),
        ("5 != [1, 2, 3, 4, 5].len()", TestValue::Bool(false)),
        ("10 > [1, 2, 3].len()", TestValue::Bool(true)),
        ("1 < [1, 2, 3].len()", TestValue::Bool(true)),
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
        let find_first = fn(arr, target) {
            let mut i = 0;
            loop {
                if (i == arr.len() as i64) {
                    return -1;
                }
                if (arr[i] == target) {
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
    // Empty array
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
    let lexer = maat_lexer::Lexer::new(input);
    let mut parser = maat_parser::Parser::new(lexer);
    let mut program = parser.parse();
    maat_ast::fold::fold_constants(&mut program);
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
    run_vm_error_test("1 << 64", "shift amount exceeds type bit width");
    run_vm_error_test("1 >> 64", "shift amount exceeds type bit width");
    // Negative shift amount
    run_vm_error_test("1 << -1", "shift amount exceeds type bit width");
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
        fn find_first_even(arr: [i64]) -> i64 {
            let mut result = -1;
            for x in arr {
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
            let arr = [10, 20, 30];
            let target: i64 = 1;
            let mut result: i64 = 0;
            for i in 0..3 {
                if (i == target) {
                    result = arr[i];
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
