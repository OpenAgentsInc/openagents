#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::gap_matrix::ParityGapMatrix;
use openagents_cad::parity::scorecard::{
    PARITY_SCORECARD_ISSUE_ID, ParityScorecard, build_scorecard,
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
fn parity_scorecard_fixture_is_well_formed() {
    let path = parity_dir().join("parity_scorecard.json");
    let scorecard: ParityScorecard = load_json(&path);
    assert_eq!(scorecard.manifest_version, 1);
    assert_eq!(scorecard.issue_id, PARITY_SCORECARD_ISSUE_ID);
    assert_eq!(scorecard.threshold_profiles.len(), 2);
    assert_eq!(scorecard.evaluations.len(), 2);

    let baseline_eval = scorecard
        .evaluations
        .iter()
        .find(|evaluation| evaluation.profile_id == "phase_a_baseline_v1")
        .expect("phase_a_baseline_v1 evaluation should exist");
    assert!(baseline_eval.pass, "baseline profile should pass");

    let parity_eval = scorecard
        .evaluations
        .iter()
        .find(|evaluation| evaluation.profile_id == "parity_complete_v1")
        .expect("parity_complete_v1 evaluation should exist");
    assert!(!parity_eval.pass, "parity_complete profile should fail");
}

#[test]
fn parity_scorecard_fixture_matches_generation() {
    let dir = parity_dir();
    let gap_matrix_path = dir.join("vcad_openagents_gap_matrix.json");
    let scorecard_path = dir.join("parity_scorecard.json");
    let gap_matrix: ParityGapMatrix = load_json(&gap_matrix_path);
    let generated = build_scorecard(&gap_matrix, &gap_matrix_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated scorecard")
    );
    let fixture_json = fs::read_to_string(scorecard_path).expect("read scorecard fixture");
    assert_eq!(generated_json, fixture_json);
}
