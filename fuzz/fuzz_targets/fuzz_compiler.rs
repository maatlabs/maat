#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_ast::MaatAst;
use maat_codegen::Compiler;
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;
use maat_types::TypeChecker;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let lexer = MaatLexer::new(source);
    let mut parser = MaatParser::new(lexer);
    let mut program = parser.parse();
    if !parser.errors().is_empty() {
        return;
    }
    let type_errors = TypeChecker::new().check_program(&mut program);
    if !type_errors.is_empty() {
        return;
    }
    let mut compiler = Compiler::new();
    let _ = compiler.compile(&MaatAst::Program(program));
});
