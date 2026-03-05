use maat_runtime::Object;
use maat_types::TypeChecker;
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

fn type_check(input: &str) -> Vec<String> {
    let mut program = maat_tests::parse(input);
    let errors = TypeChecker::new().check_program(&mut program);
    errors.iter().map(|e| e.kind.to_string()).collect()
}

fn assert_no_type_errors(input: &str) {
    let errors = type_check(input);
    assert!(errors.is_empty(), "unexpected type errors: {errors:?}");
}

fn assert_type_error_contains(input: &str, needle: &str) {
    let errors = type_check(input);
    assert!(
        errors.iter().any(|e| e.contains(needle)),
        "expected error containing `{needle}`, got: {errors:?}"
    );
}

#[test]
fn let_binding_and_inference() {
    run_vm_test("let x: i64 = 5; x", TestValue::I64(5));
    run_vm_test("let x = 5; let y = x + 10; y", TestValue::I64(15));
    run_vm_test(
        "let x: i64 = 10; let y: i64 = 20; x + y",
        TestValue::I64(30),
    );
}

#[test]
fn literal_coercion_to_annotation() {
    run_vm_test("let x: u8 = 42; x", TestValue::U8(42));
    run_vm_test("let x = 42u8; x", TestValue::U8(42));
    run_vm_test("let x: i16 = -100; x", TestValue::I16(-100));
}

#[test]
fn literal_overflow_errors() {
    let errors = maat_tests::compile_type_errors("let x: i8 = -129;");
    assert!(
        errors.iter().any(|e| e.contains("overflow")),
        "expected overflow error, got: {errors:?}"
    );

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
fn constant_folding() {
    // Simple addition
    let bytecode = maat_tests::compile("1 + 2");
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

    // Nested addition
    let bytecode = maat_tests::compile("1 + 2 + 3");
    assert_eq!(bytecode.constants.len(), 1);
    match &bytecode.constants[0] {
        Object::I64(val) => assert_eq!(*val, 6),
        other => panic!("expected I64(6), got: {other:?}"),
    }

    // Multiplication
    let bytecode = maat_tests::compile("3 * 7");
    assert_eq!(bytecode.constants.len(), 1);
    match &bytecode.constants[0] {
        Object::I64(val) => assert_eq!(*val, 21),
        other => panic!("expected I64(21), got: {other:?}"),
    }

    // Boolean comparison
    let bytecode = maat_tests::compile("1 < 2");
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    let result = vm.last_popped_stack_elem().expect("no value").clone();
    assert_eq!(result, Object::Bool(true));
}

#[test]
fn numeric_promotion_i8_to_i16() {
    run_vm_test(
        "let x: i8 = 5i8; let y: i16 = 10i16; x + y",
        TestValue::I16(15),
    );
}

#[test]
fn comparison_type_checks() {
    run_vm_test("let x = 5; let y = 10; x < y", TestValue::Bool(true));
    run_vm_test("let x = 5; let y = 5; x == y", TestValue::Bool(true));
    run_vm_test("let x = 5; let y = 10; x != y", TestValue::Bool(true));
}

#[test]
fn typed_function_params_and_return() {
    run_vm_test(
        "let add = fn(x: i64, y: i64) -> i64 { x + y; }; add(3, 4)",
        TestValue::I64(7),
    );
}

#[test]
fn generic_function() {
    let program = maat_tests::parse("let identity = fn<T>(x: T) -> T { x };");
    assert_eq!(program.statements.len(), 1);

    let bytecode = maat_tests::compile("let identity = fn<T>(x: T) -> T { x };");
    assert!(
        !bytecode.constants.is_empty(),
        "function should produce a constant"
    );
}

#[test]
fn struct_declarations() {
    assert_no_type_errors("struct Point { x: i64, y: i64 }");
    assert_no_type_errors("struct Pair<T, U> { first: T, second: U }");
}

#[test]
fn enum_declarations() {
    assert_no_type_errors("enum Direction { North, South, East, West }");
    assert_no_type_errors("enum Shape { Circle(i64), Rectangle(i64, i64) }");
    assert_no_type_errors("enum Option<T> { Some(T), None }");
}

#[test]
fn trait_declarations() {
    assert_no_type_errors("trait Greet { fn hello(self) -> bool; }");
}

#[test]
fn duplicate_type_errors() {
    assert_type_error_contains(
        "struct Point { x: i64 } struct Point { y: i64 }",
        "duplicate type",
    );
    assert_type_error_contains("enum Color { Red } enum Color { Blue }", "duplicate type");
}

#[test]
fn impl_blocks() {
    assert_no_type_errors(
        "struct Point { x: i64, y: i64 }
         impl Point {
             fn get_x(self) -> i64 { 0 }
         }",
    );

    assert_no_type_errors(
        "struct Point { x: i64, y: i64 }
         trait Greet { fn hello(self) -> bool; }
         impl Greet for Point {
             fn hello(self) -> bool { true }
         }",
    );
}

#[test]
fn impl_block_errors() {
    assert_type_error_contains(
        "struct Point { x: i64 }
         trait Greet { fn hello(self) -> bool; }
         impl Greet for Point {}",
        "missing trait method",
    );

    assert_type_error_contains(
        "struct Point { x: i64 }
         impl Unknown for Point {}",
        "unknown trait",
    );
}

#[test]
fn field_access() {
    assert_no_type_errors(
        "struct Point { x: i64, y: i64 }
         impl Point {
             fn get_x(self) -> i64 { self.x }
         }",
    );

    assert_type_error_contains(
        "struct Point { x: i64, y: i64 }
         impl Point {
             fn get_z(self) -> i64 { self.z }
         }",
        "no field `z`",
    );
}

#[test]
fn match_exhaustiveness() {
    // Wildcard covers all
    assert_no_type_errors(
        "let x = 5;
         match x { 1 => true, _ => false }",
    );

    // All enum variants covered
    assert_no_type_errors(
        "enum Direction { North, South, East, West }
         fn check(d: Direction) -> i64 {
             match d { North => 0, South => 1, East => 2, West => 3 }
         }",
    );

    // Both bool values covered
    assert_no_type_errors("let x = true; match x { true => 1, false => 0 }");

    // Tuple struct pattern with wildcard
    assert_no_type_errors(
        "enum Option<T> { Some(T), None }
         fn unwrap(opt: Option<i64>) -> i64 {
             match opt { Some(v) => v, None => 0 }
         }",
    );

    // Guard does not invalidate pattern binding
    assert_no_type_errors(
        "let x = 5;
         match x { y if (y > 3) => true, _ => false }",
    );
}

#[test]
fn match_non_exhaustive_errors() {
    assert_type_error_contains(
        "enum Direction { North, South, East, West }
         fn check(d: Direction) -> i64 {
             match d { North => 0, South => 1 }
         }",
        "non-exhaustive",
    );

    assert_type_error_contains("let x = 5; match x { 1 => true }", "non-exhaustive");

    assert_type_error_contains("let x = true; match x { true => 1 }", "non-exhaustive");
}

#[test]
fn match_arm_type_unification() {
    assert_type_error_contains("let x = 5; match x { 1 => true, _ => 42 }", "mismatch");
}

#[test]
fn type_resolution_with_custom_types() {
    assert_no_type_errors(
        "struct Point { x: i64, y: i64 }
         fn make() -> Point { make() }",
    );

    assert_no_type_errors(
        "struct Foo { val: i64 }
         fn make() -> Foo { make() }",
    );

    assert_no_type_errors(
        "enum Color { Red, Green, Blue }
         fn show(c: Color) -> i64 { 0 }",
    );
}
