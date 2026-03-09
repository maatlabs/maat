//! Module resolution and dependency graph construction for the Maat compiler.
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
//! - **Root entry files** and **`mod.mt`** files resolve submodules in
//!   their own directory: `mod foo;` in `dir/mod.mt` resolves to
//!   `dir/foo.mt` or `dir/foo/mod.mt`.
//! - **All other files** resolve submodules in a subdirectory named after
//!   the file stem: `mod bar;` in `dir/foo.mt` resolves to
//!   `dir/foo/bar.mt` or `dir/foo/bar/mod.mt`.
//!
//! If both `foo.mt` and `foo/mod.mt` exist, the resolution is ambiguous
//! and an error is produced. If neither exists, a resolution error is
//! produced.

mod graph;
mod resolve;

use maat_errors::ModuleError;

/// A specialized [`Result`](std::result::Result) type for module resolution operations.
pub type ModuleResult<T> = std::result::Result<T, ModuleError>;

pub use graph::{ModuleGraph, ModuleId, ModuleNode};
pub use resolve::resolve_module_graph;
