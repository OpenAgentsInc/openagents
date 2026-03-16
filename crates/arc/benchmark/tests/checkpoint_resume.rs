use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_benchmark::{
    ArcBenchmarkError, ArcBenchmarkUsageTotals, ArcInteractiveCheckpointBundle,
    ArcRunManifestManager, ArcTaskAttemptCheckpoint, ArcTaskCheckpointManager,
    score_interactive_recording,
};
use arc_core::{ArcAction, ArcBenchmark, ArcGrid, ArcScorecardMetadata, ArcTaskId};
use arc_engine::{ArcEngine, load_game_package};

#[test]
fn task_checkpoint_manager_resumes_attempts_from_disk_and_refuses_duplicates() {
    let temp = TestDir::new("task_checkpoint");
    let task_id = ArcTaskId::new("checkpoint-task").expect("task id should validate");
    let prediction = ArcGrid::new(1, 1, vec![7]).expect("grid should validate");

    let mut manager =
        ArcTaskCheckpointManager::open(task_id.clone(), ArcBenchmark::ArcAgi2, temp.path())
            .expect("task checkpoint should open");
    assert_eq!(manager.get_next_attempt_index(0, 2), Some(0));

    manager
        .record_attempt_at(
            ArcTaskAttemptCheckpoint {
                attempt_index: 0,
                test_pair_index: 0,
                prediction: Some(prediction.clone()),
                error: None,
                cost_usd: 0.25,
                tokens_input: 11,
                tokens_output: 7,
                duration_millis: 150,
                recorded_at_unix_s: 100,
            },
            100,
        )
        .expect("first attempt should checkpoint");
    manager
        .record_attempt_at(
            ArcTaskAttemptCheckpoint {
                attempt_index: 0,
                test_pair_index: 1,
                prediction: None,
                error: Some("timeout".to_owned()),
                cost_usd: 0.5,
                tokens_input: 13,
                tokens_output: 0,
                duration_millis: 200,
                recorded_at_unix_s: 120,
            },
            120,
        )
        .expect("second attempt should checkpoint");

    let duplicate = manager
        .record_attempt_at(
            ArcTaskAttemptCheckpoint {
                attempt_index: 0,
                test_pair_index: 0,
                prediction: Some(prediction),
                error: None,
                cost_usd: 0.25,
                tokens_input: 1,
                tokens_output: 1,
                duration_millis: 1,
                recorded_at_unix_s: 121,
            },
            121,
        )
        .expect_err("duplicate attempts should refuse");
    match duplicate {
        ArcBenchmarkError::DuplicateCheckpointAttempt {
            task_id: duplicate_task_id,
            test_pair_index,
            attempt_index,
        } => {
            assert_eq!(duplicate_task_id, task_id);
            assert_eq!(test_pair_index, 0);
            assert_eq!(attempt_index, 0);
        }
        other => panic!("unexpected checkpoint error: {other}"),
    }

    drop(manager);

    let manager =
        ArcTaskCheckpointManager::open(task_id.clone(), ArcBenchmark::ArcAgi2, temp.path())
            .expect("task checkpoint should resume");
    assert_eq!(manager.get_completed_attempts().len(), 2);
    assert_eq!(manager.get_next_attempt_index(0, 2), Some(1));
    assert_eq!(manager.get_next_attempt_index(1, 1), None);
    assert_eq!(manager.checkpoint().total_cost_usd, 0.75);
    assert_eq!(manager.checkpoint().total_tokens_input, 24);
    assert_eq!(manager.checkpoint().total_tokens_output, 7);
}

#[test]
fn run_manifest_manager_claims_resets_stale_and_resumes() {
    let temp = TestDir::new("run_manifest");
    let manifest_path = temp.path().join("run_manifest.json");
    let task_a = ArcTaskId::new("run-task-a").expect("task id should validate");
    let task_b = ArcTaskId::new("run-task-b").expect("task id should validate");

    let mut manager = ArcRunManifestManager::open_with_worker_id(
        "demo-run",
        ArcBenchmark::ArcAgi2,
        &manifest_path,
        "worker-a",
    )
    .expect("run manifest should open");
    manager
        .initialize_tasks(&[task_a.clone(), task_b.clone()], 2)
        .expect("tasks should initialize");
    assert_eq!(manager.manifest().pending_count(), 2);

    let claimed = manager
        .claim_next_task_at(100)
        .expect("claim should succeed")
        .expect("first task should be pending");
    assert_eq!(claimed, task_a);
    manager
        .update_task_progress(&task_a, 1, 0.25)
        .expect("progress should update");
    manager
        .mark_completed_at(
            &task_a,
            ArcBenchmarkUsageTotals {
                total_cost_usd: 0.25,
                total_tokens_input: 10,
                total_tokens_output: 4,
            },
            110,
        )
        .expect("completed task should persist");
    assert!(
        manager
            .claim_task_at(&task_b, 120)
            .expect("second task should exist")
    );

    drop(manager);

    let mut resumed = ArcRunManifestManager::open_with_worker_id(
        "demo-run",
        ArcBenchmark::ArcAgi2,
        &manifest_path,
        "worker-b",
    )
    .expect("run manifest should resume");
    assert_eq!(resumed.manifest().completed_count(), 1);
    assert_eq!(resumed.manifest().in_progress_count(), 1);
    assert_eq!(resumed.manifest().total_cost_usd, 0.25);

    let reset = resumed
        .reset_stale_tasks_at(1000, 60)
        .expect("stale tasks should reset");
    assert_eq!(reset, 1);
    assert_eq!(resumed.manifest().pending_count(), 1);
    let reclaimed = resumed
        .claim_next_task_at(1001)
        .expect("reclaim should succeed")
        .expect("reset task should be pending");
    assert_eq!(reclaimed, task_b);
    resumed
        .mark_failed_at(
            &task_b,
            "provider error",
            ArcBenchmarkUsageTotals {
                total_cost_usd: 0.5,
                total_tokens_input: 9,
                total_tokens_output: 0,
            },
            1010,
        )
        .expect("failed task should persist");
    assert_eq!(resumed.manifest().failed_count(), 1);
    assert!(resumed.manifest().is_complete());

    let retried = resumed
        .retry_failed_tasks()
        .expect("retry should reset failures");
    assert_eq!(retried, 1);
    assert_eq!(resumed.manifest().pending_count(), 1);

    let mismatch = ArcRunManifestManager::open("other-run", ArcBenchmark::ArcAgi2, &manifest_path)
        .expect_err("run-id mismatch should refuse");
    match mismatch {
        ArcBenchmarkError::RunManifestRunIdMismatch { expected, actual } => {
            assert_eq!(expected, "other-run");
            assert_eq!(actual, "demo-run");
        }
        other => panic!("unexpected run-manifest error: {other}"),
    }
}

#[test]
fn interactive_checkpoint_bundle_round_trips_and_validates_digest() {
    let temp = TestDir::new("interactive_checkpoint");
    let checkpoint_dir = temp.path().join(".checkpoint").join("demo-card");
    let recording = demo_recording();
    let report = score_interactive_recording(
        &recording,
        ArcScorecardMetadata {
            source_url: Some("https://example.com/arc-208".to_owned()),
            tags: vec!["arc-208".to_owned()],
            opaque: None,
        },
        &[7, 5],
    )
    .expect("recording should score");

    let bundle = ArcInteractiveCheckpointBundle::from_run_report(
        "demo-card",
        &report,
        ArcBenchmarkUsageTotals {
            total_cost_usd: 1.5,
            total_tokens_input: 101,
            total_tokens_output: 33,
        },
        200,
        recording.clone(),
    )
    .expect("bundle should build");
    bundle
        .save_to_dir(&checkpoint_dir)
        .expect("bundle should save");

    let loaded =
        ArcInteractiveCheckpointBundle::load_from_dir(&checkpoint_dir).expect("bundle should load");
    assert_eq!(loaded.metadata.checkpoint_id, "demo-card");
    assert_eq!(loaded.next_step_index(), 13);
    assert_eq!(loaded.costs.total_tokens_input, 101);
    assert_eq!(loaded.metadata.total_actions, 12);
    assert_eq!(
        loaded.recording.contract_digest().expect("digest"),
        report.recording_digest
    );

    let metadata_path = checkpoint_dir.join("metadata.json");
    let mut metadata: arc_benchmark::ArcInteractiveCheckpointMetadata =
        serde_json::from_slice(&fs::read(&metadata_path).expect("metadata should read"))
            .expect("metadata should deserialize");
    metadata.recording_digest = "bogus-digest".to_owned();
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).expect("metadata should serialize"),
    )
    .expect("metadata should write");

    let error = ArcInteractiveCheckpointBundle::load_from_dir(&checkpoint_dir)
        .expect_err("digest mismatch should refuse");
    match error {
        ArcBenchmarkError::InteractiveCheckpointRecordingDigestMismatch {
            checkpoint_id,
            expected,
            ..
        } => {
            assert_eq!(checkpoint_id, "demo-card");
            assert_eq!(expected, "bogus-digest");
        }
        other => panic!("unexpected interactive checkpoint error: {other}"),
    }
}

fn demo_recording() -> arc_core::ArcRecording {
    let package = load_game_package(engine_fixture_path("demo_game.json"))
        .expect("engine fixture should load");
    let actions = vec![
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
    ];
    ArcEngine::replay(package, &actions).expect("recording should replay")
}

fn engine_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../engine/fixtures")
        .join(name)
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
