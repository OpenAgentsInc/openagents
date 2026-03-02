use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::ci_artifacts::ParityCiArtifactManifest;
use openagents_cad::parity::dashboard::{
    PARITY_DASHBOARD_ISSUE_ID, ParityDashboard, build_dashboard, render_dashboard_markdown,
};
use openagents_cad::parity::risk_register::ParityRiskRegister;
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

fn docs_dir() -> PathBuf {
    repo_root().join("crates/cad/docs")
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed reading {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing {}: {error}", path.display()))
}

#[test]
fn parity_dashboard_fixture_is_well_formed() {
    let path = parity_dir().join("parity_dashboard.json");
    let dashboard: ParityDashboard = load_json(&path);
    assert_eq!(dashboard.manifest_version, 1);
    assert_eq!(dashboard.issue_id, PARITY_DASHBOARD_ISSUE_ID);
    assert_eq!(dashboard.phase_status, "phase_d_revolve_complete");
    assert_eq!(dashboard.artifacts.source_artifact_count, 48);
    assert!(dashboard.summary.overall_match_rate > 0.0);
}

#[test]
fn parity_dashboard_outputs_match_generation() {
    let parity = parity_dir();
    let docs = docs_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let risk_register_path = parity.join("parity_risk_register.json");
    let ci_manifest_path = parity.join("parity_ci_artifact_manifest.json");
    let dashboard_json_path = parity.join("parity_dashboard.json");
    let dashboard_markdown_path = docs.join("PARITY_BASELINE_DASHBOARD.md");

    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let risk_register: ParityRiskRegister = load_json(&risk_register_path);
    let ci_manifest: ParityCiArtifactManifest = load_json(&ci_manifest_path);
    let generated = build_dashboard(
        &scorecard,
        &risk_register,
        &ci_manifest,
        &scorecard_path.to_string_lossy(),
        &risk_register_path.to_string_lossy(),
        &ci_manifest_path.to_string_lossy(),
    );
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated dashboard")
    );
    let fixture_json = fs::read_to_string(dashboard_json_path).expect("read dashboard fixture");
    assert_eq!(generated_json, fixture_json);

    let generated_markdown = render_dashboard_markdown(&generated);
    let fixture_markdown =
        fs::read_to_string(dashboard_markdown_path).expect("read dashboard markdown");
    assert_eq!(generated_markdown, fixture_markdown);
}
