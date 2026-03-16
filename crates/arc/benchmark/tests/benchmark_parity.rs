use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_benchmark::{
    ArcBenchmarkError, ArcBenchmarkUsageTotals, ArcInteractiveCheckpointBundle,
    ArcTaskAttemptCheckpoint, ArcTaskCheckpointManager, score_exact_match_task,
    score_interactive_recording,
};
use arc_core::{ArcBenchmark, ArcGameState, ArcScorecardMetadata, ArcTask, ArcTaskId};
use arc_engine::{ArcEngine, load_game_package};
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct BenchmarkParityManifest {
    schema_version: u16,
    bounded_scope: String,
    exact_match: ExactMatchParitySection,
    task_checkpoint: TaskCheckpointParitySection,
    interactive_cases: Vec<InteractiveParityCase>,
    refusal_case: CorruptedCheckpointRefusal,
}

#[derive(Debug, Deserialize)]
struct ExactMatchParitySection {
    benchmark: ArcBenchmark,
    case_ids: Vec<String>,
    expected: ExactMatchParityExpectation,
}

#[derive(Debug, Deserialize)]
struct ExactMatchParityExpectation {
    total_tasks: u32,
    exact_match_tasks: u32,
    total_pairs: u32,
    pairs_correct: u32,
    mean_task_score: f32,
    pair_accuracy: f32,
}

#[derive(Debug, Deserialize)]
struct TaskCheckpointParitySection {
    task_id: ArcTaskId,
    benchmark: ArcBenchmark,
    attempts: Vec<ArcTaskAttemptCheckpoint>,
    next_attempts: Vec<NextAttemptExpectation>,
    expected: TaskCheckpointExpectation,
}

#[derive(Debug, Deserialize)]
struct NextAttemptExpectation {
    test_pair_index: u16,
    max_attempts: u16,
    expected_attempt_index: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct TaskCheckpointExpectation {
    total_cost_usd: f64,
    total_tokens_input: u64,
    total_tokens_output: u64,
}

#[derive(Debug, Deserialize)]
struct InteractiveParityCase {
    case_id: String,
    checkpoint_id: String,
    costs: ArcBenchmarkUsageTotals,
    checkpoint_timestamp_unix_s: u64,
    expected: InteractiveParityExpectation,
}

#[derive(Debug, Deserialize)]
struct InteractiveParityExpectation {
    total_actions: u32,
    resets: u32,
    levels_completed: u16,
    win_levels: u16,
    completed: bool,
    final_state: ArcGameState,
    overall_score: f32,
    step_count: usize,
    next_step_index: u32,
    checkpoint_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CorruptedCheckpointRefusal {
    case_id: String,
    checkpoint_id: String,
    costs: ArcBenchmarkUsageTotals,
    checkpoint_timestamp_unix_s: u64,
    corrupted_recording_digest: String,
}

#[derive(Debug, Deserialize)]
struct ExactMatchManifest {
    cases: Vec<ExactMatchCase>,
}

#[derive(Debug, Deserialize)]
struct ExactMatchCase {
    id: String,
    benchmark: ArcBenchmark,
    task: ArcTask,
    answer_key: arc_benchmark::ArcStaticAnswerKey,
    submission: arc_benchmark::ArcStaticTaskSubmission,
}

#[derive(Debug, Deserialize)]
struct InteractiveManifest {
    cases: Vec<InteractiveCase>,
}

#[derive(Debug, Deserialize)]
struct InteractiveCase {
    id: String,
    package_path: String,
    actions: Vec<arc_core::ArcAction>,
    metadata: ArcScorecardMetadata,
    baseline_actions: Vec<u32>,
}

#[test]
fn benchmark_parity_manifest_covers_exact_match_and_task_checkpoint_surfaces() {
    let parity = load_parity_manifest();
    let exact_match_manifest = load_exact_match_manifest();
    let selected_reports = parity
        .exact_match
        .case_ids
        .iter()
        .map(|case_id| {
            let case = exact_match_manifest
                .cases
                .iter()
                .find(|case| case.id == *case_id)
                .expect("exact-match parity case should exist");
            score_exact_match_task(
                case.benchmark,
                &case.task,
                &case.answer_key,
                &case.submission,
            )
            .expect("exact-match case should score")
        })
        .collect::<Vec<_>>();

    let summary = arc_benchmark::ArcExactMatchBenchmarkSummary::from_task_reports(
        parity.exact_match.benchmark,
        selected_reports,
    )
    .expect("exact-match summary should build");
    assert_eq!(summary.total_tasks, parity.exact_match.expected.total_tasks);
    assert_eq!(
        summary.exact_match_tasks,
        parity.exact_match.expected.exact_match_tasks
    );
    assert_eq!(summary.total_pairs, parity.exact_match.expected.total_pairs);
    assert_eq!(
        summary.pairs_correct,
        parity.exact_match.expected.pairs_correct
    );
    assert_eq!(
        summary.mean_task_score,
        parity.exact_match.expected.mean_task_score
    );
    assert_eq!(
        summary.pair_accuracy,
        parity.exact_match.expected.pair_accuracy
    );

    let temp = TestDir::new("benchmark_parity_static");
    let mut manager = ArcTaskCheckpointManager::open(
        parity.task_checkpoint.task_id.clone(),
        parity.task_checkpoint.benchmark,
        temp.path(),
    )
    .expect("task checkpoint manager should open");
    for attempt in parity.task_checkpoint.attempts.clone() {
        manager
            .record_attempt_at(attempt.clone(), attempt.recorded_at_unix_s)
            .expect("checkpoint attempt should persist");
    }
    drop(manager);

    let manager = ArcTaskCheckpointManager::open(
        parity.task_checkpoint.task_id.clone(),
        parity.task_checkpoint.benchmark,
        temp.path(),
    )
    .expect("task checkpoint manager should reopen");
    assert_eq!(
        manager.checkpoint().total_cost_usd,
        parity.task_checkpoint.expected.total_cost_usd
    );
    assert_eq!(
        manager.checkpoint().total_tokens_input,
        parity.task_checkpoint.expected.total_tokens_input
    );
    assert_eq!(
        manager.checkpoint().total_tokens_output,
        parity.task_checkpoint.expected.total_tokens_output
    );
    for next_attempt in parity.task_checkpoint.next_attempts {
        assert_eq!(
            manager.get_next_attempt_index(next_attempt.test_pair_index, next_attempt.max_attempts),
            next_attempt.expected_attempt_index
        );
    }
    assert!(
        temp.path()
            .join(format!("{}.json", parity.task_checkpoint.task_id.as_str()))
            .exists()
    );
}

#[test]
fn benchmark_parity_manifest_covers_interactive_checkpoint_recording_and_replay() {
    let parity = load_parity_manifest();
    let interactive_manifest = load_interactive_manifest();

    for case in parity.interactive_cases {
        let source = interactive_manifest
            .cases
            .iter()
            .find(|candidate| candidate.id == case.case_id)
            .expect("interactive parity case should exist");
        let recording = replay_case_recording(source);
        let report = score_interactive_recording(
            &recording,
            source.metadata.clone(),
            &source.baseline_actions,
        )
        .expect("interactive recording should score");

        assert_eq!(
            report.total_actions, case.expected.total_actions,
            "{}",
            case.case_id
        );
        assert_eq!(report.resets, case.expected.resets, "{}", case.case_id);
        assert_eq!(
            report.levels_completed, case.expected.levels_completed,
            "{}",
            case.case_id
        );
        assert_eq!(
            report.win_levels, case.expected.win_levels,
            "{}",
            case.case_id
        );
        assert_eq!(
            report.completed, case.expected.completed,
            "{}",
            case.case_id
        );
        assert_eq!(
            report.final_state, case.expected.final_state,
            "{}",
            case.case_id
        );
        assert_eq!(
            report.scorecard.overall_score, case.expected.overall_score,
            "{}",
            case.case_id
        );
        assert_eq!(
            report.step_summaries.len(),
            case.expected.step_count,
            "{}",
            case.case_id
        );

        let temp = TestDir::new(case.checkpoint_id.as_str());
        let checkpoint_dir = temp.path().join(".checkpoint").join(&case.checkpoint_id);
        let bundle = ArcInteractiveCheckpointBundle::from_run_report(
            case.checkpoint_id.clone(),
            &report,
            case.costs.clone(),
            case.checkpoint_timestamp_unix_s,
            recording.clone(),
        )
        .expect("interactive checkpoint bundle should build");
        bundle
            .save_to_dir(&checkpoint_dir)
            .expect("interactive checkpoint bundle should save");

        let mut actual_files = fs::read_dir(&checkpoint_dir)
            .expect("checkpoint dir should list")
            .map(|entry| {
                entry
                    .expect("checkpoint file should read")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        actual_files.sort();
        assert_eq!(
            actual_files, case.expected.checkpoint_files,
            "{}",
            case.case_id
        );

        let loaded = ArcInteractiveCheckpointBundle::load_from_dir(&checkpoint_dir)
            .expect("interactive checkpoint bundle should reload");
        assert_eq!(loaded.costs, case.costs, "{}", case.case_id);
        assert_eq!(loaded.recording, recording, "{}", case.case_id);
        assert_eq!(
            loaded.next_step_index(),
            case.expected.next_step_index,
            "{}",
            case.case_id
        );
        assert_eq!(
            loaded.metadata.total_actions, case.expected.total_actions,
            "{}",
            case.case_id
        );
        assert_eq!(
            loaded.metadata.step_count as usize, case.expected.step_count,
            "{}",
            case.case_id
        );

        let replayed = ArcEngine::replay(
            load_game_package(resolve_package_path(&source.package_path))
                .expect("interactive package should load"),
            &loaded
                .recording
                .steps
                .iter()
                .map(|step| step.action.clone())
                .collect::<Vec<_>>(),
        )
        .expect("replayed recording should reconstruct deterministically");
        assert_eq!(replayed, loaded.recording, "{}", case.case_id);
    }
}

#[test]
fn benchmark_parity_manifest_keeps_checkpoint_refusal_machine_legible() {
    let parity = load_parity_manifest();
    let interactive_manifest = load_interactive_manifest();
    let source = interactive_manifest
        .cases
        .iter()
        .find(|candidate| candidate.id == parity.refusal_case.case_id)
        .expect("refusal source case should exist");
    let recording = replay_case_recording(source);
    let report = score_interactive_recording(
        &recording,
        source.metadata.clone(),
        &source.baseline_actions,
    )
    .expect("interactive recording should score");

    let temp = TestDir::new("benchmark_parity_refusal");
    let checkpoint_dir = temp
        .path()
        .join(".checkpoint")
        .join(&parity.refusal_case.checkpoint_id);
    let bundle = ArcInteractiveCheckpointBundle::from_run_report(
        parity.refusal_case.checkpoint_id.clone(),
        &report,
        parity.refusal_case.costs.clone(),
        parity.refusal_case.checkpoint_timestamp_unix_s,
        recording,
    )
    .expect("refusal bundle should build");
    bundle
        .save_to_dir(&checkpoint_dir)
        .expect("refusal bundle should save");

    let metadata_path = checkpoint_dir.join("metadata.json");
    let mut metadata: arc_benchmark::ArcInteractiveCheckpointMetadata =
        serde_json::from_slice(&fs::read(&metadata_path).expect("metadata should read"))
            .expect("metadata should deserialize");
    metadata.recording_digest = parity.refusal_case.corrupted_recording_digest.clone();
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).expect("metadata should serialize"),
    )
    .expect("metadata should write");

    let error = ArcInteractiveCheckpointBundle::load_from_dir(&checkpoint_dir)
        .expect_err("corrupted checkpoint digest should refuse");
    match error {
        ArcBenchmarkError::InteractiveCheckpointRecordingDigestMismatch {
            checkpoint_id,
            expected,
            actual,
        } => {
            assert_eq!(checkpoint_id, parity.refusal_case.checkpoint_id);
            assert_eq!(expected, parity.refusal_case.corrupted_recording_digest);
            assert_eq!(actual, report.recording_digest);
        }
        other => panic!("unexpected checkpoint refusal: {other}"),
    }
}

fn load_parity_manifest() -> BenchmarkParityManifest {
    let manifest: BenchmarkParityManifest = serde_json::from_str(
        &fs::read_to_string(fixture_path("benchmark_parity_manifest.json"))
            .expect("benchmark parity manifest should load"),
    )
    .expect("benchmark parity manifest should deserialize");
    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("offline/local"));
    manifest
}

fn load_exact_match_manifest() -> ExactMatchManifest {
    serde_json::from_str(
        &fs::read_to_string(fixture_path("exact_match_manifest.json"))
            .expect("exact-match manifest should load"),
    )
    .expect("exact-match manifest should deserialize")
}

fn load_interactive_manifest() -> InteractiveManifest {
    serde_json::from_str(
        &fs::read_to_string(fixture_path("interactive_methodology_manifest.json"))
            .expect("interactive manifest should load"),
    )
    .expect("interactive manifest should deserialize")
}

fn replay_case_recording(case: &InteractiveCase) -> arc_core::ArcRecording {
    ArcEngine::replay(
        load_game_package(resolve_package_path(&case.package_path))
            .expect("interactive package should load"),
        &case.actions,
    )
    .expect("interactive recording should replay")
}

fn resolve_package_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "arc_benchmark_{label}_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&path).expect("temp dir should create");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
