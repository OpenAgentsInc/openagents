use std::{error::Error, path::PathBuf};

use psionic_train::{
    PSION_BENCHMARK_CATALOG_FIXTURE_PATH, PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH,
    PSION_FORGE_BENCHMARK_PACK_FIXTURE_PATH, PSION_FORGE_JUDGE_PACK_FIXTURE_PATH,
    PsionBenchmarkCatalog, PsionBenchmarkReceiptSet, record_psion_forge_benchmark_pack_manifest,
    record_psion_forge_judge_pack_manifest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let catalog: PsionBenchmarkCatalog = serde_json::from_slice(&std::fs::read(
        root.join(PSION_BENCHMARK_CATALOG_FIXTURE_PATH),
    )?)?;
    let receipt_set: PsionBenchmarkReceiptSet = serde_json::from_slice(&std::fs::read(
        root.join(PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH),
    )?)?;

    let benchmark_manifest =
        record_psion_forge_benchmark_pack_manifest(root.as_path(), &catalog, &receipt_set)?;
    benchmark_manifest.write_json(root.join(PSION_FORGE_BENCHMARK_PACK_FIXTURE_PATH))?;

    let judge_manifest = record_psion_forge_judge_pack_manifest(root.as_path(), &catalog)?;
    judge_manifest.write_json(root.join(PSION_FORGE_JUDGE_PACK_FIXTURE_PATH))?;

    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing crate parent")?
        .parent()
        .ok_or("missing workspace root")?
        .to_path_buf())
}
