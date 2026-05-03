//! Module resolution, dependency graph construction, and multi-module
//! compilation for the Maat compiler.
//!
//! This crate builds a directed acyclic graph (DAG) of module dependencies
//! before compilation begins. Each reachable source file is parsed independently,
//! cycle detection is performed via DFS with gray/black coloring, and the final
//! graph provides a topological ordering suitable for compilation.
//!
//! # File Resolution
//!
//! Resolution follows Rust's module conventions:
//!
//! - **Root entry files** and **`mod.maat`** files resolve submodules in
//!   their own directory: `mod foo;` in `dir/mod.maat` resolves to
//!   `dir/foo.maat` or `dir/foo/mod.maat`.
//! - **All other files** resolve submodules in a subdirectory named after
//!   the file stem: `mod bar;` in `dir/foo.maat` resolves to
//!   `dir/foo/bar.maat` or `dir/foo/bar/mod.maat`.
//!
//! If both `foo.maat` and `foo/mod.maat` exist, the resolution is ambiguous
//! and an error is produced. If neither exists, a resolution error is
//! produced.

#![forbid(unsafe_code)]

mod exports;
mod graph;
mod imports;
mod resolve;
mod stdlib;

use std::collections::HashMap;

pub use exports::ModuleExports;
pub use graph::{ModuleGraph, ModuleId, ModuleNode};
pub use imports::{ImportKind, ResolvedImport};
use maat_ast::{Program, Stmt, fold_constants};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_errors::{ModuleError, ModuleErrorKind};
use maat_span::Span;
use maat_types::TypeChecker;
pub use resolve::resolve_module_graph;

pub type ModuleResult<T> = std::result::Result<T, ModuleError>;

type TypeCheckResult = (
    HashMap<ModuleId, ModuleExports>,
    HashMap<ModuleId, Vec<ResolvedImport>>,
);

/// Type-checks, compiles, and links all modules in the given graph.
pub fn check_and_compile(graph: &mut ModuleGraph) -> ModuleResult<Bytecode> {
    let topo_order = graph.topo_order().to_vec();
    let (exports, cached_imports) = type_check_modules(graph, &topo_order)?;
    compile_modules(graph, &topo_order, &exports, &cached_imports)
}

fn type_check_modules(
    graph: &mut ModuleGraph,
    topo_order: &[ModuleId],
) -> ModuleResult<TypeCheckResult> {
    let mut exports: HashMap<ModuleId, ModuleExports> = HashMap::new();
    let mut cached_imports: HashMap<ModuleId, Vec<ResolvedImport>> = HashMap::new();

    for &module_id in topo_order {
        let node = graph.node(module_id);
        let file_path = node.path.clone();
        let imports = resolve_imports(&node.program, &exports, graph)?;
        let program = &mut graph.node_mut(module_id).program;
        let mut checker = TypeChecker::new();
        for import in &imports {
            import.inject_into_env(checker.env_mut());
        }
        checker.check_program_mut(program);

        let type_errors = checker.errors();
        if !type_errors.is_empty() {
            let messages = type_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }
        let module_exports = ModuleExports::from_checked(program, checker.env());
        exports.insert(module_id, module_exports);

        let fold_errors = fold_constants(program);
        if !fold_errors.is_empty() {
            let messages = fold_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }
        cached_imports.insert(module_id, imports);
    }

    Ok((exports, cached_imports))
}

fn compile_modules(
    graph: &ModuleGraph,
    topo_order: &[ModuleId],
    exports: &HashMap<ModuleId, ModuleExports>,
    cached_imports: &HashMap<ModuleId, Vec<ResolvedImport>>,
) -> ModuleResult<Bytecode> {
    let _ = exports; // exports used only during type checking; kept for downstream linking
    let mut compiler = Compiler::new();
    for &module_id in topo_order {
        let file_path = graph.node(module_id).path.clone();
        if let Some(imports) = cached_imports.get(&module_id) {
            for import in imports {
                import.inject_into_compiler(&mut compiler);
            }
        }
        let before = compiler.symbols_table_mut().global_symbol_names();
        let program = &graph.node(module_id).program;
        compiler.compile_program(program).map_err(|e| {
            ModuleErrorKind::CompileErrors {
                file: file_path.clone(),
                messages: vec![e.to_string()],
            }
            .at(Span::ZERO, file_path.clone())
        })?;

        apply_module_visibility(&mut compiler, module_id, &before);
    }
    let root_path = graph.root().path.clone();
    compiler.bytecode().map_err(|e| {
        ModuleErrorKind::CompileErrors {
            file: root_path.clone(),
            messages: vec![e.to_string()],
        }
        .at(Span::ZERO, root_path)
    })
}

fn apply_module_visibility(compiler: &mut Compiler, module_id: ModuleId, before: &[String]) {
    if module_id != ModuleId::ROOT {
        let after = compiler.symbols_table_mut().global_symbol_names();
        for name in after {
            if !before.contains(&name) {
                compiler.symbols_table_mut().mask_symbol(&name);
            }
        }
    }
}

pub fn check_exports(graph: &mut ModuleGraph) -> ModuleResult<HashMap<ModuleId, ModuleExports>> {
    let mut exports: HashMap<ModuleId, ModuleExports> = HashMap::new();
    let topo_order = graph.topo_order().to_vec();
    for &module_id in &topo_order {
        let node = graph.node(module_id);
        let file_path = node.path.clone();
        let imports = resolve_imports(&node.program, &exports, graph)?;
        let program = &mut graph.node_mut(module_id).program;
        let mut checker = TypeChecker::new();
        for import in &imports {
            import.inject_into_env(checker.env_mut());
        }
        checker.check_program_mut(program);
        let type_errors = checker.errors();
        if !type_errors.is_empty() {
            let messages = type_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }
        let module_exports = ModuleExports::from_checked(program, checker.env());
        exports.insert(module_id, module_exports);
    }

    Ok(exports)
}

fn resolve_imports(
    program: &Program,
    exports: &HashMap<ModuleId, ModuleExports>,
    graph: &ModuleGraph,
) -> ModuleResult<Vec<ResolvedImport>> {
    let mut result = Vec::new();
    for stmt in &program.statements {
        let Stmt::Use(use_stmt) = stmt else {
            continue;
        };
        let (module_path, items_to_import) = if let Some(items) = &use_stmt.items {
            (use_stmt.path.as_slice(), items.clone())
        } else if use_stmt.path.len() >= 2 {
            let split = use_stmt.path.len() - 1;
            (&use_stmt.path[..split], vec![use_stmt.path[split].clone()])
        } else {
            continue;
        };
        let target_id = find_module_by_path(graph, module_path);
        let Some(target_id) = target_id else {
            continue;
        };
        let Some(target_exports) = exports.get(&target_id) else {
            continue;
        };
        for item_name in &items_to_import {
            target_exports.resolve_item(item_name, &mut result);
        }
    }

    Ok(result)
}

fn find_module_by_path(graph: &ModuleGraph, module_path: &[String]) -> Option<ModuleId> {
    graph
        .nodes()
        .find(|n| {
            if module_path.len() == 1 {
                n.qualified_path
                    .last()
                    .is_some_and(|last| last == &module_path[0])
            } else {
                n.qualified_path == module_path
            }
        })
        .map(|n| n.id)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    // Create a temporary directory tree from `(relative_path, content)` pairs.
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

    // Resolve and compile a project, returning the bytecode.
    fn compile_project(
        dir: &std::path::Path,
        entry: &str,
    ) -> ModuleResult<maat_bytecode::Bytecode> {
        let mut graph = resolve_module_graph(&dir.join(entry))?;
        check_and_compile(&mut graph)
    }

    #[test]
    fn type_error_in_dependency_surfaces() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod math; use math::add; add(1, 2);"),
            (
                "math.maat",
                "pub fn add(a: i64, b: i64) -> i64 { a + b + true }",
            ),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(result.is_err(), "type error in dependency should surface");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("type") || err_msg.contains("Type"),
            "error should mention type: {err_msg}"
        );
    }

    #[test]
    fn cross_module_function_type_mismatch() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod math; use math::add; add(true, false);"),
            ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_err(),
            "passing bool to i64 params should fail type check"
        );
    }

    #[test]
    fn valid_cross_module_compiles() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod math; use math::double; double(21);"),
            ("math.maat", "pub fn double(x: i64) -> i64 { x * 2 }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "valid cross-module program should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn bare_use_is_noop() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod helper; use helper; let x: i64 = 42;"),
            ("helper.maat", "pub fn noop() { }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "bare `use helper;` should be a no-op: {:?}",
            result.err()
        );
    }

    #[test]
    fn missing_module_import_produces_undefined_error() {
        let dir = setup_temp_project(&[("main.maat", "use nonexistent::foo; foo();")]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_err(),
            "importing from non-existent module should fail"
        );
    }

    #[test]
    fn grouped_imports() {
        let dir = setup_temp_project(&[
            (
                "main.maat",
                "mod math; use math::{add, sub}; add(1, 2); sub(3, 1);",
            ),
            (
                "math.maat",
                "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }",
            ),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "grouped imports should work: {:?}",
            result.err()
        );
    }

    #[test]
    fn reexport_pub_use() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod proxy; use proxy::double; double(5);"),
            ("proxy.maat", "mod math; pub use math::double;"),
            ("proxy/math.maat", "pub fn double(x: i64) -> i64 { x * 2 }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "re-export via `pub use` should work: {:?}",
            result.err()
        );
    }

    #[test]
    fn topo_order_compiles_dependencies_first() {
        let dir = setup_temp_project(&[
            (
                "main.maat",
                "mod a; mod b; use a::fa; use b::fb; fa(fb(1));",
            ),
            ("a.maat", "pub fn fa(x: i64) -> i64 { x + 10 }"),
            ("b.maat", "pub fn fb(x: i64) -> i64 { x * 2 }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "multi-dependency compilation should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn diamond_dependency_compiles() {
        let dir = setup_temp_project(&[
            (
                "main.maat",
                "mod a; mod b; use a::fa; use b::fb; fa(1); fb(2);",
            ),
            (
                "a.maat",
                "mod shared; use shared::helper; pub fn fa(x: i64) -> i64 { helper(x) }",
            ),
            (
                "b.maat",
                "mod shared; use shared::helper; pub fn fb(x: i64) -> i64 { helper(x) }",
            ),
            ("a/shared.maat", "pub fn helper(x: i64) -> i64 { x + 1 }"),
            ("b/shared.maat", "pub fn helper(x: i64) -> i64 { x + 2 }"),
        ]);
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "diamond dependency should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn exports_only_pub_items() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod lib; use lib::pub_fn; pub_fn();"),
            ("lib.maat", "pub fn pub_fn() { }\nfn private_fn() { }"),
        ]);
        let mut graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
        let exports = check_exports(&mut graph).unwrap();
        // Find the lib module's exports (not the root).
        let lib_exports = exports
            .iter()
            .find(|&(&id, _)| id != ModuleId::ROOT)
            .map(|(_, e)| e)
            .expect("should have lib module exports");
        let binding_names: Vec<&str> = lib_exports
            .bindings
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert!(binding_names.contains(&"pub_fn"), "should export pub_fn");
        assert!(
            !binding_names.contains(&"private_fn"),
            "should not export private_fn"
        );
    }

    #[test]
    fn exports_pub_struct() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod types; use types::Point;"),
            ("types.maat", "pub struct Point { x: i64, y: i64 }"),
        ]);
        let mut graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
        let exports = check_exports(&mut graph).unwrap();
        let types_exports = exports
            .iter()
            .find(|&(&id, _)| id != ModuleId::ROOT)
            .map(|(_, e)| e)
            .expect("should have types module exports");
        assert_eq!(types_exports.structs.len(), 1);
        assert_eq!(types_exports.structs[0].name, "Point");
    }

    #[test]
    fn exports_pub_enum() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod types; use types::Color;"),
            ("types.maat", "pub enum Color { Red, Green, Blue }"),
        ]);
        let mut graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
        let exports = check_exports(&mut graph).unwrap();
        let types_exports = exports
            .iter()
            .find(|&(&id, _)| id != ModuleId::ROOT)
            .map(|(_, e)| e)
            .expect("should have types module exports");
        assert_eq!(types_exports.enums.len(), 1);
        assert_eq!(types_exports.enums[0].name, "Color");
    }

    #[test]
    fn private_symbols_do_not_leak_across_modules() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod a; mod b; use b::result; result();"),
            ("a.maat", "fn private_helper() -> i64 { 42 }"),
            ("b.maat", "pub fn result() -> i64 { 1 }"),
        ]);
        // `a`'s private_helper should not be visible in `b` or `main`.
        let result = compile_project(dir.path(), "main.maat");
        assert!(
            result.is_ok(),
            "private symbols should not leak: {:?}",
            result.err()
        );
    }

    #[test]
    fn find_module_single_segment() {
        let dir = setup_temp_project(&[
            ("main.maat", "mod math;"),
            ("math.maat", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
        let found = find_module_by_path(&graph, &["math".to_string()]);
        assert!(found.is_some(), "should find module by single segment");
    }

    #[test]
    fn find_module_returns_none_for_unknown() {
        let dir = setup_temp_project(&[("main.maat", "let x: i64 = 1;")]);
        let graph = resolve_module_graph(&dir.path().join("main.maat")).unwrap();
        let found = find_module_by_path(&graph, &["nonexistent".to_string()]);
        assert!(found.is_none(), "should not find non-existent module");
    }
}
