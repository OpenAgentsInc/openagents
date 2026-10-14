use robot_data::{
    dataset::FilePin,
    mapping::{JointMapping, Mapping, Unit},
};
use robot_policy::{
    act::{Activation, Config},
    artifacts::{Artifact, Binding, Normalization, Pin, Source, digest, state_features},
    preprocess::Normalize,
};
use serde_json::json;
use std::collections::BTreeMap;
fn fixture(root: &std::path::Path) -> (Pin, String) {
    let mapping = Mapping {
        schema: "robot-joint-mapping.v1".into(),
        robot_type: "fixture".into(),
        robot_version: "a".repeat(64),
        joints: vec![JointMapping {
            feature: "shoulder.pos".into(),
            joint: "shoulder".into(),
            unit: Unit::Degrees,
            offset: 0.1,
            direction: -1.,
            lower: -1.,
            upper: 1.,
        }],
    };
    let normalization = Normalization {
        state: Normalize::Identity,
        environment: Some(Normalize::Identity),
        action: Normalize::Identity,
        cameras: BTreeMap::new(),
    };
    let weights = b"bounded invalid weights";
    let normalized = serde_json::to_vec(&normalization).unwrap();
    std::fs::write(root.join("model.safetensors"), weights).unwrap();
    std::fs::write(root.join("normalization.json"), &normalized).unwrap();
    let pin = Pin {
        schema: "robot-policy.v1".into(),
        family: "act".into(),
        source: Source {
            repository: "https://github.com/huggingface/lerobot".into(),
            revision: "b883328e6c95681ca90a18b102e4ae5e1f91e2bf".into(),
            license: "Apache-2.0".into(),
            purpose: "File validation fixture".into(),
            training_evidence: json!({"fixture":true}),
        },
        architecture: Config {
            dim_model: 8,
            n_heads: 2,
            dim_feedforward: 8,
            n_encoder_layers: 1,
            n_decoder_layers: 1,
            latent_dim: 2,
            chunk_size: 2,
            state_dim: 1,
            environment_dim: 3,
            action_dim: 1,
            cameras: 0,
            pre_norm: false,
            activation: Activation::Relu,
        },
        binding: Binding {
            robot_type: "fixture".into(),
            robot_description_sha256: "a".repeat(64),
            task: robot_lab::document::Build {
                name: "fixture-task".into(),
                sha256: "b".repeat(64),
            },
            task_definition_sha256: "c".repeat(64),
            instruction: "Move the cube into the target".into(),
            state: mapping.clone(),
            actions: mapping,
            environment: [
                robot_policy::artifacts::Axis::X,
                robot_policy::artifacts::Axis::Y,
                robot_policy::artifacts::Axis::Z,
            ]
            .into_iter()
            .map(|axis| robot_policy::artifacts::Environment {
                object: "cube".into(),
                axis,
            })
            .collect(),
            cameras: vec![],
            timestep_seconds: 1. / 240.,
            control_every_steps: 8,
            action_steps: 2,
        },
        weights: "model.safetensors".into(),
        normalization: "normalization.json".into(),
        files: BTreeMap::from([
            (
                "model.safetensors".into(),
                FilePin {
                    sha256: digest(weights),
                    bytes: weights.len() as u64,
                },
            ),
            (
                "normalization.json".into(),
                FilePin {
                    sha256: digest(&normalized),
                    bytes: normalized.len() as u64,
                },
            ),
        ]),
    };
    let bytes = serde_json::to_vec(&pin).unwrap();
    std::fs::write(root.join("policy.json"), &bytes).unwrap();
    (pin, digest(&bytes))
}
#[test]
fn changed_files_and_path_escapes_fail_before_weights_can_run() {
    let root = tempfile::tempdir().unwrap();
    let (mut pin, hash) = fixture(root.path());
    let artifact = Artifact::open(root.path(), &hash).unwrap();
    assert!(artifact.model().is_err());
    assert!(Artifact::open(root.path(), &"0".repeat(64)).is_err());
    std::fs::write(root.path().join("normalization.json"), "changed").unwrap();
    assert!(Artifact::open(root.path(), &hash).is_err());
    pin.weights = "../outside.safetensors".into();
    assert!(pin.validate().is_err());
}
#[test]
fn explicit_units_offsets_and_direction_round_trip_observed_positions() {
    let root = tempfile::tempdir().unwrap();
    let (pin, _) = fixture(root.path());
    let joints = BTreeMap::from([("shoulder".into(), 0.4)]);
    let values = state_features(&pin.binding.state, &joints).unwrap();
    assert!((values[0] - (-0.3_f32).to_degrees()).abs() < 1e-5);
    let features = robot_policy::artifacts::features(&pin.binding.actions);
    let decoded = pin
        .binding
        .actions
        .convert(
            &features,
            &values.iter().map(|v| f64::from(*v)).collect::<Vec<_>>(),
        )
        .unwrap();
    assert!((decoded["shoulder"] - 0.4).abs() < 1e-7);
    assert!(state_features(&pin.binding.state, &BTreeMap::new()).is_err());
}

#[test]
fn smolvla_import_checks_prompt_tokens_camera_layout_and_each_file_hash() {
    let root = tempfile::tempdir().unwrap();
    let (act, _) = fixture(root.path());
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/smol-hybrid.json")).unwrap();
    let architecture = serde_json::from_value(reference["configuration"].clone()).unwrap();
    let mut binding = act.binding;
    binding.environment.clear();
    binding.instruction = "Move the cube".into();
    binding.cameras = vec![robot_policy::artifacts::Camera {
        id: "front".into(),
        source: [32, 32],
        target: [32, 32],
        definition_sha256: "d".repeat(64),
        max_age_steps: 8,
    }];
    let normalization = Normalization {
        state: Normalize::Identity,
        environment: None,
        action: Normalize::Identity,
        cameras: BTreeMap::from([("front".into(), Normalize::Identity)]),
    };
    let normalized = serde_json::to_vec(&normalization).unwrap();
    std::fs::write(root.path().join("normalization.json"), &normalized).unwrap();
    let tokenizer=br#"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"[UNK]":0,"Move":3,"the":8,"cube":11},"unk_token":"[UNK]"}}"#;
    std::fs::write(root.path().join("tokenizer.json"), tokenizer).unwrap();
    let mut files = act.files;
    files.insert(
        "normalization.json".into(),
        FilePin {
            sha256: digest(&normalized),
            bytes: normalized.len() as u64,
        },
    );
    files.insert(
        "tokenizer.json".into(),
        FilePin {
            sha256: digest(tokenizer),
            bytes: tokenizer.len() as u64,
        },
    );
    let mut pin = robot_policy::smol_artifacts::Pin {
        schema: "robot-smolvla.v1".into(),
        source: act.source,
        architecture,
        binding,
        weights: "model.safetensors".into(),
        normalization: "normalization.json".into(),
        tokenizer: "tokenizer.json".into(),
        language_tokens: vec![3, 8, 11],
        language_valid: vec![true; 3],
        empty_cameras: 0,
        files,
    };
    let bytes = serde_json::to_vec(&pin).unwrap();
    std::fs::write(root.path().join("policy.json"), &bytes).unwrap();
    let artifact =
        robot_policy::smol_artifacts::Artifact::open(root.path(), &digest(&bytes)).unwrap();
    assert!(
        artifact.model().is_err(),
        "invalid tensor containers cannot construct a model"
    );
    pin.language_tokens[0] = 4;
    let bytes = serde_json::to_vec(&pin).unwrap();
    std::fs::write(root.path().join("policy.json"), &bytes).unwrap();
    assert!(robot_policy::smol_artifacts::Artifact::open(root.path(), &digest(&bytes)).is_err());
    pin.empty_cameras = usize::MAX;
    assert!(
        pin.validate().is_err(),
        "extreme camera counts must refuse without overflowing"
    );
}
