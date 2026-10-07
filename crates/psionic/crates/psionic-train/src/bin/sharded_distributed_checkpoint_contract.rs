use std::{env, path::PathBuf, process};

use psionic_train::write_sharded_distributed_checkpoint_contract;

fn main() {
    let output_path = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin sharded_distributed_checkpoint_contract -- <output-path>"
            );
            process::exit(2);
        }
    };
    if let Err(error) = write_sharded_distributed_checkpoint_contract(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
