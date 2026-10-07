use std::{env, path::PathBuf, process};

use psionic_train::write_dense_rank_runtime_reference_contract;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin dense_rank_runtime_reference_contract -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_dense_rank_runtime_reference_contract(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
