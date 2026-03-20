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
use maat_lexer::Lexer;
use maat_parser::Parser;
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

/// Parses an embedded stdlib source string into a [`Program`].
fn parse_stdlib_source(module_name: &str, source: &str) -> ModuleResult<Program> {
    let mut parser = Parser::new(Lexer::new(source));
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
