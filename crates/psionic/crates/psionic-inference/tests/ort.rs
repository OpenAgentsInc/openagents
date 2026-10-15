#![cfg(feature = "ort")]

use psionic_inference::{
    EngineKind, ModelEngine, Tensors,
    ort_engine::{OrtEngine, OrtOptions},
    router::{file_sha256, pinned_manifest},
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    requests: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    inputs: Tensors,
    outputs: Tensors,
}

#[test]
fn missing_library_and_invalid_threads_are_errors() {
    let options = OrtOptions {
        library: PathBuf::from("/this-library-does-not-exist"),
        library_sha256: String::new(),
        threads: 4,
    };
    assert!(OrtEngine::load(std::path::Path::new("model.onnx"), &options).is_err());
    let options = OrtOptions {
        threads: 0,
        ..options
    };
    assert!(OrtEngine::load(std::path::Path::new("model.onnx"), &options).is_err());
}

#[test]
#[ignore = "requires pinned router and ORT library; set PSIONIC_ROUTER_MODEL_DIR and PSIONIC_ORT_LIBRARY"]
fn ort_executes_all_independent_reference_cases() {
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    let library = PathBuf::from(std::env::var_os("PSIONIC_ORT_LIBRARY").expect("ORT library"));
    pinned_manifest().unwrap().verify(&root).unwrap();
    let options = OrtOptions {
        library_sha256: file_sha256(&library).unwrap(),
        library,
        threads: 4,
    };
    let mut engine = OrtEngine::load(&root.join("model.gq.onnx"), &options).unwrap();
    assert_eq!(engine.identity().engine, EngineKind::Ort);
    assert!(engine.run(&Tensors::new()).is_err());
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap();
    for case in fixture.requests {
        let actual = engine.run(&case.inputs).unwrap();
        for (name, expected) in case.outputs {
            assert_eq!(actual[&name].shape, expected.shape);
            for (a, b) in actual[&name]
                .floats()
                .unwrap()
                .iter()
                .zip(expected.floats().unwrap())
            {
                assert!((a - b).abs() < 1e-5, "{name}: {a} != {b}");
            }
        }
    }
}
