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
