use arc_benchmark::{
    ArcBenchmarkError, ArcBenchmarkUsageTotals, ArcRepeatedRunSpec, aggregate_repeated_runs,
    score_exact_match_task, score_interactive_recording, summarize_exact_match_run,
    summarize_interactive_run,
};
use arc_core::{
    ArcAction, ArcBenchmark, ArcExample, ArcGrid, ArcScorecardMetadata, ArcTask, ArcTaskId,
};
use arc_engine::{ArcEngine, load_game_package};
use psionic_eval::{BenchmarkAggregationKind, BenchmarkExecutionMode, BenchmarkPackageKey};

fn engine_fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../engine/fixtures")
        .join(name)
}

#[test]
fn run_summaries_bridge_exact_match_and_interactive_reports() {
    let exact_match_summary = exact_match_summary_fixture();
    let exact_match_run = summarize_exact_match_run(
        "static-run-1",
        &exact_match_summary,
        ArcBenchmarkUsageTotals {
            total_cost_usd: 0.75,
            total_tokens_input: 18,
            total_tokens_output: 6,
        },
        4_200,
    )
    .expect("exact-match summary should convert");
    assert_eq!(exact_match_run.benchmark, ArcBenchmark::ArcAgi2);
    assert_eq!(exact_match_run.score_bps, 5_000);
    assert_eq!(exact_match_run.pass_rate_bps, 5_000);
    assert_eq!(exact_match_run.total_samples, 2);
    assert_eq!(exact_match_run.passed_samples, 1);
    assert!(exact_match_run.total_actions.is_none());

    let interactive_run = summarize_interactive_run(
        "interactive-run-1",
        &interactive_perfect_report(),
        ArcBenchmarkUsageTotals {
            total_cost_usd: 1.25,
            total_tokens_input: 101,
            total_tokens_output: 27,
        },
        5_000,
    )
    .expect("interactive summary should convert");
    assert_eq!(interactive_run.benchmark, ArcBenchmark::ArcAgi3);
    assert_eq!(interactive_run.score_bps, 10_000);
    assert_eq!(interactive_run.pass_rate_bps, 10_000);
    assert_eq!(interactive_run.total_actions, Some(12));
}

#[test]
fn repeated_run_aggregation_builds_psionic_eval_and_research_views() {
    let perfect = summarize_interactive_run(
        "interactive-perfect",
        &interactive_perfect_report(),
        ArcBenchmarkUsageTotals {
            total_cost_usd: 1.0,
            total_tokens_input: 120,
            total_tokens_output: 30,
        },
        5_000,
    )
    .expect("perfect run should summarize");
    let partial = summarize_interactive_run(
        "interactive-partial",
        &interactive_partial_report(),
        ArcBenchmarkUsageTotals {
            total_cost_usd: 0.5,
            total_tokens_input: 80,
            total_tokens_output: 12,
        },
        3_000,
    )
    .expect("partial run should summarize");

    let aggregate = aggregate_repeated_runs(
        &ArcRepeatedRunSpec {
            benchmark: ArcBenchmark::ArcAgi3,
            benchmark_package_key: BenchmarkPackageKey::new("arc.agi3.demo", "v1"),
            execution_mode: BenchmarkExecutionMode::OperatorSimulation,
            aggregation: BenchmarkAggregationKind::MeanScore,
            experiment_id: String::from("arc-209-demo"),
            candidate_id: String::from("candidate-a"),
            output_root: String::from("runs/arc-209-demo"),
        },
        vec![perfect.clone(), partial.clone()],
    )
    .expect("repeated runs should aggregate");

    assert_eq!(aggregate.round_count, 2);
    assert_eq!(aggregate.aggregate_score_bps, 6_666);
    assert_eq!(aggregate.aggregate_pass_rate_bps, 5_000);
    assert_eq!(aggregate.mean_actions, Some(9.5));
    assert_eq!(aggregate.eval_summary.aggregate_score_bps, Some(6_666));
    assert_eq!(aggregate.eval_summary.aggregate_pass_rate_bps, 5_000);
    assert_eq!(aggregate.research_result.scores.len(), 2);
    assert!(!aggregate.research_evaluation.hard_gate_failed);
    assert!(aggregate.research_evaluation.missing_metrics.is_empty());

    let mismatch = aggregate_repeated_runs(
        &ArcRepeatedRunSpec {
            benchmark: ArcBenchmark::ArcAgi3,
            benchmark_package_key: BenchmarkPackageKey::new("arc.agi3.demo", "v1"),
            execution_mode: BenchmarkExecutionMode::OperatorSimulation,
            aggregation: BenchmarkAggregationKind::MeanScore,
            experiment_id: String::from("arc-209-mismatch"),
            candidate_id: String::from("candidate-b"),
            output_root: String::from("runs/arc-209-mismatch"),
        },
        vec![
            perfect,
            summarize_exact_match_run(
                "static-mismatch",
                &exact_match_summary_fixture(),
                ArcBenchmarkUsageTotals::default(),
                1_000,
            )
            .expect("static run should summarize"),
        ],
    )
    .expect_err("mixed benchmarks should refuse");
    match mismatch {
        ArcBenchmarkError::RepeatedRunBenchmarkMismatch {
            expected,
            actual,
            run_id,
        } => {
            assert_eq!(expected, ArcBenchmark::ArcAgi3);
            assert_eq!(actual, ArcBenchmark::ArcAgi2);
            assert_eq!(run_id, "static-mismatch");
        }
        other => panic!("unexpected aggregation error: {other}"),
    }
}

fn exact_match_summary_fixture() -> arc_benchmark::ArcExactMatchBenchmarkSummary {
    let perfect_task = ArcTask::new(
        ArcTaskId::new("exact-perfect").expect("task id should validate"),
        vec![ArcExample {
            input: ArcGrid::new(1, 1, vec![1]).expect("grid should validate"),
            output: ArcGrid::new(1, 1, vec![2]).expect("grid should validate"),
        }],
        vec![ArcGrid::new(1, 1, vec![2]).expect("grid should validate")],
    )
    .expect("task should validate");
    let missed_task = ArcTask::new(
        ArcTaskId::new("exact-missed").expect("task id should validate"),
        vec![ArcExample {
            input: ArcGrid::new(1, 1, vec![3]).expect("grid should validate"),
            output: ArcGrid::new(1, 1, vec![4]).expect("grid should validate"),
        }],
        vec![ArcGrid::new(1, 1, vec![4]).expect("grid should validate")],
    )
    .expect("task should validate");

    let perfect_report = score_exact_match_task(
        ArcBenchmark::ArcAgi2,
        &perfect_task,
        &arc_benchmark::ArcStaticAnswerKey::new(
            perfect_task.id.clone(),
            vec![ArcGrid::new(1, 1, vec![2]).expect("grid should validate")],
        )
        .expect("answer key should validate"),
        &arc_benchmark::ArcStaticTaskSubmission {
            task_id: perfect_task.id.clone(),
            test_pairs: vec![arc_benchmark::ArcStaticPairSubmission {
                attempts: vec![Some(
                    ArcGrid::new(1, 1, vec![2]).expect("grid should validate"),
                )],
            }],
        },
    )
    .expect("perfect task should score");
    let missed_report = score_exact_match_task(
        ArcBenchmark::ArcAgi2,
        &missed_task,
        &arc_benchmark::ArcStaticAnswerKey::new(
            missed_task.id.clone(),
            vec![ArcGrid::new(1, 1, vec![4]).expect("grid should validate")],
        )
        .expect("answer key should validate"),
        &arc_benchmark::ArcStaticTaskSubmission {
            task_id: missed_task.id.clone(),
            test_pairs: vec![arc_benchmark::ArcStaticPairSubmission {
                attempts: vec![Some(
                    ArcGrid::new(1, 1, vec![0]).expect("grid should validate"),
                )],
            }],
        },
    )
    .expect("missed task should score");

    arc_benchmark::ArcExactMatchBenchmarkSummary::from_task_reports(
        ArcBenchmark::ArcAgi2,
        vec![perfect_report, missed_report],
    )
    .expect("benchmark summary should build")
}

fn interactive_perfect_report() -> arc_benchmark::ArcInteractiveRunReport {
    let recording = demo_recording(vec![
        ArcAction::Reset,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::action6(22, 22).expect("coords should validate"),
        ArcAction::Action4,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action5,
    ]);
    score_interactive_recording(
        &recording,
        ArcScorecardMetadata {
            source_url: Some("https://example.com/arc-209-perfect".to_owned()),
            tags: vec!["arc-209".to_owned()],
            opaque: None,
        },
        &[7, 5],
    )
    .expect("interactive run should score")
}

fn interactive_partial_report() -> arc_benchmark::ArcInteractiveRunReport {
    let recording = demo_recording(vec![
        ArcAction::Reset,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::action6(22, 22).expect("coords should validate"),
        ArcAction::Action4,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action2,
    ]);
    score_interactive_recording(
        &recording,
        ArcScorecardMetadata {
            source_url: Some("https://example.com/arc-209-partial".to_owned()),
            tags: vec!["arc-209".to_owned()],
            opaque: None,
        },
        &[7, 5],
    )
    .expect("interactive run should score")
}

fn demo_recording(actions: Vec<ArcAction>) -> arc_core::ArcRecording {
    let package =
        load_game_package(engine_fixture_path("demo_game.json")).expect("fixture should load");
    ArcEngine::replay(package, &actions).expect("recording should replay")
}
