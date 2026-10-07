use std::{env, path::PathBuf, process};

use psionic_train::{
    write_mlx_dense_rank_runtime_contract, MLX_DENSE_RANK_RUNTIME_CONTRACT_FIXTURE_PATH,
};

fn main() {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(MLX_DENSE_RANK_RUNTIME_CONTRACT_FIXTURE_PATH));
    if let Err(error) = write_mlx_dense_rank_runtime_contract(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
