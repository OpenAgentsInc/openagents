use psionic_inference::{
    DeviceChoice,
    models::{self, ModelFamily},
};

#[test]
fn unsupported_device_requests_do_not_fall_back() {
    let root = std::path::Path::new("/missing-model");
    // Missing artifacts and unsupported platform/provider choices must remain errors.
    assert!(models::load_native_on(ModelFamily::Potion, root, 1, DeviceChoice::Cuda).is_err());
    assert!(
        psionic_inference::modernbert::ModernBertEngine::load_on(root, 1, DeviceChoice::CoreMl)
            .is_err()
    );
}

#[cfg(feature = "ort")]
#[test]
#[ignore = "requires the CPU-only pinned ORT library and MNIST; set PSIONIC_SMALL_MODELS_DIR and PSIONIC_ORT_LIBRARY"]
fn cpu_only_runtime_refuses_explicit_cuda_registration() {
    use psionic_inference::EngineKind;
    use psionic_inference::ort_engine::{OrtEngine, OrtExecution, OrtOptions};
    let root =
        std::path::PathBuf::from(std::env::var_os("PSIONIC_SMALL_MODELS_DIR").expect("model root"))
            .join("mnist");
    models::manifest(ModelFamily::Mnist)
        .unwrap()
        .verify(&root, EngineKind::Ort)
        .unwrap();
    let library =
        std::path::PathBuf::from(std::env::var_os("PSIONIC_ORT_LIBRARY").expect("CPU ORT library"));
    let options = OrtOptions {
        library_sha256: psionic_inference::artifacts::file_sha256(&library).unwrap(),
        library,
        threads: 1,
    };
    let execution = OrtExecution {
        device: DeviceChoice::Cuda,
        allow_cpu_fallback: true,
        ..Default::default()
    };
    assert!(
        OrtEngine::load_with_execution(&root.join("model.onnx"), &options, &execution).is_err()
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires pinned model directories and explicit PSIONIC_PACKAGE_SCRATCH"]
fn native_packages_load_without_source_onnx_or_test_archives() {
    use psionic_inference::models::{MnistModel, PotionModel};
    use std::{os::unix::fs::symlink, path::PathBuf};
    let source = PathBuf::from(std::env::var_os("PSIONIC_SMALL_MODELS_DIR").expect("model root"));
    let scratch = std::env::var_os("PSIONIC_PACKAGE_SCRATCH").expect("scratch directory");
    let package = tempfile::tempdir_in(scratch).unwrap();
    for (name, files) in [
        (
            "s1-router",
            vec![
                "meta.json",
                "LICENSE",
                "tokenizer/tokenizer.json",
                "tokenizer/tokenizer_config.json",
                "native.safetensors",
                "native.json",
            ],
        ),
        (
            "potion",
            vec![
                "model.safetensors",
                "tokenizer.json",
                "config.json",
                "README.md",
            ],
        ),
        ("mnist", vec!["native.safetensors", "README.md", "LICENSE"]),
    ] {
        let root = package.path().join(name);
        std::fs::create_dir_all(&root).unwrap();
        for file in files {
            let target = root.join(file);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            symlink(source.join(name).join(file), target).unwrap();
        }
    }
    let mut router = psionic_inference::router::load_native_on(
        &package.path().join("s1-router"),
        4,
        DeviceChoice::Cpu,
    )
    .unwrap();
    let tokenizer = psionic_inference::router::RouterTokenizer::load(
        &package.path().join("s1-router/tokenizer/tokenizer.json"),
    )
    .unwrap();
    router
        .run(&tokenizer.prepare("Write a Rust function", None).unwrap())
        .unwrap();
    let mut embedding = PotionModel::load(
        &package.path().join("potion"),
        psionic_inference::EngineKind::Native,
        1,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        embedding.encode(&["Rust memory safety"]).unwrap()[0].len(),
        64
    );
    let mut digit = MnistModel::load(
        &package.path().join("mnist"),
        psionic_inference::EngineKind::Native,
        1,
        None,
        None,
    )
    .unwrap();
    assert_eq!(digit.classify(&[0; 784]).unwrap().digit, 5);
}

#[cfg(feature = "native-cuda")]
#[test]
#[ignore = "requires pinned router assets, an NVIDIA GPU, and compiled CUDA kernels"]
fn cuda_encoder_replays_mixed_shapes_without_answer_drift() {
    use psionic_inference::{ModelEngine, Tensors};
    let root =
        std::path::PathBuf::from(std::env::var_os("PSIONIC_SMALL_MODELS_DIR").expect("model root"))
            .join("s1-router");
    let mut model =
        psionic_inference::modernbert::ModernBertEngine::load_on(&root, 4, DeviceChoice::Cuda)
            .unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap();
    let cases = reference["requests"].as_array().unwrap();
    for round in 0..3 {
        for case in cases.iter().cycle().skip(round).take(cases.len()) {
            let inputs: Tensors = serde_json::from_value(case["inputs"].clone()).unwrap();
            let expected: Tensors = serde_json::from_value(case["outputs"].clone()).unwrap();
            let actual = model.run(&inputs).unwrap();
            for (name, output) in &expected {
                let distance = actual[name]
                    .floats()
                    .unwrap()
                    .iter()
                    .zip(output.floats().unwrap())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f32, f32::max);
                assert!(distance < 1e-4, "{name}: {distance}");
            }
        }
    }
    assert_eq!(model.identity().cpu_fallback, Some(false));
    assert_eq!(
        model.execution_report()["activation_allocations_per_call"],
        0
    );
}
