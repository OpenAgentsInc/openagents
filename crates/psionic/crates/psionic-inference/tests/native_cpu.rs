//! Repeated fixture replay catches stale data in reused native CPU buffers.
use psionic_inference::{DeviceChoice, ModelEngine, Tensors};

#[test]
#[ignore = "requires pinned assets in PSIONIC_SMALL_MODELS_DIR"]
fn router_cpu_reuses_buffers_across_mixed_sequences_without_answer_drift() {
    let root =
        std::path::PathBuf::from(std::env::var_os("PSIONIC_SMALL_MODELS_DIR").expect("model root"))
            .join("s1-router");
    let mut model =
        psionic_inference::modernbert::ModernBertEngine::load_on(&root, 4, DeviceChoice::Cpu)
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
    assert!(
        model.execution_report()["cached_activation_bytes"]
            .as_u64()
            .unwrap()
            <= 64 * 1024 * 1024
    );
}
