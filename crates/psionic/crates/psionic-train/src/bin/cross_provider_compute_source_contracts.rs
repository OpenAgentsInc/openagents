use std::{env, path::PathBuf, process};

use psionic_train::write_cross_provider_compute_source_contracts;

fn main() {
    let mut args = env::args().skip(1);
    let output_dir = match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin cross_provider_compute_source_contracts -- <output-dir>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_cross_provider_compute_source_contracts(&output_dir) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
