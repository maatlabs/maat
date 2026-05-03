use std::path::PathBuf;

use maat_span::Span;
use thiserror::Error;

/// A module resolution error with file context and source span.
#[derive(Debug, Error)]
#[error("{file}: {kind}", file = file.display())]
pub struct ModuleError {
    pub kind: ModuleErrorKind,
    pub span: Span,
    pub file: PathBuf,
}

#[derive(Debug, Error)]
pub enum ModuleErrorKind {
    #[error("module `{module_name}` not found; searched: {}", candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    FileNotFound {
        module_name: String,
        candidates: Vec<PathBuf>,
    },

    #[error("cyclic module dependency: {}", cycle.join(" -> "))]
    CyclicDependency {
        /// The module names forming the cycle, in visitation order.
        cycle: Vec<String>,
    },

    #[error("duplicate module declaration `{module_name}`")]
    DuplicateModule { module_name: String },

    #[error("parse errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    ParseErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    #[error("type errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    TypeErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

    #[error("compile errors in `{}`:{}", file.display(), messages.iter().map(|m| format!("\n  {m}")).collect::<String>())]
    CompileErrors {
        file: PathBuf,
        messages: Vec<String>,
    },

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
