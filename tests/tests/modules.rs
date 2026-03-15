use std::fs;

use maat_bytecode::Bytecode;
use maat_module::{ModuleId, ModuleResult, check_and_compile, check_exports, resolve_module_graph};
use maat_runtime::Object;
use maat_vm::VM;

/// Creates a temporary directory tree from a list of `(relative_path, content)` pairs.
fn setup_temp_project(pairs: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    for (path, content) in pairs {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("failed to create directory");
        }
        fs::write(&full, content).expect("failed to write file");
    }
    dir
}

/// Resolves and compiles a multi-file project, returning linked bytecode.
fn compile_project(pairs: &[(&str, &str)]) -> ModuleResult<Bytecode> {
    let dir = setup_temp_project(pairs);
    let mut graph = resolve_module_graph(&dir.path().join("main.maat"))?;
    check_and_compile(&mut graph)
}

/// Compiles and runs a multi-file project, returning the VM's last result.
fn run_project(pairs: &[(&str, &str)]) -> Object {
    let bytecode = compile_project(pairs).expect("compilation failed");
    let mut vm = VM::new(bytecode);
    vm.run().expect("vm error");
    vm.last_popped_stack_elem().cloned().unwrap_or(Object::Null)
}

#[test]
fn single_module_compiles() {
    let result = compile_project(&[("main.maat", "let x: i64 = 42;")]);
    assert!(result.is_ok());
}

#[test]
fn import_pub_function() {
    let result = compile_project(&[
        (
            "main.maat",
            "mod math;\nuse math::add;\nlet result: i64 = add(1, 2);",
        ),
        ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
}

#[test]
fn import_pub_function_executes() {
    let result = run_project(&[
        ("main.maat", "mod math;\nuse math::add;\nadd(10, 32)"),
        ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert_eq!(result, Object::I64(42));
}

#[test]
fn try_import_private_function() {
    let result = compile_project(&[
        (
            "main.maat",
            "mod math;\nuse math::secret;\nlet x: i64 = secret();",
        ),
        ("math.maat", "fn secret() -> i64 { 42 }"),
    ]);
    assert!(result.is_err());
}

#[test]
fn import_grouped_pub_items() {
    let result = run_project(&[
        (
            "main.maat",
            "mod math;\nuse math::{add, sub};\nadd(1, 2) + sub(5, 3)",
        ),
        (
            "math.maat",
            "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }",
        ),
    ]);
    assert_eq!(result, Object::I64(5));
}

#[test]
fn bare_use_module_is_noop() {
    // `use math;` without qualified path is silently ignored.
    // Items remain inaccessible without explicit import paths.
    let result = compile_project(&[
        ("main.maat", "mod math;\nuse math;\nlet x: i64 = add(1, 2);"),
        ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert!(result.is_err());
}

#[test]
fn import_specific_items_from_group() {
    let result = compile_project(&[
        (
            "main.maat",
            "mod math;\nuse math::{add, sub};\nlet x: i64 = add(1, 2);\nlet y: i64 = sub(5, 3);",
        ),
        (
            "math.maat",
            "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }\nfn internal() -> i64 { 0 }",
        ),
    ]);
    assert!(result.is_ok());
}

#[test]
fn import_pub_struct() {
    let result = run_project(&[
        (
            "main.maat",
            "mod types;\nuse types::Point;\nlet p = Point { x: 10, y: 20 };\np.x + p.y",
        ),
        (
            "types.maat",
            "pub struct Point {\n    pub x: i64,\n    pub y: i64,\n}",
        ),
    ]);
    assert_eq!(result, Object::I64(30));
}

#[test]
fn import_pub_enum() {
    let result = compile_project(&[
        (
            "main.maat",
            "mod types;\nuse types::Color;\nlet c = Color::Red;",
        ),
        (
            "types.maat",
            "pub enum Color {\n    Red,\n    Green,\n    Blue,\n}",
        ),
    ]);
    assert!(result.is_ok());
}

#[test]
fn type_error_in_dependency_module() {
    let result = compile_project(&[
        ("main.maat", "mod bad;"),
        ("bad.maat", "pub fn broken() -> i64 { true }"),
    ]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("type error"),
        "expected type error, got: {}",
        err
    );
}

#[test]
fn diamond_dependency_compiles() {
    let result = run_project(&[
        (
            "main.maat",
            "mod a;\nmod b;\nuse a::from_a;\nuse b::from_b;\nfrom_a() + from_b()",
        ),
        ("a.maat", "pub fn from_a() -> i64 { 1 }"),
        ("b.maat", "pub fn from_b() -> i64 { 2 }"),
    ]);
    assert_eq!(result, Object::I64(3));
}

#[test]
fn reexport_pub_use() {
    let result = run_project(&[
        ("main.maat", "mod facade;\nuse facade::helper;\nhelper()"),
        ("facade.maat", "mod utils;\npub use utils::helper;"),
        ("facade/utils.maat", "pub fn helper() -> i64 { 42 }"),
    ]);
    assert_eq!(result, Object::I64(42));
}

#[test]
fn exports_only_pub_items() {
    let dir = setup_temp_project(&[
        ("main.maat", "mod lib;"),
        (
            "lib.maat",
            "pub fn visible() -> i64 { 1 }\nfn hidden() -> i64 { 2 }",
        ),
    ]);
    let mut graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
    let exports = check_exports(&mut graph).unwrap();

    let lib_exports = exports
        .iter()
        .find(|(id, _)| **id != ModuleId::ROOT)
        .map(|(_, e)| e)
        .unwrap();
    assert_eq!(lib_exports.bindings.len(), 1);
    assert_eq!(lib_exports.bindings[0].0, "visible");
}

#[test]
fn impl_blocks_export_only_pub_methods() {
    let dir = setup_temp_project(&[
        ("main.maat", "mod shapes;"),
        (
            "shapes.maat",
            "pub struct Circle {\n    pub radius: i64,\n}\n\nimpl Circle {\n    pub fn area(self) -> i64 { self.radius }\n    fn secret(self) -> i64 { self.radius }\n}",
        ),
    ]);
    let mut graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
    let exports = check_exports(&mut graph).unwrap();
    let shapes_exports = exports
        .iter()
        .find(|(id, _)| **id != ModuleId::ROOT)
        .map(|(_, e)| e)
        .unwrap();
    assert_eq!(shapes_exports.impls.len(), 1);
    let imp = &shapes_exports.impls[0];
    assert_eq!(imp.methods.len(), 1, "only pub methods should be exported");
    assert_eq!(imp.methods[0].0, "area");
}

#[test]
fn nested_module_import() {
    let result = run_project(&[
        ("main.maat", "mod outer;\nuse outer::greet;\ngreet()"),
        (
            "outer.maat",
            "mod inner;\nuse inner::value;\npub fn greet() -> i64 { value() }",
        ),
        ("outer/inner.maat", "pub fn value() -> i64 { 99 }"),
    ]);
    assert_eq!(result, Object::I64(99));
}

#[test]
fn linked_bytecode_serialization_roundtrip() {
    let bytecode = compile_project(&[
        ("main.maat", "mod math;\nuse math::add;\nadd(10, 32)"),
        ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ])
    .expect("compilation failed");
    let serialized = bytecode.serialize().expect("serialization failed");
    let deserialized = Bytecode::deserialize(&serialized).expect("deserialization failed");
    let mut vm = VM::new(deserialized);
    vm.run().expect("vm error");
    assert_eq!(vm.last_popped_stack_elem(), Some(&Object::I64(42)));
}

#[test]
fn module_with_struct_method_call() {
    let result = run_project(&[
        (
            "main.maat",
            "mod geo;\nuse geo::Point;\nlet p = Point { x: 3, y: 4 };\np.sum()",
        ),
        (
            "geo.maat",
            "pub struct Point {\n    pub x: i64,\n    pub y: i64,\n}\n\nimpl Point {\n    pub fn sum(self) -> i64 { self.x + self.y }\n}",
        ),
    ]);
    assert_eq!(result, Object::I64(7));
}

#[test]
fn multiple_modules_with_internal_state() {
    let result = run_project(&[
        (
            "main.maat",
            "mod counter;\nuse counter::make_value;\nmake_value()",
        ),
        (
            "counter.maat",
            "let base: i64 = 100;\npub fn make_value() -> i64 { base + 42 }",
        ),
    ]);
    assert_eq!(result, Object::I64(142));
}

#[test]
fn std_math() {
    // abs()
    let result = run_project(&[("main.maat", "use std::math::abs;\nabs(7)")]);
    assert_eq!(result, Object::I64(7));
    let result = run_project(&[("main.maat", "use std::math::abs;\nabs(-42)")]);
    assert_eq!(result, Object::I64(42));

    // min()
    let result = run_project(&[("main.maat", "use std::math::min;\nmin(3, 7)")]);
    assert_eq!(result, Object::I64(3));
    // max()
    let result = run_project(&[("main.maat", "use std::math::max;\nmax(3, 7)")]);
    assert_eq!(result, Object::I64(7));

    // pow()
    let result = run_project(&[("main.maat", "use std::math::pow;\npow(2, 10)")]);
    assert_eq!(result, Object::I64(1024));
    let result = run_project(&[("main.maat", "use std::math::pow;\npow(5, 0)")]);
    assert_eq!(result, Object::I64(1));

    // gcd()
    let result = run_project(&[("main.maat", "use std::math::gcd;\ngcd(12, 8)")]);
    assert_eq!(result, Object::I64(4));

    // lcm()
    let result = run_project(&[("main.maat", "use std::math::lcm;\nlcm(4, 6)")]);
    assert_eq!(result, Object::I64(12));
}

#[test]
fn std_string() {
    // trim()
    let result = run_project(&[("main.maat", "use std::string::trim;\ntrim(\"  hello  \")")]);
    assert_eq!(result, Object::Str("hello".to_string()));

    // contains()
    let result = run_project(&[(
        "main.maat",
        "use std::string::contains;\ncontains(\"hello world\", \"world\")",
    )]);
    assert_eq!(result, Object::Bool(true));
    let result = run_project(&[(
        "main.maat",
        "use std::string::contains;\ncontains(\"hello\", \"xyz\")",
    )]);
    assert_eq!(result, Object::Bool(false));

    // starts_with()
    let result = run_project(&[(
        "main.maat",
        "use std::string::starts_with;\nstarts_with(\"hello world\", \"hello\")",
    )]);
    assert_eq!(result, Object::Bool(true));

    // ends_with()
    let result = run_project(&[(
        "main.maat",
        "use std::string::ends_with;\nends_with(\"hello world\", \"world\")",
    )]);
    assert_eq!(result, Object::Bool(true));

    // split()
    let result = run_project(&[(
        "main.maat",
        "use std::string::split;\nsplit(\"a,b,c\", \",\")",
    )]);
    assert_eq!(
        result,
        Object::Array(vec![
            Object::Str("a".to_string()),
            Object::Str("b".to_string()),
            Object::Str("c".to_string()),
        ])
    );

    // join()
    let result = run_project(&[(
        "main.maat",
        "use std::string::join;\njoin([\"a\", \"b\", \"c\"], \"-\")",
    )]);
    assert_eq!(result, Object::Str("a-b-c".to_string()));

    // parse_int()
    let result = run_project(&[(
        "main.maat",
        "use std::string::parse_int;\nparse_int(\"42\")",
    )]);
    assert_eq!(result, Object::I64(42));
}

#[test]
fn std_collections_set() {
    // new(), len()
    let result = run_project(&[(
        "main.maat",
        "use std::collections::{new_set, set_len};\nlet s = new_set();\nset_len(s)",
    )]);
    assert_eq!(result, Object::Usize(0));

    // insert(), contains()
    let result = run_project(&[(
        "main.maat",
        "use std::collections::{new_set, set_insert, set_contains};\nlet s = new_set();\nlet s = set_insert(s, 42);\nset_contains(s, 42)",
    )]);
    assert_eq!(result, Object::Bool(true));

    // remove()
    let result = run_project(&[(
        "main.maat",
        "use std::collections::{new_set, set_insert, set_remove, set_contains};\nlet s = new_set();\nlet s = set_insert(s, 1);\nlet s = set_remove(s, 1);\nset_contains(s, 1)",
    )]);
    assert_eq!(result, Object::Bool(false));

    // uniqueness
    let result = run_project(&[(
        "main.maat",
        "use std::collections::{new_set, set_insert, set_len};\nlet s = new_set();\nlet s = set_insert(s, 1);\nlet s = set_insert(s, 1);\nlet s = set_insert(s, 2);\nset_len(s)",
    )]);
    assert_eq!(result, Object::Usize(2));
}

#[test]
fn str_methods() {
    // s.trim()
    let result = run_project(&[("main.maat", "let s: str = \"  hello  \";\ns.trim()")]);
    assert_eq!(result, Object::Str("hello".to_string()));

    // s.contains()
    let result = run_project(&[(
        "main.maat",
        "let s: str = \"hello world\";\ns.contains(\"world\")",
    )]);
    assert_eq!(result, Object::Bool(true));

    // s.starts_with()
    let result = run_project(&[(
        "main.maat",
        "let s: str = \"hello world\";\ns.starts_with(\"hello\")",
    )]);
    assert_eq!(result, Object::Bool(true));

    // s.ends_with()
    let result = run_project(&[(
        "main.maat",
        "let s: str = \"hello world\";\ns.ends_with(\"world\")",
    )]);
    assert_eq!(result, Object::Bool(true));

    // s.split()
    let result = run_project(&[("main.maat", "let s: str = \"a,b,c\";\ns.split(\",\")")]);
    assert_eq!(
        result,
        Object::Array(vec![
            Object::Str("a".to_string()),
            Object::Str("b".to_string()),
            Object::Str("c".to_string()),
        ])
    );

    // s.parse_int()
    let result = run_project(&[("main.maat", "let s: str = \"123\";\ns.parse_int()")]);
    assert_eq!(result, Object::I64(123));
}

#[test]
fn array_join_method() {
    let result = run_project(&[(
        "main.maat",
        "let arr = [\"x\", \"y\", \"z\"];\narr.join(\", \")",
    )]);
    assert_eq!(result, Object::Str("x, y, z".to_string()));
}

#[test]
fn stdlib_combined_with_user_modules() {
    let result = run_project(&[
        (
            "main.maat",
            "mod helpers;\nuse helpers::double;\nuse std::math::abs;\nabs(double(-5))",
        ),
        ("helpers.maat", "pub fn double(x: i64) -> i64 { x * 2 }"),
    ]);
    assert_eq!(result, Object::I64(10));
}
