use maat_runtime::Object;
use maat_types::TypeChecker;
use maat_vm::VM;

fn run(input: &str) -> Object {
    let bytecode = maat_tests::compile(input);
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    vm.last_popped_stack_elem()
        .expect("no value on stack")
        .clone()
}

fn run_i64(input: &str, expected: i64) {
    match run(input) {
        Object::I64(v) => assert_eq!(v, expected, "wrong value for:\n{input}"),
        other => panic!("expected I64({expected}), got {other:?} for:\n{input}"),
    }
}

fn run_bool(input: &str, expected: bool) {
    match run(input) {
        Object::Bool(v) => assert_eq!(v, expected, "wrong value for:\n{input}"),
        other => panic!("expected Bool({expected}), got {other:?} for:\n{input}"),
    }
}

fn run_str(input: &str, expected: &str) {
    match run(input) {
        Object::Str(v) => assert_eq!(v, expected, "wrong value for:\n{input}"),
        other => panic!("expected Str({expected:?}), got {other:?} for:\n{input}"),
    }
}

fn assert_no_type_errors(input: &str) {
    let mut program = maat_tests::parse(input);
    let errors = TypeChecker::new().check_program(&mut program);
    assert!(
        errors.is_empty(),
        "unexpected type errors: {:?}",
        errors
            .iter()
            .map(|e| e.kind.to_string())
            .collect::<Vec<_>>()
    );
}

fn assert_type_error_contains(needle: &str, input: &str) {
    let mut program = maat_tests::parse(input);
    let errors = TypeChecker::new().check_program(&mut program);
    let msgs = errors
        .iter()
        .map(|e| e.kind.to_string())
        .collect::<Vec<String>>();

    assert!(
        msgs.iter().any(|m| m.contains(needle)),
        "expected error containing `{needle}`, got: {msgs:?}"
    );
}

#[test]
fn structs() {
    assert_type_error_contains(
        "no field",
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 1, z: 2 };",
    );

    assert_type_error_contains(
        "missing field",
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 1 };",
    );

    run_i64(
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 3, y: 4 };
         p.x",
        3,
    );

    run_i64(
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 10, y: 20 };
         p.y",
        20,
    );

    run_i64(
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 5, y: 7 };
         p.x + p.y",
        12,
    );

    run_i64(
        "struct Inner { val: i64 }
         struct Outer { inner: Inner }
         let o = Outer { inner: Inner { val: 42 } };
         o.inner.val",
        42,
    );

    run_i64(
        "struct Wrapper<T> { inner: T }
         let w = Wrapper { inner: 42 };
         w.inner",
        42,
    );

    run_bool(
        "struct Wrapper<T> { inner: T }
         let w = Wrapper { inner: true };
         w.inner",
        true,
    );

    run_i64(
        "struct Stack { items: [i64] }
         impl Stack {
             fn peek(self) -> Option<i64> {
                 if (len(self.items) == 0usize) {
                     Option::None
                 } else {
                     Option::Some(first(self.items))
                 }
             }
         }
         let s = Stack { items: [10, 20, 30] };
         match s.peek() { Some(v) => v, None => -1 }",
        10,
    );

    run_i64(
        "struct Stack { items: [i64] }
         impl Stack {
             fn peek(self) -> Option<i64> {
                 if (len(self.items) == 0usize) {
                     Option::None
                 } else {
                     Option::Some(first(self.items))
                 }
             }
         }
         let s = Stack { items: [] };
         match s.peek() { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn enums() {
    assert_type_error_contains(
        "duplicate type definition",
        "enum Option<T> { Some(T), None }",
    );

    assert_type_error_contains(
        "duplicate type definition",
        "enum Result<T, E> { Ok(T), Err(E) }",
    );

    run_i64(
        "enum Color { Red, Green, Blue }
         let c = Color::Red;
         match c { Red => 0, Green => 1, Blue => 2 }",
        0,
    );

    run_i64(
        "enum Color { Red, Green, Blue }
         let c = Color::Blue;
         match c { Red => 0, Green => 1, Blue => 2 }",
        2,
    );

    run_i64(
        "enum Shape { Circle(i64), Rect(i64, i64) }
         let s = Shape::Circle(5);
         match s { Circle(r) => r, Rect(w, h) => w * h }",
        5,
    );

    run_i64(
        "enum Shape { Circle(i64), Rect(i64, i64) }
         let s = Shape::Rect(3, 4);
         match s { Circle(r) => r, Rect(w, h) => w * h }",
        12,
    );

    run_bool(
        "enum Direction { North, South, East, West }
         impl Direction {
             fn is_north(self) -> bool {
                 match self { North => true, _ => false }
             }
         }
         let d = Direction::North;
         d.is_north()",
        true,
    );

    run_bool(
        "enum Direction { North, South, East, West }
         impl Direction {
             fn is_north(self) -> bool {
                 match self { North => true, _ => false }
             }
         }
         let d = Direction::South;
         d.is_north()",
        false,
    );

    run_i64(
        "let arr = [Option::Some(1), Option::Some(3), Option::Some(6)];
         let total = 0;
         let i = 0;
         while (i < len(arr) as i64) {
             let val = match arr[i] { Some(v) => v, _ => 0 };
             let total = total + val;
             let i = i + 1;
         }
         total",
        10,
    );
}

#[test]
fn option_builtin_type() {
    assert_no_type_errors(
        "fn unwrap(opt: Option<i64>) -> i64 {
             match opt { Some(v) => v, None => 0 }
         }",
    );

    assert_no_type_errors(
        "fn map_opt(opt: Option<i64>) -> Option<i64> {
             match opt { Some(v) => Option::Some(v + 1), None => Option::None }
         }",
    );

    run_i64(
        "let x = Option::Some(42);
         match x { Some(v) => v, None => 0 }",
        42,
    );

    run_i64(
        "let x: Option<i64> = Option::None;
         match x { Some(v) => v, None => -1 }",
        -1,
    );

    run_i64(
        "fn safe_div(a: i64, b: i64) -> Option<i64> {
             if (b == 0) { Option::None } else { Option::Some(a / b) }
         }
         let result = safe_div(10, 2);
         match result { Some(v) => v, None => -1 }",
        5,
    );

    run_i64(
        "fn safe_div(a: i64, b: i64) -> Option<i64> {
             if (b == 0) { Option::None } else { Option::Some(a / b) }
         }
         let result = safe_div(10, 0);
         match result { Some(v) => v, None => -1 }",
        -1,
    );

    run_i64(
        "fn unwrap_or(opt: Option<i64>, default: i64) -> i64 {
             match opt { Some(v) => v, None => default }
         }
         unwrap_or(Option::Some(7), 0)",
        7,
    );

    run_i64(
        "fn unwrap_or(opt: Option<i64>, default: i64) -> i64 {
             match opt { Some(v) => v, None => default }
         }
         unwrap_or(Option::None, 42)",
        42,
    );
}

#[test]
fn result_builtin_type() {
    assert_no_type_errors(
        "fn handle(r: Result<i64, String>) -> i64 {
             match r { Ok(v) => v, Err(e) => 0 }
         }",
    );

    run_i64(
        "let r = Result::Ok(100);
         match r { Ok(v) => v, Err(e) => e }",
        100,
    );

    run_i64(
        "let r: Result<i64, i64> = Result::Err(-1);
         match r { Ok(v) => v, Err(e) => e }",
        -1,
    );

    run_i64(
        "fn checked_div(a: i64, b: i64) -> Result<i64, i64> {
             if (b == 0) { Result::Err(-1) } else { Result::Ok(a / b) }
         }
         let r = checked_div(20, 5);
         match r { Ok(v) => v, Err(e) => e }",
        4,
    );

    run_i64(
        "fn checked_div(a: i64, b: i64) -> Result<i64, i64> {
             if (b == 0) { Result::Err(-1) } else { Result::Ok(a / b) }
         }
         let r = checked_div(20, 0);
         match r { Ok(v) => v, Err(e) => e }",
        -1,
    );

    run_str(
        r#"fn validate(x: i64) -> Result<i64, String> {
             if (x > 0) { Result::Ok(x) } else { Result::Err("negative") }
         }
         let r = validate(-5);
         match r { Ok(v) => "ok", Err(e) => e }"#,
        "negative",
    );

    run_i64(
        "fn map_result(r: Result<i64, i64>, f: fn(i64) -> i64) -> Result<i64, i64> {
             match r { Ok(v) => Result::Ok(f(v)), Err(e) => Result::Err(e) }
         }
         let r = map_result(Result::Ok(5), fn(x) { x * 2 });
         match r { Ok(v) => v, Err(e) => e }",
        10,
    );
}

#[test]
fn method_calls() {
    run_i64(
        "struct Point { x: i64, y: i64 }
         impl Point {
             fn sum(self) -> i64 { self.x + self.y }
         }
         let p = Point { x: 3, y: 4 };
         p.sum()",
        7,
    );

    run_i64(
        "struct Counter { val: i64 }
         impl Counter {
             fn new(v: i64) -> Counter { Counter { val: v } }
             fn get(self) -> i64 { self.val }
         }
         let c = Counter::new(99);
         c.get()",
        99,
    );

    run_i64(
        "struct Calc { base: i64 }
         impl Calc {
             fn add(self, n: i64) -> i64 { self.base + n }
         }
         let c = Calc { base: 10 };
         c.add(5)",
        15,
    );

    run_i64(
        "trait Describable { fn describe(self) -> i64; }
         struct Box { size: i64 }
         impl Describable for Box {
             fn describe(self) -> i64 { self.size }
         }
         let b = Box { size: 42 };
         b.describe()",
        42,
    );
}

#[test]
fn option_chained_matching() {
    run_i64(
        "fn add_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
             match a {
                 Some(va) => match b {
                     Some(vb) => Option::Some(va + vb),
                     None => Option::None
                 },
                 None => Option::None
             }
         }
         let r = add_opt(Option::Some(3), Option::Some(4));
         match r { Some(v) => v, None => 0 }",
        7,
    );

    run_i64(
        "fn add_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
             match a {
                 Some(va) => match b {
                     Some(vb) => Option::Some(va + vb),
                     None => Option::None
                 },
                 None => Option::None
             }
         }
         let r = add_opt(Option::Some(3), Option::None);
         match r { Some(v) => v, None => 0 }",
        0,
    );
}

#[test]
fn roundtrip_struct() {
    let bytecode = maat_tests::roundtrip(
        "struct Point { x: i64, y: i64 }
         let p = Point { x: 3, y: 4 };
         p.x + p.y",
    );
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error after roundtrip");
    match vm.last_popped_stack_elem() {
        Some(Object::I64(7)) => {}
        other => panic!("expected I64(7), got {other:?}"),
    }
}

#[test]
fn roundtrip_option() {
    let bytecode = maat_tests::roundtrip(
        "let x = Option::Some(42);
         match x { Some(v) => v, None => 0 }",
    );
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error after roundtrip");
    match vm.last_popped_stack_elem() {
        Some(Object::I64(42)) => {}
        other => panic!("expected I64(42), got {other:?}"),
    }
}

#[test]
fn roundtrip_result() {
    let bytecode = maat_tests::roundtrip(
        "let r = Result::Ok(99);
         match r { Ok(v) => v, Err(e) => e }",
    );
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error after roundtrip");
    match vm.last_popped_stack_elem() {
        Some(Object::I64(99)) => {}
        other => panic!("expected I64(99), got {other:?}"),
    }
}
