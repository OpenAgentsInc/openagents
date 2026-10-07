use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_FIXTURE_PATH,
    build_psion_actual_pretraining_continuation_eval_benchmark_package,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let benchmark_package = build_psion_actual_pretraining_continuation_eval_benchmark_package()?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_FIXTURE_PATH),
        &benchmark_package,
    )?;
    println!(
        "wrote {}",
        PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_FIXTURE_PATH
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}
