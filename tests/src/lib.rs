//! Shared utilities for integration tests.

use maat_ast::{Node, Program, fold_constants};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_lexer::MaatLexer;
use maat_parser::Parser;
use maat_types::TypeChecker;

/// Parses the given source string into an AST [`Program`].
///
/// # Panics
///
/// Panics if the parser encounters any errors.
pub fn parse(input: &str) -> Program {
    let lexer = MaatLexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "parser errors: {:?}",
        parser.errors()
    );
    program
}

/// Compiles the given source string into [`Bytecode`].
///
/// Runs the full pipeline: parse -> type check -> constant fold -> compile.
///
/// # Panics
///
/// Panics if parsing, type checking, or compilation fails.
pub fn compile(input: &str) -> Bytecode {
    let mut program = parse(input);

    let type_errors = TypeChecker::new().check_program(&mut program);
    assert!(type_errors.is_empty(), "type errors: {:?}", type_errors);

    let fold_errors = fold_constants(&mut program);
    assert!(
        fold_errors.is_empty(),
        "constant folding errors: {:?}",
        fold_errors
    );
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    compiler.bytecode().expect("bytecode extraction failed")
}

/// Compiles the given source string into [`Bytecode`] without type checking
/// or constant folding.
///
/// This is used by compiler tests that assert on exact bytecode layout,
/// where constant folding would alter the expected instruction sequences.
///
/// # Panics
///
/// Panics if parsing or compilation fails.
pub fn compile_raw(input: &str) -> Bytecode {
    let program = parse(input);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    compiler.bytecode().expect("bytecode extraction failed")
}

/// Compiles the given source string, expecting type errors.
///
/// Returns the type error messages for assertion.
///
/// # Panics
///
/// Panics if parsing fails.
pub fn compile_type_errors(input: &str) -> Vec<String> {
    let mut program = parse(input);
    let type_errors = TypeChecker::new().check_program(&mut program);
    type_errors.iter().map(|e| e.kind.to_string()).collect()
}

/// Compiles the given source string, serializes the bytecode, deserializes
/// it, and returns the restored [`Bytecode`].
///
/// This exercises the full round-trip through the binary format, ensuring
/// that execution from deserialized bytecode produces the same results as
/// direct compilation.
///
/// # Panics
///
/// Panics if parsing, compilation, serialization, or deserialization fails.
pub fn roundtrip(input: &str) -> Bytecode {
    let bytecode = compile(input);
    let bytes = bytecode.serialize().expect("serialization failed");
    Bytecode::deserialize(&bytes).expect("deserialization failed")
}

/// Example programs for running the benchmark suite.
pub mod benchmark_programs {
    pub fn fib_source(n: u32) -> String {
        format!("{FIB_DEF} fibonacci({n});")
    }

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

    pub const CLOSURE_SOURCE: &str = "
let makeAdder = fn(x) { fn(y) { x + y } };
let add5 = makeAdder(5);
let add10 = makeAdder(10);
add5(add10(1));
";

    pub const VECTOR_SOURCE: &str = "
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

    pub const STRING_SOURCE: &str = r#"
let greet = fn(name) {
    "Hello, " + name + "!"
};
greet("World");
"#;

    pub const STRUCT_SOURCE: &str = "
struct Point { x: i64, y: i64 }
impl Point {
    fn sum(self) -> i64 { self.x + self.y }
}
let p = Point { x: 3, y: 4 };
p.sum()
";

    pub const ENUM_MATCH_SOURCE: &str = "
enum Shape { Circle(i64), Rect(i64, i64) }
let s = Shape::Rect(3, 4);
match s { Circle(r) => r, Rect(w, h) => w * h }
";

    pub const OPTION_SOURCE: &str = "
fn safe_div(a: i64, b: i64) -> Option<i64> {
    if (b == 0) { None } else { Some(a / b) }
}
let r = safe_div(10, 2);
match r { Some(v) => v, None => -1 }
";

    pub const METHOD_DISPATCH_SOURCE: &str = r#"
let arr = [1, 2, 3, 4, 5];
let a = arr.len();
let b = arr.first();
let c = arr.last();
let s = "  hello  ";
let t = s.trim();
let u = s.len();
a + u
"#;

    pub const RANGE_LOOP_10: &str = "
let mut acc: i64 = 0;
for i in 0..10 { acc = acc + i; }
acc
";

    pub const RANGE_LOOP_100: &str = "
let mut acc: i64 = 0;
for i in 0..100 { acc = acc + i; }
acc
";

    pub const RANGE_LOOP_1000: &str = "
let mut acc: i64 = 0;
for i in 0..1000 { acc = acc + i; }
acc
";

    pub const WHILE_LOOP_1000: &str = "
let mut acc: i64 = 0;
let mut i: i64 = 0;
while (i < 1000) { acc = acc + i; i = i + 1; }
acc
";

    pub const BITWISE_SOURCE: &str = "
let mut x: i64 = 0;
for i in 0..100 {
    x = (x ^ i) & 0xFF;
    x = x | (i << 1);
    x = x >> 1;
}
x
";

    pub const EMPTY_PROGRAM: &str = "let x: i64 = 0;";

    pub const ARITHMETIC_BASELINE: &str = "
let a: i64 = 1 + 2;
let b: i64 = a * 3;
let c: i64 = b - a;
let d: i64 = c / 2;
d
";
}
