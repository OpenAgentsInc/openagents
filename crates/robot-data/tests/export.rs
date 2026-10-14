#![cfg(feature = "export")]
use robot_data::export::{self, Episode, Frame, Spec, Termination, Unit};
use serde_json::json;
use std::collections::BTreeMap;

fn png(value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[value, 40, 80, value, 80, 120])
            .unwrap();
    }
    bytes
}
fn fixture() -> (Spec, Vec<Episode>) {
    let spec = Spec {
        robot_type: "test-arm".into(),
        joints: vec!["shoulder.pos".into(), "wrist.pos".into()],
        units: vec![Unit::Radians; 2],
        fps: 30,
        task: "Move the cube into the target".into(),
        cameras: BTreeMap::from([("observation.images.front".into(), [2, 1])]),
        license: "cc0-1.0".into(),
        license_text: "Original fixture released under CC0-1.0".into(),
        notice: "Original test geometry and pixels; no imported assets".into(),
        source: json!({"scene":{"document":"test-scene","version":"a".repeat(64)}}),
    };
    let episodes = (0..2)
        .map(|episode| Episode {
            frames: (0..4)
                .map(|i| Frame {
                    state: vec![i as f32 * 0.1, episode as f32 * 0.2],
                    action: vec![0.3, 0.2],
                    cameras: BTreeMap::from([(
                        "observation.images.front".into(),
                        png(20 + 10 * i),
                    )]),
                })
                .collect(),
            termination: if episode == 0 {
                Termination::Complete
            } else {
                Termination::Interrupted
            },
        })
        .collect();
    (spec, episodes)
}
#[test]
fn export_round_trips_states_actions_times_and_keeps_interrupted_outcome() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("dataset");
    let (spec, episodes) = fixture();
    let manifest = export::write(&destination, &spec, &episodes).unwrap();
    let data = robot_data::dataset::Dataset::open(&destination).unwrap();
    assert_eq!(data.info.total_frames, 8);
    let images = data.images(0).unwrap();
    assert_eq!(images["observation.images.front"].len(), 4);
    assert_eq!(
        images["observation.images.front"][0].as_ref().unwrap(),
        &episodes[0].frames[0].cameras["observation.images.front"]
    );
    for index in 0..2 {
        let actual = data.episode(index).unwrap();
        for (i, frame) in actual.frames.iter().enumerate() {
            assert_eq!(
                frame.state,
                episodes[index as usize].frames[i]
                    .state
                    .iter()
                    .map(|v| f64::from(*v))
                    .collect::<Vec<_>>()
            );
            assert_eq!(frame.action, vec![f64::from(0.3_f32), f64::from(0.2_f32)]);
            assert_eq!(frame.timestamp, f64::from(i as f32 / 30.));
        }
    }
    assert!(matches!(manifest.terminations[1], Termination::Interrupted));
    assert_eq!(data.stats["observation.state"]["count"], json!([8]));
    assert!((data.stats["observation.state"]["mean"][0].as_f64().unwrap() - 0.15).abs() < 1e-7);
    assert!(
        (data.stats["observation.images.front"]["mean"][0][0][0]
            .as_f64()
            .unwrap()
            - 35. / 255.)
            .abs()
            < 1e-12
    );
    let before = std::fs::read(destination.join("meta/info.json")).unwrap();
    assert!(export::write(&destination, &spec, &episodes).is_err());
    assert_eq!(
        std::fs::read(destination.join("meta/info.json")).unwrap(),
        before
    );
    for (name, pin) in manifest.files {
        let bytes = std::fs::read(destination.join(name)).unwrap();
        assert_eq!(bytes.len() as u64, pin.bytes);
        assert_eq!(robot_description_hash(&bytes), pin.sha256);
    }
}
fn robot_description_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
#[test]
fn missing_or_malformed_images_fail_before_a_dataset_is_visible() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("missing");
    let (spec, mut episodes) = fixture();
    episodes[0].frames[1].cameras.clear();
    assert!(export::write(&destination, &spec, &episodes).is_err());
    assert!(!destination.exists());
    episodes[0].frames[1]
        .cameras
        .insert("observation.images.front".into(), png(20));
    episodes[0].frames[1]
        .cameras
        .insert("observation.images.front".into(), b"not an image".to_vec());
    assert!(export::write(&destination, &spec, &episodes).is_err());
    assert!(!destination.exists());
}

#[test]
fn rgba_captures_export_as_three_channel_images_for_the_reference_loader() {
    let root = tempfile::tempdir().unwrap();
    let (spec, mut episodes) = fixture();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[20, 40, 80, 255, 20, 80, 120, 255])
            .unwrap();
    }
    episodes[0].frames[0]
        .cameras
        .insert("observation.images.front".into(), bytes);
    let destination = root.path().join("rgb");
    export::write(&destination, &spec, &episodes).unwrap();
    let data = robot_data::dataset::Dataset::open(&destination).unwrap();
    let images = data.images(0).unwrap();
    let bytes = images["observation.images.front"][0].as_ref().unwrap();
    let reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Rgb);
    assert_eq!(
        export::rgb(bytes, [2, 1]).unwrap(),
        vec![20, 40, 80, 20, 80, 120]
    );
}
