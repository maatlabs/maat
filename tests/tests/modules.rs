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
    let mut graph = resolve_module_graph(&dir.path().join("main.mt"))?;
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
    let result = compile_project(&[("main.mt", "let x: i64 = 42;")]);
    assert!(result.is_ok());
}

#[test]
fn import_pub_function() {
    let result = compile_project(&[
        (
            "main.mt",
            "mod math;\nuse math::add;\nlet result: i64 = add(1, 2);",
        ),
        ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
}

#[test]
fn import_pub_function_executes() {
    let result = run_project(&[
        ("main.mt", "mod math;\nuse math::add;\nadd(10, 32)"),
        ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert_eq!(result, Object::I64(42));
}

#[test]
fn try_import_private_function() {
    let result = compile_project(&[
        (
            "main.mt",
            "mod math;\nuse math::secret;\nlet x: i64 = secret();",
        ),
        ("math.mt", "fn secret() -> i64 { 42 }"),
    ]);
    assert!(result.is_err());
}

#[test]
fn import_grouped_pub_items() {
    let result = run_project(&[
        (
            "main.mt",
            "mod math;\nuse math::{add, sub};\nadd(1, 2) + sub(5, 3)",
        ),
        (
            "math.mt",
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
        ("main.mt", "mod math;\nuse math;\nlet x: i64 = add(1, 2);"),
        ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
    ]);
    assert!(result.is_err());
}

#[test]
fn import_specific_items_from_group() {
    let result = compile_project(&[
        (
            "main.mt",
            "mod math;\nuse math::{add, sub};\nlet x: i64 = add(1, 2);\nlet y: i64 = sub(5, 3);",
        ),
        (
            "math.mt",
            "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }\nfn internal() -> i64 { 0 }",
        ),
    ]);
    assert!(result.is_ok());
}

#[test]
fn import_pub_struct() {
    let result = run_project(&[
        (
            "main.mt",
            "mod types;\nuse types::Point;\nlet p = Point { x: 10, y: 20 };\np.x + p.y",
        ),
        (
            "types.mt",
            "pub struct Point {\n    pub x: i64,\n    pub y: i64,\n}",
        ),
    ]);
    assert_eq!(result, Object::I64(30));
}

#[test]
fn import_pub_enum() {
    let result = compile_project(&[
        (
            "main.mt",
            "mod types;\nuse types::Color;\nlet c = Color::Red;",
        ),
        (
            "types.mt",
            "pub enum Color {\n    Red,\n    Green,\n    Blue,\n}",
        ),
    ]);
    assert!(result.is_ok());
}

#[test]
fn type_error_in_dependency_module() {
    let result = compile_project(&[
        ("main.mt", "mod bad;"),
        ("bad.mt", "pub fn broken() -> i64 { true }"),
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
            "main.mt",
            "mod a;\nmod b;\nuse a::from_a;\nuse b::from_b;\nfrom_a() + from_b()",
        ),
        ("a.mt", "pub fn from_a() -> i64 { 1 }"),
        ("b.mt", "pub fn from_b() -> i64 { 2 }"),
    ]);
    assert_eq!(result, Object::I64(3));
}

#[test]
fn reexport_pub_use() {
    let result = run_project(&[
        ("main.mt", "mod facade;\nuse facade::helper;\nhelper()"),
        ("facade.mt", "mod utils;\npub use utils::helper;"),
        ("facade/utils.mt", "pub fn helper() -> i64 { 42 }"),
    ]);
    assert_eq!(result, Object::I64(42));
}

#[test]
fn exports_only_pub_items() {
    let dir = setup_temp_project(&[
        ("main.mt", "mod lib;"),
        (
            "lib.mt",
            "pub fn visible() -> i64 { 1 }\nfn hidden() -> i64 { 2 }",
        ),
    ]);
    let mut graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
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
        ("main.mt", "mod shapes;"),
        (
            "shapes.mt",
            "pub struct Circle {\n    pub radius: i64,\n}\n\nimpl Circle {\n    pub fn area(self) -> i64 { self.radius }\n    fn secret(self) -> i64 { self.radius }\n}",
        ),
    ]);
    let mut graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
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
        ("main.mt", "mod outer;\nuse outer::greet;\ngreet()"),
        (
            "outer.mt",
            "mod inner;\nuse inner::value;\npub fn greet() -> i64 { value() }",
        ),
        ("outer/inner.mt", "pub fn value() -> i64 { 99 }"),
    ]);
    assert_eq!(result, Object::I64(99));
}

#[test]
fn linked_bytecode_serialization_roundtrip() {
    let bytecode = compile_project(&[
        ("main.mt", "mod math;\nuse math::add;\nadd(10, 32)"),
        ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
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
            "main.mt",
            "mod geo;\nuse geo::Point;\nlet p = Point { x: 3, y: 4 };\np.sum()",
        ),
        (
            "geo.mt",
            "pub struct Point {\n    pub x: i64,\n    pub y: i64,\n}\n\nimpl Point {\n    pub fn sum(self) -> i64 { self.x + self.y }\n}",
        ),
    ]);
    assert_eq!(result, Object::I64(7));
}

#[test]
fn multiple_modules_with_internal_state() {
    let result = run_project(&[
        (
            "main.mt",
            "mod counter;\nuse counter::make_value;\nmake_value()",
        ),
        (
            "counter.mt",
            "let base: i64 = 100;\npub fn make_value() -> i64 { base + 42 }",
        ),
    ]);
    assert_eq!(result, Object::I64(142));
}
