use maat_runtime::{Integer, Value};
use maat_tests::compile;
use maat_vm::VM;

fn run_roundtrip_test(input: &str, expected: Value) {
    let bytecode = maat_tests::roundtrip(input);
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    let result = vm
        .last_popped_stack_elem()
        .expect("no value on stack")
        .clone();
    assert_eq!(result, expected, "mismatch for input: {input}");
}

#[test]
fn integer_arithmetic() {
    run_roundtrip_test("1 + 2", Value::Integer(Integer::I64(3)));
    run_roundtrip_test("10 - 3", Value::Integer(Integer::I64(7)));
    run_roundtrip_test("2 * 6", Value::Integer(Integer::I64(12)));
    run_roundtrip_test("10 / 2", Value::Integer(Integer::I64(5)));
}

#[test]
fn boolean_expressions() {
    run_roundtrip_test("true", Value::Bool(true));
    run_roundtrip_test("false", Value::Bool(false));
    run_roundtrip_test("1 < 2", Value::Bool(true));
    run_roundtrip_test("1 > 2", Value::Bool(false));
    run_roundtrip_test("1 == 1", Value::Bool(true));
    run_roundtrip_test("1 != 2", Value::Bool(true));
}

#[test]
fn string_expressions() {
    run_roundtrip_test(
        r#""hello" + " " + "world""#,
        Value::Str("hello world".to_owned()),
    );
    run_roundtrip_test(r#""maat""#, Value::Str("maat".to_owned()));
}

#[test]
fn let_bindings() {
    run_roundtrip_test("let x = 5; x", Value::Integer(Integer::I64(5)));
    run_roundtrip_test(
        "let x = 5; let y = 10; x + y",
        Value::Integer(Integer::I64(15)),
    );
}

#[test]
fn conditionals() {
    run_roundtrip_test("if true { 10 }", Value::Integer(Integer::I64(10)));
    run_roundtrip_test(
        "if false { 10 } else { 20 }",
        Value::Integer(Integer::I64(20)),
    );
}

#[test]
fn functions_and_closures() {
    run_roundtrip_test(
        "let add = fn(a, b) { a + b }; add(3, 4)",
        Value::Integer(Integer::I64(7)),
    );
    run_roundtrip_test(
        "let make_adder = fn(x) { fn(y) { x + y } }; let add5 = make_adder(5); add5(3)",
        Value::Integer(Integer::I64(8)),
    );
}

#[test]
fn recursive_fibonacci() {
    run_roundtrip_test(
        "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)",
        Value::Integer(Integer::I64(55)),
    );
}

#[test]
fn recursive_factorial() {
    run_roundtrip_test(
        "let factorial = fn(n) { if (n == 0) { 1 } else { n * factorial(n - 1) } }; factorial(5)",
        Value::Integer(Integer::I64(120)),
    );
}

#[test]
fn arrays() {
    run_roundtrip_test("[1, 2, 3][1]", Value::Integer(Integer::I64(2)));
    run_roundtrip_test("[1, 2, 3].len()", Value::Integer(Integer::Usize(3)));
}

#[test]
fn hash_literals() {
    run_roundtrip_test("{1: 10, 2: 20}[1]", Value::Integer(Integer::I64(10)));
}

#[test]
fn cast_expressions() {
    run_roundtrip_test("42 as i32", Value::Integer(Integer::I32(42)));
    run_roundtrip_test("[1, 2].len() as i64", Value::Integer(Integer::I64(2)));
}

#[test]
fn nested_closures() {
    run_roundtrip_test(
        "let a = fn(x) { let b = fn(y) { let c = fn(z) { x + y + z }; c }; b }; a(1)(2)(3)",
        Value::Integer(Integer::I64(6)),
    );
}

/// Verify that compiling the same source twice produces byte-identical `.mtc` output
#[test]
fn bytecode_determinism() {
    let sources = [
        "let x = 1; let y = 2; x + y",
        "let add = fn(a, b) { a + b }; add(3, 4)",
        "{1: 10, 2: 20, 3: 30}[2]",
        "[1, 2, 3, 4, 5]",
        r#"let greet = fn(name) { "hello " + name }; greet("maat")"#,
        "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)",
        "let a = fn(x) { let b = fn(y) { x + y }; b }; a(1)(2)",
        "{true: 1, false: 0}[true]",
    ];
    for source in sources {
        let bytes_a = compile(source)
            .serialize()
            .expect("first serialization failed");
        let bytes_b = compile(source)
            .serialize()
            .expect("second serialization failed");

        assert_eq!(
            bytes_a, bytes_b,
            "non-deterministic bytecode for input: {source}"
        );
    }
}
