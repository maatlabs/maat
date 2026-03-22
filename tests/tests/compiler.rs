use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_runtime::{Integer, Value};

/// A constant expectation that can be either an integer or a compiled function's instructions.
enum Constant {
    Int(i64),
    Fn(Vec<Vec<u8>>),
}

type ConstantTestCase<'a> = (&'a str, Vec<Constant>, Vec<Vec<u8>>);

fn assert_constants(bytecode: &Bytecode, expected: &[Constant], input: &str) {
    assert_eq!(
        bytecode.constants.len(),
        expected.len(),
        "wrong number of constants for input: {input}"
    );
    for (i, expected_const) in expected.iter().enumerate() {
        match (expected_const, &bytecode.constants[i]) {
            (Constant::Int(expected_val), Value::Integer(Integer::I64(actual_val))) => {
                assert_eq!(
                    actual_val, expected_val,
                    "constant {i} wrong for input: {input}"
                );
            }
            (Constant::Fn(expected_insts), Value::CompiledFn(cf)) => {
                let expected_ins = concat_instructions(expected_insts);
                let actual_ins = Instructions::from_bytes(cf.instructions.to_vec());
                assert_eq!(
                    actual_ins.as_bytes(),
                    expected_ins.as_bytes(),
                    "wrong instructions for CompiledFn constant {i} in input: {input}\n  got: {actual_ins}\n  want: {expected_ins}",
                );
            }
            (Constant::Int(_), actual) => {
                panic!("expected integer constant at index {i}, got: {actual:?}");
            }
            (Constant::Fn(_), actual) => {
                panic!("expected CompiledFn constant at index {i}, got: {actual:?}");
            }
        }
    }
}

fn concat_instructions(instructions: &[Vec<u8>]) -> Instructions {
    let mut result = Instructions::new();
    for ins in instructions {
        result.extend(&Instructions::from(ins.clone()));
    }
    result
}

fn assert_integer_constants(bytecode: &Bytecode, expected: &[i64], input: &str) {
    assert_eq!(
        bytecode.constants.len(),
        expected.len(),
        "wrong number of constants for input: {input}"
    );
    for (i, expected_val) in expected.iter().enumerate() {
        match &bytecode.constants[i] {
            Value::Integer(Integer::I64(value)) => {
                assert_eq!(
                    *value, *expected_val,
                    "constant {i} wrong for input: {input}"
                )
            }
            _ => panic!(
                "expected integer constant at index {i}, got: {:?}",
                bytecode.constants[i]
            ),
        }
    }
}

fn assert_instructions(bytecode: &Bytecode, expected: &[Vec<u8>], input: &str) {
    let expected_ins = concat_instructions(expected);
    assert_eq!(
        bytecode.instructions.as_bytes(),
        expected_ins.as_bytes(),
        "wrong instructions for input: {input}\n  got: {}\n  want: {}",
        bytecode.instructions,
        expected_ins,
    );
}

#[test]
fn compile_integer_arithmetic() {
    let cases = vec![
        (
            "1 + 2",
            vec![1, 2],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Add, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1; 2",
            vec![1, 2],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Pop, &[]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1 - 2",
            vec![1, 2],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Sub, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1 * 2",
            vec![1, 2],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Mul, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "2 / 1",
            vec![2, 1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Div, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "-1",
            vec![1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Minus, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_boolean_expressions() {
    let tests = vec![
        (
            "true",
            vec![encode(Opcode::True, &[]), encode(Opcode::Pop, &[])],
        ),
        (
            "false",
            vec![encode(Opcode::False, &[]), encode(Opcode::Pop, &[])],
        ),
        (
            "1 > 2",
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::GreaterThan, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1 < 2",
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::LessThan, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1 == 2",
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Equal, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "1 != 2",
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::NotEqual, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "true == false",
            vec![
                encode(Opcode::True, &[]),
                encode(Opcode::False, &[]),
                encode(Opcode::Equal, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "true != false",
            vec![
                encode(Opcode::True, &[]),
                encode(Opcode::False, &[]),
                encode(Opcode::NotEqual, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "!true",
            vec![
                encode(Opcode::True, &[]),
                encode(Opcode::Bang, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_instructions) in tests {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
    }
}

#[test]
fn compile_conditionals() {
    let tests = vec![
        (
            "if (true) { 10 }; 3333;",
            vec![10, 3333],
            vec![
                encode(Opcode::True, &[]),
                encode(Opcode::CondJump, &[10]),
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Jump, &[11]),
                encode(Opcode::Null, &[]),
                encode(Opcode::Pop, &[]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "if (true) { 10 } else { 20 }; 3333;",
            vec![10, 20, 3333],
            vec![
                encode(Opcode::True, &[]),
                encode(Opcode::CondJump, &[10]),
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Jump, &[13]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Pop, &[]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in tests {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_global_let_statements() {
    let tests = vec![
        (
            "let one = 1; let two = 2;",
            vec![1, 2],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::SetGlobal, &[1]),
            ],
        ),
        (
            "let one = 1; one;",
            vec![1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "let one = 1; let two = one; two;",
            vec![1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::SetGlobal, &[1]),
                encode(Opcode::GetGlobal, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in tests {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_strings() {
    let cases = vec![
        (
            r#""zero-knowledge""#,
            vec!["zero-knowledge"],
            vec![encode(Opcode::Constant, &[0]), encode(Opcode::Pop, &[])],
        ),
        (
            r#""zero" + "knowledge""#,
            vec!["zero", "knowledge"],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Add, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_eq!(
            bytecode.constants.len(),
            expected_constants.len(),
            "wrong number of constants for input: {input}"
        );
        for (i, expected) in expected_constants.iter().enumerate() {
            match &bytecode.constants[i] {
                Value::Str(value) => {
                    assert_eq!(value, expected, "constant {i} wrong for input: {input}")
                }
                _ => panic!("expected string constant at index {i}"),
            }
        }
    }
}

#[test]
fn compile_arrays() {
    let cases = vec![
        (
            "[]",
            vec![],
            vec![encode(Opcode::Vector, &[0]), encode(Opcode::Pop, &[])],
        ),
        (
            "[1, 2, 3]",
            vec![1, 2, 3],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Vector, &[3]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "[1 + 2, 3 - 4, 5 * 6]",
            vec![1, 2, 3, 4, 5, 6],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Add, &[]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Sub, &[]),
                encode(Opcode::Constant, &[4]),
                encode(Opcode::Constant, &[5]),
                encode(Opcode::Mul, &[]),
                encode(Opcode::Vector, &[3]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_maps() {
    let cases = vec![
        (
            "{}",
            vec![],
            vec![encode(Opcode::Map, &[0]), encode(Opcode::Pop, &[])],
        ),
        (
            "{1: 2, 3: 4, 5: 6}",
            vec![1, 2, 3, 4, 5, 6],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Constant, &[4]),
                encode(Opcode::Constant, &[5]),
                encode(Opcode::Map, &[6]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "{1: 2 + 3, 4: 5 * 6}",
            vec![1, 2, 3, 4, 5, 6],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Add, &[]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Constant, &[4]),
                encode(Opcode::Constant, &[5]),
                encode(Opcode::Mul, &[]),
                encode(Opcode::Map, &[4]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_index_expressions() {
    let cases = vec![
        (
            "[1, 2, 3][1 + 1]",
            vec![1, 2, 3, 1, 1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Vector, &[3]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Constant, &[4]),
                encode(Opcode::Add, &[]),
                encode(Opcode::Index, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "{1: 2}[2 - 1]",
            vec![1, 2, 2, 1],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Map, &[2]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Sub, &[]),
                encode(Opcode::Index, &[]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_constants, expected_instructions) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_instructions, input);
        assert_integer_constants(&bytecode, &expected_constants, input);
    }
}

#[test]
fn compile_functions() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        (
            "fn() { return 5 + 10 }",
            vec![
                Constant::Int(5),
                Constant::Int(10),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[2, 0]), encode(Opcode::Pop, &[])],
        ),
        (
            "fn() { 5 + 10 }",
            vec![
                Constant::Int(5),
                Constant::Int(10),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[2, 0]), encode(Opcode::Pop, &[])],
        ),
        (
            "fn() { 1; 2 }",
            vec![
                Constant::Int(1),
                Constant::Int(2),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Pop, &[]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[2, 0]), encode(Opcode::Pop, &[])],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_functions_without_return_value() {
    let cases: Vec<ConstantTestCase<'_>> = vec![(
        "fn() { }",
        vec![Constant::Fn(vec![encode(Opcode::Return, &[])])],
        vec![encode(Opcode::Closure, &[0, 0]), encode(Opcode::Pop, &[])],
    )];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_function_calls() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        (
            "fn() { 24 }();",
            vec![
                Constant::Int(24),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![
                encode(Opcode::Closure, &[1, 0]),
                encode(Opcode::Call, &[0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "let noArg = fn() { 24 }; noArg();",
            vec![
                Constant::Int(24),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![
                encode(Opcode::Closure, &[1, 0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Call, &[0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "let oneArg = fn(a) { a }; oneArg(24);",
            vec![
                Constant::Fn(vec![
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Int(24),
            ],
            vec![
                encode(Opcode::Closure, &[0, 0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Call, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "let manyArgs = fn(a, b, c) { a; b; c }; manyArgs(24, 25, 26);",
            vec![
                Constant::Fn(vec![
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Pop, &[]),
                    encode(Opcode::GetLocal, &[1]),
                    encode(Opcode::Pop, &[]),
                    encode(Opcode::GetLocal, &[2]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Int(24),
                Constant::Int(25),
                Constant::Int(26),
            ],
            vec![
                encode(Opcode::Closure, &[0, 0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Constant, &[1]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Constant, &[3]),
                encode(Opcode::Call, &[3]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_let_statement_scopes() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        (
            "let num = 55; fn() { num }",
            vec![
                Constant::Int(55),
                Constant::Fn(vec![
                    encode(Opcode::GetGlobal, &[0]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::Closure, &[1, 0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            "fn() { let num = 55; num }",
            vec![
                Constant::Int(55),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[1, 0]), encode(Opcode::Pop, &[])],
        ),
        (
            "fn() { let a = 55; let b = 77; a + b }",
            vec![
                Constant::Int(55),
                Constant::Int(77),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::SetLocal, &[1]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::GetLocal, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[2, 0]), encode(Opcode::Pop, &[])],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_builtins() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        // Method calls: Vector::len (builtin index 1)
        (
            "[].len();",
            vec![],
            vec![
                encode(Opcode::GetBuiltin, &[1]),
                encode(Opcode::Vector, &[0]),
                encode(Opcode::Call, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        // Method calls: Vector::push (builtin index 5)
        (
            "[].push(1);",
            vec![Constant::Int(1)],
            vec![
                encode(Opcode::GetBuiltin, &[5]),
                encode(Opcode::Vector, &[0]),
                encode(Opcode::Constant, &[0]),
                encode(Opcode::Call, &[2]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_closures() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        (
            "fn(a) { fn(b) { a + b } }",
            vec![
                Constant::Fn(vec![
                    encode(Opcode::GetFree, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Fn(vec![
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Closure, &[0, 1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[1, 0]), encode(Opcode::Pop, &[])],
        ),
        (
            "fn(a) { fn(b) { fn(c) { a + b + c } } }",
            vec![
                Constant::Fn(vec![
                    encode(Opcode::GetFree, &[0]),
                    encode(Opcode::GetFree, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Fn(vec![
                    encode(Opcode::GetFree, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Closure, &[0, 2]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Fn(vec![
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Closure, &[1, 1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![encode(Opcode::Closure, &[2, 0]), encode(Opcode::Pop, &[])],
        ),
        (
            r#"
            let global = 55;
            fn() {
                let a = 66;
                fn() {
                    let b = 77;
                    fn() {
                        let c = 88;
                        global + a + b + c
                    }
                }
            }
            "#,
            vec![
                Constant::Int(55),
                Constant::Int(66),
                Constant::Int(77),
                Constant::Int(88),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[3]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::GetGlobal, &[0]),
                    encode(Opcode::GetFree, &[0]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::GetFree, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[2]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::GetFree, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Closure, &[4, 2]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Fn(vec![
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Closure, &[5, 1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![
                encode(Opcode::Constant, &[0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::Closure, &[6, 0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}

#[test]
fn compile_recursive_functions() {
    let cases: Vec<ConstantTestCase<'_>> = vec![
        (
            "let countDown = fn(x) { countDown(x - 1); }; countDown(1);",
            vec![
                Constant::Int(1),
                Constant::Fn(vec![
                    encode(Opcode::CurrentClosure, &[]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Sub, &[]),
                    encode(Opcode::Call, &[1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Int(1),
            ],
            vec![
                encode(Opcode::Closure, &[1, 0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Constant, &[2]),
                encode(Opcode::Call, &[1]),
                encode(Opcode::Pop, &[]),
            ],
        ),
        (
            r#"
            let wrapper = fn() {
                let countDown = fn(x) { countDown(x - 1); };
                countDown(1);
            };
            wrapper();
            "#,
            vec![
                Constant::Int(1),
                Constant::Fn(vec![
                    encode(Opcode::CurrentClosure, &[]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Sub, &[]),
                    encode(Opcode::Call, &[1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
                Constant::Int(1),
                Constant::Fn(vec![
                    encode(Opcode::Closure, &[1, 0]),
                    encode(Opcode::SetLocal, &[0]),
                    encode(Opcode::GetLocal, &[0]),
                    encode(Opcode::Constant, &[2]),
                    encode(Opcode::Call, &[1]),
                    encode(Opcode::ReturnValue, &[]),
                ]),
            ],
            vec![
                encode(Opcode::Closure, &[3, 0]),
                encode(Opcode::SetGlobal, &[0]),
                encode(Opcode::GetGlobal, &[0]),
                encode(Opcode::Call, &[0]),
                encode(Opcode::Pop, &[]),
            ],
        ),
    ];
    for (input, expected_consts, expected_insts) in cases {
        let bytecode = maat_tests::compile_raw(input);
        assert_instructions(&bytecode, &expected_insts, input);
        assert_constants(&bytecode, &expected_consts, input);
    }
}
