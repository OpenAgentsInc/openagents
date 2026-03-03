use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::raytrace_quality_mode_parity::{
    PARITY_RAYTRACE_QUALITY_MODE_ISSUE_ID, RaytraceQualityModeParityManifest,
    build_raytrace_quality_mode_parity_manifest,
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
fn raytrace_quality_mode_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("raytrace_quality_mode_parity_manifest.json");
    let manifest: RaytraceQualityModeParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_RAYTRACE_QUALITY_MODE_ISSUE_ID);
    assert!(manifest.reference_commit_match);
    assert!(manifest.default_quality_match);
    assert!(manifest.quality_mode_set_match);
    assert!(manifest.quality_scale_table_match);
    assert!(manifest.interaction_override_match);
    assert!(manifest.sample_set_match);
    assert!(manifest.cap_enforcement_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.default_quality, "draft");
    assert_eq!(manifest.quality_modes, vec!["draft", "standard", "high"]);
    assert_eq!(manifest.samples.len(), 6);
}

#[test]
fn raytrace_quality_mode_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("raytrace_quality_mode_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_raytrace_quality_mode_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build raytrace quality mode parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated raytrace quality mode parity manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read raytrace quality mode fixture");
    assert_eq!(generated_json, fixture_json);
}
