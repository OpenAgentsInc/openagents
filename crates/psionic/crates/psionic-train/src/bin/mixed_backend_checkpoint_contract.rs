use std::{env, path::PathBuf, process};

use psionic_train::{
    write_mixed_backend_checkpoint_contract, MIXED_BACKEND_CHECKPOINT_CONTRACT_FIXTURE_PATH,
};

fn main() {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(MIXED_BACKEND_CHECKPOINT_CONTRACT_FIXTURE_PATH));
    if let Err(error) = write_mixed_backend_checkpoint_contract(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
