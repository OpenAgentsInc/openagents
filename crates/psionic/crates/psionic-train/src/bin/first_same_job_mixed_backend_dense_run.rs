use std::{env, path::PathBuf, process};

use psionic_train::write_first_same_job_mixed_backend_dense_run_bundle;

fn main() {
    let mut args = env::args().skip(1);
    let output_path = match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin first_same_job_mixed_backend_dense_run -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_first_same_job_mixed_backend_dense_run_bundle(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
