use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_client::{
    ArcClientError, ArcEnvironmentInfo, ArcJsonlImportContext, ArcRecordingTransportPolicy,
    LocalArcEnvironment, jsonl_entries_to_recording, read_jsonl_recording_file,
    recording_to_jsonl_entries, write_jsonl_recording_file,
};
use arc_core::{ArcAction, ArcOperationMode, ArcTaskId};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join(name)
}

fn recording_fixture() -> arc_core::ArcRecording {
    let info = ArcEnvironmentInfo {
        game_id: ArcTaskId::new("bt11-fd9df0622a1a").expect("task id should validate"),
        title: Some("BT11".to_owned()),
        tags: Vec::new(),
        private_tags: Vec::new(),
        level_tags: Vec::new(),
        baseline_actions: vec![4],
        class_name: None,
        local_package_path: None,
    };
    let mut environment = LocalArcEnvironment::load_from_path(
        info,
        fixture_path("upstream/bt11-fd9df0622a1a.json"),
        "local-card",
    )
    .expect("local fixture should load");
    environment.reset().expect("reset should succeed");
    environment
        .step(ArcAction::Action3)
        .expect("step should succeed");
    let mut recording = environment
        .recording()
        .expect("recording should be readable")
        .expect("recording should exist after reset + step");
    recording.operation_mode = Some(ArcOperationMode::Online);
    recording
}

fn temp_file_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "arc-client-{name}-{}-{}.jsonl",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for temp file")
            .as_nanos()
    ))
}

#[test]
fn jsonl_round_trip_preserves_online_recordings_with_frame_data() {
    let recording = recording_fixture();
    let entries = recording_to_jsonl_entries(&recording, true);
    let path = temp_file_path("recording-roundtrip");

    write_jsonl_recording_file(&path, &entries).expect("jsonl file should write");
    let loaded = read_jsonl_recording_file(&path).expect("jsonl file should read");
    let imported = jsonl_entries_to_recording(
        &loaded,
        ArcJsonlImportContext {
            operation_mode: ArcOperationMode::Online,
            score_policy_id: None,
        },
    )
    .expect("jsonl entries with frames should import");

    assert_eq!(loaded, entries);
    assert_eq!(imported, recording);
    assert_eq!(
        ArcRecordingTransportPolicy::for_operation_mode(ArcOperationMode::Online),
        ArcRecordingTransportPolicy::OnlineJsonl
    );

    let _ = fs::remove_file(path);
}

#[test]
fn jsonl_import_refuses_sparse_entries_without_frame_data() {
    let recording = recording_fixture();
    let entries = recording_to_jsonl_entries(&recording, false);

    let error = jsonl_entries_to_recording(
        &entries,
        ArcJsonlImportContext {
            operation_mode: ArcOperationMode::Online,
            score_policy_id: None,
        },
    )
    .expect_err("sparse jsonl entries should refuse canonical import");

    match error {
        ArcClientError::JsonlFrameDataMissing { line_index, .. } => {
            assert_eq!(line_index, 0);
        }
        other => panic!("unexpected sparse-jsonl error: {other}"),
    }
    assert_eq!(
        ArcRecordingTransportPolicy::for_operation_mode(ArcOperationMode::Offline),
        ArcRecordingTransportPolicy::LocalCanonical
    );
}
