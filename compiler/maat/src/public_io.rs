//! Helpers for reading and writing public-I/O bundles.

use std::path::Path;
use std::process;

use maat_field::BaseElement;
use maat_prover::{PublicIo, parse_felt};

pub fn load_bundle(path: &Path) -> PublicIo {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read '{}': {e}", path.display());
        process::exit(1);
    });
    PublicIo::from_json(&text).unwrap_or_else(|e| {
        eprintln!("error: '{}': {e}", path.display());
        process::exit(1);
    })
}

pub fn write_bundle(bundle: &PublicIo, path: &Path) {
    if let Err(e) = std::fs::write(path, bundle.to_json()) {
        eprintln!("error: cannot write '{}': {e}", path.display());
        process::exit(1);
    }
}

pub fn parse_expected_felt(value: &str, source: &str) -> BaseElement {
    parse_felt(value).unwrap_or_else(|e| {
        eprintln!("error: {source}: {e}");
        process::exit(1);
    })
}

pub fn load_expected_inputs(path: &Path) -> Vec<BaseElement> {
    load_bundle(path).inputs_felt().unwrap_or_else(|e| {
        eprintln!("error: '{}': {e}", path.display());
        process::exit(1);
    })
}
