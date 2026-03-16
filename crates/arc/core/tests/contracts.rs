#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_used
)]

use std::fs;

use arc_core::{
    ArcAction, ArcActionKind, ArcBenchmark, ArcGameState, ArcOperationMode, ArcRecording,
    ArcRecordingEnvelopeId, ArcRefusalCode, ArcScorePolicyId, ArcScorecard, ArcSolveOutcome,
    ArcSolveRefusal, ArcSolveResultEnvelope, ArcTask, ArcTaskId, GridAnalysisSummary, SolveBudget,
    TraceLocator, canonicalize_task, extract_relation_graph,
    extract_train_correspondence_candidates, summarize_grid, summarize_task_dimensions,
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

#[test]
fn policy_contracts_round_trip_without_breaking_base_recordings() {
    let fixture = serde_json::json!({
        "benchmark": "arc_agi3",
        "task_id": "demo-bridge-task",
        "envelope_id": "recording://demo-bridge-task/session-1",
        "operation_mode": "online",
        "score_policy_id": "arc_agi3_methodology_v1",
        "steps": [
            {
                "step_index": 0,
                "action": { "kind": "RESET" },
                "observation": {
                    "frame": {
                        "width": 2,
                        "height": 2,
                        "pixels": [0, 0, 0, 0]
                    },
                    "available_actions": ["RESET", "ACTION6", "ACTION7"],
                    "game_state": "not_finished"
                },
                "terminal": false
            }
        ]
    });

    let recording: ArcRecording =
        serde_json::from_value(fixture).expect("policy fixture should deserialize");

    assert_eq!(
        recording.envelope_id,
        Some(
            ArcRecordingEnvelopeId::new("recording://demo-bridge-task/session-1")
                .expect("envelope id should validate")
        )
    );
    assert_eq!(recording.operation_mode, Some(ArcOperationMode::Online));
    assert_eq!(
        recording.score_policy_id,
        Some(ArcScorePolicyId::ArcAgi3MethodologyV1)
    );
    assert_eq!(
        recording.steps[0].observation.available_actions,
        vec![
            ArcActionKind::Reset,
            ArcActionKind::Action6,
            ArcActionKind::Action7
        ]
    );
    assert_eq!(
        recording.steps[0].observation.game_state,
        ArcGameState::NotFinished
    );
    assert_eq!(recording.steps[0].action.kind(), ArcActionKind::Reset);
}

#[test]
fn canonicalization_normalizes_colors_and_dimension_summaries() {
    let fixture = fs::read_to_string("fixtures/canonicalization_task.json")
        .expect("canonicalization fixture should be readable from the crate root");
    let task: ArcTask = serde_json::from_str(&fixture).expect("fixture should deserialize");

    let dimension_summary =
        summarize_task_dimensions(&task).expect("dimension summary should compute");
    assert_eq!(dimension_summary.max_width, 3);
    assert_eq!(dimension_summary.max_height, 3);
    assert_eq!(dimension_summary.train_inputs[0].width, 2);
    assert_eq!(dimension_summary.train_outputs[0].width, 3);
    assert_eq!(dimension_summary.test_inputs[0].height, 3);

    let canonical = canonicalize_task(&task).expect("task should canonicalize");
    assert_eq!(
        canonical.color_normalization,
        vec![
            arc_core::ArcColorNormalization {
                original: 0,
                normalized: 0
            },
            arc_core::ArcColorNormalization {
                original: 7,
                normalized: 1
            },
            arc_core::ArcColorNormalization {
                original: 3,
                normalized: 2
            },
            arc_core::ArcColorNormalization {
                original: 5,
                normalized: 3
            },
        ]
    );
    assert_eq!(
        canonical.normalized_train[0].input.grid.cells(),
        &[0, 1, 2, 1]
    );
    assert_eq!(
        canonical.normalized_train[0]
            .input
            .padding_to_task_max
            .right,
        1
    );
    assert_eq!(
        canonical.normalized_train[0]
            .input
            .padding_to_task_max
            .bottom,
        1
    );
    assert_eq!(
        canonical.normalized_train[0].output.grid.cells(),
        &[1, 3, 0]
    );
    assert_eq!(canonical.normalized_test_inputs[0].grid.cells(), &[2, 0, 1]);
}

#[test]
fn object_graph_extraction_is_deterministic() {
    let fixture = fs::read_to_string("fixtures/object_graph_grid.json")
        .expect("object graph fixture should be readable from the crate root");
    let grid: arc_core::ArcGrid =
        serde_json::from_str(&fixture).expect("fixture should deserialize");

    let graph = extract_relation_graph(&grid);
    assert_eq!(graph.objects.len(), 4);
    assert_eq!(graph.objects[0].bbox.min_x, 0);
    assert_eq!(graph.objects[0].bbox.min_y, 0);
    assert_eq!(graph.objects[0].holes, 1);
    assert!(graph.objects[0].shape_signature.horizontal);
    assert!(graph.objects[0].shape_signature.vertical);
    assert!(graph.objects[0].shape_signature.rotational_180);
    assert_eq!(graph.objects[0].mask.width, 3);
    assert_eq!(graph.objects[0].mask.height, 3);
    assert!(!graph.objects[0].mask.is_set(1, 1));
    assert_eq!(graph.objects[1].bbox.min_x, 4);
    assert_eq!(graph.objects[2].bbox.min_y, 4);

    assert!(graph.edges.iter().any(|edge| {
        edge.source == arc_core::ObjectId(0)
            && edge.target == arc_core::ObjectId(1)
            && matches!(edge.kind, arc_core::ObjectRelationKind::LeftOf { gap: 1 })
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == arc_core::ObjectId(2)
            && edge.target == arc_core::ObjectId(3)
            && edge.kind == arc_core::ObjectRelationKind::RowAligned
    }));
}

#[test]
fn correspondence_candidates_match_train_objects_by_shape_and_degree() {
    let fixture = fs::read_to_string("fixtures/correspondence_task.json")
        .expect("correspondence fixture should be readable from the crate root");
    let task: ArcTask = serde_json::from_str(&fixture).expect("fixture should deserialize");

    let correspondence = extract_train_correspondence_candidates(&task)
        .expect("correspondence candidates should extract");

    assert_eq!(correspondence.len(), 1);
    assert_eq!(correspondence[0].input_graph.objects.len(), 2);
    assert_eq!(correspondence[0].output_graph.objects.len(), 2);
    assert_eq!(
        correspondence[0].candidates[0].input_object,
        arc_core::ObjectId(0)
    );
    assert_eq!(
        correspondence[0].candidates[0].output_object,
        arc_core::ObjectId(0)
    );
    assert!(
        correspondence[0].candidates[0]
            .features
            .same_shape_signature
    );
    assert_eq!(correspondence[0].candidates[0].score, 10);
    assert_eq!(
        correspondence[0].candidates[1].input_object,
        arc_core::ObjectId(1)
    );
    assert_eq!(
        correspondence[0].candidates[1].output_object,
        arc_core::ObjectId(1)
    );
}
