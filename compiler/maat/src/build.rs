//! Bytecode compilation for the `maat build` command.
//!
//! Compiles a `.mt` source file to bytecode and writes the serialized
//! binary to a `.mtc` output file.

use std::path::Path;
use std::process;

use crate::pipeline;

/// Compiles a source file and writes serialized bytecode to disk.
///
/// If `output_path` is `None`, the output file is derived from the
/// source path by replacing its extension with `.mtc`.
pub fn compile_to_file(source_path: &Path, output_path: Option<&Path>) {
    let bytecode = pipeline::compile_source(source_path);

    let bytes = match bytecode.serialize() {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "error: failed to serialize bytecode for '{}': {e}",
                source_path.display()
            );
            process::exit(1);
        }
    };

    let default_output = source_path.with_extension("mtc");
    let out = output_path.unwrap_or(&default_output);

    if let Err(e) = std::fs::write(out, bytes) {
        eprintln!("error: cannot write '{}': {e}", out.display());
        process::exit(1);
    }

    eprintln!("compiled {} -> {}", source_path.display(), out.display());
}
