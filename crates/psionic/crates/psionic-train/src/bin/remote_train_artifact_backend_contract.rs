use std::{env, path::PathBuf, process};

use psionic_train::write_remote_train_artifact_backend_contract_set;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin remote_train_artifact_backend_contract -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_remote_train_artifact_backend_contract_set(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
