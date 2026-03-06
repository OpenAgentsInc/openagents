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

use openagents_cad::parity::raytrace_ui_toggle_fallback_parity::{
    PARITY_RAYTRACE_UI_TOGGLE_FALLBACK_ISSUE_ID, RaytraceUiToggleFallbackParityManifest,
    build_raytrace_ui_toggle_fallback_parity_manifest,
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
fn raytrace_ui_toggle_fallback_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("raytrace_ui_toggle_fallback_parity_manifest.json");
    let manifest: RaytraceUiToggleFallbackParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(
        manifest.issue_id,
        PARITY_RAYTRACE_UI_TOGGLE_FALLBACK_ISSUE_ID
    );
    assert!(manifest.reference_commit_match);
    assert!(manifest.default_state_match);
    assert!(manifest.init_outcome_match);
    assert!(manifest.keyboard_guard_match);
    assert!(manifest.quality_selection_toggle_match);
    assert!(manifest.overlay_gate_match);
    assert!(manifest.fallback_behavior_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.default_state.render_mode, "standard");
    assert_eq!(manifest.default_state.raytrace_quality, "draft");
    assert!(!manifest.default_state.raytrace_available);
    assert_eq!(manifest.init_outcomes.len(), 3);
    assert_eq!(manifest.toggle_samples.len(), 5);
    assert_eq!(manifest.overlay_samples.len(), 4);
}

#[test]
fn raytrace_ui_toggle_fallback_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("raytrace_ui_toggle_fallback_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_raytrace_ui_toggle_fallback_parity_manifest(
        &scorecard,
        &scorecard_path.to_string_lossy(),
    )
    .expect("build raytrace ui toggle/fallback parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated raytrace ui toggle/fallback parity manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read raytrace ui toggle/fallback fixture");
    assert_eq!(generated_json, fixture_json);
}
