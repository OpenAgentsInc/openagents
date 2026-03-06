#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::drafting_gdt_parity::{
    DraftingGdtParityManifest, PARITY_DRAFTING_GDT_ISSUE_ID, build_drafting_gdt_parity_manifest,
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
fn drafting_gdt_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("drafting_gdt_parity_manifest.json");
    let manifest: DraftingGdtParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_DRAFTING_GDT_ISSUE_ID);
    assert_eq!(manifest.symbol_cases.len(), 14);
    assert_eq!(manifest.material_condition_cases.len(), 3);
    assert_eq!(manifest.frame_cases.len(), 3);
    assert_eq!(manifest.datum_cases.len(), 2);
    assert!(manifest.reference_commit_match);
    assert!(manifest.symbol_contract_match);
    assert!(manifest.material_condition_contract_match);
    assert!(manifest.frame_contract_match);
    assert!(manifest.datum_contract_match);
    assert!(manifest.deterministic_replay_match);
}

#[test]
fn drafting_gdt_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("drafting_gdt_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_drafting_gdt_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build drafting gdt parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read drafting gdt fixture");
    assert_eq!(generated_json, fixture_json);
}
