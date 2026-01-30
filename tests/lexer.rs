use maat::{Lexer, TokenKind};

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
        (TokenKind::Int64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "ten"),
        (TokenKind::Assign, "="),
        (TokenKind::Int64, "10"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Let, "let"),
        (TokenKind::Ident, "add"),
        (TokenKind::Assign, "="),
        (TokenKind::Function, "fn"),
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
        (TokenKind::Int64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Int64, "5"),
        (TokenKind::Less, "<"),
        (TokenKind::Int64, "10"),
        (TokenKind::Greater, ">"),
        (TokenKind::Int64, "5"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::If, "if"),
        (TokenKind::LParen, "("),
        (TokenKind::Int64, "5"),
        (TokenKind::Less, "<"),
        (TokenKind::Int64, "10"),
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
        (TokenKind::Int64, "10"),
        (TokenKind::Equal, "=="),
        (TokenKind::Int64, "10"),
        (TokenKind::Semicolon, ";"),
        (TokenKind::Int64, "10"),
        (TokenKind::NotEqual, "!="),
        (TokenKind::Int64, "9"),
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
fn single_character_tokens() {
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
}

#[test]
fn two_character_tokens() {
    let source = "== !=";
    let expected = [
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
fn keywords() {
    let source = "let fn if else return true false";
    let expected = [
        (TokenKind::Let, "let"),
        (TokenKind::Function, "fn"),
        (TokenKind::If, "if"),
        (TokenKind::Else, "else"),
        (TokenKind::Return, "return"),
        (TokenKind::True, "true"),
        (TokenKind::False, "false"),
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
        (TokenKind::Int64, "0"),
        (TokenKind::Int64, "1"),
        (TokenKind::Int64, "42"),
        (TokenKind::Int64, "1234567890"),
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
        (TokenKind::Int64, "5"),
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
fn string_literals() {
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
}

#[test]
fn float64_literals() {
    let source = "3.14 0.5 123.456 0.0 999.999";
    let expected = [
        (TokenKind::Float64, "3.14"),
        (TokenKind::Float64, "0.5"),
        (TokenKind::Float64, "123.456"),
        (TokenKind::Float64, "0.0"),
        (TokenKind::Float64, "999.999"),
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
fn number_suffixes() {
    let source = "123_i64 456_f64 0_i64 999_f64";
    let expected = [
        (TokenKind::Int64, "123_i64"),
        (TokenKind::Float64, "456_f64"),
        (TokenKind::Int64, "0_i64"),
        (TokenKind::Float64, "999_f64"),
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
fn scientific_notation() {
    let source = "1e10 1.5e10 2E5 3.14E-2 1e+5 6.022e23 0e0";
    let expected = [
        (TokenKind::Float64, "1e10"),
        (TokenKind::Float64, "1.5e10"),
        (TokenKind::Float64, "2E5"),
        (TokenKind::Float64, "3.14E-2"),
        (TokenKind::Float64, "1e+5"),
        (TokenKind::Float64, "6.022e23"),
        (TokenKind::Float64, "0e0"),
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
fn mixed_numbers() {
    let source = "3.14_f64 1.5e10_f64 100_i64";
    let expected = [
        (TokenKind::Float64, "3.14_f64"),
        (TokenKind::Float64, "1.5e10_f64"),
        (TokenKind::Int64, "100_i64"),
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
        (TokenKind::Int64, "5"),
        (TokenKind::Invalid, "."),
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
fn binary_literals() {
    let source = "0b1010 0B1111 0b0";
    let expected = [
        (TokenKind::Int64, "0b1010"),
        (TokenKind::Int64, "0B1111"),
        (TokenKind::Int64, "0b0"),
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
fn octal_literals() {
    let source = "0o755 0O644 0o0";
    let expected = [
        (TokenKind::Int64, "0o755"),
        (TokenKind::Int64, "0O644"),
        (TokenKind::Int64, "0o0"),
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
fn hex_literals() {
    let source = "0xff 0xFF 0x0 0xDEADBEEF 0X1a2B";
    let expected = [
        (TokenKind::Int64, "0xff"),
        (TokenKind::Int64, "0xFF"),
        (TokenKind::Int64, "0x0"),
        (TokenKind::Int64, "0xDEADBEEF"),
        (TokenKind::Int64, "0X1a2B"),
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
fn rust_style_suffixes() {
    let source = "123i64 456f64 0i64 999f64";
    let expected = [
        (TokenKind::Int64, "123i64"),
        (TokenKind::Float64, "456f64"),
        (TokenKind::Int64, "0i64"),
        (TokenKind::Float64, "999f64"),
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
fn mixed_radix_and_suffixes() {
    let source = "0b1010i64 0xFFi64 3.14f64";
    let expected = [
        (TokenKind::Int64, "0b1010i64"),
        (TokenKind::Int64, "0xFFi64"),
        (TokenKind::Float64, "3.14f64"),
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
        (TokenKind::Int64, "1"),
        (TokenKind::Comma, ","),
        (TokenKind::Int64, "2"),
        (TokenKind::Comma, ","),
        (TokenKind::Int64, "3"),
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
        (TokenKind::Int64, "0"),
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
fn signed_integer_suffixes() {
    let source = "42i8 42_i8 127i16 32767i32 2147483647i64 170141183460469231731687303715884105727i128 42isize";
    let expected = [
        (TokenKind::Int64, "42i8"),
        (TokenKind::Int64, "42_i8"),
        (TokenKind::Int64, "127i16"),
        (TokenKind::Int64, "32767i32"),
        (TokenKind::Int64, "2147483647i64"),
        (
            TokenKind::Int64,
            "170141183460469231731687303715884105727i128",
        ),
        (TokenKind::Int64, "42isize"),
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
fn unsigned_integer_suffixes() {
    let source = "42u8 42_u8 255u16 65535u32 4294967295u64 340282366920938463463374607431768211455u128 42usize";
    let expected = [
        (TokenKind::Int64, "42u8"),
        (TokenKind::Int64, "42_u8"),
        (TokenKind::Int64, "255u16"),
        (TokenKind::Int64, "65535u32"),
        (TokenKind::Int64, "4294967295u64"),
        (
            TokenKind::Int64,
            "340282366920938463463374607431768211455u128",
        ),
        (TokenKind::Int64, "42usize"),
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
fn float_suffixes() {
    let source = "3.14f32 3.14_f32 2.718f64 2.718_f64 1e10f32 1.5e-5f64";
    let expected = [
        (TokenKind::Float64, "3.14f32"),
        (TokenKind::Float64, "3.14_f32"),
        (TokenKind::Float64, "2.718f64"),
        (TokenKind::Float64, "2.718_f64"),
        (TokenKind::Float64, "1e10f32"),
        (TokenKind::Float64, "1.5e-5f64"),
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
fn suffix_boundary_checking() {
    let source = "42i641 42u641 42f641 42i12 42u12 42isizes";
    let expected = [
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "i641"),
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "u641"),
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "f641"),
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "i12"),
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "u12"),
        (TokenKind::Int64, "42"),
        (TokenKind::Ident, "isizes"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);

    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind, "for literal: {}", literal);
        assert_eq!(token.literal, literal);
    }
}

#[test]
fn radix_with_suffix() {
    let source = "0b1010i8 0o755u16 0xFFi32 0b11111111u8 0xDEADBEEFu64 0o777isize";
    let expected = [
        (TokenKind::Int64, "0b1010i8"),
        (TokenKind::Int64, "0o755u16"),
        (TokenKind::Int64, "0xFFi32"),
        (TokenKind::Int64, "0b11111111u8"),
        (TokenKind::Int64, "0xDEADBEEFu64"),
        (TokenKind::Int64, "0o777isize"),
        (TokenKind::Eof, ""),
    ];

    let mut lexer = Lexer::new(source);

    for (kind, literal) in expected {
        let token = lexer.next_token();
        assert_eq!(token.kind, kind);
        assert_eq!(token.literal, literal);
    }
}
