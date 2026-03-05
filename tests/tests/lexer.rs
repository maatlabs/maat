use maat_lexer::{Lexer, TokenKind};

#[test]
fn next_token() {
    let source = r#"let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
	return true;
} else {
	return false;
}

10 == 10;
10 != 9;
"#;

    let expected = [
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "five"),
        (TokenKind::Assign, "="),
        (TokenKind::I64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "ten"),
        (TokenKind::Assign, "="),
        (TokenKind::I64, "10"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "add"),
        (TokenKind::Assign, "="),
        (TokenKind::Fn, "fn"),
        (TokenKind::LParen, "("),
        (TokenKind::Ident, "x"),
        (TokenKind::Comma, ","),
        (TokenKind::Ident, "y"),
        (TokenKind::RParen, ")"),
        (TokenKind::LBrace, "{"),
        (TokenKind::Ident, "x"),
        (TokenKind::Plus, "+"),
        (TokenKind::Ident, "y"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::RBrace, "}"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "result"),
        (TokenKind::Assign, "="),
        (TokenKind::Ident, "add"),
        (TokenKind::LParen, "("),
        (TokenKind::Ident, "five"),
        (TokenKind::Comma, ","),
        (TokenKind::Ident, "ten"),
        (TokenKind::RParen, ")"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Bang, "!"),
        (TokenKind::Minus, "-"),
        (TokenKind::Slash, "/"),
        (TokenKind::Asterisk, "*"),
        (TokenKind::I64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::I64, "5"),
        (TokenKind::Less, "<"),
        (TokenKind::I64, "10"),
        (TokenKind::Greater, ">"),
        (TokenKind::I64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::If, "if"),
        (TokenKind::LParen, "("),
        (TokenKind::I64, "5"),
        (TokenKind::Less, "<"),
        (TokenKind::I64, "10"),
        (TokenKind::RParen, ")"),
        (TokenKind::LBrace, "{"),
        (TokenKind::Return, "return"),
        (TokenKind::True, "true"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::RBrace, "}"),
        (TokenKind::Else, "else"),
        (TokenKind::LBrace, "{"),
        (TokenKind::Return, "return"),
        (TokenKind::False, "false"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::RBrace, "}"),
        (TokenKind::I64, "10"),
        (TokenKind::Equal, "=="),
        (TokenKind::I64, "10"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::I64, "10"),
        (TokenKind::NotEqual, "!="),
        (TokenKind::I64, "9"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);

    for (i, (kind, literal)) in expected.iter().enumerate() {
        let token = lexer.next_token();

        assert_eq!(
            token.kind, *kind,
            "tests[{}]: token kind wrong. expected={:?}, got={:?}",
            i, kind, token.kind
        );
        assert_eq!(
            token.literal, *literal,
            "tests[{}]: literal wrong. expected={:?}, got={:?}",
            i, literal, token.literal
        );
    }
}

#[test]
fn operator_and_delimiter_tokens() {
    // Single-character tokens
    let source = "=+(){},;";
    let expected = [
        (TokenKind::Assign, "="),
        (TokenKind::Plus, "+"),
        (TokenKind::LParen, "("),
        (TokenKind::RParen, ")"),
        (TokenKind::LBrace, "{"),
        (TokenKind::RBrace, "}"),
        (TokenKind::Comma, ","),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Two-character tokens
    let source = "== != <= >=";
    let expected = [
        (TokenKind::Equal, "=="),
        (TokenKind::NotEqual, "!="),
        (TokenKind::LessEqual, "<="),
        (TokenKind::GreaterEqual, ">="),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn keywords() {
    let source = "let fn if else return true false struct enum match impl trait self Self";
    let expected = [
        (TokenKind::Let, "let"),
        (TokenKind::Fn, "fn"),
        (TokenKind::If, "if"),
        (TokenKind::Else, "else"),
        (TokenKind::Return, "return"),
        (TokenKind::True, "true"),
        (TokenKind::False, "false"),
        (TokenKind::Struct, "struct"),
        (TokenKind::Enum, "enum"),
        (TokenKind::Match, "match"),
        (TokenKind::Impl, "impl"),
        (TokenKind::Trait, "trait"),
        (TokenKind::SelfValue, "self"),
        (TokenKind::SelfType, "Self"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn identifiers() {
    let source = "foo bar _baz qux123";
    let expected = [
        (TokenKind::Ident, "foo"),
        (TokenKind::Ident, "bar"),
        (TokenKind::Ident, "_baz"),
        (TokenKind::Ident, "qux123"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn int64() {
    let source = "0 1 42 1234567890";
    let expected = [
        (TokenKind::I64, "0"),
        (TokenKind::I64, "1"),
        (TokenKind::I64, "42"),
        (TokenKind::I64, "1234567890"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn operators() {
    let source = "+ - * / < > ! = == !=";
    let expected = [
        (TokenKind::Plus, "+"),
        (TokenKind::Minus, "-"),
        (TokenKind::Asterisk, "*"),
        (TokenKind::Slash, "/"),
        (TokenKind::Less, "<"),
        (TokenKind::Greater, ">"),
        (TokenKind::Bang, "!"),
        (TokenKind::Assign, "="),
        (TokenKind::Equal, "=="),
        (TokenKind::NotEqual, "!="),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn whitespace() {
    let source = "  let   \t\n  x  \r\n =   \t 5  ";
    let expected = [
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "x"),
        (TokenKind::Assign, "="),
        (TokenKind::I64, "5"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn empty_input() {
    let source = "";
    let mut lexer = Lexer::new(source);
    let token = lexer.next_token();
    assert_eq!(token.kind, TokenKind::Eof);
    assert_eq!(token.literal, "");
}

#[test]
fn invalid_characters() {
    let source = "@ # $";
    let expected = [
        (TokenKind::Invalid, "@"),
        (TokenKind::Invalid, "#"),
        (TokenKind::Invalid, "$"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn string_tokens() {
    // Basic string literals
    let source = r#""hello world" "foo bar" "" "with\nnewlines""#;
    let expected = [
        (TokenKind::String, "hello world"),
        (TokenKind::String, "foo bar"),
        (TokenKind::String, ""),
        (TokenKind::String, "with\\nnewlines"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Escape sequences
    let source = r#""hello \"world\"" "line1\nline2" "tab\there" "backslash\\" "quote\"" "null\0char" "mixed\t\n\r\\\"""#;
    let expected = [
        (TokenKind::String, r#"hello \"world\""#),
        (TokenKind::String, r"line1\nline2"),
        (TokenKind::String, r"tab\there"),
        (TokenKind::String, r"backslash\\"),
        (TokenKind::String, r#"quote\""#),
        (TokenKind::String, r"null\0char"),
        (TokenKind::String, r#"mixed\t\n\r\\\""#),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn float_literals_are_invalid() {
    let source = "3.15 0.5 123.456 1e10 1.5e10";
    let expected = [
        (TokenKind::Invalid, "3.15"),
        (TokenKind::Invalid, "0.5"),
        (TokenKind::Invalid, "123.456"),
        (TokenKind::Invalid, "1e10"),
        (TokenKind::Invalid, "1.5e10"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn non_decimal_literals() {
    // Binary
    let source = "0b1010 0B1111 0b0";
    let expected = [
        (TokenKind::I64, "0b1010"),
        (TokenKind::I64, "0B1111"),
        (TokenKind::I64, "0b0"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Octal
    let source = "0o755 0O644 0o0";
    let expected = [
        (TokenKind::I64, "0o755"),
        (TokenKind::I64, "0O644"),
        (TokenKind::I64, "0o0"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Hex
    let source = "0xff 0xFF 0x0 0xDEADBEEF 0X1a2B";
    let expected = [
        (TokenKind::I64, "0xff"),
        (TokenKind::I64, "0xFF"),
        (TokenKind::I64, "0x0"),
        (TokenKind::I64, "0xDEADBEEF"),
        (TokenKind::I64, "0X1a2B"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn integer_suffixes() {
    // Underscore-separated suffix
    let source = "123_i64 0_i64";
    let expected = [
        (TokenKind::I64, "123"),
        (TokenKind::I64, "0"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Rust-style direct suffix
    let source = "123i64 0i64";
    let expected = [
        (TokenKind::I64, "123"),
        (TokenKind::I64, "0"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Mixed radix with suffix
    let source = "0b1010i64 0xFFi64";
    let expected = [
        (TokenKind::I64, "0b1010"),
        (TokenKind::I64, "0xFF"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn typed_integer_suffixes() {
    // Signed suffixes
    let source = "42i8 42_i8 127i16 32767i32 2147483647i64 170141183460469231731687303715884105727i128 42isize";
    let expected = [
        (TokenKind::I8, "42"),
        (TokenKind::I8, "42"),
        (TokenKind::I16, "127"),
        (TokenKind::I32, "32767"),
        (TokenKind::I64, "2147483647"),
        (TokenKind::I128, "170141183460469231731687303715884105727"),
        (TokenKind::Isize, "42"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Unsigned suffixes
    let source = "42u8 42_u8 255u16 65535u32 4294967295u64 340282366920938463463374607431768211455u128 42usize";
    let expected = [
        (TokenKind::U8, "42"),
        (TokenKind::U8, "42"),
        (TokenKind::U16, "255"),
        (TokenKind::U32, "65535"),
        (TokenKind::U64, "4294967295"),
        (TokenKind::U128, "340282366920938463463374607431768211455"),
        (TokenKind::Usize, "42"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }

    // Suffix boundary checking (invalid suffixes become separate idents)
    let source = "42i641 42u641 42i12 42u12 42isizes";
    let expected = [
        (TokenKind::I64, "42"),
        (TokenKind::Ident, "i641"),
        (TokenKind::I64, "42"),
        (TokenKind::Ident, "u641"),
        (TokenKind::I64, "42"),
        (TokenKind::Ident, "i12"),
        (TokenKind::I64, "42"),
        (TokenKind::Ident, "u12"),
        (TokenKind::I64, "42"),
        (TokenKind::Ident, "isizes"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind, "for literal: {}", literal);
        assert_eq!(token.literal, literal);
    }

    // Radix with typed suffix
    let source = "0b1010i8 0o755u16 0xFFi32 0b11111111u8 0xDEADBEEFu64 0o777isize";
    let expected = [
        (TokenKind::I8, "0b1010"),
        (TokenKind::U16, "0o755"),
        (TokenKind::I32, "0xFF"),
        (TokenKind::U8, "0b11111111"),
        (TokenKind::U64, "0xDEADBEEF"),
        (TokenKind::Isize, "0o777"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn integer_followed_by_dot_method() {
    let source = "5.abs()";
    let expected = [
        (TokenKind::I64, "5"),
        (TokenKind::Dot, "."),
        (TokenKind::Ident, "abs"),
        (TokenKind::LParen, "("),
        (TokenKind::RParen, ")"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn arrays_and_hashes() {
    let source = r#"[1, 2, 3]; {"key": "value"}; arr[0]"#;
    let expected = [
        (TokenKind::LBracket, "["),
        (TokenKind::I64, "1"),
        (TokenKind::Comma, ","),
        (TokenKind::I64, "2"),
        (TokenKind::Comma, ","),
        (TokenKind::I64, "3"),
        (TokenKind::RBracket, "]"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::LBrace, "{"),
        (TokenKind::String, "key"),
        (TokenKind::Colon, ":"),
        (TokenKind::String, "value"),
        (TokenKind::RBrace, "}"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Ident, "arr"),
        (TokenKind::LBracket, "["),
        (TokenKind::I64, "0"),
        (TokenKind::RBracket, "]"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);

    for (i, (kind, literal)) in expected.iter().enumerate() {
        let token = lexer.next_token();
        assert_eq!(
            token.kind, *kind,
            "tests[{}]: token kind wrong. expected={:?}, got={:?}",
            i, kind, token.kind
        );
        assert_eq!(
            token.literal, *literal,
            "tests[{}]: literal wrong. expected={:?}, got={:?}",
            i, literal, token.literal
        );
    }
}

#[test]
fn custom_type_tokens() {
    let source = "=> :: . : :: = =>";
    let expected = [
        (TokenKind::FatArrow, "=>"),
        (TokenKind::PathSep, "::"),
        (TokenKind::Dot, "."),
        (TokenKind::Colon, ":"),
        (TokenKind::PathSep, "::"),
        (TokenKind::Assign, "="),
        (TokenKind::FatArrow, "=>"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);
    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}
