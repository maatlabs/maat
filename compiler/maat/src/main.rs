//! Maat CLI entry point.
//!
//! Provides the `maat` command with subcommands for running source files,
//! starting the interactive REPL, and compiling to
//! bytecode and executing pre-compiled bytecode.

mod repl;
mod run;

use std::io;
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
        /// Path to the `.mt` source file.
        file: PathBuf,
    },

    /// Start the interactive REPL.
    Repl,

    /// Compile a source file to bytecode.
    Build {
        /// Path to the `.mt` source file.
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run { file }) => run::execute(&file),

        Some(Command::Repl) | None => {
            let username = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| String::from("user"));

            println!("\nHello {username}! This is the Maat programming language!");
            println!("Feel free to type in commands\n");

            let reader = io::stdin().lock();
            let mut writer = io::stdout().lock();

            if let Err(e) = repl::start(reader, &mut writer) {
                eprintln!("repl error: {e}");
                std::process::exit(1);
            }
        }

        Some(Command::Build { .. }) => {
            eprintln!("maat build: not yet implemented");
            std::process::exit(1);
        }

        Some(Command::Exec { .. }) => {
            eprintln!("maat exec: not yet implemented");
            std::process::exit(1);
        }
    }
}
