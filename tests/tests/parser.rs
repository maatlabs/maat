use maat_ast::*;

fn parse(input: &str) -> Program {
    maat_tests::parse(input)
}

fn expect_single_stmt(program: &Program) -> &Stmt {
    assert_eq!(program.statements.len(), 1);
    &program.statements[0]
}

#[test]
fn parse_let_statements() {
    [
        ("let x = 5;", "x", "5"),
        ("let y = true;", "y", "true"),
        ("let foobar = y;", "foobar", "y"),
    ]
    .iter()
    .for_each(|(input, ident, value)| {
        let program = parse(input);
        let Stmt::Let(let_stmt) = expect_single_stmt(&program) else {
            panic!("expected Let statement");
        };
        assert_eq!(let_stmt.ident, *ident);
        assert_eq!(let_stmt.value.to_string(), *value);
    });
}

#[test]
fn parse_return_statements() {
    [
        ("return 5;", "5"),
        ("return true;", "true"),
        ("return foobar;", "foobar"),
    ]
    .iter()
    .for_each(|(input, value)| {
        let program = parse(input);
        let Stmt::Return(ret) = expect_single_stmt(&program) else {
            panic!("expected Return statement");
        };
        assert_eq!(ret.value.to_string(), *value);
    });
}

#[test]
fn parse_use_statements() {
    // Simple paths use
    let program = parse("use foo::bar;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar"]);
    assert!(use_stmt.items.is_none());

    // Multiple paths
    let program = parse("use foo::bar::baz::qux;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar", "baz", "qux"]);
    assert!(use_stmt.items.is_none());

    // Multiple paths, nested items
    let program = parse("use foo::bar::{baz, qux};");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar"]);
    assert_eq!(
        use_stmt.items.as_ref().unwrap(),
        &vec!["baz".to_string(), "qux".to_string()]
    );

    // Grouped single item
    let program = parse("use math::{abs};");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["math"]);
    assert_eq!(use_stmt.items.as_ref().unwrap(), &vec!["abs".to_string()]);

    // Use path directly
    let program = parse("use foo;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo"]);
    assert!(use_stmt.items.is_none());
}

#[test]
fn parse_reexports() {
    let program = parse("pub use foo::bar;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert!(use_stmt.is_public);
    assert_eq!(use_stmt.path, vec!["foo", "bar"]);
    assert!(use_stmt.items.is_none());

    let program = parse("pub use math::{sin, cos};");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert!(use_stmt.is_public);
    assert_eq!(use_stmt.path, vec!["math"]);
    assert_eq!(
        use_stmt.items.as_ref().unwrap(),
        &vec!["sin".to_string(), "cos".to_string()]
    );
}

#[test]
fn parse_mod_stmt() {
    // External module, public
    let program = parse("pub mod math;");
    let Stmt::Mod(mod_stmt) = expect_single_stmt(&program) else {
        panic!("expected Mod statement");
    };
    assert_eq!(mod_stmt.name, "math");
    assert!(mod_stmt.body.is_none());
    assert!(mod_stmt.is_public);

    // Inline module, private
    let program = parse("mod foo { let x = 5; }");
    let Stmt::Mod(mod_stmt) = expect_single_stmt(&program) else {
        panic!("expected Mod statement");
    };
    assert_eq!(mod_stmt.name, "foo");
    assert!(mod_stmt.body.is_some());
    assert_eq!(mod_stmt.body.as_ref().unwrap().len(), 1);
    assert!(!mod_stmt.is_public);
}

#[test]
fn parse_prefix_expressions() {
    [
        ("!5;", "!", "5"),
        ("-15;", "-", "15"),
        ("!foobar;", "!", "foobar"),
        ("-foobar;", "-", "foobar"),
        ("!true;", "!", "true"),
        ("!false;", "!", "false"),
    ]
    .iter()
    .for_each(|(input, op, operand)| {
        let program = parse(input);
        let Stmt::Expr(ExprStmt {
            value: Expr::Prefix(prefix),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Prefix expression");
        };
        assert_eq!(prefix.operator, *op);
        assert_eq!(prefix.operand.to_string(), *operand);
    });
}

#[test]
fn parse_infix_expressions() {
    [
        ("5 + 5;", "5", "+", "5"),
        ("5 - 5;", "5", "-", "5"),
        ("5 * 5;", "5", "*", "5"),
        ("5 / 5;", "5", "/", "5"),
        ("5 > 5;", "5", ">", "5"),
        ("5 < 5;", "5", "<", "5"),
        ("5 >= 5;", "5", ">=", "5"),
        ("5 <= 5;", "5", "<=", "5"),
        ("5 == 5;", "5", "==", "5"),
        ("5 != 5;", "5", "!=", "5"),
        ("true == true", "true", "==", "true"),
        ("true != false", "true", "!=", "false"),
        ("false == false", "false", "==", "false"),
    ]
    .iter()
    .for_each(|(input, lhs, op, rhs)| {
        let program = parse(input);
        let Stmt::Expr(ExprStmt {
            value: Expr::Infix(infix),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Infix expression");
        };
        assert_eq!(infix.lhs.to_string(), *lhs);
        assert_eq!(infix.operator, *op);
        assert_eq!(infix.rhs.to_string(), *rhs);
    });
}

#[test]
fn parse_string_literal() {
    [
        (r#""hello world""#, "hello world"),
        (r#""foo bar""#, "foo bar"),
        (r#""""#, ""),
    ]
    .iter()
    .for_each(|(input, expected)| {
        let program = parse(input);
        let Stmt::Expr(ExprStmt {
            value: Expr::Str(s),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected string literal");
        };
        assert_eq!(s.value, *expected);
    });
}

#[test]
fn parse_arrays() {
    // Non-empty array
    let program = parse("[1, 2 * 2, 3 + 3]");
    let Stmt::Expr(ExprStmt {
        value: Expr::Array(array),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 3);
    assert_eq!(array.elements[0].to_string(), "1");
    assert_eq!(array.elements[1].to_string(), "(2 * 2)");
    assert_eq!(array.elements[2].to_string(), "(3 + 3)");

    // Empty array
    let program = parse("[]");
    let Stmt::Expr(ExprStmt {
        value: Expr::Array(array),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 0);
}

#[test]
fn parse_index_expression() {
    let program = parse("myArray[1 + 1]");
    let Stmt::Expr(ExprStmt {
        value: Expr::Index(index),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected index expression");
    };
    assert!(matches!(&*index.expr, Expr::Ident(id) if id.value == "myArray"));
    assert_eq!(index.index.to_string(), "(1 + 1)");
}

#[test]
fn parse_hashes() {
    // Non-empty hash
    let program = parse(r#"{"one": 1, "two": 2, "three": 3}"#);
    let Stmt::Expr(ExprStmt {
        value: Expr::Map(map),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(map.pairs.len(), 3);
    let expected = [("one", "1"), ("two", "2"), ("three", "3")];
    for (key, value) in expected {
        let found = map
            .pairs
            .iter()
            .any(|(k, v)| k.to_string() == key && v.to_string() == value);
        assert!(found, "expected key-value pair: {} => {}", key, value);
    }
    // Empty hash
    let program = parse("{}");
    let Stmt::Expr(ExprStmt {
        value: Expr::Map(hash),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(hash.pairs.len(), 0);

    // Hash with expressions
    let program = parse(r#"{"one": 0 + 1, "two": 10 - 8}"#);
    let Stmt::Expr(ExprStmt {
        value: Expr::Map(hash),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected hash literal");
    };
    assert_eq!(hash.pairs.len(), 2);
}

#[test]
fn parse_non_decimal_literals() {
    // Binary
    [("0b1010;", 10), ("0B1111;", 15), ("0b0;", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Stmt::Expr(ExprStmt {
                value: Expr::Number(num),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Number expression");
            };
            assert_eq!(num.radix, Radix::Bin);
            assert_eq!(num.value, *expected, "input: {input}");
        });
    // Octal
    [("0o755;", 493), ("0O644;", 420), ("0o0;", 0)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Stmt::Expr(ExprStmt {
                value: Expr::Number(num),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Number expression");
            };
            assert_eq!(num.radix, Radix::Oct);
            assert_eq!(num.value, *expected, "input: {input}");
        });
    // Hex
    [("0xff;", 255), ("0xFF;", 255), ("0xDEAD;", 57005)]
        .iter()
        .for_each(|(input, expected)| {
            let program = parse(input);
            let Stmt::Expr(ExprStmt {
                value: Expr::Number(num),
                ..
            }) = expect_single_stmt(&program)
            else {
                panic!("expected Number expression");
            };
            assert_eq!(num.radix, Radix::Hex);
            assert_eq!(num.value, *expected, "input: {input}");
        });
}

#[test]
fn parse_rust_style_suffixes() {
    let program = parse("123i64;");
    let Stmt::Expr(ExprStmt {
        value: Expr::Number(num),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Number expression");
    };
    assert_eq!(num.value, 123);
}

#[test]
fn parse_operator_precedence() {
    [
        ("-a * b", "((-a) * b);"),
        ("!-a", "(!(-a));"),
        ("a + b + c", "((a + b) + c);"),
        ("a + b - c", "((a + b) - c);"),
        ("a * b * c", "((a * b) * c);"),
        ("a * b / c", "((a * b) / c);"),
        ("a + b / c", "(a + (b / c));"),
        ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f);"),
        ("3 + 4; -5 * 5", "(3 + 4);((-5) * 5);"),
        ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4));"),
        ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4));"),
        (
            "3 + 4 * 5 == 3 * 1 + 4 * 5",
            "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)));",
        ),
        ("true", "true;"),
        ("false", "false;"),
        ("3 > 5 == false", "((3 > 5) == false);"),
        ("3 < 5 == true", "((3 < 5) == true);"),
        ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4);"),
        ("(5 + 5) * 2", "((5 + 5) * 2);"),
        ("2 / (5 + 5)", "(2 / (5 + 5));"),
        ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5));"),
        ("-(5 + 5)", "(-(5 + 5));"),
        ("!(true == true)", "(!(true == true));"),
        ("a + add(b * c) + d", "((a + add((b * c))) + d);"),
        (
            "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
            "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)));",
        ),
        (
            "add(a + b + c * d / f + g)",
            "add((((a + b) + ((c * d) / f)) + g));",
        ),
    ]
    .iter()
    .for_each(|(input, expected)| {
        assert_eq!(parse(input).to_string(), *expected);
    });
}

#[test]
fn parse_conditionals() {
    // If without else
    let program = parse("if (x < y) { x }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Cond(cond),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Cond expression");
    };
    let Expr::Infix(infix) = cond.condition.as_ref() else {
        panic!("expected Infix condition");
    };
    assert_eq!(infix.to_string(), "(x < y)");
    assert_eq!(cond.consequence.statements.len(), 1);
    assert_eq!(cond.consequence.statements[0].to_string(), "x;");
    assert!(cond.alternative.is_none());

    // If with else
    let program = parse("if (x < y) { x } else { y }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Cond(cond),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Cond expression");
    };
    assert_eq!(cond.condition.to_string(), "(x < y)");
    assert_eq!(cond.consequence.statements[0].to_string(), "x;");
    assert_eq!(
        cond.alternative.as_ref().unwrap().statements[0].to_string(),
        "y;"
    );
}

#[test]
fn parse_functions() {
    // Function literal
    let program = parse("fn(x, y) { x + y; }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Lambda(func),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Lambda expression");
    };
    assert_eq!(func.param_names().collect::<Vec<_>>(), vec!["x", "y"]);
    assert_eq!(func.body.statements.len(), 1);
    assert_eq!(func.body.statements[0].to_string(), "(x + y);");

    // Function parameter variations
    [
        ("fn() {};", vec![]),
        ("fn(x) {};", vec!["x"]),
        ("fn(x, y, z) {};", vec!["x", "y", "z"]),
    ]
    .iter()
    .for_each(|(input, expected_params)| {
        let program = parse(input);
        let Stmt::Expr(ExprStmt {
            value: Expr::Lambda(func),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Lambda expression");
        };
        let names: Vec<&str> = func.param_names().collect();
        assert_eq!(names, *expected_params);
    });
    // Public function
    let program = parse("pub fn add(x: i64, y: i64) -> i64 { x + y }");
    let Stmt::FuncDef(func) = expect_single_stmt(&program) else {
        panic!("expected FuncDef statement");
    };
    assert_eq!(func.name, "add");
    assert!(func.is_public);
}

#[test]
fn parse_call_expressions() {
    // Single call with expressions
    let program = parse("add(1, 2 * 3, 4 + 5);");
    let Stmt::Expr(ExprStmt {
        value: Expr::Call(call),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Call expression");
    };
    assert_eq!(call.function.to_string(), "add");
    assert_eq!(call.arguments.len(), 3);
    assert_eq!(call.arguments[0].to_string(), "1");
    assert_eq!(call.arguments[1].to_string(), "(2 * 3)");
    assert_eq!(call.arguments[2].to_string(), "(4 + 5)");

    // Argument count variations
    [
        ("add();", "add", vec![]),
        ("add(1);", "add", vec!["1"]),
        (
            "add(1, 2 * 3, 4 + 5);",
            "add",
            vec!["1", "(2 * 3)", "(4 + 5)"],
        ),
    ]
    .iter()
    .for_each(|(input, func_name, expected_args)| {
        let program = parse(input);
        let Stmt::Expr(ExprStmt {
            value: Expr::Call(call),
            ..
        }) = expect_single_stmt(&program)
        else {
            panic!("expected Call expression");
        };
        assert_eq!(call.function.to_string(), *func_name);
        assert_eq!(
            call.arguments
                .iter()
                .map(|arg| arg.to_string())
                .collect::<Vec<_>>(),
            *expected_args
        );
    });
}

#[test]
fn parse_loops() {
    // Loop
    let program = parse("loop { 1; }");
    let Stmt::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    assert_eq!(loop_stmt.body.statements.len(), 1);
    assert_eq!(loop_stmt.body.statements[0].to_string(), "1;");

    // While
    let program = parse("while (x < 10) { x; }");
    let Stmt::While(while_stmt) = expect_single_stmt(&program) else {
        panic!("expected While statement");
    };
    assert_eq!(while_stmt.condition.to_string(), "(x < 10)");
    assert_eq!(while_stmt.body.statements.len(), 1);
    assert_eq!(while_stmt.body.statements[0].to_string(), "x;");

    // For
    let program = parse("for x in [1, 2, 3] { x; }");
    let Stmt::For(for_stmt) = expect_single_stmt(&program) else {
        panic!("expected For statement");
    };
    assert_eq!(for_stmt.ident, "x");
    assert_eq!(for_stmt.iterable.to_string(), "[1, 2, 3]");
    assert_eq!(for_stmt.body.statements.len(), 1);
    assert_eq!(for_stmt.body.statements[0].to_string(), "x;");
}

#[test]
fn parse_loop_control() {
    // Break without value
    let program = parse("loop { break; }");
    let Stmt::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Stmt::Expr(ExprStmt {
        value: Expr::Break(break_expr),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert!(break_expr.value.is_none());

    // Break with value
    let program = parse("loop { break 42; }");
    let Stmt::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Stmt::Expr(ExprStmt {
        value: Expr::Break(break_expr),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert_eq!(break_expr.value.as_ref().unwrap().to_string(), "42");

    // Continue
    let program = parse("loop { continue; }");
    let Stmt::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    let Stmt::Expr(ExprStmt {
        value: Expr::Continue(_),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Continue expression");
    };

    // Labeled loop
    let program = parse("'outer: loop { break 'outer; }");
    let Stmt::Loop(loop_stmt) = expect_single_stmt(&program) else {
        panic!("expected Loop statement");
    };
    assert_eq!(loop_stmt.label.as_deref(), Some("outer"));
    let Stmt::Expr(ExprStmt {
        value: Expr::Break(break_expr),
        ..
    }) = &loop_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert_eq!(break_expr.label.as_deref(), Some("outer"));

    // Labeled while
    let program = parse("'outer: while (true) { continue 'outer; }");
    let Stmt::While(while_stmt) = expect_single_stmt(&program) else {
        panic!("expected While statement");
    };
    assert_eq!(while_stmt.label.as_deref(), Some("outer"));
    let Stmt::Expr(ExprStmt {
        value: Expr::Continue(cont_expr),
        ..
    }) = &while_stmt.body.statements[0]
    else {
        panic!("expected Continue expression");
    };
    assert_eq!(cont_expr.label.as_deref(), Some("outer"));

    // Labeled for
    let program = parse("'rows: for i in 0..10 { break 'rows 42; }");
    let Stmt::For(for_stmt) = expect_single_stmt(&program) else {
        panic!("expected For statement");
    };
    assert_eq!(for_stmt.label.as_deref(), Some("rows"));
    let Stmt::Expr(ExprStmt {
        value: Expr::Break(break_expr),
        ..
    }) = &for_stmt.body.statements[0]
    else {
        panic!("expected Break expression");
    };
    assert_eq!(break_expr.label.as_deref(), Some("rows"));
    assert!(break_expr.value.is_some());
}

#[test]
fn parse_struct_declarations() {
    // Basic struct
    let program = parse("struct Point { x: i64, y: i64 }");
    let Stmt::StructDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected StructDecl");
    };
    assert_eq!(decl.name, "Point");
    assert!(decl.generic_params.is_empty());
    assert_eq!(decl.fields.len(), 2);
    assert_eq!(decl.fields[0].name, "x");
    assert_eq!(decl.fields[0].ty.to_string(), "i64");
    assert_eq!(decl.fields[1].name, "y");
    assert_eq!(decl.fields[1].ty.to_string(), "i64");

    // Generic struct
    let program = parse("struct Pair<T, U> { first: T, second: U }");
    let Stmt::StructDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected StructDecl");
    };
    assert_eq!(decl.name, "Pair");
    assert_eq!(decl.generic_params.len(), 2);
    assert_eq!(decl.generic_params[0].name, "T");
    assert_eq!(decl.generic_params[1].name, "U");
    assert_eq!(decl.fields.len(), 2);

    // Public struct, private fields
    let program = parse("pub struct Point { x: i64, y: i64 }");
    let Stmt::StructDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected StructDecl statement");
    };
    assert_eq!(decl.name, "Point");
    assert!(decl.is_public);

    // Public struct, mixed visibility for fields
    let program = parse("pub struct Point { pub x: i64, y: i64 }");
    let Stmt::StructDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected StructDecl statement");
    };
    assert!(decl.is_public);
    assert_eq!(decl.fields.len(), 2);
    assert!(decl.fields[0].is_public);
    assert_eq!(decl.fields[0].name, "x");
    assert!(!decl.fields[1].is_public);
    assert_eq!(decl.fields[1].name, "y");
}

#[test]
fn parse_enum_declarations() {
    // Unit variants
    let program = parse("enum Direction { North, South, East, West }");
    let Stmt::EnumDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected EnumDecl");
    };
    assert_eq!(decl.name, "Direction");
    assert_eq!(decl.variants.len(), 4);
    assert!(matches!(decl.variants[0].kind, EnumVariantKind::Unit));
    assert_eq!(decl.variants[0].name, "North");
    assert_eq!(decl.variants[3].name, "West");

    // Tuple variants
    let program = parse("enum Shape { Circle(i64), Rectangle(i64, i64) }");
    let Stmt::EnumDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected EnumDecl");
    };
    assert_eq!(decl.name, "Shape");
    assert_eq!(decl.variants.len(), 2);
    let EnumVariantKind::Tuple(ref fields) = decl.variants[0].kind else {
        panic!("expected Tuple variant");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].to_string(), "i64");
    let EnumVariantKind::Tuple(ref fields2) = decl.variants[1].kind else {
        panic!("expected Tuple variant");
    };
    assert_eq!(fields2.len(), 2);

    // Public enum
    let program = parse("pub enum Color { Red, Green, Blue }");
    let Stmt::EnumDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected EnumDecl statement");
    };
    assert_eq!(decl.name, "Color");
    assert!(decl.is_public);
}

#[test]
fn parse_trait_decl() {
    // Private trait
    let program = parse("trait Greet { fn hello(self) -> bool; }");
    let Stmt::TraitDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected TraitDecl");
    };
    assert_eq!(decl.name, "Greet");
    assert_eq!(decl.methods.len(), 1);
    assert_eq!(decl.methods[0].name, "hello");
    assert!(decl.methods[0].default_body.is_none());

    // Public trait
    let program = parse("pub trait Display { fn show(self) -> i64; }");
    let Stmt::TraitDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected TraitDecl statement");
    };
    assert_eq!(decl.name, "Display");
    assert!(decl.is_public);
}

#[test]
fn parse_impl_blocks() {
    // Inherent impl
    let program = parse("impl Point { fn new(x: i64, y: i64) -> Point { x } }");
    let Stmt::ImplBlock(block) = expect_single_stmt(&program) else {
        panic!("expected ImplBlock");
    };
    assert!(block.trait_name.is_none());
    assert_eq!(block.self_type.to_string(), "Point");
    assert_eq!(block.methods.len(), 1);
    assert_eq!(block.methods[0].name, "new");

    // Trait impl
    let program = parse("impl Greet for Point { fn hello(self) -> bool { true } }");
    let Stmt::ImplBlock(block) = expect_single_stmt(&program) else {
        panic!("expected ImplBlock");
    };
    assert!(block.trait_name.is_some());
    assert_eq!(block.trait_name.as_ref().unwrap().to_string(), "Greet");
    assert_eq!(block.self_type.to_string(), "Point");
    assert_eq!(block.methods.len(), 1);
}

#[test]
fn parse_pub_impl_methods() {
    let program = parse(
        r#"
        impl Point {
            pub fn new(x: i64, y: i64) -> Point {
                Point { x: x, y: y }
            }
            fn private_helper(self) -> i64 {
                self.x
            }
            pub fn distance(self) -> i64 {
                self.x + self.y
            }
        }
    "#,
    );
    let Stmt::ImplBlock(impl_block) = expect_single_stmt(&program) else {
        panic!("expected ImplBlock statement");
    };
    assert_eq!(impl_block.methods.len(), 3);
    assert!(impl_block.methods[0].is_public);
    assert_eq!(impl_block.methods[0].name, "new");
    assert!(!impl_block.methods[1].is_public);
    assert_eq!(impl_block.methods[1].name, "private_helper");
    assert!(impl_block.methods[2].is_public);
    assert_eq!(impl_block.methods[2].name, "distance");
}

#[test]
fn parse_field_access() {
    // Simple field access
    let program = parse("point.x;");
    let Stmt::Expr(ExprStmt {
        value: Expr::FieldAccess(access),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected FieldAccess");
    };
    assert_eq!(access.object.to_string(), "point");
    assert_eq!(access.field, "x");

    // Chained field access
    let program = parse("a.b.c;");
    let Stmt::Expr(ExprStmt {
        value: Expr::FieldAccess(outer),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected chained FieldAccess");
    };
    assert_eq!(outer.field, "c");
    let Expr::FieldAccess(inner) = outer.object.as_ref() else {
        panic!("expected inner FieldAccess");
    };
    assert_eq!(inner.field, "b");
    assert_eq!(inner.object.to_string(), "a");
}

#[test]
fn parse_method_call() {
    let program = parse("obj.foo(1, 2);");
    let Stmt::Expr(ExprStmt {
        value: Expr::MethodCall(call),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected MethodCall");
    };
    assert_eq!(call.object.to_string(), "obj");
    assert_eq!(call.method, "foo");
    assert_eq!(call.arguments.len(), 2);
}

#[test]
fn parse_match_expressions() {
    // Basic match with literal and wildcard
    let program = parse("match x { 1 => true, _ => false }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Match(m),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Match expression");
    };
    assert_eq!(m.scrutinee.to_string(), "x");
    assert_eq!(m.arms.len(), 2);
    assert!(matches!(m.arms[0].pattern, Pattern::Literal(_)));
    assert!(matches!(m.arms[1].pattern, Pattern::Wildcard(_)));

    // Match with ident pattern
    let program = parse("match x { y => y }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Match(m),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Match expression");
    };
    assert_eq!(m.arms.len(), 1);
    let Pattern::Ident(ref name, _) = m.arms[0].pattern else {
        panic!("expected Ident pattern");
    };
    assert_eq!(name, "y");

    // Match with tuple struct pattern
    let program = parse("match v { Some(x) => x, None => 0 }");
    let Stmt::Expr(ExprStmt {
        value: Expr::Match(m),
        ..
    }) = expect_single_stmt(&program)
    else {
        panic!("expected Match expression");
    };
    assert_eq!(m.arms.len(), 2);
    let Pattern::TupleStruct {
        ref path,
        ref fields,
        ..
    } = m.arms[0].pattern
    else {
        panic!("expected TupleStruct pattern");
    };
    assert_eq!(path, "Some");
    assert_eq!(fields.len(), 1);
}

#[test]
fn parse_mixed_module_items() {
    let program = parse(
        r#"
        use math::abs;
        mod utils;
        pub fn helper() -> i64 { 42 }
        pub struct Config { value: i64 }
    "#,
    );
    assert_eq!(program.statements.len(), 4);

    assert!(matches!(&program.statements[0], Stmt::Use(_)));
    assert!(matches!(&program.statements[1], Stmt::Mod(_)));
    assert!(matches!(&program.statements[2], Stmt::FuncDef(f) if f.is_public));
    assert!(matches!(&program.statements[3], Stmt::StructDecl(s) if s.is_public));
}

#[test]
fn parse_struct_update_syntax() {
    // Struct update with one explicit field
    let program = parse("let p = Point { x: 10, ..base };");
    let stmt = expect_single_stmt(&program);
    let Stmt::Let(let_stmt) = stmt else {
        panic!("expected let statement");
    };
    let Expr::StructLit(lit) = &let_stmt.value else {
        panic!("expected struct literal");
    };
    assert_eq!(lit.name, "Point");
    assert_eq!(lit.fields.len(), 1);
    assert_eq!(lit.fields[0].0, "x");
    assert!(lit.base.is_some());

    // Struct update with no explicit fields
    let program = parse("let p = Config { ..defaults };");
    let stmt = expect_single_stmt(&program);
    let Stmt::Let(let_stmt) = stmt else {
        panic!("expected let statement");
    };
    let Expr::StructLit(lit) = &let_stmt.value else {
        panic!("expected struct literal");
    };
    assert_eq!(lit.name, "Config");
    assert!(lit.fields.is_empty());
    assert!(lit.base.is_some());

    // Regular struct literal (no base)
    let program = parse("let p = Point { x: 1, y: 2 };");
    let stmt = expect_single_stmt(&program);
    let Stmt::Let(let_stmt) = stmt else {
        panic!("expected let statement");
    };
    let Expr::StructLit(lit) = &let_stmt.value else {
        panic!("expected struct literal");
    };
    assert_eq!(lit.name, "Point");
    assert_eq!(lit.fields.len(), 2);
    assert!(lit.base.is_none());
}
