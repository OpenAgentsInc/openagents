use arc_benchmark::{ArcBenchmarkError, ArcInteractiveRunReport, score_interactive_recording};
use arc_core::{ArcAction, ArcGameState, ArcRecording, ArcScorecardMetadata};
use arc_engine::{ArcEngine, load_game_package};
use serde::Deserialize;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct InteractiveManifest {
    schema_version: u16,
    bounded_scope: String,
    cases: Vec<InteractiveCase>,
}

#[derive(Debug, Deserialize)]
struct InteractiveCase {
    id: String,
    package_path: String,
    actions: Vec<ArcAction>,
    metadata: ArcScorecardMetadata,
    baseline_actions: Vec<u32>,
    expected: InteractiveExpectation,
}

#[derive(Debug, Deserialize)]
struct InteractiveExpectation {
    total_actions: u32,
    resets: u32,
    levels_completed: u16,
    win_levels: u16,
    overall_score: f32,
    final_state: ArcGameState,
    level_scores: Vec<f32>,
    level_actions: Vec<u32>,
    step_count: usize,
    completed: bool,
}

#[test]
fn interactive_manifest_scores_deterministic_recordings() {
    let manifest: InteractiveManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_methodology_manifest.json"))
            .expect("interactive manifest should load"),
    )
    .expect("interactive manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("methodology scoring"));

    for case in manifest.cases {
        let recording = replay_case_recording(&case);
        let report =
            score_interactive_recording(&recording, case.metadata.clone(), &case.baseline_actions)
                .expect("fixture case should score");

        assert_report_matches(&case.id, &report, &case.expected);
        assert_eq!(report.scorecard.metadata, case.metadata, "{}", case.id);
        assert_eq!(
            report
                .scorecard
                .levels
                .iter()
                .map(|level| level.level_index)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "{}",
            case.id
        );
        assert!(!report.recording_digest.is_empty(), "{}", case.id);
    }
}

#[test]
fn interactive_scoring_refuses_baseline_length_mismatch() {
    let case = load_case("demo-perfect-win");
    let recording = replay_case_recording(&case);

    let error = score_interactive_recording(&recording, case.metadata, &[case.baseline_actions[0]])
        .expect_err("baseline mismatch should refuse");

    match error {
        ArcBenchmarkError::BaselineActionLengthMismatch {
            task_id,
            expected,
            actual,
        } => {
            assert_eq!(task_id.as_str(), "arc-engine-demo");
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("unexpected benchmark error: {other}"),
    }
}

#[test]
fn interactive_scoring_refuses_non_initial_full_reset() {
    let case = load_case("demo-perfect-win");
    let mut actions = case.actions[..8].to_vec();
    actions.push(ArcAction::Reset);

    let package = load_game_package(resolve_package_path(&case.package_path))
        .expect("fixture package should load");
    let recording = ArcEngine::replay(package, &actions).expect("recording should replay");

    let error = score_interactive_recording(&recording, case.metadata, &case.baseline_actions)
        .expect_err("non-initial full reset should refuse");

    match error {
        ArcBenchmarkError::UnexpectedFullReset {
            task_id,
            step_index,
        } => {
            assert_eq!(task_id.as_str(), "arc-engine-demo");
            assert_eq!(step_index, 8);
        }
        other => panic!("unexpected benchmark error: {other}"),
    }
}

fn load_case(id: &str) -> InteractiveCase {
    let manifest: InteractiveManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_methodology_manifest.json"))
            .expect("interactive manifest should load"),
    )
    .expect("interactive manifest should deserialize");

    manifest
        .cases
        .into_iter()
        .find(|case| case.id == id)
        .expect("fixture case should exist")
}

fn replay_case_recording(case: &InteractiveCase) -> ArcRecording {
    let package = load_game_package(resolve_package_path(&case.package_path))
        .expect("fixture package should load");
    ArcEngine::replay(package, &case.actions).expect("replay should produce recording")
}

fn resolve_package_path(path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn assert_report_matches(
    case_id: &str,
    report: &ArcInteractiveRunReport,
    expected: &InteractiveExpectation,
) {
    assert_eq!(report.total_actions, expected.total_actions, "{case_id}");
    assert_eq!(report.resets, expected.resets, "{case_id}");
    assert_eq!(
        report.levels_completed, expected.levels_completed,
        "{case_id}"
    );
    assert_eq!(report.win_levels, expected.win_levels, "{case_id}");
    assert_eq!(
        report.scorecard.overall_score, expected.overall_score,
        "{case_id}"
    );
    assert_eq!(report.final_state, expected.final_state, "{case_id}");
    assert_eq!(report.completed, expected.completed, "{case_id}");
    assert_eq!(
        report.step_summaries.len(),
        expected.step_count,
        "{case_id}"
    );
    assert_eq!(
        report
            .scorecard
            .levels
            .iter()
            .map(|level| level.score)
            .collect::<Vec<_>>(),
        expected.level_scores,
        "{case_id}"
    );
    assert_eq!(
        report
            .scorecard
            .levels
            .iter()
            .map(|level| level.action_count)
            .collect::<Vec<_>>(),
        expected.level_actions,
        "{case_id}"
    );

    let last_step = report
        .step_summaries
        .last()
        .expect("run report should include at least one step");
    assert_eq!(last_step.total_actions, expected.total_actions, "{case_id}");
    assert_eq!(
        last_step.levels_completed, expected.levels_completed,
        "{case_id}"
    );
    assert_eq!(last_step.game_state, expected.final_state, "{case_id}");
}
