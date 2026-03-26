#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_lexer::{MaatLexer, TokenKind};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let mut lexer = MaatLexer::new(source);
    loop {
        let token = lexer.next_token();
        if token.kind == TokenKind::Eof {
            break;
        }
    }
});
