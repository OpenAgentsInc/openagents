use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::sketch_undo_redo_parity::{
    PARITY_SKETCH_UNDO_REDO_ISSUE_ID, SketchUndoRedoParityManifest,
    build_sketch_undo_redo_parity_manifest,
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
fn sketch_undo_redo_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_undo_redo_parity_manifest.json");
    let manifest: SketchUndoRedoParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SKETCH_UNDO_REDO_ISSUE_ID);
    assert_eq!(manifest.history_max_steps, 50);
    assert_eq!(manifest.undo_binding, "Cmd/Ctrl+Z");
    assert_eq!(manifest.redo_binding, "Cmd/Ctrl+Shift+Z");
    assert_eq!(manifest.sequence.len(), 7);
    assert!(
        manifest
            .sequence
            .iter()
            .any(|entry| entry.transition_id == "sketch.exit.confirm")
    );
    assert!(manifest.undo_trace_matches_reverse_sequence);
    assert!(manifest.redo_trace_matches_forward_sequence);
    assert!(manifest.redo_cleared_on_new_edit);
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_undo_redo_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_undo_redo_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_sketch_undo_redo_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build sketch undo/redo parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read sketch undo/redo fixture");
    assert_eq!(generated_json, fixture_json);
}
