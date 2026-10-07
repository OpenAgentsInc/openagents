use std::{env, path::PathBuf, process};

use psionic_train::write_hybrid_pretraining_plan;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin hybrid_pretraining_plan -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_hybrid_pretraining_plan(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
