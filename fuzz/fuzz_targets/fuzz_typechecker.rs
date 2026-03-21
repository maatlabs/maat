#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_lexer::MaatLexer;
use maat_parser::Parser;
use maat_types::TypeChecker;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let lexer = MaatLexer::new(source);
    let mut parser = Parser::new(lexer);
    let mut program = parser.parse();
    if !parser.errors().is_empty() {
        return;
    }
    let _errors = TypeChecker::new().check_program(&mut program);
});
