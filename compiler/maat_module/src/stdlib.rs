//! Embedded standard library sources.
//!
//! The stdlib ships alongside the compiler as embedded `.maat` source strings.
//! When a `use std::X::...` import is encountered, the module resolver
//! fetches the corresponding source from this module rather than the
//! file system.

use std::collections::HashMap;
use std::path::PathBuf;

use maat_ast::{Program, Stmt};
use maat_errors::ModuleErrorKind;
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;
use maat_span::Span;

use crate::{ModuleGraph, ModuleId, ModuleResult};

/// Embedded source for `std::math`.
const STD_MATH: &str = include_str!("../../../library/std/math.maat");

/// Embedded source for `std::string`.
const STD_STRING: &str = include_str!("../../../library/std/string.maat");

/// Embedded source for `std::vec`.
const STD_VEC: &str = include_str!("../../../library/std/vec.maat");

/// Embedded source for `std::set`.
const STD_SET: &str = include_str!("../../../library/std/set.maat");

/// Embedded source for `std::map`.
const STD_MAP: &str = include_str!("../../../library/std/map.maat");

/// Scans all modules in the graph for `use std::X` imports and adds the
/// corresponding standard library modules to the graph.
///
/// Stdlib modules are added with a synthetic path (`<std>/X.maat`) and
/// qualified path `["std", "X"]`. They participate in the normal
/// type-checking and compilation pipeline.
pub(crate) fn inject_stdlib_modules(graph: &mut ModuleGraph) -> ModuleResult<()> {
    let existing_count = graph.len();
    let mut needed: HashMap<String, Vec<ModuleId>> = HashMap::new();

    for id in 0..existing_count {
        let module_id = ModuleId(id as u32);
        let program = &graph.node(module_id).program;
        for name in collect_std_imports(program) {
            needed.entry(name).or_default().push(module_id);
        }
    }
    for (module_name, importers) in &needed {
        let source = lookup_stdlib_source(module_name).ok_or_else(|| {
            ModuleErrorKind::FileNotFound {
                module_name: format!("std::{module_name}"),
                candidates: vec![],
            }
            .at(Span::ZERO, PathBuf::from("<std>"))
        })?;
        let program = parse_stdlib_source(module_name, source)?;
        let synthetic_path = PathBuf::from(format!("<std>/{module_name}.maat"));
        let qualified_path = vec!["std".to_string(), module_name.clone()];
        let stdlib_id = graph.add_node(program, synthetic_path, qualified_path);

        // Add dependency edges so the stdlib module is compiled before importers.
        for &importer_id in importers {
            graph.add_edge(importer_id, stdlib_id);
        }
    }
    Ok(())
}

/// Collects unique `std::X` module names from `use` statements in a program.
fn collect_std_imports(program: &Program) -> Vec<String> {
    let mut names = Vec::new();
    for stmt in &program.statements {
        let Stmt::Use(use_stmt) = stmt else {
            continue;
        };
        if use_stmt.path.first().is_some_and(|seg| seg == "std") && use_stmt.path.len() >= 2 {
            let name = use_stmt.path[1].clone();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// Returns the embedded source for a standard library module, if it exists.
fn lookup_stdlib_source(module_name: &str) -> Option<&'static str> {
    match module_name {
        "math" => Some(STD_MATH),
        "string" => Some(STD_STRING),
        "vec" => Some(STD_VEC),
        "set" => Some(STD_SET),
        "map" => Some(STD_MAP),
        _ => None,
    }
}

/// Parses an embedded stdlib source string into a [`Program`].
fn parse_stdlib_source(module_name: &str, source: &str) -> ModuleResult<Program> {
    let mut parser = MaatParser::new(MaatLexer::new(source));
    let program = parser.parse();
    if parser.errors().is_empty() {
        Ok(program)
    } else {
        let path = PathBuf::from(format!("<std>/{module_name}.maat"));
        Err(ModuleErrorKind::ParseErrors {
            file: path.clone(),
            messages: parser.errors().iter().map(|e| e.to_string()).collect(),
        }
        .at(Span::ZERO, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_program(source: &str) -> Program {
        let mut parser = MaatParser::new(MaatLexer::new(source));
        parser.parse()
    }

    #[test]
    fn lookup_known_modules() {
        for name in &["math", "string", "vec", "set", "map"] {
            assert!(
                lookup_stdlib_source(name).is_some(),
                "stdlib module `{name}` should be found"
            );
        }
    }

    #[test]
    fn lookup_unknown_module_returns_none() {
        assert!(lookup_stdlib_source("nonexistent").is_none());
        assert!(lookup_stdlib_source("").is_none());
        assert!(lookup_stdlib_source("io").is_none());
    }

    #[test]
    fn all_stdlib_modules_parse_successfully() {
        for name in &["math", "string", "vec", "set", "map"] {
            let source = lookup_stdlib_source(name).expect("module should exist");
            let result = parse_stdlib_source(name, source);
            assert!(
                result.is_ok(),
                "stdlib module `{name}` should parse without errors: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn parse_invalid_source_returns_error() {
        let result = parse_stdlib_source("test", "fn incomplete(");
        assert!(result.is_err(), "invalid source should produce an error");
    }

    #[test]
    fn collects_std_imports() {
        // This test is purely syntactic: it extracts module names
        // from `use std::X::...` paths without validating the imported item.
        let source = "use std::math::abs;\nuse std::math::min;\nlet x: i64 = 1;";
        let program = parse_program(source);
        let names = collect_std_imports(&program);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "math");
    }

    #[test]
    fn deduplicates_std_imports() {
        let source = "use std::math::abs;\nuse std::math::min;\nuse std::math::max;";
        let program = parse_program(source);
        let names = collect_std_imports(&program);
        assert_eq!(
            names.len(),
            1,
            "duplicate std::math imports should be deduplicated"
        );
        assert_eq!(names[0], "math");
    }

    #[test]
    fn ignores_non_std_imports() {
        let source = "use foo::bar;\nuse baz::qux;";
        let program = parse_program(source);
        let names = collect_std_imports(&program);
        assert!(names.is_empty(), "non-std imports should be ignored");
    }

    #[test]
    fn ignores_bare_std_import() {
        let source = "use std;";
        let program = parse_program(source);
        let names = collect_std_imports(&program);
        assert!(
            names.is_empty(),
            "bare `use std;` should produce no stdlib imports"
        );
    }

    #[test]
    fn inject_adds_stdlib_modules_to_graph() {
        use std::fs;

        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let main_path = dir.path().join("main.maat");
        fs::write(&main_path, "use std::math::abs; abs(-1);").expect("write failed");

        let mut graph = crate::resolve::resolve_module_graph(&main_path).unwrap();
        let before = graph.len();
        inject_stdlib_modules(&mut graph).unwrap();
        let after = graph.len();
        assert_eq!(
            after,
            before + 1,
            "injecting std::math should add exactly one module"
        );
    }

    #[test]
    fn inject_unknown_stdlib_module_errors() {
        use std::fs;

        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let main_path = dir.path().join("main.maat");
        fs::write(&main_path, "use std::fantasy::unicorn;").expect("write failed");

        let result = crate::resolve::resolve_module_graph(&main_path);
        assert!(
            result.is_err(),
            "importing non-existent stdlib module should fail"
        );
    }

    #[test]
    fn inject_multiple_stdlib_modules() {
        use std::fs;

        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let main_path = dir.path().join("main.maat");
        fs::write(
            &main_path,
            "use std::math::abs;\nuse std::set::insert;\nabs(-1);",
        )
        .expect("write failed");

        let mut graph = crate::resolve::resolve_module_graph(&main_path).unwrap();
        let before = graph.len();
        inject_stdlib_modules(&mut graph).unwrap();
        let after = graph.len();
        assert_eq!(
            after,
            before + 2,
            "injecting std::math and std::set should add two modules"
        );
    }

    #[test]
    fn inject_deduplicates_stdlib_modules() {
        use std::fs;

        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let main_path = dir.path().join("main.maat");
        fs::write(&main_path, "use std::math::abs;\nuse std::math::min;").expect("write failed");

        let mut graph = crate::resolve::resolve_module_graph(&main_path).unwrap();
        let before = graph.len();
        inject_stdlib_modules(&mut graph).unwrap();
        let after = graph.len();
        assert_eq!(
            after,
            before + 1,
            "multiple imports from same stdlib module should add it only once"
        );
    }
}
