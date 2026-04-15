use std::path::PathBuf;

use maat_span::Span;
use thiserror::Error;

/// A module resolution error with file context and source span.
///
/// Wraps [`ModuleErrorKind`] with the originating file path and span
/// for rich diagnostic output.
#[derive(Debug, Error)]
#[error("{file}: {kind}", file = file.display())]
pub struct ModuleError {
    pub kind: ModuleErrorKind,
    pub span: Span,
    /// The file in which the error was encountered.
    pub file: PathBuf,
}

/// The underlying variant of a module resolution error.
#[derive(Debug, Error)]
pub enum ModuleErrorKind {
    /// A `mod foo;` declaration could not be resolved to a source file.
    #[error("module `{module_name}` not found; searched: {}", candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    FileNotFound {
        module_name: String,
        candidates: Vec<PathBuf>,
    },

    /// A cycle was detected in the module dependency graph.
    #[error("cyclic module dependency: {}", cycle.join(" -> "))]
    CyclicDependency {
        /// The module names forming the cycle, in visitation order.
        cycle: Vec<String>,
    },

    /// A module was declared more than once in the same parent module.
    #[error("duplicate module declaration `{module_name}`")]
    DuplicateModule { module_name: String },

    /// The parser encountered errors in a module source file.
    #[error("parse errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    ParseErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// Type errors encountered during module type checking.
    #[error("type errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    TypeErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// Compilation errors encountered during module code generation.
    #[error("compile errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    CompileErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    /// An I/O error occurred while reading a source file.
    #[error("cannot read `{}`: {message}", path.display())]
    Io { path: PathBuf, message: String },
}

impl ModuleErrorKind {
    /// Attaches a source span and file path to this error kind,
    /// producing a [`ModuleError`].
    pub fn at(self, span: Span, file: PathBuf) -> ModuleError {
        ModuleError {
            kind: self,
            span,
            file,
        }
    }
}
