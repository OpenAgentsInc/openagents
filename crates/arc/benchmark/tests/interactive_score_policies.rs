use arc_benchmark::{ArcBenchmarkError, score_interactive_recording};
use arc_core::{
    ArcAction, ArcGameState, ArcOperationMode, ArcRecording, ArcScorePolicyId, ArcScorecardMetadata,
};
use arc_engine::{ArcEngine, load_game_package};
use serde::Deserialize;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct InteractiveScorePolicyManifest {
    schema_version: u16,
    bounded_scope: String,
    success_cases: Vec<InteractiveScorePolicyCase>,
    refusal_cases: Vec<InteractiveScorePolicyRefusalCase>,
}

#[derive(Debug, Deserialize)]
struct InteractiveScorePolicyCase {
    id: String,
    package_path: String,
    actions: Vec<ArcAction>,
    #[serde(default)]
    operation_mode: Option<ArcOperationMode>,
    #[serde(default)]
    score_policy_id: Option<ArcScorePolicyId>,
    metadata: ArcScorecardMetadata,
    baseline_actions: Vec<u32>,
    expected: InteractiveScorePolicyExpectation,
}

#[derive(Debug, Deserialize)]
struct InteractiveScorePolicyExpectation {
    score_policy_id: ArcScorePolicyId,
    overall_score: f32,
    total_actions: u32,
    levels_completed: u16,
    final_state: ArcGameState,
}

#[derive(Debug, Deserialize)]
struct InteractiveScorePolicyRefusalCase {
    id: String,
    package_path: String,
    actions: Vec<ArcAction>,
    operation_mode: Option<ArcOperationMode>,
    score_policy_id: Option<ArcScorePolicyId>,
    metadata: ArcScorecardMetadata,
    baseline_actions: Vec<u32>,
    expected_error: InteractivePolicyErrorExpectation,
}

#[derive(Debug, Deserialize)]
struct InteractivePolicyErrorExpectation {
    kind: String,
    score_policy_id: ArcScorePolicyId,
    operation_mode: Option<ArcOperationMode>,
}

#[test]
fn interactive_score_policy_manifest_scores_supported_policy_variants() {
    let manifest = load_manifest();

    for case in manifest.success_cases {
        let report = score_interactive_recording(
            &case.recording(),
            case.metadata.clone(),
            &case.baseline_actions,
        )
        .expect("supported interactive score policy should score");

        assert_eq!(
            report.score_policy_id, case.expected.score_policy_id,
            "{}",
            case.id
        );
        assert_eq!(
            report.scorecard.score_policy_id,
            Some(case.expected.score_policy_id),
            "{}",
            case.id
        );
        assert_eq!(
            report.scorecard.overall_score, case.expected.overall_score,
            "{}",
            case.id
        );
        assert_eq!(
            report.total_actions, case.expected.total_actions,
            "{}",
            case.id
        );
        assert_eq!(
            report.levels_completed, case.expected.levels_completed,
            "{}",
            case.id
        );
        assert_eq!(report.final_state, case.expected.final_state, "{}", case.id);
    }
}

#[test]
fn interactive_score_policy_manifest_refuses_policy_mode_mismatch() {
    let manifest = load_manifest();

    for case in manifest.refusal_cases {
        let error = score_interactive_recording(
            &case.recording(),
            case.metadata.clone(),
            &case.baseline_actions,
        )
        .expect_err("policy mismatch should refuse");

        match error {
            ArcBenchmarkError::InteractiveScorePolicyModeMismatch {
                score_policy_id,
                operation_mode,
            } => {
                assert_eq!(
                    case.expected_error.kind, "policy_mode_mismatch",
                    "{}",
                    case.id
                );
                assert_eq!(
                    score_policy_id, case.expected_error.score_policy_id,
                    "{}",
                    case.id
                );
                assert_eq!(
                    operation_mode, case.expected_error.operation_mode,
                    "{}",
                    case.id
                );
            }
            other => panic!(
                "unexpected interactive policy error for {}: {other}",
                case.id
            ),
        }
    }
}

impl InteractiveScorePolicyCase {
    fn recording(&self) -> ArcRecording {
        let mut recording = replay_case_recording(&self.package_path, &self.actions);
        recording.operation_mode = self.operation_mode;
        recording.score_policy_id = self.score_policy_id;
        recording
    }
}

impl InteractiveScorePolicyRefusalCase {
    fn recording(&self) -> ArcRecording {
        let mut recording = replay_case_recording(&self.package_path, &self.actions);
        recording.operation_mode = self.operation_mode;
        recording.score_policy_id = self.score_policy_id;
        recording
    }
}

fn load_manifest() -> InteractiveScorePolicyManifest {
    let manifest: InteractiveScorePolicyManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_score_policies_manifest.json"))
            .expect("interactive score policy manifest should load"),
    )
    .expect("interactive score policy manifest should deserialize");
    assert_eq!(manifest.schema_version, 1);
    assert!(
        manifest
            .bounded_scope
            .contains("versioned interactive score-policy behavior")
    );
    manifest
}

fn replay_case_recording(package_path: &str, actions: &[ArcAction]) -> ArcRecording {
    let package = load_game_package(resolve_package_path(package_path))
        .expect("interactive policy package should load");
    ArcEngine::replay(package, actions).expect("interactive policy recording should replay")
}

fn resolve_package_path(path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}
