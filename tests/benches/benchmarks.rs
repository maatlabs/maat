use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use maat_ast::{Node, fold_constants};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_lexer::{MaatLexer, TokenKind};
use maat_parser::Parser;
use maat_tests::benchmark_programs::{
    ARITHMETIC_BASELINE, BITWISE_SOURCE, CLOSURE_SOURCE, EMPTY_PROGRAM, ENUM_MATCH_SOURCE,
    METHOD_DISPATCH_SOURCE, OPTION_SOURCE, RANGE_LOOP_10, RANGE_LOOP_100, RANGE_LOOP_1000,
    STRING_SOURCE, STRUCT_SOURCE, VECTOR_SOURCE, WHILE_LOOP_1000, fib_source,
};
use maat_tests::compile;
use maat_types::TypeChecker;
use maat_vm::VM;

/// A larger program for pipeline breakdown benchmarks.
fn large_program() -> String {
    let mut src = String::with_capacity(4096);
    src.push_str("let mut total: i64 = 0;\n");
    for i in 0..50 {
        src.push_str(&format!(
            "fn f{i}(x: i64) -> i64 {{ x + {i} }}\ntotal = total + f{i}({i});\n"
        ));
    }
    src.push_str("total\n");
    src
}

fn run_vm(source: &str) {
    let bytecode = compile(black_box(source));
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    black_box(vm.last_popped_stack_elem());
}

fn lex_all(source: &str) {
    let mut lexer = MaatLexer::new(black_box(source));
    loop {
        let tok = lexer.next_token();
        if tok.kind == TokenKind::Eof {
            break;
        }
    }
}

fn parse_source(source: &str) {
    let lexer = MaatLexer::new(black_box(source));
    let mut parser = Parser::new(lexer);
    let program = parser.parse();
    black_box(program);
}

fn typecheck_source(source: &str) {
    let lexer = MaatLexer::new(source);
    let mut parser = Parser::new(lexer);
    let mut program = parser.parse();
    let errors = TypeChecker::new().check_program(&mut program);
    black_box(errors);
}

fn check_and_compile(source: &str) {
    let lexer = MaatLexer::new(source);
    let mut parser = Parser::new(lexer);
    let mut program = parser.parse();
    let _ = TypeChecker::new().check_program(&mut program);
    let _ = fold_constants(&mut program);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    let _ = black_box(compiler.bytecode());
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

fn bench_vector_iteration(c: &mut Criterion) {
    c.bench_function("vector_iteration/vm", |b| {
        b.iter(|| run_vm(VECTOR_SOURCE));
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

fn bench_method_dispatch(c: &mut Criterion) {
    c.bench_function("method_dispatch/vm", |b| {
        b.iter(|| run_vm(METHOD_DISPATCH_SOURCE));
    });
}

fn bench_range_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_iteration");
    for (n, src) in [
        (10, RANGE_LOOP_10),
        (100, RANGE_LOOP_100),
        (1000, RANGE_LOOP_1000),
    ] {
        group.bench_with_input(BenchmarkId::new("for_in", n), src, |b, s| {
            b.iter(|| run_vm(s));
        });
    }
    group.bench_function("while_1000", |b| {
        b.iter(|| run_vm(WHILE_LOOP_1000));
    });
    group.finish();
}

fn bench_bitwise(c: &mut Criterion) {
    c.bench_function("bitwise/vm", |b| {
        b.iter(|| run_vm(BITWISE_SOURCE));
    });
}

fn bench_pipeline_stages(c: &mut Criterion) {
    let source = large_program();
    let mut group = c.benchmark_group("pipeline");

    group.bench_function("lexer", |b| {
        b.iter(|| lex_all(&source));
    });
    group.bench_function("parser", |b| {
        b.iter(|| parse_source(&source));
    });
    group.bench_function("typechecker", |b| {
        b.iter(|| typecheck_source(&source));
    });
    group.bench_function("codegen", |b| {
        b.iter(|| check_and_compile(&source));
    });
    group.bench_function("full_compile", |b| {
        b.iter(|| {
            let bc = compile(black_box(&source));
            black_box(bc);
        });
    });

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let small_bc = compile(ARITHMETIC_BASELINE);
    let large_bc = compile(&large_program());

    group.bench_function("serialize/small", |b| {
        b.iter(|| {
            let bytes = black_box(&small_bc).serialize().expect("serialize");
            black_box(bytes);
        });
    });
    group.bench_function("serialize/large", |b| {
        b.iter(|| {
            let bytes = black_box(&large_bc).serialize().expect("serialize");
            black_box(bytes);
        });
    });

    let small_bytes = small_bc.serialize().expect("serialize");
    let large_bytes = large_bc.serialize().expect("serialize");

    group.bench_function("deserialize/small", |b| {
        b.iter(|| {
            let bc = Bytecode::deserialize(black_box(&small_bytes)).expect("deserialize");
            black_box(bc);
        });
    });
    group.bench_function("deserialize/large", |b| {
        b.iter(|| {
            let bc = Bytecode::deserialize(black_box(&large_bytes)).expect("deserialize");
            black_box(bc);
        });
    });

    group.bench_function("roundtrip/large", |b| {
        b.iter(|| {
            let bytes = black_box(&large_bc).serialize().expect("serialize");
            let bc = Bytecode::deserialize(&bytes).expect("deserialize");
            black_box(bc);
        });
    });

    group.finish();
}

fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    group.bench_function("empty_program", |b| {
        b.iter(|| run_vm(EMPTY_PROGRAM));
    });
    group.bench_function("arithmetic", |b| {
        b.iter(|| run_vm(ARITHMETIC_BASELINE));
    });

    group.finish();
}

criterion_group! {
    name = fibonacci_benches;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(10));
    targets = bench_fibonacci_vm, bench_compile_fibonacci, bench_vm_exec_only
}
criterion_group!(
    feature_benches,
    bench_closures,
    bench_vector_iteration,
    bench_string_operations,
    bench_struct_method,
    bench_enum_match,
    bench_option_match,
    bench_method_dispatch,
);
criterion_group!(
    pipeline_benches,
    bench_range_iteration,
    bench_bitwise,
    bench_pipeline_stages,
    bench_serialization,
    bench_baseline,
);
criterion_main!(fibonacci_benches, feature_benches, pipeline_benches);
