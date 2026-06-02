//! Maat CLI entry point.
//!
//! Provides the `maat` command with subcommands for running source files,
//! starting the interactive REPL, compiling source code to bytecode, and
//! executing pre-compiled bytecode.

#![forbid(unsafe_code)]

mod cmd;
mod diagnostic;
mod public_io;
mod repl;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
        /// Comma-separated private (witness) input values; never serialized into the proof.
        #[arg(short = 'P', long, allow_hyphen_values = true)]
        private_input: Option<String>,
        /// Path to JSON file containing private inputs array (alternative to --private-input).
        #[arg(long)]
        private_inputs_file: Option<PathBuf>,
        /// Prover-side assertion on the scalar output; trace must produce this value.
        #[arg(long, allow_hyphen_values = true)]
        expect_output: Option<String>,
        /// Prover-side bundle (`inputs`, `private_inputs`, `output`).
        #[arg(long)]
        public_io: Option<PathBuf>,
        /// After a successful prove, emit the public bundle (inputs, output, program_hash).
        #[arg(long)]
        write_public_io: Option<PathBuf>,
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
        /// Comma-separated expected public input values (pinned cell-by-cell).
        #[arg(short, long, allow_hyphen_values = true)]
        input: Option<String>,
        /// JSON file containing expected public inputs (alternative to --input).
        #[arg(long)]
        inputs_file: Option<PathBuf>,
        /// Expected scalar output (decimal).
        #[arg(long, allow_hyphen_values = true)]
        expect_output: Option<String>,
        /// Verifier-side bundle (`inputs`, `output`, optional `program_hash`).
        #[arg(long)]
        public_io: Option<PathBuf>,
        /// Compile a source file and pin its program hash to the proof.
        #[arg(long)]
        expect_program: Option<PathBuf>,
        /// Pin a `0x`-prefixed program hash directly (no source recompilation).
        #[arg(long)]
        expect_program_hash: Option<String>,
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
            private_input,
            private_inputs_file,
            expect_output,
            public_io,
            write_public_io,
            output,
            trace,
            production,
        }) => {
            cmd::prove(cmd::ProveArgs {
                source: &file,
                input: input.as_deref(),
                inputs_file: inputs_file.as_deref(),
                private_input: private_input.as_deref(),
                private_inputs_file: private_inputs_file.as_deref(),
                expect_output: expect_output.as_deref(),
                public_io: public_io.as_deref(),
                write_public_io: write_public_io.as_deref(),
                output: output.as_deref(),
                trace: trace.as_deref(),
                production,
            });
        }
        Some(Command::Verify {
            file,
            input,
            inputs_file,
            expect_output,
            public_io,
            expect_program,
            expect_program_hash,
        }) => {
            cmd::verify(cmd::VerifyArgs {
                proof: &file,
                input: input.as_deref(),
                inputs_file: inputs_file.as_deref(),
                expect_output: expect_output.as_deref(),
                public_io: public_io.as_deref(),
                expect_program: expect_program.as_deref(),
                expect_program_hash: expect_program_hash.as_deref(),
            });
        }
    }
}
