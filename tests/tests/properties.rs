use maat_ast::{MaatAst, Program};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_lexer::{MaatLexer, TokenKind};
use maat_parser::MaatParser;
use maat_types::TypeChecker;
use maat_vm::VM;
use proptest::prelude::*;

fn arb_identifier() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z][a-z0-9_]{0,7}")
        .unwrap()
        .prop_filter("must not be keyword", |s| {
            !matches!(
                s.as_str(),
                "let"
                    | "fn"
                    | "if"
                    | "else"
                    | "return"
                    | "true"
                    | "false"
                    | "while"
                    | "for"
                    | "in"
                    | "loop"
                    | "break"
                    | "continue"
                    | "struct"
                    | "enum"
                    | "trait"
                    | "impl"
                    | "match"
                    | "use"
                    | "mod"
                    | "pub"
                    | "mut"
                    | "as"
                    | "self"
            )
        })
}

fn arb_integer_literal() -> impl Strategy<Value = String> {
    prop_oneof![
        (-1000i64..=1000i64).prop_map(|n| n.to_string()),
        (0u64..=255).prop_map(|n| format!("0x{n:x}")),
        (0u64..=255).prop_map(|n| format!("0b{n:b}")),
        (0u64..=255).prop_map(|n| format!("0o{n:o}")),
    ]
}

fn arb_simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_integer_literal(),
        Just("true".to_owned()),
        Just("false".to_owned()),
        any::<u8>().prop_map(|b| format!("\"{}\"", (b % 26 + b'a') as char)),
    ]
}

fn arb_let_stmt() -> impl Strategy<Value = String> {
    (arb_identifier(), arb_simple_expr()).prop_map(|(ident, val)| format!("let {ident} = {val};"))
}

fn arb_arithmetic_expr() -> impl Strategy<Value = String> {
    (arb_integer_literal(), arb_integer_literal(), 0..4usize).prop_map(|(a, b, op)| {
        let operator = ["+", "-", "*", "%"][op];
        format!("({a} {operator} {b})")
    })
}

fn arb_program() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            arb_let_stmt(),
            arb_arithmetic_expr().prop_map(|e| format!("{e};")),
        ],
        1..8,
    )
    .prop_map(|stmts| stmts.join("\n"))
}

fn arb_well_typed_program() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_let_stmt(), 1..6).prop_map(|stmts| stmts.join("\n"))
}

fn parse_source(source: &str) -> Option<Program> {
    let mut parser = MaatParser::new(MaatLexer::new(source));
    let program = parser.parse();
    if parser.errors().is_empty() {
        Some(program)
    } else {
        None
    }
}

fn type_check_and_compile(source: &str) -> Option<Bytecode> {
    let mut program = parse_source(source)?;
    let type_errors = TypeChecker::new().check_program(&mut program);
    if !type_errors.is_empty() {
        return None;
    }
    let mut compiler = Compiler::new();
    compiler.compile(&MaatAst::Program(program)).ok()?;
    compiler.bytecode().ok()
}

// Property: Lexer never panics on arbitrary UTF-8
proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn lexer_never_panics(source in "\\PC{0,256}") {
        let mut lexer = MaatLexer::new(&source);
        loop {
            let token = lexer.next_token();
            if token.kind == TokenKind::Eof {
                break;
            }
        }
    }
}

// Property: MaatParser never panics on arbitrary UTF-8
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn parser_never_panics(source in "\\PC{0,256}") {
        let lexer = MaatLexer::new(&source);
        let mut parser = MaatParser::new(lexer);
        let _program = parser.parse();
    }
}

// Property: Type checker never panics on syntactically valid programs
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn typechecker_never_panics(source in arb_program()) {
        let Some(mut program) = parse_source(&source) else { return Ok(()); };
        let _errors = TypeChecker::new().check_program(&mut program);
    }
}

// Property: Compiler never panics on well-typed programs
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn compiler_never_panics(source in arb_well_typed_program()) {
        let _ = type_check_and_compile(&source);
    }
}

// Property: Bytecode deserialization never panics on arbitrary bytes
proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn deserializer_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = Bytecode::deserialize(&data);
    }
}

// Property: AST round-trip (parse -> Display -> parse)
//
// If a program parses successfully, its Display output should also parse
// successfully. We compare the re-parsed Display output to itself (idempotent
// Display) rather than structural AST equality, because Display may normalize
// optional syntax (e.g., trailing commas, parenthesization).
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn ast_display_roundtrip(source in arb_program()) {
        let Some(program) = parse_source(&source) else { return Ok(()); };
        let displayed = program.to_string();
        let Some(reparsed) = parse_source(&displayed) else {
            return Err(TestCaseError::fail(
                format!("re-parse failed for Display output: {displayed}")
            ));
        };
        // Idempotency: displaying the reparsed AST should yield the same string.
        let redisplayed = reparsed.to_string();
        prop_assert_eq!(
            displayed, redisplayed,
            "Display is not idempotent"
        );
    }
}

// Property: Bytecode serialization round-trip
//
// Compiling and serializing bytecode, then deserializing it, should produce
// identical bytecode.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn bytecode_roundtrip(source in arb_well_typed_program()) {
        let Some(bytecode) = type_check_and_compile(&source) else { return Ok(()); };
        let serialized = bytecode.serialize().expect("serialization must not fail");
        let deserialized = Bytecode::deserialize(&serialized)
            .expect("deserialization must not fail");
        prop_assert_eq!(
            bytecode, deserialized,
            "bytecode round-trip mismatch"
        );
    }
}

// Property: Execution determinism
//
// Running the same program twice must produce the same result.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn execution_determinism(source in arb_well_typed_program()) {
        let Some(bytecode) = type_check_and_compile(&source) else { return Ok(()); };

        let mut vm1 = VM::new(bytecode.clone());
        let result1 = vm1.run();

        let mut vm2 = VM::new(bytecode);
        let result2 = vm2.run();

        match (&result1, &result2) {
            (Ok(()), Ok(())) => {
                let last1 = vm1.last_popped_stack_elem().cloned();
                let last2 = vm2.last_popped_stack_elem().cloned();
                prop_assert_eq!(last1, last2, "non-deterministic execution");
            }
            (Err(_), Err(_)) => {}
            _ => {
                return Err(TestCaseError::fail(
                    "one run succeeded, the other failed"
                ));
            }
        }
    }
}

// Property: Type soundness
//
// Well-typed programs must not produce runtime type errors in the VM.
// Arithmetic overflow and stack overflow are acceptable VM errors; only
// type-related failures indicate unsoundness.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn type_soundness(source in arb_well_typed_program()) {
        let Some(bytecode) = type_check_and_compile(&source) else { return Ok(()); };
        let mut vm = VM::new(bytecode);
        if let Err(e) = vm.run() {
            let msg = e.to_string();
            let is_type_error = msg.contains("type mismatch")
                || msg.contains("expected")
                || msg.contains("cannot apply");
            prop_assert!(
                !is_type_error,
                "well-typed program produced runtime type error: {msg}"
            );
        }
    }
}
