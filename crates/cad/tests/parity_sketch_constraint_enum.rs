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
use openagents_cad::parity::sketch_constraint_enum_parity::{
    PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID, SketchConstraintEnumParityManifest,
    build_sketch_constraint_enum_parity_manifest,
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
fn sketch_constraint_enum_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("sketch_constraint_enum_parity_manifest.json");
    let manifest: SketchConstraintEnumParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(
        manifest.issue_id,
        PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID.to_string()
    );
    assert_eq!(manifest.supported_geometric_kinds.len(), 15);
    assert_eq!(manifest.supported_dimensional_kinds.len(), 8);
    assert_eq!(manifest.legacy_kinds, vec!["dimension"]);
    assert_eq!(manifest.constraint_kind_summaries.len(), 24);
    assert!(
        manifest
            .solver_summary
            .warning_codes
            .iter()
            .any(|code| code == "SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED")
    );
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn sketch_constraint_enum_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("sketch_constraint_enum_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_sketch_constraint_enum_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build sketch constraint enum parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read sketch constraint enum fixture");
    assert_eq!(generated_json, fixture_json);
}
