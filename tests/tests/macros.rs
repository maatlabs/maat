use maat_ast::{Expr, Node};
use maat_eval::{define_macros, eval, expand_macros};
use maat_runtime::{Env, Value};

fn test_macros(input: &str, expected: &str) {
    let program = maat_tests::parse(input);
    let env = Env::default();
    let program = define_macros(program, &env);
    let expanded = expand_macros(Node::Program(program), &env);

    let expected_prog = maat_tests::parse(expected);
    if let Node::Program(expanded_prog) = expanded {
        assert_eq!(
            expanded_prog.statements.len(),
            expected_prog.statements.len(),
            "Statement count mismatch.\nExpanded: {expanded_prog}\nExpected: {expected_prog}"
        );
        for (i, (expanded_stmt, expected_stmt)) in expanded_prog
            .statements
            .iter()
            .zip(expected_prog.statements.iter())
            .enumerate()
        {
            assert_eq!(
                format!("{expanded_stmt}"),
                format!("{expected_stmt}"),
                "Statement {i} mismatch.\nExpanded: {expanded_stmt}\nExpected: {expected_stmt}"
            );
        }
    } else {
        panic!("Expected Program node");
    }
}

#[test]
fn test_define_macros() {
    let input = r#"
        let number = 1;
        let function = fn(x, y) { x + y; };
        let mymacro = macro(x, y) { x + y; };
    "#;
    let program = maat_tests::parse(input);
    let env = Env::default();
    let modified = define_macros(program, &env);
    assert_eq!(modified.statements.len(), 2);
    assert!(env.get("mymacro").is_some());

    if let Some(Value::Macro(macro_obj)) = env.get("mymacro") {
        assert_eq!(macro_obj.params.len(), 2);
        assert_eq!(macro_obj.params[0], "x");
        assert_eq!(macro_obj.params[1], "y");
        assert_eq!(format!("{}", macro_obj.body), "{\n(x + y);\n}");
    } else {
        panic!("Expected macro");
    }
}

#[test]
fn test_quote_builtin() {
    let input = "quote(5 + 5)";
    let program = maat_tests::parse(input);
    let env = Env::default();
    let result = eval(Node::Program(program), &env).unwrap();
    if let Value::Quote(quote_obj) = result {
        if let Node::Expr(Expr::Infix(infix)) = &quote_obj.node {
            assert_eq!(infix.operator, "+");
            assert_eq!(format!("{infix}"), "(5 + 5)");
        } else {
            panic!("Expected infix expression in quote");
        }
    } else {
        panic!("Expected Quote");
    }
}

#[test]
fn test_macro_expansion() {
    // Simple expansion
    test_macros(
        r#"
        let infixExpr = macro() { quote(1 + 2); };
        infixExpr();
        "#,
        "(1 + 2)",
    );
    // Expansion with unquote
    test_macros(
        r#"
        let reverse = macro(a, b) { quote(unquote(b) - unquote(a)); };
        reverse(2 + 2, 10 - 5);
        "#,
        "(10 - 5) - (2 + 2)",
    );
    // Unless macro (conditional rewriting)
    test_macros(
        r#"
        let unless = macro(cond, cons, alt) {
            quote(if (!(unquote(cond))) {
                unquote(cons);
            } else {
                unquote(alt);
            });
        };

        unless(10 > 5, print("not greater"), print("greater"));
        "#,
        r#"
        if (!(10 > 5)) {
            print("not greater");
        } else {
            print("greater");
        }
        "#,
    );
    // Double macro
    test_macros(
        r#"
        let double = macro(x) { quote(unquote(x) * 2); };
        double(5);
        "#,
        "(5 * 2)",
    );
    // Multiple arguments
    test_macros(
        r#"
        let add = macro(a, b) { quote(unquote(a) + unquote(b)); };
        add(3, 7);
        "#,
        "(3 + 7)",
    );
}
