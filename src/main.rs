//! Maat REPL entry point.
//!
//! This binary provides an interactive Read-Eval-Print Loop for the Maat
//! programming language, allowing users to enter and parse Maat programs
//! interactively.

use std::io::{self, Result};

fn main() -> Result<()> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| String::from("user"));

    println!("\nHello {username}! This is the Maat programming language!");
    println!("Feel free to type in commands\n");

    let reader = io::stdin().lock();
    let mut writer = io::stdout().lock();

    maat::repl::start(reader, &mut writer)
}
