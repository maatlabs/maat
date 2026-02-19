use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use maat_tests::{compile, run_eval};
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
        if (idx == len(arr)) {
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

fn run_vm(source: &str) {
    let bytecode = compile(black_box(source));
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    black_box(vm.last_popped_stack_elem());
}

fn run_evaluator(source: &str) {
    let result = run_eval(black_box(source)).expect("eval error");
    black_box(result);
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

fn bench_fibonacci_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci/eval");
    for n in [10u32, 15, 20] {
        let source = fib_source(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &source, |b, src| {
            b.iter(|| run_evaluator(src));
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
    c.bench_function("closures/eval", |b| {
        b.iter(|| run_evaluator(CLOSURE_SOURCE));
    });
}

fn bench_array_iteration(c: &mut Criterion) {
    c.bench_function("array_iteration/vm", |b| {
        b.iter(|| run_vm(ARRAY_SOURCE));
    });
    c.bench_function("array_iteration/eval", |b| {
        b.iter(|| run_evaluator(ARRAY_SOURCE));
    });
}

fn bench_string_operations(c: &mut Criterion) {
    c.bench_function("string_operations/vm", |b| {
        b.iter(|| run_vm(STRING_SOURCE));
    });
    c.bench_function("string_operations/eval", |b| {
        b.iter(|| run_evaluator(STRING_SOURCE));
    });
}

criterion_group!(
    fibonacci_benches,
    bench_fibonacci_vm,
    bench_fibonacci_eval,
    bench_compile_fibonacci,
    bench_vm_exec_only,
);
criterion_group!(
    feature_benches,
    bench_closures,
    bench_array_iteration,
    bench_string_operations,
);
criterion_main!(fibonacci_benches, feature_benches);
