use std::{env, path::PathBuf, process};

use psionic_train::write_training_execution_evidence_bundle;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin training_execution_evidence_bundle -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_training_execution_evidence_bundle(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
