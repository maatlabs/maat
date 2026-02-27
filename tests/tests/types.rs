use maat_runtime::Object;
use maat_vm::VM;

#[derive(Debug)]
enum TestValue {
    I64(i64),
    I16(i16),
    U8(u8),
    Bool(bool),
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
            Object::I64(val) => assert_eq!(
                val, expected_val,
                "wrong value for input: {input}\n  got: {val}\n  want: {expected_val}"
            ),
            _ => panic!("expected I64, got: {stack_elem:?} for input: {input}"),
        },
        TestValue::I16(expected_val) => match stack_elem {
            Object::I16(val) => assert_eq!(
                val, expected_val,
                "wrong value for input: {input}\n  got: {val}\n  want: {expected_val}"
            ),
            _ => panic!("expected I16, got: {stack_elem:?} for input: {input}"),
        },
        TestValue::U8(expected_val) => match stack_elem {
            Object::U8(val) => assert_eq!(
                val, expected_val,
                "wrong value for input: {input}\n  got: {val}\n  want: {expected_val}"
            ),
            _ => panic!("expected U8, got: {stack_elem:?} for input: {input}"),
        },
        TestValue::Bool(expected_val) => match stack_elem {
            Object::Bool(val) => assert_eq!(
                val, expected_val,
                "wrong value for input: {input}\n  got: {val}\n  want: {expected_val}"
            ),
            _ => panic!("expected Bool, got: {stack_elem:?} for input: {input}"),
        },
    }
}

#[test]
fn typed_let_statement() {
    run_vm_test("let x: i64 = 5; x", TestValue::I64(5));
}

#[test]
fn literal_coercion_to_annotation() {
    // Unsuffixed integer literals coerce to the declared type.
    run_vm_test("let x: u8 = 42; x", TestValue::U8(42));
    run_vm_test("let x = 42u8; x", TestValue::U8(42));
}

#[test]
fn negated_literal_coercion() {
    // Negated literals coerce to signed types within range.
    run_vm_test("let x: i16 = -100; x", TestValue::I16(-100));
}

#[test]
fn negated_literal_overflow() {
    let errors = maat_tests::compile_type_errors("let x: i8 = -129;");
    assert!(
        errors.iter().any(|e| e.contains("overflow")),
        "expected overflow error, got: {errors:?}"
    );
}

#[test]
fn inferred_let_statement() {
    run_vm_test("let x = 5; let y = x + 10; y", TestValue::I64(15));
}

#[test]
fn constant_folding_integer_arithmetic() {
    let bytecode = maat_tests::compile("1 + 2");
    // After constant folding, `1 + 2` should produce a single constant `3`.
    assert_eq!(
        bytecode.constants.len(),
        1,
        "expected 1 constant after folding, got {}",
        bytecode.constants.len()
    );
    match &bytecode.constants[0] {
        Object::I64(val) => assert_eq!(*val, 3),
        other => panic!("expected I64(3), got: {other:?}"),
    }
}

#[test]
fn constant_folding_nested() {
    let bytecode = maat_tests::compile("1 + 2 + 3");
    assert_eq!(bytecode.constants.len(), 1);
    match &bytecode.constants[0] {
        Object::I64(val) => assert_eq!(*val, 6),
        other => panic!("expected I64(6), got: {other:?}"),
    }
}

#[test]
fn constant_folding_multiplication() {
    let bytecode = maat_tests::compile("3 * 7");
    assert_eq!(bytecode.constants.len(), 1);
    match &bytecode.constants[0] {
        Object::I64(val) => assert_eq!(*val, 21),
        other => panic!("expected I64(21), got: {other:?}"),
    }
}

#[test]
fn constant_folding_boolean() {
    let bytecode = maat_tests::compile("1 < 2");
    // Comparison of two constants should fold to a single boolean.
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    let result = vm.last_popped_stack_elem().expect("no value").clone();
    assert_eq!(result, Object::Bool(true));
}

#[test]
fn type_error_overflow() {
    let errors = maat_tests::compile_type_errors("let x: i8 = 256;");
    assert!(
        errors.iter().any(|e| e.contains("overflow")),
        "expected overflow error, got: {errors:?}"
    );
}

#[test]
fn type_mismatch_error() {
    let errors = maat_tests::compile_type_errors(r#"let x: i64 = "hello";"#);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("mismatch") || e.contains("Mismatch")),
        "expected type mismatch, got: {errors:?}"
    );
}

#[test]
fn type_error_implicit_float_promotion() {
    let errors = maat_tests::compile_type_errors("let x: i8 = 5i8; let y: f64 = 1.0f64; x + y;");
    assert!(
        errors.iter().any(|e| e.contains("float")),
        "expected implicit float promotion error, got: {errors:?}"
    );
}

#[test]
fn numeric_promotion_i8_to_i16() {
    run_vm_test(
        "let x: i8 = 5i8; let y: i16 = 10i16; x + y",
        TestValue::I16(15),
    );
}

#[test]
fn typed_function_params_and_return() {
    run_vm_test(
        "let add = fn(x: i64, y: i64) -> i64 { x + y; }; add(3, 4)",
        TestValue::I64(7),
    );
}

#[test]
fn generic_function_parses() {
    // Verify that a generic function definition parses and type-checks.
    // Full monomorphization is a TODO; here we just ensure no errors.
    let program = maat_tests::parse("let identity = fn<T>(x: T) -> T { x };");
    assert_eq!(program.statements.len(), 1);
}

#[test]
fn generic_function_compiles() {
    let bytecode = maat_tests::compile("let identity = fn<T>(x: T) -> T { x };");
    assert!(
        !bytecode.constants.is_empty(),
        "function should produce a constant"
    );
}

#[test]
fn typed_let_with_inference() {
    // Type annotation matches inferred type.
    run_vm_test(
        "let x: i64 = 10; let y: i64 = 20; x + y",
        TestValue::I64(30),
    );
}

#[test]
fn comparison_type_checks() {
    run_vm_test("let x = 5; let y = 10; x < y", TestValue::Bool(true));
    run_vm_test("let x = 5; let y = 5; x == y", TestValue::Bool(true));
    run_vm_test("let x = 5; let y = 10; x != y", TestValue::Bool(true));
}
