use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::gpu_acceleration_parity::{
    GpuAccelerationParityManifest, PARITY_GPU_ACCELERATION_ISSUE_ID,
    build_gpu_acceleration_parity_manifest,
};
use openagents_cad::parity::scorecard::ParityScorecard;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

fn parity_dir() -> PathBuf {
    repo_root().join("crates/cad/parity")
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed reading {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing {}: {error}", path.display()))
}

#[test]
fn gpu_acceleration_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("gpu_acceleration_parity_manifest.json");
    let manifest: GpuAccelerationParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_GPU_ACCELERATION_ISSUE_ID);
    assert!(manifest.reference_commit_match);
    assert!(manifest.backend_contract_match);
    assert!(manifest.capability_export_match);
    assert!(manifest.context_error_contract_match);
    assert!(manifest.normals_contract_match);
    assert!(manifest.decimation_ratio_contract_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.exported_capabilities.len(), 2);
}

#[test]
fn gpu_acceleration_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("gpu_acceleration_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_gpu_acceleration_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build gpu acceleration parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated gpu acceleration parity manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read gpu acceleration parity fixture");
    assert_eq!(generated_json, fixture_json);
}
