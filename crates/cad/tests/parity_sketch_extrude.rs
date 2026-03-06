#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sketch_extrude_parity::{
    PARITY_SKETCH_EXTRUDE_ISSUE_ID, SketchExtrudeParityManifest,
    build_sketch_extrude_parity_manifest,
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
fn sketch_extrude_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_extrude_parity_manifest.json");
    let manifest: SketchExtrudeParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SKETCH_EXTRUDE_ISSUE_ID);
    assert!(manifest.closed_case.profile_closed_loop);
    assert!(!manifest.open_case.profile_closed_loop);
    assert!(manifest.profile_hash_order_stable);
    assert!(manifest.zero_depth_error.contains("depth_mm"));
    assert!(manifest.empty_profile_error.contains("profile_entity_ids"));
    assert!(
        manifest
            .unsolved_constraint_error
            .contains("unsolved constraints")
    );
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_extrude_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_extrude_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_sketch_extrude_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build sketch extrude parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read sketch extrude fixture");
    assert_eq!(generated_json, fixture_json);
}
