use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sketch_revolve_parity::{
    PARITY_SKETCH_REVOLVE_ISSUE_ID, SketchRevolveParityManifest,
    build_sketch_revolve_parity_manifest,
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
fn sketch_revolve_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_revolve_parity_manifest.json");
    let manifest: SketchRevolveParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SKETCH_REVOLVE_ISSUE_ID);
    assert_eq!(manifest.full_case.revolve_angle_deg, 360.0);
    assert_eq!(manifest.partial_case.revolve_angle_deg, 90.0);
    assert!(manifest.full_case.warning_codes.is_empty());
    assert!(
        manifest
            .partial_case
            .warning_codes
            .iter()
            .any(|code| code == "CAD-WARN-SLIVER-FACE")
    );
    assert!(manifest.profile_hash_order_stable);
    assert!(
        manifest
            .invalid_zero_angle_error
            .contains("revolve_angle_deg")
    );
    assert!(
        manifest
            .invalid_over_360_angle_error
            .contains("revolve_angle_deg")
    );
    assert!(manifest.missing_axis_error.contains("axis_anchor_ids"));
    assert!(
        manifest
            .unsolved_constraint_error
            .contains("unsolved constraints")
    );
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_revolve_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_revolve_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_sketch_revolve_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build sketch revolve parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read sketch revolve fixture");
    assert_eq!(generated_json, fixture_json);
}
