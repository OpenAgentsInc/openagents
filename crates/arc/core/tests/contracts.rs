#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_used
)]

use std::fs;

use arc_core::{
    ArcAction, ArcBenchmark, ArcRecording, ArcRefusalCode, ArcScorecard, ArcSolveOutcome,
    ArcSolveRefusal, ArcSolveResultEnvelope, ArcTask, ArcTaskId, GridAnalysisSummary, SolveBudget,
    TraceLocator, summarize_grid,
};

#[test]
fn minimal_task_fixture_round_trips() {
    let fixture = fs::read_to_string("fixtures/minimal_task.json")
        .expect("fixture should be readable from the crate root");
    let task: ArcTask = serde_json::from_str(&fixture).expect("fixture should deserialize");

    assert_eq!(
        task.id,
        ArcTaskId::new("demo-bridge-task").expect("valid task id")
    );
    assert_eq!(task.train.len(), 1);
    assert_eq!(task.test.len(), 1);
}

#[test]
fn analysis_summary_matches_expected_contract() {
    let fixture = fs::read_to_string("fixtures/minimal_task.json")
        .expect("fixture should be readable from the crate root");
    let task: ArcTask = serde_json::from_str(&fixture).expect("fixture should deserialize");
    let analysis = summarize_grid(&task.train[0].input);

    assert_eq!(
        analysis,
        GridAnalysisSummary {
            palette: vec![0, 1, 2],
            non_background_cell_count: 2,
            bounding_box: Some(arc_core::ArcBoundingBox {
                min_x: 0,
                min_y: 0,
                max_x: 1,
                max_y: 1,
            }),
        }
    );
}

#[test]
fn solve_budget_serializes_stably() {
    let budget = SolveBudget::new(8, 64, 1_500).expect("budget should validate");

    let serialized = serde_json::to_string(&budget).expect("budget should serialize");
    assert_eq!(
        serialized,
        "{\"max_attempts\":8,\"max_steps\":64,\"max_runtime_millis\":1500}"
    );
}

#[test]
fn recording_fixture_round_trips() {
    let fixture = fs::read_to_string("fixtures/minimal_recording.json")
        .expect("recording fixture should be readable from the crate root");
    let recording: ArcRecording =
        serde_json::from_str(&fixture).expect("recording fixture should deserialize");

    assert_eq!(recording.benchmark, ArcBenchmark::ArcAgi3);
    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.steps[0].action, ArcAction::Reset);
    assert_eq!(
        recording.steps[1].action,
        ArcAction::action6(1, 0).expect("valid coordinate")
    );
    assert!(recording.steps[1].terminal);
}

#[test]
fn scorecard_fixture_round_trips() {
    let fixture = fs::read_to_string("fixtures/minimal_scorecard.json")
        .expect("scorecard fixture should be readable from the crate root");
    let scorecard: ArcScorecard =
        serde_json::from_str(&fixture).expect("scorecard fixture should deserialize");

    assert_eq!(scorecard.benchmark, ArcBenchmark::ArcAgi3);
    assert_eq!(scorecard.metadata.tags, vec!["fixture", "demo"]);
    assert_eq!(scorecard.levels.len(), 1);
    assert_eq!(scorecard.levels[0].action_count, 2);
}

#[test]
fn solve_result_envelope_validates_attempts_and_trace_locator() {
    let budget = SolveBudget::new(4, 64, 2_000).expect("budget should validate");
    let trace_locator =
        TraceLocator::new("trace://arc-core/demo-bridge-task/attempt-1").expect("valid trace");
    let outcome = ArcSolveOutcome::Refused(
        ArcSolveRefusal::new(ArcRefusalCode::BudgetExhausted, "attempt budget spent")
            .expect("refusal detail should validate"),
    );

    let envelope = ArcSolveResultEnvelope::new(
        ArcTaskId::new("demo-bridge-task").expect("valid task id"),
        budget,
        2,
        Some(trace_locator),
        outcome,
    )
    .expect("envelope should validate");

    let serialized = serde_json::to_string(&envelope).expect("envelope should serialize");
    assert!(serialized.contains("trace://arc-core/demo-bridge-task/attempt-1"));
}

#[test]
fn solve_budget_and_refusal_reject_empty_contracts() {
    assert!(SolveBudget::new(0, 1, 1).is_err());
    assert!(TraceLocator::new(" ").is_err());
    assert!(ArcSolveRefusal::new(ArcRefusalCode::UnsupportedTask, "   ").is_err());
}

#[test]
fn frozen_task_body_contracts_remain_stable() {
    let fixture = fs::read_to_string("fixtures/minimal_task.json")
        .expect("fixture should be readable from the crate root");
    let task: ArcTask = serde_json::from_str(&fixture).expect("fixture should deserialize");

    assert_eq!(
        task.canonical_body_json()
            .expect("task body should serialize canonically"),
        "{\"test\":[{\"cells\":[0,2,1,0],\"height\":2,\"width\":2}],\"train\":[{\"input\":{\"cells\":[0,1,2,0],\"height\":2,\"width\":2},\"output\":{\"cells\":[1,1,2,2],\"height\":2,\"width\":2}}]}"
    );
    assert_eq!(
        task.body_digest().expect("task body digest should compute"),
        "032cb5b41656d23b3e965ed6bd549ba9429580eceb28f4a8689a8f662ebc137b"
    );
    assert_eq!(
        task.derived_task_id()
            .expect("derived task id should compute"),
        ArcTaskId::new("task-032cb5b41656d23b").expect("derived task id should validate")
    );
}

#[test]
fn frozen_interactive_contract_digests_remain_stable() {
    let recording_fixture = fs::read_to_string("fixtures/minimal_recording.json")
        .expect("recording fixture should be readable from the crate root");
    let recording: ArcRecording =
        serde_json::from_str(&recording_fixture).expect("recording fixture should deserialize");

    let scorecard_fixture = fs::read_to_string("fixtures/minimal_scorecard.json")
        .expect("scorecard fixture should be readable from the crate root");
    let scorecard: ArcScorecard =
        serde_json::from_str(&scorecard_fixture).expect("scorecard fixture should deserialize");

    let solve_result_fixture = fs::read_to_string("fixtures/minimal_solve_result.json")
        .expect("solve result fixture should be readable from the crate root");
    let solve_result: ArcSolveResultEnvelope = serde_json::from_str(&solve_result_fixture)
        .expect("solve result fixture should deserialize");

    assert_eq!(
        recording
            .contract_digest()
            .expect("recording digest should compute"),
        "3bb2d28e5bddfafab4bcad32e0718b0868320104475ccb5a3e990171ffccf8d2"
    );
    assert_eq!(
        scorecard
            .contract_digest()
            .expect("scorecard digest should compute"),
        "ad551bc4aae918fc9a6e7ef4e0174462664e2e417b4c08d4a82c3699ad578316"
    );
    assert_eq!(
        solve_result
            .canonical_json()
            .expect("solve result should serialize canonically"),
        "{\"attempts_used\":2,\"budget\":{\"max_attempts\":4,\"max_runtime_millis\":2000,\"max_steps\":64},\"outcome\":{\"Refused\":{\"code\":\"BudgetExhausted\",\"detail\":\"attempt budget spent\"}},\"schema_version\":1,\"task_id\":\"demo-bridge-task\",\"trace_locator\":\"trace://arc-core/demo-bridge-task/attempt-1\"}"
    );
    assert_eq!(
        solve_result
            .contract_digest()
            .expect("solve result digest should compute"),
        "c9b702998afabbb88f07636836efdefb0f15c29141bda940e247f7dcf674e045"
    );
}
