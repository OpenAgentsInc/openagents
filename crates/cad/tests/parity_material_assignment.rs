use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::material_assignment_parity::{
    MaterialAssignmentParityManifest, PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID,
    build_material_assignment_parity_manifest,
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
fn material_assignment_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("material_assignment_parity_manifest.json");
    let manifest: MaterialAssignmentParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID);
    assert_eq!(manifest.feature_scope_snapshot.scope, "feature");
    assert_eq!(manifest.part_scope_snapshot.scope, "part");
    assert_eq!(manifest.default_scope_snapshot.scope, "default");
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn material_assignment_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("material_assignment_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_material_assignment_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read material assignment fixture");
    assert_eq!(generated_json, fixture_json);
}
