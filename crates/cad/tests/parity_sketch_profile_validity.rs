use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sketch_profile_validity_parity::{
    PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID, SketchProfileValidityParityManifest,
    build_sketch_profile_validity_parity_manifest,
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
fn sketch_profile_validity_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_profile_validity_parity_manifest.json");
    let manifest: SketchProfileValidityParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID);
    assert!(manifest.closed_case.profile_closed_loop);
    assert!(manifest.closed_case.warning_codes.is_empty());
    assert!(!manifest.open_case.profile_closed_loop);
    assert!(
        manifest
            .open_case
            .warning_codes
            .iter()
            .any(|code| code == "CAD-WARN-NON-MANIFOLD")
    );
    assert!(
        manifest
            .duplicate_profile_entity_error
            .contains("must not contain duplicates")
    );
    assert!(manifest.degenerate_line_error.contains("degenerate"));
    assert!(
        manifest
            .unknown_entity_error
            .contains("unknown sketch entity")
    );
    assert!(
        manifest
            .unsolved_constraint_error
            .contains("unsolved constraints")
    );
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_profile_validity_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_profile_validity_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_sketch_profile_validity_parity_manifest(
        &scorecard,
        &scorecard_path.to_string_lossy(),
    )
    .expect("build sketch profile validity parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read sketch profile validity fixture");
    assert_eq!(generated_json, fixture_json);
}
