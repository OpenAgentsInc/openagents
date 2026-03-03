use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::fixture_corpus::ParityFixtureCorpus;
use openagents_cad::parity::risk_register::{
    PARITY_RISK_REGISTER_ISSUE_ID, ParityRiskRegister, build_risk_register,
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
fn parity_risk_register_fixture_is_well_formed() {
    let path = parity_dir().join("parity_risk_register.json");
    let register: ParityRiskRegister = load_json(&path);
    assert_eq!(register.manifest_version, 1);
    assert_eq!(register.issue_id, PARITY_RISK_REGISTER_ISSUE_ID);
    assert_eq!(register.summary.open_total, register.risks.len());
    assert_eq!(register.summary.open_hard_blockers, 16);

    let baseline_eval = register
        .workflow
        .evaluations
        .iter()
        .find(|evaluation| evaluation.profile_id == "phase_a_baseline_v1")
        .expect("phase_a_baseline_v1 evaluation should exist");
    assert!(baseline_eval.pass);

    let parity_eval = register
        .workflow
        .evaluations
        .iter()
        .find(|evaluation| evaluation.profile_id == "parity_complete_v1")
        .expect("parity_complete_v1 evaluation should exist");
    assert!(!parity_eval.pass);
}

#[test]
fn parity_risk_register_fixture_matches_generation() {
    let parity = parity_dir();
    let fixture_corpus_path = parity.join("fixtures/parity_fixture_corpus.json");
    let scorecard_path = parity.join("parity_scorecard.json");
    let register_path = parity.join("parity_risk_register.json");

    let fixture_corpus: ParityFixtureCorpus = load_json(&fixture_corpus_path);
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated = build_risk_register(
        &fixture_corpus,
        &scorecard,
        &fixture_corpus_path.to_string_lossy(),
        &scorecard_path.to_string_lossy(),
    );
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated risk register")
    );
    let fixture_json = fs::read_to_string(register_path).expect("read risk register fixture");
    assert_eq!(generated_json, fixture_json);
}
