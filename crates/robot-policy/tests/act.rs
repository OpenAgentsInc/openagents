use candle_core::{DType, Device, Tensor};
use robot_policy::{
    act::{Act, Config},
    preprocess::{self, Normalize},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

fn reference() -> Value {
    serde_json::from_str(include_str!("../fixtures/act-reference.json")).unwrap()
}
fn values(value: &Value) -> Vec<f32> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect()
}
fn close(actual: &[f32], expected: &[f32], tolerance: f32) -> f32 {
    assert_eq!(actual.len(), expected.len());
    let error = actual
        .iter()
        .zip(expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0., f32::max);
    assert!(
        actual.iter().all(|v| v.is_finite()) && error <= tolerance,
        "maximum error {error}, tolerance {tolerance}"
    );
    error
}
#[test]
fn preprocessing_matches_pinned_python_state_and_rgb_resize() {
    let data = reference();
    let tolerance = data["preprocessing_absolute_tolerance"].as_f64().unwrap() as f32;
    for case in data["cases"].as_array().unwrap() {
        let normalization: Normalize =
            serde_json::from_value(case["state_normalization"].clone()).unwrap();
        let actual = normalization.apply(&values(&case["raw_state"])).unwrap();
        close(&actual, &values(&case["normalized_state"]), tolerance);
        if !case["image"].is_null() {
            let image = &case["image"];
            let bytes = std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures")
                    .join(image["path"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                image["sha256"].as_str().unwrap()
            );
            let normalization: Normalize =
                serde_json::from_value(image["normalization"].clone()).unwrap();
            let dimensions =
                |name| serde_json::from_value::<[u32; 2]>(image[name].clone()).unwrap();
            let actual = preprocess::image(
                &bytes,
                dimensions("source"),
                dimensions("target"),
                &normalization,
            )
            .unwrap();
            close(&actual, &values(&case["normalized_image"]), tolerance);
        }
    }
}
#[test]
fn unsupported_dimensions_and_invalid_statistics_are_refused() {
    let case = &reference()["cases"][0];
    let mut config: Config = serde_json::from_value(case["architecture"].clone()).unwrap();
    config.dim_model = 0;
    assert!(config.validate().is_err());
    config.dim_model = 32;
    config.n_heads = 0;
    assert!(config.validate().is_err());
    assert!(
        Normalize::MeanStd {
            mean: vec![0.],
            std: vec![-1.]
        }
        .apply(&[0.])
        .is_err()
    );
    assert!(Normalize::Identity.apply(&[f32::NAN]).is_err());
    assert!(
        Normalize::MeanStd {
            mean: vec![0.],
            std: vec![0.]
        }
        .apply(&[0., 1.])
        .is_err()
    );
}
#[test]
#[ignore = "requires reference weights in ROBOT_ACT_ARTIFACTS, kept outside git"]
fn candle_matches_reference_actions_for_both_norm_styles_and_resnet18() {
    let root = std::path::PathBuf::from(std::env::var("ROBOT_ACT_ARTIFACTS").unwrap());
    let data = reference();
    let mut maximum = 0_f32;
    for case in data["cases"].as_array().unwrap() {
        let file = &case["weights"];
        let bytes = std::fs::read(root.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(bytes.len() as u64, file["bytes"].as_u64().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            file["sha256"].as_str().unwrap()
        );
        let config: Config = serde_json::from_value(case["architecture"].clone()).unwrap();
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(bytes, DType::F32, &Device::Cpu)
            .unwrap();
        let state = Tensor::from_vec(
            values(&case["normalized_state"]),
            (1, config.state_dim),
            &Device::Cpu,
        )
        .unwrap();
        let environment = if config.environment_dim == 0 {
            None
        } else {
            Some(
                Tensor::from_vec(
                    values(&case["environment"]),
                    (1, config.environment_dim),
                    &Device::Cpu,
                )
                .unwrap(),
            )
        };
        let images = if config.cameras == 0 {
            vec![]
        } else {
            vec![
                Tensor::from_vec(
                    values(&case["normalized_image"]),
                    (1, 3, 64, 96),
                    &Device::Cpu,
                )
                .unwrap(),
            ]
        };
        let model = Act::load(config, vb.pp("model")).unwrap();
        let output = model
            .forward(&state, environment.as_ref(), &images)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        let error = close(
            &output,
            &values(&case["actions"]),
            data["action_absolute_tolerance"].as_f64().unwrap() as f32,
        );
        maximum = maximum.max(error);
        println!("{}: maximum action difference {error}", case["name"]);
    }
    println!("all ACT cases: maximum difference {maximum}");
}
