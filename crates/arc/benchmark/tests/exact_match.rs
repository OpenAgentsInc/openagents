use arc_benchmark::{
    ArcBenchmarkError, ArcExactMatchBenchmarkSummary, ArcStaticPairSubmission,
    ArcStaticTaskSubmission, score_exact_match_task,
};
use arc_core::{ArcBenchmark, ArcGrid, ArcTask, ArcTaskId};
use serde::Deserialize;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct ExactMatchManifest {
    schema_version: u16,
    bounded_scope: String,
    cases: Vec<ExactMatchCase>,
}

#[derive(Debug, Deserialize)]
struct ExactMatchCase {
    id: String,
    benchmark: ArcBenchmark,
    task: ArcTask,
    answer_key: arc_benchmark::ArcStaticAnswerKey,
    submission: ArcStaticTaskSubmission,
    expected: ExactMatchExpectation,
}

#[derive(Debug, Deserialize)]
struct ExactMatchExpectation {
    pairs_correct: u16,
    total_pairs: u16,
    score: f32,
    matched_attempt_indices: Vec<Option<u16>>,
}

#[test]
fn exact_match_manifest_scores_static_tasks_and_aggregates_reports() {
    let manifest: ExactMatchManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("exact_match_manifest.json"))
            .expect("exact-match manifest should load"),
    )
    .expect("exact-match manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("exact-match scorer"));

    let mut agi2_reports = Vec::new();

    for case in manifest.cases {
        let report = score_exact_match_task(
            case.benchmark,
            &case.task,
            &case.answer_key,
            &case.submission,
        )
        .expect("fixture case should score");

        assert_eq!(report.task_id, case.task.id, "{}", case.id);
        assert_eq!(
            report.pairs_correct, case.expected.pairs_correct,
            "{}",
            case.id
        );
        assert_eq!(report.total_pairs, case.expected.total_pairs, "{}", case.id);
        assert_eq!(report.score, case.expected.score, "{}", case.id);
        assert_eq!(
            report
                .pair_reports
                .iter()
                .map(|pair| pair.matched_attempt_index)
                .collect::<Vec<_>>(),
            case.expected.matched_attempt_indices,
            "{}",
            case.id
        );
        assert!(
            report
                .pair_reports
                .iter()
                .all(|pair| !pair.expected_output_digest.is_empty()),
            "{}",
            case.id
        );

        if case.benchmark == ArcBenchmark::ArcAgi2 {
            agi2_reports.push(report);
        } else {
            assert!(report.is_exact_match(), "{}", case.id);
        }
    }

    let summary =
        ArcExactMatchBenchmarkSummary::from_task_reports(ArcBenchmark::ArcAgi2, agi2_reports)
            .expect("summary should build");
    assert_eq!(summary.total_tasks, 2);
    assert_eq!(summary.exact_match_tasks, 0);
    assert_eq!(summary.total_pairs, 4);
    assert_eq!(summary.pairs_correct, 2);
    assert_eq!(summary.mean_task_score, 0.5);
    assert_eq!(summary.pair_accuracy, 0.5);
}

#[test]
fn exact_match_refuses_extra_submission_pairs() {
    let task = ArcTask::new(
        ArcTaskId::new("demo_extra_pairs").expect("task id should validate"),
        vec![arc_core::ArcExample {
            input: ArcGrid::new(1, 1, vec![1]).expect("grid should validate"),
            output: ArcGrid::new(1, 1, vec![1]).expect("grid should validate"),
        }],
        vec![ArcGrid::new(1, 1, vec![2]).expect("grid should validate")],
    )
    .expect("task should validate");
    let answer_key = arc_benchmark::ArcStaticAnswerKey::new(
        task.id.clone(),
        vec![ArcGrid::new(1, 1, vec![2]).expect("grid should validate")],
    )
    .expect("answer key should validate");
    let submission = ArcStaticTaskSubmission {
        task_id: task.id.clone(),
        test_pairs: vec![
            ArcStaticPairSubmission {
                attempts: vec![Some(
                    ArcGrid::new(1, 1, vec![2]).expect("grid should validate"),
                )],
            },
            ArcStaticPairSubmission {
                attempts: vec![Some(
                    ArcGrid::new(1, 1, vec![3]).expect("grid should validate"),
                )],
            },
        ],
    };

    let error = score_exact_match_task(ArcBenchmark::ArcAgi1, &task, &answer_key, &submission)
        .expect_err("extra pairs should refuse");
    match error {
        ArcBenchmarkError::ExtraSubmissionPairs {
            task_id,
            expected,
            actual,
        } => {
            assert_eq!(task_id, task.id);
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("unexpected benchmark error: {other}"),
    }
}
