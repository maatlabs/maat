use maat_ast::*;
use maat_lexer::{Lexer, TokenKind};

fn parse(input: &str) -> Program {
    maat_tests::parse(input)
}

fn expect_single_stmt(program: &Program) -> &Stmt {
    assert_eq!(program.statements.len(), 1);
    &program.statements[0]
}

#[test]
fn lex_module_keywords_and_idents() {
    let input = "mod use pub";
    let mut lexer = Lexer::new(input);

    assert_eq!(lexer.next_token().kind, TokenKind::Mod);
    assert_eq!(lexer.next_token().kind, TokenKind::Use);
    assert_eq!(lexer.next_token().kind, TokenKind::Pub);
    assert_eq!(lexer.next_token().kind, TokenKind::Eof);

    // keywords
    assert_eq!(TokenKind::keyword_or_ident("mod"), TokenKind::Mod);
    assert_eq!(TokenKind::keyword_or_ident("use"), TokenKind::Use);
    assert_eq!(TokenKind::keyword_or_ident("pub"), TokenKind::Pub);

    // regular identifiers
    assert_eq!(TokenKind::keyword_or_ident("module"), TokenKind::Ident);
    assert_eq!(TokenKind::keyword_or_ident("used"), TokenKind::Ident);
    assert_eq!(TokenKind::keyword_or_ident("public"), TokenKind::Ident);
}

#[test]
fn parse_use_statements() {
    // simple paths use
    let program = parse("use foo::bar;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar"]);
    assert!(use_stmt.items.is_none());

    // multiple paths
    let program = parse("use foo::bar::baz::qux;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar", "baz", "qux"]);
    assert!(use_stmt.items.is_none());

    // multiple paths, nested items
    let program = parse("use foo::bar::{baz, qux};");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo", "bar"]);
    assert_eq!(
        use_stmt.items.as_ref().unwrap(),
        &vec!["baz".to_string(), "qux".to_string()]
    );

    // grouped single item
    let program = parse("use math::{abs};");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["math"]);
    assert_eq!(use_stmt.items.as_ref().unwrap(), &vec!["abs".to_string()]);

    // use path directly
    let program = parse("use foo;");
    let Stmt::Use(use_stmt) = expect_single_stmt(&program) else {
        panic!("expected Use statement");
    };
    assert_eq!(use_stmt.path, vec!["foo"]);
    assert!(use_stmt.items.is_none());
}

#[test]
fn parse_mod_stmt() {
    // external module, public
    let program = parse("pub mod math;");
    let Stmt::Mod(mod_stmt) = expect_single_stmt(&program) else {
        panic!("expected Mod statement");
    };
    assert_eq!(mod_stmt.name, "math");
    assert!(mod_stmt.body.is_none());
    assert!(mod_stmt.is_public);

    // inline module, private
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
fn parse_pub_fn() {
    let program = parse("pub fn add(x: i64, y: i64) -> i64 { x + y }");
    let Stmt::FuncDef(func) = expect_single_stmt(&program) else {
        panic!("expected FuncDef statement");
    };
    assert_eq!(func.name, "add");
    assert!(func.is_public);
}

#[test]
fn parse_pub_structs() {
    // public struct, private fields
    let program = parse("pub struct Point { x: i64, y: i64 }");
    let Stmt::StructDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected StructDecl statement");
    };
    assert_eq!(decl.name, "Point");
    assert!(decl.is_public);

    // public struct, mixed visibility for fields
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
fn parse_pub_enum() {
    let program = parse("pub enum Color { Red, Green, Blue }");
    let Stmt::EnumDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected EnumDecl statement");
    };
    assert_eq!(decl.name, "Color");
    assert!(decl.is_public);
}

#[test]
fn parse_pub_trait() {
    let program = parse("pub trait Display { fn show(self) -> i64; }");
    let Stmt::TraitDecl(decl) = expect_single_stmt(&program) else {
        panic!("expected TraitDecl statement");
    };
    assert_eq!(decl.name, "Display");
    assert!(decl.is_public);
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
