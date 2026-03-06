#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::direct_brep_raytrace_scaffolding_parity::{
    DirectBrepRaytraceScaffoldingParityManifest, PARITY_DIRECT_BREP_RAYTRACE_SCAFFOLDING_ISSUE_ID,
    build_direct_brep_raytrace_scaffolding_parity_manifest,
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
fn direct_brep_raytrace_scaffolding_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("direct_brep_raytrace_scaffolding_parity_manifest.json");
    let manifest: DirectBrepRaytraceScaffoldingParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(
        manifest.issue_id,
        PARITY_DIRECT_BREP_RAYTRACE_SCAFFOLDING_ISSUE_ID
    );
    assert!(manifest.reference_commit_match);
    assert!(manifest.module_graph_match);
    assert!(manifest.public_exports_match);
    assert!(manifest.intersection_registry_match);
    assert!(manifest.gpu_feature_gate_match);
    assert!(manifest.cpu_renderer_contract_match);
    assert!(manifest.ray_contract_match);
    assert!(manifest.no_tessellation_contract_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.modules.len(), 6);
}

#[test]
fn direct_brep_raytrace_scaffolding_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("direct_brep_raytrace_scaffolding_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_direct_brep_raytrace_scaffolding_parity_manifest(
        &scorecard,
        &scorecard_path.to_string_lossy(),
    )
    .expect("build direct BRep raytrace scaffolding parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated direct BRep raytrace scaffolding parity manifest")
    );
    let fixture_json =
        fs::read_to_string(fixture_path).expect("read direct BRep raytrace scaffolding fixture");
    assert_eq!(generated_json, fixture_json);
}
