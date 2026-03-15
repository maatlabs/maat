//! Module graph resolution from an entry-point source file.
//!
//! Walks `mod` declarations recursively, parsing each discovered file,
//! building the dependency DAG, detecting cycles, and producing a
//! topological ordering.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use maat_ast::{Program, Stmt};
use maat_errors::ModuleErrorKind;
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_span::Span;

use crate::{ModuleGraph, ModuleId, ModuleResult};

/// DFS coloring for cycle detection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    /// Not yet visited.
    White,
    /// Currently on the DFS stack (visiting descendants).
    Gray,
    /// Fully processed (all descendants visited).
    Black,
}

/// Resolves the complete module dependency graph starting from `entry`.
///
/// Parses all reachable source files, builds the DAG, detects cycles,
/// and returns a [`ModuleGraph`] with modules in topological order
/// (leaves first, root last).
///
/// # Errors
///
/// Returns a [`ModuleError`](maat_errors::ModuleError) if:
/// - A `mod` declaration cannot be resolved to a file
/// - A cycle is detected in the dependency graph
/// - A source file contains parse errors
/// - A source file cannot be read
pub fn resolve_module_graph(entry: &Path) -> ModuleResult<ModuleGraph> {
    let canonical = entry.canonicalize().map_err(|e| {
        ModuleErrorKind::Io {
            path: entry.to_path_buf(),
            message: e.to_string(),
        }
        .at(Span::ZERO, entry.to_path_buf())
    })?;
    let mut resolver = Resolver::new();
    resolver.resolve_file(&canonical, Vec::new())?;
    crate::stdlib::inject_stdlib_modules(&mut resolver.graph)?;
    resolver.compute_topo_order()?;
    Ok(resolver.graph)
}

/// Internal state for the recursive module resolution pass.
struct Resolver {
    graph: ModuleGraph,
    /// Maps canonical file paths to their assigned `ModuleId` to avoid
    /// parsing the same file twice.
    file_to_id: HashMap<PathBuf, ModuleId>,
}

impl Resolver {
    fn new() -> Self {
        Self {
            graph: ModuleGraph::new(),
            file_to_id: HashMap::new(),
        }
    }

    /// Parses a source file and recursively resolves its `mod` declarations.
    fn resolve_file(&mut self, path: &Path, qualified_path: Vec<String>) -> ModuleResult<ModuleId> {
        if let Some(&id) = self.file_to_id.get(path) {
            return Ok(id);
        }
        let source = read_source(path)?;
        let program = parse_source(path, &source)?;
        let id = self
            .graph
            .add_node(program, path.to_path_buf(), qualified_path.clone());
        self.file_to_id.insert(path.to_path_buf(), id);

        let is_root = qualified_path.is_empty();
        let mod_decls = collect_mod_declarations(&self.graph.node(id).program, path, is_root)?;
        for (mod_name, mod_path, _span) in mod_decls {
            let mut child_qualified = qualified_path.clone();
            child_qualified.push(mod_name);

            let child_id = self.resolve_file(&mod_path, child_qualified)?;
            self.graph.add_edge(id, child_id);
        }
        Ok(id)
    }

    /// Computes and stores the topological order, detecting cycles via DFS.
    fn compute_topo_order(&mut self) -> ModuleResult<()> {
        let n = self.graph.len();
        let mut colors = vec![Color::White; n];
        let mut order = Vec::with_capacity(n);
        let mut path_stack: Vec<ModuleId> = Vec::new();

        for i in 0..n {
            let id = ModuleId(i as u32);
            if colors[i] == Color::White {
                self.dfs(id, &mut colors, &mut order, &mut path_stack)?;
            }
        }
        self.graph.set_topo_order(order);
        Ok(())
    }

    /// Depth-first traversal with gray/black coloring for cycle detection.
    fn dfs(
        &self,
        id: ModuleId,
        colors: &mut [Color],
        order: &mut Vec<ModuleId>,
        path_stack: &mut Vec<ModuleId>,
    ) -> ModuleResult<()> {
        let idx = id.0 as usize;
        colors[idx] = Color::Gray;
        path_stack.push(id);
        for &dep in self.graph.dependencies(id) {
            let dep_idx = dep.0 as usize;
            match colors[dep_idx] {
                Color::White => {
                    self.dfs(dep, colors, order, path_stack)?;
                }
                Color::Gray => {
                    let node = self.graph.node(id);
                    let cycle = self
                        .extract_cycle(path_stack, dep)
                        .unwrap_or_else(|| vec![self.module_display_name(dep)]);
                    return Err(ModuleErrorKind::CyclicDependency { cycle }
                        .at(Span::ZERO, node.path.clone()));
                }
                Color::Black => {}
            }
        }
        path_stack.pop();
        colors[idx] = Color::Black;
        order.push(id);
        Ok(())
    }

    /// Extracts a human-readable cycle from the DFS path stack.
    ///
    /// Returns `None` if the back-edge target is not found on the stack,
    /// which should never happen in a correct DFS but we handle gracefully
    /// rather than panicking.
    fn extract_cycle(
        &self,
        path_stack: &[ModuleId],
        back_edge_target: ModuleId,
    ) -> Option<Vec<String>> {
        let start_pos = path_stack.iter().position(|&id| id == back_edge_target)?;
        let mut cycle = path_stack[start_pos..]
            .iter()
            .map(|&id| self.module_display_name(id))
            .collect::<Vec<String>>();
        // Close the cycle by repeating the first element.
        cycle.push(cycle[0].clone());
        Some(cycle)
    }

    /// Returns the display name for a module (its qualified path or filename).
    fn module_display_name(&self, id: ModuleId) -> String {
        let node = self.graph.node(id);
        if node.qualified_path.is_empty() {
            node.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "root".to_string())
        } else {
            node.qualified_path.join("::")
        }
    }
}

/// Reads a source file as UTF-8 with BOM rejection.
fn read_source(path: &Path) -> ModuleResult<String> {
    let bytes = std::fs::read(path).map_err(|e| {
        ModuleErrorKind::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
        .at(Span::ZERO, path.to_path_buf())
    })?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(ModuleErrorKind::Io {
            path: path.to_path_buf(),
            message:
                "file starts with a UTF-8 BOM; Maat source files must not contain a byte-order mark"
                    .to_string(),
        }
        .at(Span::ZERO, path.to_path_buf()));
    }
    String::from_utf8(bytes).map_err(|_| {
        ModuleErrorKind::Io {
            path: path.to_path_buf(),
            message: "file is not valid UTF-8".to_string(),
        }
        .at(Span::ZERO, path.to_path_buf())
    })
}

/// Parses a source file into an AST, collecting all parse errors.
fn parse_source(path: &Path, source: &str) -> ModuleResult<Program> {
    let mut parser = Parser::new(Lexer::new(source));
    let program = parser.parse();
    if parser.errors().is_empty() {
        Ok(program)
    } else {
        Err(ModuleErrorKind::ParseErrors {
            file: path.to_path_buf(),
            messages: parser.errors().iter().map(|e| e.to_string()).collect(),
        }
        .at(Span::ZERO, path.to_path_buf()))
    }
}

/// Scans a parsed program for `mod` declarations and resolves their file paths.
///
/// Only external `mod foo;` declarations (without an inline body) trigger
/// file resolution. Inline modules (`mod foo { ... }`) remain in the parent's
/// AST and do not create separate graph nodes.
fn collect_mod_declarations(
    program: &Program,
    parent_path: &Path,
    is_root: bool,
) -> ModuleResult<Vec<(String, PathBuf, Span)>> {
    let parent_dir = parent_path.parent().ok_or_else(|| {
        ModuleErrorKind::Io {
            path: parent_path.to_path_buf(),
            message: "source file has no parent directory".to_string(),
        }
        .at(Span::ZERO, parent_path.to_path_buf())
    })?;
    // Root files and `mod.mt` files resolve submodules in their own directory.
    // Other files resolve submodules in a subdirectory named after the file stem
    // (e.g., `foo.mt` with `mod bar;` looks for `foo/bar.mt`).
    let is_mod_file = parent_path.file_stem().is_some_and(|s| s == "mod");
    let base_dir = if is_root || is_mod_file {
        parent_dir.to_path_buf()
    } else {
        let stem = parent_path.file_stem().ok_or_else(|| {
            ModuleErrorKind::Io {
                path: parent_path.to_path_buf(),
                message: "source file has no file stem".to_string(),
            }
            .at(Span::ZERO, parent_path.to_path_buf())
        })?;
        parent_dir.join(stem)
    };
    let mut seen = HashMap::new();
    let mut result = Vec::new();
    for stmt in &program.statements {
        let Stmt::Mod(mod_stmt) = stmt else {
            continue;
        };
        // Inline modules stay in the parent AST.
        if mod_stmt.body.is_some() {
            continue;
        }
        if seen.contains_key(&mod_stmt.name) {
            return Err(ModuleErrorKind::DuplicateModule {
                module_name: mod_stmt.name.clone(),
            }
            .at(mod_stmt.span, parent_path.to_path_buf()));
        }
        seen.insert(mod_stmt.name.clone(), mod_stmt.span);
        let resolved = resolve_mod_path(&base_dir, &mod_stmt.name, mod_stmt.span, parent_path)?;
        result.push((mod_stmt.name.clone(), resolved, mod_stmt.span));
    }
    Ok(result)
}

/// Resolves a `mod foo;` declaration to a canonical file path.
///
/// Tries `<base_dir>/foo.mt` first, then `<base_dir>/foo/mod.mt`.
/// If both exist, the resolution is ambiguous and an error is returned.
fn resolve_mod_path(
    base_dir: &Path,
    module_name: &str,
    span: Span,
    parent_path: &Path,
) -> ModuleResult<PathBuf> {
    let file_path = base_dir.join(format!("{module_name}.mt"));
    let dir_path = base_dir.join(module_name).join("mod.mt");
    let file_exists = file_path.is_file();
    let dir_exists = dir_path.is_file();

    match (file_exists, dir_exists) {
        (true, true) => Err(ModuleErrorKind::FileNotFound {
            module_name: module_name.to_string(),
            candidates: vec![file_path, dir_path],
        }
        .at(span, parent_path.to_path_buf())),
        (true, false) => file_path.canonicalize().map_err(|e| {
            ModuleErrorKind::Io {
                path: file_path,
                message: e.to_string(),
            }
            .at(span, parent_path.to_path_buf())
        }),
        (false, true) => dir_path.canonicalize().map_err(|e| {
            ModuleErrorKind::Io {
                path: dir_path,
                message: e.to_string(),
            }
            .at(span, parent_path.to_path_buf())
        }),
        (false, false) => Err(ModuleErrorKind::FileNotFound {
            module_name: module_name.to_string(),
            candidates: vec![file_path, dir_path],
        }
        .at(span, parent_path.to_path_buf())),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

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

    #[test]
    fn single_file_no_modules() {
        let dir = setup_temp_project(&[("main.mt", "let x = 42;")]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 1);
        assert_eq!(graph.topo_order().len(), 1);
        assert_eq!(graph.topo_order()[0], ModuleId::ROOT);
    }

    #[test]
    fn single_submodule() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod math;"),
            ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 2);
        let order = graph.topo_order();
        assert_eq!(order.len(), 2);
        let first = &graph.node(order[0]).qualified_path;
        assert_eq!(first, &["math"]);
        let second = &graph.node(order[1]).qualified_path;
        assert!(second.is_empty());
    }

    #[test]
    fn nested_submodule_via_dir() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod math;"),
            ("math/mod.mt", "mod ops;"),
            ("math/ops.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 3);
        let order = graph.topo_order();
        let names = order
            .iter()
            .map(|&id| graph.node(id).qualified_path.clone())
            .collect::<Vec<_>>();
        assert_eq!(names[0], vec!["math", "ops"]);
        assert_eq!(names[1], vec!["math"]);
        assert!(names[2].is_empty());
    }

    #[test]
    fn module_not_found() {
        let dir = setup_temp_project(&[("main.mt", "mod nonexistent;")]);
        let err = resolve_module_graph(&dir.path().join("main.mt")).unwrap_err();
        assert!(
            matches!(&err.kind, ModuleErrorKind::FileNotFound { module_name, .. } if module_name == "nonexistent")
        );
    }

    #[test]
    fn duplicate_module_declaration() {
        let dir =
            setup_temp_project(&[("main.mt", "mod foo;\nmod foo;"), ("foo.mt", "let x = 1;")]);
        let err = resolve_module_graph(&dir.path().join("main.mt")).unwrap_err();
        assert!(
            matches!(&err.kind, ModuleErrorKind::DuplicateModule { module_name } if module_name == "foo")
        );
    }

    #[test]
    fn inline_module_not_resolved_as_file() {
        let dir = setup_temp_project(&[("main.mt", "mod utils { pub fn helper() -> i64 { 42 } }")]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn parse_error_in_submodule() {
        let dir = setup_temp_project(&[("main.mt", "mod bad;"), ("bad.mt", "let = ;")]);
        let err = resolve_module_graph(&dir.path().join("main.mt")).unwrap_err();
        assert!(matches!(&err.kind, ModuleErrorKind::ParseErrors { .. }));
    }

    #[test]
    fn multiple_submodules() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod alpha;\nmod beta;"),
            ("alpha.mt", "pub fn a() -> i64 { 1 }"),
            ("beta.mt", "pub fn b() -> i64 { 2 }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 3);
        let last = graph.topo_order().last().unwrap();
        assert!(graph.node(*last).qualified_path.is_empty());
    }

    #[test]
    fn diamond_dependency() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod a;\nmod b;"),
            ("a.mt", "mod shared;"),
            ("b.mt", "mod shared;"),
            ("a/shared.mt", "pub fn sa() -> i64 { 1 }"),
            ("b/shared.mt", "pub fn sb() -> i64 { 2 }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 5);
    }

    #[test]
    fn file_not_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.mt");
        fs::write(&path, [0xFF, 0xFE, 0x00]).unwrap();
        let err = resolve_module_graph(&path).unwrap_err();
        assert!(matches!(&err.kind, ModuleErrorKind::Io { .. }));
    }

    #[test]
    fn mod_mt_submodule_resolution() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod utils;"),
            ("utils/mod.mt", "mod helpers;"),
            ("utils/helpers.mt", "pub fn h() -> i64 { 99 }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 3);
        let order = graph.topo_order();
        let names = order
            .iter()
            .map(|&id| graph.node(id).qualified_path.clone())
            .collect::<Vec<_>>();
        assert_eq!(names[0], vec!["utils", "helpers"]);
        assert_eq!(names[1], vec!["utils"]);
        assert!(names[2].is_empty());
    }

    #[test]
    fn ambiguous_module_both_file_and_dir() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod foo;"),
            ("foo.mt", "let x = 1;"),
            ("foo/mod.mt", "let y = 2;"),
        ]);
        let err = resolve_module_graph(&dir.path().join("main.mt")).unwrap_err();
        assert!(
            matches!(&err.kind, ModuleErrorKind::FileNotFound { module_name, candidates } if module_name == "foo" && candidates.len() == 2)
        );
    }

    #[test]
    fn deeply_nested_modules() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod a;"),
            ("a.mt", "mod b;"),
            ("a/b.mt", "mod c;"),
            ("a/b/c.mt", "pub fn deep() -> i64 { 42 }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        assert_eq!(graph.len(), 4);
        let order = graph.topo_order();
        let names = order
            .iter()
            .map(|&id| graph.node(id).qualified_path.clone())
            .collect::<Vec<_>>();
        assert_eq!(names[0], vec!["a", "b", "c"]);
        assert_eq!(names[1], vec!["a", "b"]);
        assert_eq!(names[2], vec!["a"]);
        assert!(names[3].is_empty());
    }

    #[test]
    fn topo_order_leaves_first() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod x;\nmod y;"),
            ("x.mt", "mod z;"),
            ("x/z.mt", "pub fn zz() -> i64 { 0 }"),
            ("y.mt", "pub fn yy() -> i64 { 1 }"),
        ]);
        let graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        let order = graph.topo_order();
        assert!(graph.node(*order.last().unwrap()).qualified_path.is_empty());
        let z_pos = order
            .iter()
            .position(|&id| graph.node(id).qualified_path == vec!["x", "z"])
            .unwrap();
        let x_pos = order
            .iter()
            .position(|&id| graph.node(id).qualified_path == vec!["x"])
            .unwrap();
        assert!(z_pos < x_pos);
    }
}
