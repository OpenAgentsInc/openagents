use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::slicer_bambu_lane_parity::{
    PARITY_SLICER_BAMBU_LANE_ISSUE_ID, ParityManifest, build_slicer_bambu_lane_parity_manifest,
};

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
fn slicer_bambu_lane_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("slicer_bambu_lane_parity_manifest.json");
    let manifest: ParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SLICER_BAMBU_LANE_ISSUE_ID);
    assert!(manifest.reference_issue_match);
    assert!(manifest.reference_commit_match);
    assert!(manifest.row_set_match);
    assert!(manifest.contract_set_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.rows.len(), 3);
    assert_eq!(manifest.contracts.len(), 3);
}

#[test]
fn slicer_bambu_lane_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("slicer_bambu_lane_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_slicer_bambu_lane_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build slicer_bambu_lane parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated slicer_bambu_lane parity manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read slicer_bambu_lane fixture");
    assert_eq!(generated_json, fixture_json);
}
