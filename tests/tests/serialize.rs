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
fn roundtrip_complex_program() {
    // Nested closures: a graph of captured `CompiledFn` constants.
    run_roundtrip_test(
        "let a = fn(x) { let b = fn(y) { let c = fn(z) { x + y + z }; c }; b }; a(1)(2)(3)",
        Value::Integer(Integer::I64(6)),
    );
    // Self-recursion through a serialized-then-restored function constant.
    run_roundtrip_test(
        "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)",
        Value::Integer(Integer::I64(55)),
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
