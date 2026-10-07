use std::path::PathBuf;

use psionic_train::{
    materialize_tassadar_reference_run_hull_benchmark, TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR));
    materialize_tassadar_reference_run_hull_benchmark(output_dir.as_path(), None)?;
    Ok(())
}
