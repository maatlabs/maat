use maat_ast::*;
use maat_span::Span;
use maat_types::TypeChecker;

const S: Span = Span::ZERO;

fn int_expr(value: i128, kind: NumKind) -> Expr {
    Expr::Number(Number {
        kind,
        value,
        radix: Radix::Dec,
        span: S,
    })
}

fn i64_expr(value: i128) -> Expr {
    int_expr(value, NumKind::I64)
}

fn bool_expr(value: bool) -> Expr {
    Expr::Bool(BoolLit { value, span: S })
}

fn str_expr(value: &str) -> Expr {
    Expr::Str(StrLit {
        value: value.to_string(),
        span: S,
    })
}

fn ident_expr(name: &str) -> Expr {
    Expr::Ident(Ident {
        value: name.to_string(),
        span: S,
    })
}

fn infix(lhs: Expr, op: &str, rhs: Expr) -> Expr {
    Expr::Infix(InfixExpr {
        lhs: Box::new(lhs),
        operator: op.to_string(),
        rhs: Box::new(rhs),
        op_class: BinOpClass::default(),
        span: S,
    })
}

fn prefix(op: &str, operand: Expr) -> Expr {
    Expr::Prefix(PrefixExpr {
        operator: op.to_string(),
        operand: Box::new(operand),
        span: S,
    })
}

fn cond(condition: Expr, consequence: Vec<Stmt>, alternative: Option<Vec<Stmt>>) -> Expr {
    Expr::Cond(CondExpr {
        condition: Box::new(condition),
        consequence: BlockStmt {
            statements: consequence,
            span: S,
        },
        alternative: alternative.map(|stmts| BlockStmt {
            statements: stmts,
            span: S,
        }),
        span: S,
    })
}

fn call(func: Expr, args: Vec<Expr>) -> Expr {
    Expr::Call(CallExpr {
        function: Box::new(func),
        arguments: args,
        span: S,
    })
}

fn lambda(params: Vec<(&str, Option<TypeExpr>)>, body: Vec<Stmt>) -> Expr {
    Expr::Lambda(Lambda {
        params: params
            .into_iter()
            .map(|(name, te)| TypedParam {
                name: name.to_string(),
                type_expr: te,
                span: S,
            })
            .collect(),
        generic_params: vec![],
        return_type: None,
        body: BlockStmt {
            statements: body,
            span: S,
        },
        span: S,
    })
}

fn method_call(obj: Expr, method: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall(MethodCallExpr {
        object: Box::new(obj),
        method: method.to_string(),
        arguments: args,
        receiver: None,
        span: S,
    })
}

fn expr_stmt(e: Expr) -> Stmt {
    Stmt::Expr(ExprStmt { value: e, span: S })
}

fn let_stmt(name: &str, value: Expr, ty: Option<TypeExpr>) -> Stmt {
    Stmt::Let(LetStmt {
        ident: name.to_string(),
        mutable: false,
        type_annotation: ty,
        value,
        pattern: None,
        span: S,
    })
}

fn func_def(
    name: &str,
    params: Vec<(&str, Option<TypeExpr>)>,
    ret: Option<TypeExpr>,
    body: Vec<Stmt>,
) -> Stmt {
    Stmt::FuncDef(FuncDef {
        name: name.to_string(),
        params: params
            .into_iter()
            .map(|(n, te)| TypedParam {
                name: n.to_string(),
                type_expr: te,
                span: S,
            })
            .collect(),
        generic_params: vec![],
        return_type: ret,
        body: BlockStmt {
            statements: body,
            span: S,
        },
        is_public: false,
        doc: None,
        span: S,
    })
}

fn named_type(name: &str) -> TypeExpr {
    TypeExpr::Named(NamedType {
        name: name.to_string(),
        span: S,
    })
}

fn struct_decl(name: &str, fields: Vec<(&str, TypeExpr)>) -> Stmt {
    Stmt::StructDecl(StructDecl {
        name: name.to_string(),
        generic_params: vec![],
        fields: fields
            .into_iter()
            .map(|(n, ty)| StructField {
                name: n.to_string(),
                ty,
                is_public: true,
                doc: None,
                span: S,
            })
            .collect(),
        is_public: false,
        doc: None,
        span: S,
    })
}

fn generic_struct_decl(name: &str, generics: Vec<&str>, fields: Vec<(&str, TypeExpr)>) -> Stmt {
    Stmt::StructDecl(StructDecl {
        name: name.to_string(),
        generic_params: generics
            .into_iter()
            .map(|g| GenericParam {
                name: g.to_string(),
                bounds: vec![],
                span: S,
            })
            .collect(),
        fields: fields
            .into_iter()
            .map(|(n, ty)| StructField {
                name: n.to_string(),
                ty,
                is_public: true,
                doc: None,
                span: S,
            })
            .collect(),
        is_public: false,
        doc: None,
        span: S,
    })
}

fn enum_decl(name: &str, variants: Vec<(&str, EnumVariantKind)>) -> Stmt {
    Stmt::EnumDecl(EnumDecl {
        name: name.to_string(),
        generic_params: vec![],
        variants: variants
            .into_iter()
            .map(|(n, kind)| EnumVariant {
                name: n.to_string(),
                kind,
                doc: None,
                span: S,
            })
            .collect(),
        is_public: false,
        doc: None,
        span: S,
    })
}

fn generic_enum_decl(
    name: &str,
    generics: Vec<&str>,
    variants: Vec<(&str, EnumVariantKind)>,
) -> Stmt {
    Stmt::EnumDecl(EnumDecl {
        name: name.to_string(),
        generic_params: generics
            .into_iter()
            .map(|g| GenericParam {
                name: g.to_string(),
                bounds: vec![],
                span: S,
            })
            .collect(),
        variants: variants
            .into_iter()
            .map(|(n, kind)| EnumVariant {
                name: n.to_string(),
                kind,
                doc: None,
                span: S,
            })
            .collect(),
        is_public: false,
        doc: None,
        span: S,
    })
}

fn trait_decl(name: &str, methods: Vec<TraitMethod>) -> Stmt {
    Stmt::TraitDecl(TraitDecl {
        name: name.to_string(),
        generic_params: vec![],
        methods,
        is_public: false,
        doc: None,
        span: S,
    })
}

fn trait_method(
    name: &str,
    params: Vec<(&str, Option<TypeExpr>)>,
    ret: Option<TypeExpr>,
) -> TraitMethod {
    TraitMethod {
        name: name.to_string(),
        generic_params: vec![],
        params: params
            .into_iter()
            .map(|(n, te)| TypedParam {
                name: n.to_string(),
                type_expr: te,
                span: S,
            })
            .collect(),
        return_type: ret,
        default_body: None,
        doc: None,
        span: S,
    }
}

fn impl_block(self_type: TypeExpr, trait_name: Option<TypeExpr>, methods: Vec<FuncDef>) -> Stmt {
    Stmt::ImplBlock(ImplBlock {
        trait_name,
        self_type,
        generic_params: vec![],
        methods,
        doc: None,
        span: S,
    })
}

fn method_def(
    name: &str,
    params: Vec<(&str, Option<TypeExpr>)>,
    ret: Option<TypeExpr>,
    body: Vec<Stmt>,
) -> FuncDef {
    FuncDef {
        name: name.to_string(),
        params: params
            .into_iter()
            .map(|(n, te)| TypedParam {
                name: n.to_string(),
                type_expr: te,
                span: S,
            })
            .collect(),
        generic_params: vec![],
        return_type: ret,
        body: BlockStmt {
            statements: body,
            span: S,
        },
        is_public: false,
        doc: None,
        span: S,
    }
}

fn match_expr(scrutinee: Expr, arms: Vec<MatchArm>) -> Expr {
    Expr::Match(MatchExpr {
        scrutinee: Box::new(scrutinee),
        arms,
        span: S,
    })
}

fn match_arm(pattern: Pattern, body: Expr) -> MatchArm {
    MatchArm {
        pattern,
        guard: None,
        body,
        span: S,
    }
}

fn struct_lit(name: &str, fields: Vec<(&str, Expr)>) -> Expr {
    Expr::StructLit(StructLitExpr {
        name: name.to_string(),
        fields: fields
            .into_iter()
            .map(|(n, e)| (n.to_string(), e))
            .collect(),
        base: None,
        span: S,
    })
}

fn path_expr(segments: Vec<&str>) -> Expr {
    Expr::PathExpr(PathExpr {
        segments: segments.into_iter().map(|s| s.to_string()).collect(),
        span: S,
    })
}

/// Run the type checker on a program and return errors.
fn check(stmts: Vec<Stmt>) -> Vec<String> {
    let mut program = Program { statements: stmts };
    TypeChecker::new()
        .check_program(&mut program)
        .into_iter()
        .map(|e| e.kind.to_string())
        .collect()
}

/// Run the type checker expecting no errors.
fn check_ok(stmts: Vec<Stmt>) {
    let errs = check(stmts);
    assert!(errs.is_empty(), "expected no errors, got: {errs:?}");
}

#[test]
fn infer_literals() {
    check_ok(vec![let_stmt("x", i64_expr(42), None)]);
    check_ok(vec![let_stmt("x", bool_expr(true), None)]);
    check_ok(vec![let_stmt("x", str_expr("hello"), None)]);

    check_ok(vec![let_stmt("x", i64_expr(10), Some(named_type("i64")))]);
    check_ok(vec![let_stmt(
        "x",
        int_expr(5, NumKind::I8),
        Some(named_type("i8")),
    )]);
}

#[test]
fn infer_binary_expressions() {
    // same types
    check_ok(vec![let_stmt(
        "x",
        infix(i64_expr(1), "+", i64_expr(2)),
        None,
    )]);

    // i8 + i16 is a type mismatch (no implicit promotion)
    {
        let errs = check(vec![let_stmt(
            "x",
            infix(int_expr(1, NumKind::I8), "+", int_expr(2, NumKind::I16)),
            None,
        )]);
        assert!(!errs.is_empty());
    }

    // comparison should return bool
    check_ok(vec![let_stmt(
        "x",
        infix(i64_expr(1), "<", i64_expr(2)),
        Some(named_type("bool")),
    )]);

    // equality check
    check_ok(vec![let_stmt(
        "x",
        infix(bool_expr(true), "==", bool_expr(false)),
        None,
    )]);

    // string concat
    check_ok(vec![let_stmt(
        "x",
        infix(str_expr("a"), "+", str_expr("b")),
        None,
    )]);

    // logical AND
    check_ok(vec![let_stmt(
        "x",
        infix(bool_expr(true), "&&", bool_expr(false)),
        None,
    )]);

    // logical OR requires bool
    let errs = check(vec![let_stmt(
        "x",
        infix(i64_expr(1), "||", bool_expr(true)),
        None,
    )]);
    assert!(!errs.is_empty());
    assert!(errs[0].contains("bool"));
}

#[test]
fn infer_unary_expressions() {
    check_ok(vec![let_stmt("x", prefix("-", i64_expr(5)), None)]);
    check_ok(vec![let_stmt("x", prefix("!", bool_expr(true)), None)]);

    // unary NOT rejects non-bool
    let errs = check(vec![let_stmt("x", prefix("!", i64_expr(1)), None)]);
    assert!(!errs.is_empty());
    assert!(errs[0].contains("bool"));

    // unary negation rejects string
    let errs = check(vec![let_stmt("x", prefix("-", str_expr("a")), None)]);
    assert!(!errs.is_empty());
    assert!(errs[0].contains("numeric"));
}

#[test]
fn infer_conditionals() {
    // unify if-else branch
    check_ok(vec![let_stmt(
        "x",
        cond(
            bool_expr(true),
            vec![expr_stmt(i64_expr(1))],
            Some(vec![expr_stmt(i64_expr(2))]),
        ),
        None,
    )]);

    // if-else branch mismatch
    let errs = check(vec![let_stmt(
        "x",
        cond(
            bool_expr(true),
            vec![expr_stmt(i64_expr(1))],
            Some(vec![expr_stmt(bool_expr(false))]),
        ),
        None,
    )]);
    assert!(!errs.is_empty());

    // if condition must be bool
    let errs = check(vec![let_stmt(
        "x",
        cond(i64_expr(1), vec![expr_stmt(i64_expr(2))], None),
        None,
    )]);
    assert!(!errs.is_empty());
    assert!(errs[0].contains("bool"));
}

#[test]
fn infer_function_and_method_calls() {
    // fn add(a: i64, b: i64) -> i64 { a }
    // let x = add(1, 2);
    check_ok(vec![
        func_def(
            "add",
            vec![
                ("a", Some(named_type("i64"))),
                ("b", Some(named_type("i64"))),
            ],
            Some(named_type("i64")),
            vec![expr_stmt(ident_expr("a"))],
        ),
        let_stmt(
            "x",
            call(ident_expr("add"), vec![i64_expr(1), i64_expr(2)]),
            None,
        ),
    ]);

    // wrong arity
    let errs = check(vec![
        func_def(
            "f",
            vec![("a", Some(named_type("i64")))],
            Some(named_type("i64")),
            vec![expr_stmt(ident_expr("a"))],
        ),
        let_stmt(
            "x",
            call(ident_expr("f"), vec![i64_expr(1), i64_expr(2)]),
            None,
        ),
    ]);
    assert!(!errs.is_empty());

    // method call on `Vector`
    // let v = [1, 2, 3];
    // let n = v.len();
    check_ok(vec![
        let_stmt(
            "v",
            Expr::Vector(Vector {
                elements: vec![i64_expr(1), i64_expr(2), i64_expr(3)],
                span: S,
            }),
            None,
        ),
        let_stmt("n", method_call(ident_expr("v"), "len", vec![]), None),
    ]);

    // lambda/closure
    // let f = |x: i64| -> i64 { x };
    check_ok(vec![let_stmt(
        "f",
        lambda(
            vec![("x", Some(named_type("i64")))],
            vec![expr_stmt(ident_expr("x"))],
        ),
        None,
    )]);

    // lambda call
    // let f = |x: i64| -> i64 { x };
    // let r = f(42);
    check_ok(vec![
        let_stmt(
            "f",
            lambda(
                vec![("x", Some(named_type("i64")))],
                vec![expr_stmt(ident_expr("x"))],
            ),
            None,
        ),
        let_stmt("r", call(ident_expr("f"), vec![i64_expr(42)]), None),
    ]);
}

#[test]
fn exhaustive_enum_all_variants() {
    // enum Color { Red, Green, Blue }
    // match c { Red => 1, Green => 2, Blue => 3 }
    check_ok(vec![
        enum_decl(
            "Color",
            vec![
                ("Red", EnumVariantKind::Unit),
                ("Green", EnumVariantKind::Unit),
                ("Blue", EnumVariantKind::Unit),
            ],
        ),
        let_stmt("c", path_expr(vec!["Color", "Red"]), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("c"),
                vec![
                    match_arm(
                        Pattern::Ident {
                            name: "Red".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(1),
                    ),
                    match_arm(
                        Pattern::Ident {
                            name: "Green".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(2),
                    ),
                    match_arm(
                        Pattern::Ident {
                            name: "Blue".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(3),
                    ),
                ],
            ),
            None,
        ),
    ]);
}

#[test]
fn non_exhaustive_enum_missing_variant() {
    let errs = check(vec![
        enum_decl(
            "Color",
            vec![
                ("Red", EnumVariantKind::Unit),
                ("Green", EnumVariantKind::Unit),
                ("Blue", EnumVariantKind::Unit),
            ],
        ),
        let_stmt("c", path_expr(vec!["Color", "Red"]), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("c"),
                vec![
                    match_arm(
                        Pattern::Ident {
                            name: "Red".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(1),
                    ),
                    match_arm(
                        Pattern::Ident {
                            name: "Green".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(2),
                    ),
                ],
            ),
            None,
        ),
    ]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("Blue")));
}

#[test]
fn exhaustive_bool_true_false() {
    check_ok(vec![
        let_stmt("b", bool_expr(true), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("b"),
                vec![
                    match_arm(Pattern::Literal(Box::new(bool_expr(true))), i64_expr(1)),
                    match_arm(Pattern::Literal(Box::new(bool_expr(false))), i64_expr(0)),
                ],
            ),
            None,
        ),
    ]);
}

#[test]
fn non_exhaustive_bool_missing_false() {
    let errs = check(vec![
        let_stmt("b", bool_expr(true), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("b"),
                vec![match_arm(
                    Pattern::Literal(Box::new(bool_expr(true))),
                    i64_expr(1),
                )],
            ),
            None,
        ),
    ]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("boolean")));
}

#[test]
fn wildcard_makes_match_exhaustive() {
    check_ok(vec![
        let_stmt("x", i64_expr(5), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("x"),
                vec![
                    match_arm(Pattern::Literal(Box::new(i64_expr(1))), str_expr("one")),
                    match_arm(Pattern::Wildcard(S), str_expr("other")),
                ],
            ),
            None,
        ),
    ]);
}

#[test]
fn non_exhaustive_integer_without_wildcard() {
    let errs = check(vec![
        let_stmt("x", i64_expr(5), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("x"),
                vec![match_arm(
                    Pattern::Literal(Box::new(i64_expr(1))),
                    str_expr("one"),
                )],
            ),
            None,
        ),
    ]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("wildcard")));
}

#[test]
fn tuple_struct_destructuring() {
    // enum Wrapper { Val(i64) }
    // let w = Wrapper::Val(42);
    // match w { Val(x) => x }
    check_ok(vec![
        generic_enum_decl(
            "Wrapper",
            vec![],
            vec![("Val", EnumVariantKind::Tuple(vec![named_type("i64")]))],
        ),
        let_stmt(
            "w",
            call(path_expr(vec!["Wrapper", "Val"]), vec![i64_expr(42)]),
            None,
        ),
        let_stmt(
            "r",
            match_expr(
                ident_expr("w"),
                vec![match_arm(
                    Pattern::TupleStruct {
                        path: "Val".to_string(),
                        fields: vec![Pattern::Ident {
                            name: "x".to_string(),
                            mutable: false,
                            span: S,
                        }],
                        span: S,
                    },
                    ident_expr("x"),
                )],
            ),
            None,
        ),
    ]);
}

#[test]
fn enum_wildcard_covers_missing() {
    check_ok(vec![
        enum_decl(
            "Dir",
            vec![
                ("Up", EnumVariantKind::Unit),
                ("Down", EnumVariantKind::Unit),
                ("Left", EnumVariantKind::Unit),
                ("Right", EnumVariantKind::Unit),
            ],
        ),
        let_stmt("d", path_expr(vec!["Dir", "Up"]), None),
        let_stmt(
            "r",
            match_expr(
                ident_expr("d"),
                vec![
                    match_arm(
                        Pattern::Ident {
                            name: "Up".to_string(),
                            mutable: false,
                            span: S,
                        },
                        i64_expr(1),
                    ),
                    match_arm(Pattern::Wildcard(S), i64_expr(0)),
                ],
            ),
            None,
        ),
    ]);
}

#[test]
fn check_struct_literals() {
    // valid field
    check_ok(vec![
        struct_decl(
            "Point",
            vec![("x", named_type("i64")), ("y", named_type("i64"))],
        ),
        let_stmt(
            "p",
            struct_lit("Point", vec![("x", i64_expr(1)), ("y", i64_expr(2))]),
            None,
        ),
    ]);

    // field type mismatch
    let errs = check(vec![
        struct_decl(
            "Point",
            vec![("x", named_type("i64")), ("y", named_type("i64"))],
        ),
        let_stmt(
            "p",
            struct_lit("Point", vec![("x", bool_expr(true)), ("y", i64_expr(2))]),
            None,
        ),
    ]);
    assert!(!errs.is_empty());

    // missing field
    let errs = check(vec![
        struct_decl(
            "Point",
            vec![("x", named_type("i64")), ("y", named_type("i64"))],
        ),
        let_stmt("p", struct_lit("Point", vec![("x", i64_expr(1))]), None),
    ]);
    assert!(!errs.is_empty());

    // extra field
    let errs = check(vec![
        struct_decl(
            "Point",
            vec![("x", named_type("i64")), ("y", named_type("i64"))],
        ),
        let_stmt(
            "p",
            struct_lit(
                "Point",
                vec![("x", i64_expr(1)), ("y", i64_expr(2)), ("z", i64_expr(3))],
            ),
            None,
        ),
    ]);
    assert!(!errs.is_empty());

    // unknown type
    let errs = check(vec![let_stmt(
        "p",
        struct_lit("Nonexistent", vec![("x", i64_expr(1))]),
        None,
    )]);
    assert!(!errs.is_empty());
}

#[test]
fn generic_struct_instantiation() {
    // struct Pair<T> { first: T, second: T }
    // let p = Pair { first: 1, second: 2 };
    check_ok(vec![
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![("first", named_type("T")), ("second", named_type("T"))],
        ),
        let_stmt(
            "p",
            struct_lit(
                "Pair",
                vec![("first", i64_expr(1)), ("second", i64_expr(2))],
            ),
            None,
        ),
    ]);
}

#[test]
fn generic_struct_field_type_mismatch() {
    // struct Pair<T> { first: T, second: T }
    // let p = Pair { first: 1, second: true };  <-- T cannot unify i64 and bool
    let errs = check(vec![
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![("first", named_type("T")), ("second", named_type("T"))],
        ),
        let_stmt(
            "p",
            struct_lit(
                "Pair",
                vec![("first", i64_expr(1)), ("second", bool_expr(true))],
            ),
            None,
        ),
    ]);
    assert!(!errs.is_empty());
}

#[test]
fn mixed_integer_types_rejected() {
    // i8 + i32 is a type mismatch
    let errs = check(vec![let_stmt(
        "x",
        infix(int_expr(1, NumKind::I8), "+", int_expr(2, NumKind::I32)),
        None,
    )]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("i8") && e.contains("i32")));
}

#[test]
fn mixed_sign_integers_rejected() {
    // u8 + i8 is a type mismatch
    let errs = check(vec![let_stmt(
        "x",
        infix(int_expr(1, NumKind::U8), "+", int_expr(2, NumKind::I8)),
        None,
    )]);
    assert!(!errs.is_empty());
}

#[test]
fn mixed_integer_no_cast_insertion() {
    // Verify that mismatched integers produce errors, not Cast nodes.
    let mut program = Program {
        statements: vec![let_stmt(
            "x",
            infix(int_expr(1, NumKind::I8), "+", int_expr(2, NumKind::I16)),
            None,
        )],
    };
    let errs = TypeChecker::new().check_program(&mut program);
    assert!(!errs.is_empty(), "expected type errors for i8 + i16");
    if let Stmt::Let(ref ls) = program.statements[0]
        && let Expr::Infix(ref inf) = ls.value
    {
        assert!(
            !matches!(inf.lhs.as_ref(), Expr::Cast(_)),
            "should not insert Cast node for mismatched types",
        );
    }
}

#[test]
fn mixed_width_u64_i128_rejected() {
    // u64 + i128 is a type mismatch
    let errs = check(vec![let_stmt(
        "x",
        infix(int_expr(1, NumKind::U64), "+", int_expr(2, NumKind::I128)),
        None,
    )]);
    assert!(!errs.is_empty());
}

#[test]
fn mixed_width_comparison_rejected() {
    // u8 < i16 is a type mismatch
    let errs = check(vec![let_stmt(
        "x",
        infix(int_expr(1, NumKind::U8), "<", int_expr(2, NumKind::I16)),
        None,
    )]);
    assert!(!errs.is_empty());
}

#[test]
fn impl_self_type_resolution() {
    // struct Counter { val: i64 }
    // impl Counter { fn get(self) -> i64 { self.val } }
    check_ok(vec![
        struct_decl("Counter", vec![("val", named_type("i64"))]),
        impl_block(
            named_type("Counter"),
            None,
            vec![method_def(
                "get",
                vec![("self", None)],
                Some(named_type("i64")),
                vec![expr_stmt(Expr::FieldAccess(FieldAccessExpr {
                    object: Box::new(ident_expr("self")),
                    field: "val".to_string(),
                    span: S,
                }))],
            )],
        ),
    ]);
}

#[test]
fn trait_method_signature_conformance() {
    // trait Greet { fn greet(self) -> str; }
    // struct Bot {}
    // impl Greet for Bot { fn greet(self) -> str { "hi" } }
    check_ok(vec![
        trait_decl(
            "Greet",
            vec![trait_method(
                "greet",
                vec![("self", None)],
                Some(named_type("str")),
            )],
        ),
        struct_decl("Bot", vec![]),
        impl_block(
            named_type("Bot"),
            Some(named_type("Greet")),
            vec![method_def(
                "greet",
                vec![("self", None)],
                Some(named_type("str")),
                vec![expr_stmt(str_expr("hi"))],
            )],
        ),
    ]);
}

#[test]
fn trait_missing_method() {
    // trait Greet { fn greet(self) -> str; }
    // struct Bot {}
    // impl Greet for Bot {}   <-- missing greet
    let errs = check(vec![
        trait_decl(
            "Greet",
            vec![trait_method(
                "greet",
                vec![("self", None)],
                Some(named_type("str")),
            )],
        ),
        struct_decl("Bot", vec![]),
        impl_block(named_type("Bot"), Some(named_type("Greet")), vec![]),
    ]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("greet")));
}

#[test]
fn trait_method_signature_mismatch() {
    // trait Greet { fn greet(self) -> str; }
    // struct Bot {}
    // impl Greet for Bot { fn greet(self) -> i64 { 1 } }
    let errs = check(vec![
        trait_decl(
            "Greet",
            vec![trait_method(
                "greet",
                vec![("self", None)],
                Some(named_type("str")),
            )],
        ),
        struct_decl("Bot", vec![]),
        impl_block(
            named_type("Bot"),
            Some(named_type("Greet")),
            vec![method_def(
                "greet",
                vec![("self", None)],
                Some(named_type("i64")),
                vec![expr_stmt(i64_expr(1))],
            )],
        ),
    ]);
    assert!(!errs.is_empty());
}

#[test]
fn impl_unknown_trait() {
    let errs = check(vec![
        struct_decl("Bot", vec![]),
        impl_block(named_type("Bot"), Some(named_type("Nonexistent")), vec![]),
    ]);
    assert!(!errs.is_empty());
}

#[test]
fn impl_unknown_self_type() {
    let errs = check(vec![impl_block(
        named_type("Nonexistent"),
        None,
        vec![method_def(
            "foo",
            vec![("self", None)],
            Some(named_type("i64")),
            vec![],
        )],
    )]);
    assert!(!errs.is_empty());
    assert!(errs.iter().any(|e| e.contains("Nonexistent")));
}

#[test]
fn numeric_overflow_detected() {
    // let x: i8 = 200;  <-- overflow
    let errs = check(vec![let_stmt("x", i64_expr(200), Some(named_type("i8")))]);
    assert!(!errs.is_empty());
    assert!(
        errs.iter()
            .any(|e| e.contains("overflow") || e.contains("200"))
    );
}

#[test]
fn not_callable() {
    let errs = check(vec![
        let_stmt("x", i64_expr(5), None),
        let_stmt("r", call(ident_expr("x"), vec![i64_expr(1)]), None),
    ]);
    assert!(!errs.is_empty());
}

#[test]
fn duplicate_struct_type() {
    let errs = check(vec![struct_decl("Foo", vec![]), struct_decl("Foo", vec![])]);
    assert!(!errs.is_empty());
}

#[test]
fn duplicate_enum_type() {
    let errs = check(vec![
        enum_decl("Color", vec![("Red", EnumVariantKind::Unit)]),
        enum_decl("Color", vec![("Blue", EnumVariantKind::Unit)]),
    ]);
    assert!(!errs.is_empty());
}

#[test]
fn for_loop_infers_element_type() {
    check_ok(vec![Stmt::For(ForStmt {
        ident: "x".to_string(),
        pattern: None,
        iterable: Box::new(Expr::Vector(Vector {
            elements: vec![i64_expr(1), i64_expr(2)],
            span: S,
        })),
        body: BlockStmt {
            statements: vec![expr_stmt(ident_expr("x"))],
            span: S,
        },
        label: None,
        span: S,
    })]);
}

#[test]
fn while_condition_must_be_bool() {
    let errs = check(vec![Stmt::While(WhileStmt {
        condition: Box::new(i64_expr(1)),
        body: BlockStmt {
            statements: vec![],
            span: S,
        },
        label: None,
        span: S,
    })]);
    assert!(!errs.is_empty());
    assert!(errs[0].contains("bool"));
}

#[test]
fn generic_enum_option_instantiation() {
    // enum MyOption<T> { Some(T), None }
    // let x = MyOption::Some(42);
    check_ok(vec![
        generic_enum_decl(
            "MyOption",
            vec!["T"],
            vec![
                ("Some", EnumVariantKind::Tuple(vec![named_type("T")])),
                ("None", EnumVariantKind::Unit),
            ],
        ),
        let_stmt(
            "x",
            call(path_expr(vec!["MyOption", "Some"]), vec![i64_expr(42)]),
            None,
        ),
    ]);
}
