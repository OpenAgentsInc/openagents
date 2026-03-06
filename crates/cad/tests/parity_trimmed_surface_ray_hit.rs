#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::trimmed_surface_ray_hit_parity::{
    PARITY_TRIMMED_SURFACE_RAY_HIT_ISSUE_ID, TrimmedSurfaceRayHitParityManifest,
    build_trimmed_surface_ray_hit_parity_manifest,
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
fn trimmed_surface_ray_hit_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("trimmed_surface_ray_hit_parity_manifest.json");
    let manifest: TrimmedSurfaceRayHitParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_TRIMMED_SURFACE_RAY_HIT_ISSUE_ID);
    assert!(manifest.reference_commit_match);
    assert!(manifest.sample_set_match);
    assert!(manifest.concave_behavior_match);
    assert!(manifest.outer_inner_loop_filter_match);
    assert!(manifest.hole_rejection_match);
    assert!(manifest.closest_hit_filter_match);
    assert!(manifest.winding_rule_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.samples.len(), 4);
    assert_eq!(manifest.concave_checks.len(), 3);
}

#[test]
fn trimmed_surface_ray_hit_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("trimmed_surface_ray_hit_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_trimmed_surface_ray_hit_parity_manifest(
        &scorecard,
        &scorecard_path.to_string_lossy(),
    )
    .expect("build trimmed-surface ray hit parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated trimmed-surface ray hit parity manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read trimmed-surface ray hit parity fixture");
    assert_eq!(generated_json, fixture_json);
}
