use maat_runtime::{Integer, Value};
use maat_types::TypeChecker;
use maat_vm::VM;

fn run(input: &str) -> Value {
    let bytecode = maat_tests::compile(input);
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    vm.last_popped_stack_elem()
        .expect("no value on stack")
        .clone()
}

fn run_i64(input: &str, expected: i64) {
    match run(input) {
        Value::Integer(Integer::I64(v)) => assert_eq!(v, expected, "wrong value for:\n{input}"),
        other => panic!("expected I64({expected}), got {other:?} for:\n{input}"),
    }
}

fn run_bool(input: &str, expected: bool) {
    match run(input) {
        Value::Bool(v) => assert_eq!(v, expected, "wrong value for:\n{input}"),
        other => panic!("expected Bool({expected}), got {other:?} for:\n{input}"),
    }
}

fn run_str(input: &str, expected: &str) {
    match run(input) {
        Value::Str(v) => assert_eq!(v, expected, "wrong value for:\n{input}"),
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
                 self.items.first()
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
                 self.items.first()
             }
         }
         let s = Stack { items: [] };
         match s.peek() { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn enums() {
    let errs = maat_tests::parse_errors("enum Option<T> { Some(T), None }");
    assert!(
        errs.iter().any(|e| e.contains("reserved type name")),
        "expected reserved type name error for Option, got: {errs:?}"
    );
    let errs = maat_tests::parse_errors("enum Result<T, E> { Ok(T), Err(E) }");
    assert!(
        errs.iter().any(|e| e.contains("reserved type name")),
        "expected reserved type name error for Result, got: {errs:?}"
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
        "let arr = [Some(1), Some(3), Some(6)];
         let mut total = 0;
         let mut i = 0;
         while (i < arr.len() as i64) {
             let val = match arr[i] { Some(v) => v, _ => 0 };
             total = total + val;
             i = i + 1;
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
             match opt { Some(v) => Some(v + 1), None => None }
         }",
    );
    run_i64(
        "let x = Some(42);
         match x { Some(v) => v, None => 0 }",
        42,
    );
    run_i64(
        "let x: Option<i64> = None;
         match x { Some(v) => v, None => -1 }",
        -1,
    );
    run_i64(
        "fn safe_div(a: i64, b: i64) -> Option<i64> {
             if (b == 0) { None } else { Some(a / b) }
         }
         let result = safe_div(10, 2);
         match result { Some(v) => v, None => -1 }",
        5,
    );
    run_i64(
        "fn safe_div(a: i64, b: i64) -> Option<i64> {
             if (b == 0) { None } else { Some(a / b) }
         }
         let result = safe_div(10, 0);
         match result { Some(v) => v, None => -1 }",
        -1,
    );
    run_i64(
        "fn unwrap_or(opt: Option<i64>, default: i64) -> i64 {
             match opt { Some(v) => v, None => default }
         }
         unwrap_or(Some(7), 0)",
        7,
    );
    run_i64(
        "fn unwrap_or(opt: Option<i64>, default: i64) -> i64 {
             match opt { Some(v) => v, None => default }
         }
         unwrap_or(None, 42)",
        42,
    );
}

#[test]
fn result_builtin_type() {
    assert_no_type_errors(
        "fn handle(r: Result<i64, str>) -> i64 {
             match r { Ok(v) => v, Err(e) => 0 }
         }",
    );
    run_i64(
        "let r = Ok(100);
         match r { Ok(v) => v, Err(e) => e }",
        100,
    );
    run_i64(
        "let r: Result<i64, i64> = Err(-1);
         match r { Ok(v) => v, Err(e) => e }",
        -1,
    );
    run_i64(
        "fn checked_div(a: i64, b: i64) -> Result<i64, i64> {
             if (b == 0) { Err(-1) } else { Ok(a / b) }
         }
         let r = checked_div(20, 5);
         match r { Ok(v) => v, Err(e) => e }",
        4,
    );
    run_i64(
        "fn checked_div(a: i64, b: i64) -> Result<i64, i64> {
             if (b == 0) { Err(-1) } else { Ok(a / b) }
         }
         let r = checked_div(20, 0);
         match r { Ok(v) => v, Err(e) => e }",
        -1,
    );
    run_str(
        r#"fn validate(x: i64) -> Result<i64, str> {
             if (x > 0) { Ok(x) } else { Err("negative") }
         }
         let r = validate(-5);
         match r { Ok(v) => "ok", Err(e) => e }"#,
        "negative",
    );
    run_i64(
        "fn map_result(r: Result<i64, i64>, f: fn(i64) -> i64) -> Result<i64, i64> {
             match r { Ok(v) => Ok(f(v)), Err(e) => Err(e) }
         }
         let r = map_result(Ok(5), fn(x) { x * 2 });
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
                     Some(vb) => Some(va + vb),
                     None => None
                 },
                 None => None
             }
         }
         let r = add_opt(Some(3), Some(4));
         match r { Some(v) => v, None => 0 }",
        7,
    );
    run_i64(
        "fn add_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
             match a {
                 Some(va) => match b {
                     Some(vb) => Some(va + vb),
                     None => None
                 },
                 None => None
             }
         }
         let r = add_opt(Some(3), None);
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
        Some(Value::Integer(Integer::I64(7))) => {}
        other => panic!("expected I64(7), got {other:?}"),
    }
}

#[test]
fn roundtrip_option() {
    let bytecode = maat_tests::roundtrip(
        "let x = Some(42);
         match x { Some(v) => v, None => 0 }",
    );
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error after roundtrip");
    match vm.last_popped_stack_elem() {
        Some(Value::Integer(Integer::I64(42))) => {}
        other => panic!("expected I64(42), got {other:?}"),
    }
}

#[test]
fn roundtrip_result() {
    let bytecode = maat_tests::roundtrip(
        "let r = Ok(99);
         match r { Ok(v) => v, Err(e) => e }",
    );
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error after roundtrip");
    match vm.last_popped_stack_elem() {
        Some(Value::Integer(Integer::I64(99))) => {}
        other => panic!("expected I64(99), got {other:?}"),
    }
}

#[test]
fn option_unwrap_some() {
    run_i64("Some(42).unwrap()", 42);
}

#[test]
#[should_panic(expected = "vm error")]
fn option_unwrap_none() {
    run_i64("let x: Option<i64> = None; x.unwrap()", 0);
}

#[test]
fn option_unwrap_or() {
    run_i64("Some(10).unwrap_or(0)", 10);
    run_i64("let x: Option<i64> = None; x.unwrap_or(99)", 99);
}

#[test]
fn option_is_some() {
    run_bool("Some(1).is_some()", true);
    run_bool("let x: Option<i64> = None; x.is_some()", false);
}

#[test]
fn option_is_none() {
    run_bool("Some(1).is_none()", false);
    run_bool("let x: Option<i64> = None; x.is_none()", true);
}

#[test]
fn option_map_some() {
    run_i64(
        "let x = Some(5);
         match x.map(fn(v) { v * 2 }) { Some(v) => v, None => 0 }",
        10,
    );
}

#[test]
fn option_map_none() {
    run_i64(
        "let x: Option<i64> = None;
         match x.map(fn(v) { v * 2 }) { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn option_and_then_some() {
    run_i64(
        "let x = Some(5);
         match x.and_then(fn(v) { Some(v + 10) }) { Some(v) => v, None => 0 }",
        15,
    );
}

#[test]
fn option_and_then_none_input() {
    run_i64(
        "let x: Option<i64> = None;
         match x.and_then(fn(v) { Some(v + 10) }) { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn option_and_then_returns_none() {
    run_i64(
        "let x = Some(5);
         match x.and_then(fn(v: i64) -> Option<i64> { None }) { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn result_unwrap_ok() {
    run_i64("Ok(42).unwrap()", 42);
}

#[test]
#[should_panic(expected = "vm error")]
fn result_unwrap_err() {
    run_i64("Err(-1).unwrap()", 0);
}

#[test]
fn result_unwrap_or() {
    run_i64("Ok(10).unwrap_or(0)", 10);
    run_i64("Err(-1).unwrap_or(99)", 99);
}

#[test]
fn result_is_ok() {
    run_bool("Ok(1).is_ok()", true);
    run_bool("Err(-1).is_ok()", false);
}

#[test]
fn result_is_err() {
    run_bool("Ok(1).is_err()", false);
    run_bool("Err(-1).is_err()", true);
}

#[test]
fn result_map_ok() {
    run_i64(
        "let r = Ok(5);
         match r.map(fn(v) { v * 3 }) { Ok(v) => v, Err(e) => 0 }",
        15,
    );
}

#[test]
fn result_map_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(-1);
         match r.map(fn(v) { v * 3 }) { Ok(v) => v, Err(e) => e }",
        -1,
    );
}

#[test]
fn result_and_then_ok() {
    run_i64(
        "let r = Ok(5);
         match r.and_then(fn(v) { Ok(v + 100) }) { Ok(v) => v, Err(e) => 0 }",
        105,
    );
}

#[test]
fn result_and_then_err_input() {
    run_i64(
        "let r: Result<i64, i64> = Err(-1);
         match r.and_then(fn(v) { Ok(v + 100) }) { Ok(v) => v, Err(e) => e }",
        -1,
    );
}

#[test]
fn result_and_then_returns_err() {
    run_i64(
        "let r = Ok(5);
         match r.and_then(fn(v: i64) -> Result<i64, i64> { Err(-99) }) { Ok(v) => v, Err(e) => e }",
        -99,
    );
}

#[test]
fn option_unwrap_or_else_some() {
    run_i64("Some(42).unwrap_or_else(|| 0)", 42);
}

#[test]
fn option_unwrap_or_else_none() {
    run_i64("let x: Option<i64> = None; x.unwrap_or_else(|| 99)", 99);
}

#[test]
fn option_ok_some() {
    run_i64("match Some(10).ok() { Ok(v) => v, Err(_) => -1 }", 10);
}

#[test]
fn option_ok_none() {
    run_i64(
        "let x: Option<i64> = None; match x.ok() { Ok(v) => v, Err(_) => -1 }",
        -1,
    );
}

#[test]
fn option_flatten_some_some() {
    run_i64(
        "let x = Some(Some(42)); match x.flatten() { Some(v) => v, None => -1 }",
        42,
    );
}

#[test]
fn option_flatten_some_none() {
    run_i64(
        "let inner: Option<i64> = None; let x = Some(inner); match x.flatten() { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn option_flatten_none() {
    // Construct a None that the type checker infers as Option<Option<i64>>
    // by branching: one arm produces Some(Some(v)), the other produces None.
    run_i64(
        "fn test(flag: bool) -> i64 {
            let x = if flag { Some(Some(100)) } else { None };
            match x.flatten() { Some(v) => v, None => -1 }
         }
         test(true)",
        100,
    );
}

#[test]
fn option_zip_both_some() {
    run_i64(
        "match Some(1).zip(Some(2)) { Some(pair) => pair.0 + pair.1, None => -1 }",
        3,
    );
}

#[test]
fn option_zip_first_none() {
    run_i64(
        "let a: Option<i64> = None; match a.zip(Some(2)) { Some(pair) => pair.0 + pair.1, None => -1 }",
        -1,
    );
}

#[test]
fn option_zip_second_none() {
    run_i64(
        "let b: Option<i64> = None; match Some(1).zip(b) { Some(pair) => pair.0 + pair.1, None => -1 }",
        -1,
    );
}

#[test]
fn result_map_err_ok() {
    run_i64(
        "let r: Result<i64, i64> = Ok(10);
         match r.map_err(|e| e * 2) { Ok(v) => v, Err(e) => e }",
        10,
    );
}

#[test]
fn result_map_err_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(5);
         match r.map_err(|e| e * 2) { Ok(v) => v, Err(e) => e }",
        10,
    );
}

#[test]
fn result_unwrap_err_on_err() {
    run_i64("let r: Result<i64, i64> = Err(42); r.unwrap_err()", 42);
}

#[test]
#[should_panic(expected = "vm error")]
fn result_unwrap_err_on_ok() {
    run("let r: Result<i64, i64> = Ok(1); r.unwrap_err()");
}

#[test]
fn result_unwrap_or_else_ok() {
    run_i64(
        "let r: Result<i64, i64> = Ok(42); r.unwrap_or_else(|e| e * 10)",
        42,
    );
}

#[test]
fn result_unwrap_or_else_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(5); r.unwrap_or_else(|e| e * 10)",
        50,
    );
}

#[test]
fn result_ok_on_ok() {
    run_i64(
        "let r: Result<i64, i64> = Ok(42); match r.ok() { Some(v) => v, None => -1 }",
        42,
    );
}

#[test]
fn result_ok_on_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(5); match r.ok() { Some(v) => v, None => -1 }",
        -1,
    );
}

#[test]
fn result_err_on_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(42); match r.err() { Some(e) => e, None => -1 }",
        42,
    );
}

#[test]
fn result_err_on_ok() {
    run_i64(
        "let r: Result<i64, i64> = Ok(5); match r.err() { Some(e) => e, None => -1 }",
        -1,
    );
}

#[test]
fn result_or_else_ok() {
    run_i64(
        "let r: Result<i64, i64> = Ok(42);
         match r.or_else(|e| Err(e * 2)) { Ok(v) => v, Err(e) => e }",
        42,
    );
}

#[test]
fn result_or_else_err_to_ok() {
    run_i64(
        "let r: Result<i64, i64> = Err(5);
         match r.or_else(|e| Ok(e * 10)) { Ok(v) => v, Err(e) => e }",
        50,
    );
}

#[test]
fn result_or_else_err_to_err() {
    run_i64(
        "let r: Result<i64, i64> = Err(3);
         match r.or_else(|e| Err(e + 100)) { Ok(v) => v, Err(e) => e }",
        103,
    );
}
