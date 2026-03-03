use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::assembly_checkpoint_parity::{
    AssemblyCheckpointParityManifest, PARITY_ASSEMBLY_CHECKPOINT_ISSUE_ID,
    build_assembly_checkpoint_parity_manifest,
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
fn assembly_checkpoint_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("assembly_checkpoint_parity_manifest.json");
    let manifest: AssemblyCheckpointParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_ASSEMBLY_CHECKPOINT_ISSUE_ID);
    assert_eq!(manifest.required_issue_ids.len(), 10);
    assert_eq!(manifest.validated_issue_ids.len(), 10);
    assert!(manifest.missing_issue_ids.is_empty());
    assert!(manifest.mismatched_issue_ids.is_empty());
    assert!(manifest.plan_items_checked);
    assert_eq!(manifest.parity_completion_percent, 100.0);
    assert!(manifest.checkpoint_pass);
}

#[test]
fn assembly_checkpoint_manifest_fixture_matches_generation() {
    let repo = repo_root();
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("assembly_checkpoint_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_assembly_checkpoint_parity_manifest(
        &scorecard,
        &scorecard_path.to_string_lossy(),
        &repo,
    )
    .expect("build assembly checkpoint parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read assembly checkpoint fixture");
    assert_eq!(generated_json, fixture_json);
}
