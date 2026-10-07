use std::{env, path::PathBuf, process};

use psionic_train::write_cross_provider_runtime_binder;

fn main() {
    let mut args = env::args().skip(1);
    let output_path = match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin cross_provider_runtime_binder -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_cross_provider_runtime_binder(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
