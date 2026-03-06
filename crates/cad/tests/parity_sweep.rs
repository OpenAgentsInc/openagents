#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sweep_parity::{
    PARITY_SWEEP_ISSUE_ID, SweepParityManifest, build_sweep_parity_manifest,
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
fn sweep_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sweep_parity_manifest.json");
    let manifest: SweepParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SWEEP_ISSUE_ID);
    assert_eq!(manifest.default_segments_snapshot.segment_count, 32);
    assert_eq!(manifest.explicit_segments_snapshot.segment_count, 8);
    assert!(manifest.control_snapshot.default_segments_applied);
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sweep_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sweep_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_sweep_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read sweep fixture");
    assert_eq!(generated_json, fixture_json);
}
