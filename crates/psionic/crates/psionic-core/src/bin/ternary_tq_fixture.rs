#![allow(clippy::print_stderr)]

//! Fixture generator for the ternary TQ-class format contract
//! (psionic#1115). Writes the full fixture document, including the real CPU
//! determinism receipt and the explicitly pending backend receipts, to the
//! provided output path. The committed copy lives at
//! `fixtures/quant/ternary_tq_formats_v1.json`;
//! `scripts/check-ternary-tq-formats.sh` regenerates and compares.

use std::{env, fs, process::ExitCode};

use psionic_core::ternary::ternary_tq_fixture_document;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: cargo run -p psionic-core --bin ternary_tq_fixture -- <output-path>");
            return ExitCode::FAILURE;
        }
    };

    let document = match ternary_tq_fixture_document() {
        Ok(document) => document,
        Err(error) => {
            eprintln!("failed to build ternary TQ fixture document: {error}");
            return ExitCode::FAILURE;
        }
    };
    let json = match serde_json::to_string_pretty(&document) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("failed to serialize ternary TQ fixture document: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = fs::write(&output_path, format!("{json}\n")) {
        eprintln!("failed to write ternary TQ fixture: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
