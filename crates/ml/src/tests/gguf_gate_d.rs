#[cfg(all(feature = "native", feature = "wgpu"))]
#[test]
fn test_gguf_gate_d_q8_0_matches_cpu() {
    use crate::{run_q8_0_gate, GateConfig};
    use std::path::Path;

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf");
    if !path.exists() {
        eprintln!("gguf missing: {}", path.display());
        return;
    }

    let config = GateConfig {
        path,
        tensor_name: Some("output.weight".to_string()),
        tensor_index: None,
        k: 128,
        n: 64,
    };

    let outcome = run_q8_0_gate(&config).expect("gate D should run");
    let tolerance = 0.01f32;
    assert!(
        outcome.max_abs <= tolerance,
        "max_abs_diff {} exceeds tolerance {}",
        outcome.max_abs,
        tolerance
    );
}
