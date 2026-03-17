#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_lexer::Lexer;
use maat_parser::Parser;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    let _program = parser.parse();
});
