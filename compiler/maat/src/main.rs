//! Maat compiler and interpreter entry point.

use std::process;

fn main() {
    eprintln!("maat-v0.3.0");
    eprintln!("To run the REPL, use: cargo run --bin repl");
    eprintln!("\nFor more information, see: https://github.com/maatlabs/maat");
    process::exit(1);
}
