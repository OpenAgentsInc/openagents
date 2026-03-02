use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sketch_interaction_parity::{
    PARITY_SKETCH_INTERACTION_ISSUE_ID, SketchInteractionParityManifest,
    build_sketch_interaction_parity_manifest,
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
fn sketch_interaction_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_interaction_parity_manifest.json");
    let manifest: SketchInteractionParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SKETCH_INTERACTION_ISSUE_ID);
    assert_eq!(manifest.shortcut_bindings.len(), 8);
    assert!(
        manifest
            .shortcut_bindings
            .iter()
            .any(|binding| binding.key == "Escape" && binding.shortcut_id == "sketch.escape")
    );
    assert!(manifest.enter_without_parts_case.sketch_active);
    assert!(manifest.enter_with_parts_case.face_selection_mode);
    assert!(manifest.face_selection_confirm_case.sketch_active);
    assert!(
        manifest
            .horizontal_constraint_case
            .command_codes
            .iter()
            .any(|code| code == "SKETCH-CMD-RUN-SOLVER")
    );
    assert!(manifest.escape_request_exit_case.pending_exit_confirmation);
    assert!(!manifest.escape_confirm_exit_case.sketch_active);
    assert!(manifest.constraint_shortcut_requires_single_selection);
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_interaction_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_interaction_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_sketch_interaction_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build sketch interaction parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read sketch interaction fixture");
    assert_eq!(generated_json, fixture_json);
}
