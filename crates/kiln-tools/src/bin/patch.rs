//! `patch PATH`: apply the search-and-replace blocks on standard input to
//! PATH. See [`kiln_tools::apply_patch`] for the format.

use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: patch PATH < blocks");
        return ExitCode::from(2);
    };
    let mut patch = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut patch) {
        eprintln!("patch: reading input: {error}");
        return ExitCode::FAILURE;
    }
    let original = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("patch: {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    match kiln_tools::apply_patch(&original, &patch) {
        Ok(text) => match std::fs::write(&path, text) {
            Ok(()) => {
                println!("patched {path}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("patch: {path}: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("patch: {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
