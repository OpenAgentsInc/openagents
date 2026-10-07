use std::{env, path::PathBuf, process};

use psionic_train::{
    write_cross_backend_cuda_mlx_dense_mesh_contract,
    CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_FIXTURE_PATH,
};

fn main() {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_FIXTURE_PATH));
    if let Err(error) = write_cross_backend_cuda_mlx_dense_mesh_contract(&output_path) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
