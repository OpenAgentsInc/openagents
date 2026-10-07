use std::{env, path::PathBuf, process};

use psionic_train::write_runpod_local_training_binder_projection_set;

fn main() {
    let mut args = env::args().skip(1);
    let output_path = match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin runpod_local_training_binder_projection -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_runpod_local_training_binder_projection_set(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
