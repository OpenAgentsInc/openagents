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
