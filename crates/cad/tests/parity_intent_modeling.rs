#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::intent_modeling_parity::{
    IntentModelingParityManifest, PARITY_INTENT_MODELING_ISSUE_ID,
    build_intent_modeling_parity_manifest,
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
fn intent_modeling_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("intent_modeling_parity_manifest.json");
    let manifest: IntentModelingParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_INTENT_MODELING_ISSUE_ID);
    assert!(manifest.reference_commit_match);
    assert!(manifest.execution_path_match);
    assert!(manifest.confirmation_contract_match);
    assert!(manifest.clarification_contract_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.snapshot.json_state_revision, 1);
    assert_eq!(manifest.snapshot.natural_language_confirmed_revision, 2);
}

#[test]
fn intent_modeling_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("intent_modeling_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_intent_modeling_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build intent modeling parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated intent modeling manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read intent modeling fixture");
    assert_eq!(generated_json, fixture_json);
}
