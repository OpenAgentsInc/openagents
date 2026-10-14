use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use robot_data::dataset::{Dataset, SourcePin};
use robot_data::mapping::{JointMapping, Mapping, Unit};
use robot_data::replay::{CameraAt, Playback};
use serde_json::Value;

fn fixture(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(version)
}

fn dataset(version: &str) -> Dataset {
    let root = fixture(version);
    let pin: SourcePin =
        serde_json::from_slice(&std::fs::read(root.join("source.json")).unwrap()).unwrap();
    Dataset::open_pinned(root, &pin).unwrap()
}

#[test]
fn both_layouts_reproduce_pinned_python_reference_values_exactly() {
    let expected: Value = serde_json::from_str(include_str!("fixtures/expected.json")).unwrap();
    for version in ["v2.1", "v3.0"] {
        let dataset = dataset(version);
        assert_eq!(dataset.episode_indices(), vec![0, 1]);
        for index in [0, 1] {
            let episode = dataset.episode(index).unwrap();
            assert_eq!(episode.frames.len(), 4);
            assert_eq!(episode.state_features.len(), 6);
            for (frame, raw) in episode.frames.iter().zip(
                expected["frames"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|f| f["episode_index"] == index),
            ) {
                assert_eq!(frame.frame_index, raw["frame_index"].as_u64().unwrap());
                assert_eq!(
                    frame.timestamp.to_bits(),
                    raw["timestamp"].as_f64().unwrap().to_bits()
                );
                for (value, recorded) in frame
                    .state
                    .iter()
                    .zip(raw["observation.state"].as_array().unwrap())
                {
                    assert_eq!(value.to_bits(), recorded.as_f64().unwrap().to_bits());
                }
                for (value, recorded) in frame.action.iter().zip(raw["action"].as_array().unwrap())
                {
                    assert_eq!(value.to_bits(), recorded.as_f64().unwrap().to_bits());
                }
                assert_eq!(episode.tasks[&frame.task_index], "pick_and_place");
            }
        }
    }
}

#[test]
fn shared_video_times_include_the_episode_offset_and_missing_segments_are_blank() {
    let episode = dataset("v3.0").episode(1).unwrap();
    let selected = episode.frame_at(0.04).unwrap();
    assert_eq!(selected.frame_index, 1);
    let camera = &episode.cameras["observation.images.camera1"];
    assert!(camera.from_seconds > 20.0);
    let CameraAt::Frame {
        timestamp, frame, ..
    } = episode
        .camera_at("observation.images.camera1", selected)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(timestamp, camera.from_seconds + selected.timestamp);
    assert_eq!(frame, (timestamp * camera.fps).round() as u64);
    assert_eq!(
        episode.camera_at("absent", selected).unwrap(),
        CameraAt::Missing
    );
    let mut shortened = episode.clone();
    shortened
        .cameras
        .get_mut("observation.images.camera1")
        .unwrap()
        .to_seconds = camera.from_seconds + 0.01;
    assert_eq!(
        shortened
            .camera_at("observation.images.camera1", selected)
            .unwrap(),
        CameraAt::Missing
    );
}

#[test]
fn scrubbing_speed_pause_and_hidden_playback_keep_one_recorded_clock() {
    let episode = Arc::new(dataset("v3.0").episode(0).unwrap());
    let mut playback = Playback::new(episode).unwrap();
    playback.play();
    assert_eq!(playback.advance(0.04).unwrap().frame_index, 1);
    playback.pause();
    assert_eq!(playback.advance(0.05).unwrap().frame_index, 1);
    playback.seek(0.0).unwrap();
    playback.speed(2.0).unwrap();
    playback.play();
    assert_eq!(playback.advance(0.04).unwrap().frame_index, 2);
    playback.set_visible(false);
    assert_eq!(playback.advance(0.01).unwrap().frame_index, 2);
    playback.set_visible(true);
    assert_eq!(playback.advance(0.1).unwrap().frame_index, 3);
    assert!(!playback.is_playing());
    assert!(playback.seek(100.0).is_err());
    assert!(playback.speed(f64::NAN).is_err());
}

#[test]
fn mapping_is_explicit_in_units_offsets_directions_ranges_and_order() {
    let mapping = Mapping {
        schema: "robot-joint-mapping.v1".into(),
        robot_type: "test-arm".into(),
        robot_version: "a".repeat(64),
        joints: vec![
            JointMapping {
                feature: "lift.pos".into(),
                joint: "lift".into(),
                unit: Unit::Degrees,
                offset: 0.1,
                direction: -1.0,
                lower: -4.0,
                upper: 4.0,
            },
            JointMapping {
                feature: "grip.pos".into(),
                joint: "grip".into(),
                unit: Unit::Normalized {
                    source_min: 0.0,
                    source_max: 100.0,
                    target_min: 0.0,
                    target_max: 1.0,
                },
                offset: 0.0,
                direction: 1.0,
                lower: 0.0,
                upper: 1.0,
            },
        ],
    };
    let features = vec!["grip.pos".into(), "lift.pos".into()];
    mapping.validate("test-arm", &features).unwrap();
    let values = mapping.convert(&features, &[50.0, 90.0]).unwrap();
    assert_eq!(
        values,
        BTreeMap::from([
            ("grip".into(), 0.5),
            ("lift".into(), 0.1 - std::f64::consts::FRAC_PI_2)
        ])
    );
    assert!(mapping.validate("different-arm", &features).is_err());
    assert!(mapping.convert(&features, &[101.0, 90.0]).is_err());
    assert!(mapping.convert(&features, &[50.0, f64::NAN]).is_err());
    let mut duplicate = mapping;
    duplicate.joints[1].joint = "lift".into();
    assert!(duplicate.validate("test-arm", &features).is_err());
}

fn copy_fixture(version: &str, target: &Path) {
    fn copy(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    copy(&fixture(version), target);
}

#[test]
fn unsupported_versions_bad_paths_and_changed_pins_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    copy_fixture("v2.1", directory.path());
    let path = directory.path().join("meta/info.json");
    let original = std::fs::read(&path).unwrap();
    let mut info: Value = serde_json::from_slice(&original).unwrap();
    info["codebase_version"] = "v4.0".into();
    std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
    assert!(
        matches!(Dataset::open(directory.path()), Err(robot_data::Error::Version(v)) if v == "v4.0")
    );
    info["codebase_version"] = "v2.1".into();
    info["data_path"] = "../../episode_{episode_index:06d}.parquet".into();
    std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
    assert!(Dataset::open(directory.path()).is_err());
    std::fs::write(&path, original).unwrap();
    let pin: SourcePin =
        serde_json::from_slice(&std::fs::read(directory.path().join("source.json")).unwrap())
            .unwrap();
    std::fs::write(directory.path().join("meta/stats.json"), b"{}").unwrap();
    assert!(Dataset::open_pinned(directory.path(), &pin).is_err());
}

#[cfg(unix)]
#[test]
fn symlinked_recording_files_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    copy_fixture("v2.1", directory.path());
    let path = directory
        .path()
        .join("data/chunk-000/episode_000000.parquet");
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(
        fixture("v2.1").join("data/chunk-000/episode_000000.parquet"),
        path,
    )
    .unwrap();
    let dataset = Dataset::open(directory.path()).unwrap_err();
    assert!(oa_copy::violations(&dataset.to_string(), &[]).is_empty());
}

#[test]
fn damaged_table_sizes_are_refused_before_decode() {
    let directory = tempfile::tempdir().unwrap();
    copy_fixture("v2.1", directory.path());
    let path = directory
        .path()
        .join("data/chunk-000/episode_000000.parquet");
    let mut bytes = std::fs::read(&path).unwrap();
    let at = bytes.len() - 8;
    bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let dataset = Dataset::open(directory.path()).unwrap();
    assert!(dataset.episode(0).is_err());
}
