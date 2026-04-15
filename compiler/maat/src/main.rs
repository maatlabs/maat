//! Maat CLI entry point.
//!
//! Provides the `maat` command with subcommands for running source files,
//! starting the interactive REPL, compiling to bytecode, and executing
//! pre-compiled bytecode.
#![forbid(unsafe_code)]

mod cmd;
mod diagnostic;
mod repl;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The Maat programming language compiler and runtime.
#[derive(Parser)]
#[command(name = "maat", version, about = "Maat programming language")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Compile and execute a Maat source file.
    Run {
        /// Path to the `.maat` source file.
        file: PathBuf,
    },
    /// Start the interactive REPL.
    Repl,
    /// Compile a source file to bytecode.
    Build {
        /// Path to the `.maat` source file.
        file: PathBuf,
        /// Output path for the compiled `.mtc` bytecode file.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Execute a pre-compiled bytecode file.
    Exec {
        /// Path to the `.mtc` bytecode file.
        file: PathBuf,
    },
    /// Compile and trace-execute a Maat source file, dumping the execution trace as CSV.
    Trace {
        /// Path to the `.maat` source file.
        file: PathBuf,
        /// Output path for the CSV trace (defaults to stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate a STARK proof of correct program execution.
    Prove {
        /// Path to the `.maat` source file.
        file: PathBuf,
        /// Comma-separated public input values (integers or field elements with `fe` suffix).
        #[arg(short, long, allow_hyphen_values = true)]
        input: Option<String>,
        /// Path to JSON file containing public inputs array (alternative to --input).
        #[arg(long)]
        inputs_file: Option<PathBuf>,
        /// Proof output path (default: `<program>.proof.bin`).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Also dump the execution trace to the given path.
        #[arg(short, long)]
        trace: Option<PathBuf>,
        /// Use production proof options (~97 bits conjectural security).
        #[arg(short, long)]
        production: bool,
    },
    /// Verify a STARK proof file.
    Verify {
        /// Path to the `.proof.bin` file.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run { file }) => {
            cmd::run(&file);
        }
        Some(Command::Repl) | None => {
            println!(
                "\nMaat {} ({} {})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
            );
            println!("Type \"exit\", \"quit\" or press Ctrl+D to quit.\n");
            repl::start_interactive();
        }
        Some(Command::Build { file, output }) => {
            cmd::build(&file, output.as_deref());
        }
        Some(Command::Exec { file }) => {
            cmd::execute(&file);
        }
        Some(Command::Trace { file, output }) => {
            cmd::trace(&file, output.as_deref());
        }
        Some(Command::Prove {
            file,
            input,
            inputs_file,
            output,
            trace,
            production,
        }) => {
            cmd::prove(
                &file,
                input.as_deref(),
                inputs_file.as_deref(),
                output.as_deref(),
                trace.as_deref(),
                production,
            );
        }
        Some(Command::Verify { file }) => {
            cmd::verify(&file);
        }
    }
}
