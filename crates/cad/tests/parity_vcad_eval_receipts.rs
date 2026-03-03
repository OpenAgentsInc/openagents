use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::vcad_eval_receipts_parity::{
    PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID, VcadEvalReceiptsParityManifest,
    build_vcad_eval_receipts_parity_manifest,
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
fn vcad_eval_receipts_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("vcad_eval_receipts_parity_manifest.json");
    let manifest: VcadEvalReceiptsParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID);
    assert!(manifest.deterministic_replay_match);
    assert!(manifest.timing_contract_match);
    assert!(manifest.baseline_snapshot.parse_ms.is_none());
    assert!(manifest.baseline_snapshot.serialize_ms.is_none());
}

#[test]
fn vcad_eval_receipts_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("vcad_eval_receipts_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_vcad_eval_receipts_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read vcad-eval receipts fixture");
    assert_eq!(generated_json, fixture_json);
}
