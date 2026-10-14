use candle_core::{DType, Device, Tensor};
use robot_policy::smolvla::{Config, SmolVla};
use serde_json::Value;
use sha2::{Digest, Sha256};
fn tensor(value: &Value) -> Tensor {
    let shape: Vec<usize> = serde_json::from_value(value["shape"].clone()).unwrap();
    let values: Vec<f32> = serde_json::from_value(value["values"].clone()).unwrap();
    Tensor::from_vec(values, shape, &Device::Cpu).unwrap()
}
#[test]
fn smolvla_shapes_and_reference_configs_are_bounded() {
    let value: Value = serde_json::from_str(include_str!("../fixtures/smol-hybrid.json")).unwrap();
    let mut cfg: Config = serde_json::from_value(value["configuration"].clone()).unwrap();
    cfg.validate().unwrap();
    cfg.kv_heads = 0;
    assert!(cfg.validate().is_err());
    cfg.kv_heads = 2;
    cfg.vision.shuffle = 3;
    assert!(cfg.validate().is_err());
    cfg.vision.shuffle = 2;
    cfg.denoise_steps = 0;
    assert!(cfg.validate().is_err());
}
#[test]
#[ignore = "requires original SmolVLA reference weights in ROBOT_SMOL_ARTIFACTS outside Git"]
fn candle_matches_official_smolvla_hybrid_cross_self_and_reduced_experts() {
    let root = std::path::PathBuf::from(std::env::var("ROBOT_SMOL_ARTIFACTS").unwrap());
    let mut maximum = 0_f32;
    for name in ["hybrid", "cross", "self", "reduced"] {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(format!("smol-{name}.json"));
        let value: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
        let bytes = std::fs::read(root.join(format!("smol-{name}.safetensors"))).unwrap();
        assert_eq!(
            bytes.len() as u64,
            value["weights"]["bytes"].as_u64().unwrap()
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            value["weights"]["sha256"].as_str().unwrap()
        );
        let cfg: Config = serde_json::from_value(value["configuration"].clone()).unwrap();
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(bytes, DType::F32, &Device::Cpu)
            .unwrap();
        let model = SmolVla::load(cfg, vb.pp("model")).unwrap();
        let images = value["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(tensor)
            .collect::<Vec<_>>();
        let actual = model
            .forward(
                &tensor(&value["state"]),
                &images,
                &serde_json::from_value::<Vec<bool>>(value["image_valid"].clone()).unwrap(),
                &serde_json::from_value::<Vec<u32>>(value["tokens"].clone()).unwrap(),
                &serde_json::from_value::<Vec<bool>>(value["language_valid"].clone()).unwrap(),
                &tensor(&value["noise"]),
            )
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        let expected: Vec<f32> = serde_json::from_value(value["output"]["values"].clone()).unwrap();
        let error = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0_f32, f32::max);
        assert!(
            actual.iter().all(|a| a.is_finite())
                && error <= value["absolute_tolerance"].as_f64().unwrap() as f32,
            "{name}: maximum error {error}"
        );
        maximum = maximum.max(error);
    }
    eprintln!("SmolVLA maximum absolute action error: {maximum}");
}

#[test]
fn smolvla_preprocessing_preserves_aspect_ratio_and_matches_reference_padding() {
    let value: Value =
        serde_json::from_str(include_str!("../fixtures/smol-preprocess.json")).unwrap();
    let bytes = include_bytes!("../fixtures/camera-pattern.png");
    let actual = robot_policy::preprocess::smol_image(
        bytes,
        serde_json::from_value(value["source"].clone()).unwrap(),
        value["size"].as_u64().unwrap() as u32,
    )
    .unwrap();
    let expected: Vec<f32> = serde_json::from_value(value["output"].clone()).unwrap();
    let error = actual
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0_f32, f32::max);
    assert!(error <= 2e-6, "maximum preprocessing error {error}");
    let a = robot_policy::smol_artifacts::noise(7, 0, 128);
    assert_eq!(a, robot_policy::smol_artifacts::noise(7, 0, 128));
    assert_ne!(a, robot_policy::smol_artifacts::noise(7, 8, 128));
}
