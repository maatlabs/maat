use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use maat_tests::compile;
use maat_vm::VM;

const FIB_DEF: &str = "
let fibonacci = fn(x) {
    if (x == 0) {
        0
    } else {
        if (x == 1) {
            return 1;
        } else {
            fibonacci(x - 1) + fibonacci(x - 2);
        }
    }
};
";

fn fib_source(n: u32) -> String {
    format!("{FIB_DEF} fibonacci({n});")
}

const CLOSURE_SOURCE: &str = "
let makeAdder = fn(x) { fn(y) { x + y } };
let add5 = makeAdder(5);
let add10 = makeAdder(10);
add5(add10(1));
";

const ARRAY_SOURCE: &str = "
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
";

const STRING_SOURCE: &str = r#"
let greet = fn(name) {
    "Hello, " + name + "!"
};
greet("World");
"#;

const STRUCT_SOURCE: &str = "
struct Point { x: i64, y: i64 }
impl Point {
    fn sum(self) -> i64 { self.x + self.y }
}
let p = Point { x: 3, y: 4 };
p.sum()
";

const ENUM_MATCH_SOURCE: &str = "
enum Shape { Circle(i64), Rect(i64, i64) }
let s = Shape::Rect(3, 4);
match s { Circle(r) => r, Rect(w, h) => w * h }
";

const OPTION_SOURCE: &str = "
fn safe_div(a: i64, b: i64) -> Option<i64> {
    if (b == 0) { None } else { Some(a / b) }
}
let r = safe_div(10, 2);
match r { Some(v) => v, None => -1 }
";

fn run_vm(source: &str) {
    let bytecode = compile(black_box(source));
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    black_box(vm.last_popped_stack_elem());
}

fn bench_fibonacci_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci/vm");
    for n in [10u32, 15, 20] {
        let source = fib_source(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &source, |b, src| {
            b.iter(|| run_vm(src));
        });
    }
    group.finish();
}

fn bench_compile_fibonacci(c: &mut Criterion) {
    let source = fib_source(20);
    c.bench_function("compile/fibonacci_20", |b| {
        b.iter(|| {
            let bc = compile(black_box(&source));
            black_box(bc);
        });
    });
}

fn bench_vm_exec_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_exec_only");

    for n in [15u32, 20] {
        let source = fib_source(n);
        let bytecode = compile(&source);
        group.bench_with_input(
            BenchmarkId::new("fibonacci", n),
            &bytecode,
            |b, precompiled| {
                b.iter(|| {
                    let mut vm = VM::new(black_box(precompiled.clone()));
                    vm.run().expect("vm error");
                    black_box(vm.last_popped_stack_elem());
                });
            },
        );
    }

    group.finish();
}

fn bench_closures(c: &mut Criterion) {
    c.bench_function("closures/vm", |b| {
        b.iter(|| run_vm(CLOSURE_SOURCE));
    });
}

fn bench_array_iteration(c: &mut Criterion) {
    c.bench_function("array_iteration/vm", |b| {
        b.iter(|| run_vm(ARRAY_SOURCE));
    });
}

fn bench_string_operations(c: &mut Criterion) {
    c.bench_function("string_operations/vm", |b| {
        b.iter(|| run_vm(STRING_SOURCE));
    });
}

fn bench_struct_method(c: &mut Criterion) {
    c.bench_function("struct_method/vm", |b| {
        b.iter(|| run_vm(STRUCT_SOURCE));
    });
}

fn bench_enum_match(c: &mut Criterion) {
    c.bench_function("enum_match/vm", |b| {
        b.iter(|| run_vm(ENUM_MATCH_SOURCE));
    });
}

fn bench_option_match(c: &mut Criterion) {
    c.bench_function("option_match/vm", |b| {
        b.iter(|| run_vm(OPTION_SOURCE));
    });
}

criterion_group! {
    name = fibonacci_benches;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(10));
    targets = bench_fibonacci_vm, bench_compile_fibonacci, bench_vm_exec_only
}
criterion_group!(
    feature_benches,
    bench_closures,
    bench_array_iteration,
    bench_string_operations,
    bench_struct_method,
    bench_enum_match,
    bench_option_match,
);
criterion_main!(fibonacci_benches, feature_benches);
