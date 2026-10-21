#![cfg(all(target_os = "macos", feature = "metal"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! `--backend metal` stage processes over loopback (#12420): the V4.1 stage
//! on an Apple GPU behind W9's executor, on the synthetic model in
//! checkpoint form.

mod common;

use common::{Front, reference_with, weights};
use psionic_v41_stage::{RUNTIME, RUNTIME_METAL};
use serde_json::json;

fn prompt() -> serde_json::Value {
    json!([{"role": "user", "content": "t3 t5 t0 t6 t4 t7 t21 t40"}])
}

const CONTEXT: &str = "64";
const METAL: [&str; 4] = ["--backend", "metal", "--max-context", CONTEXT];

/// Split Metal groups answer as the unsplit Metal stage does, greedy and
/// sampled on the device with the front's settings.
#[test]
fn metal_groups_answer_like_the_unsplit_metal_stage() {
    for split in [vec![(0, 13), (13, 40)], vec![(0, 9), (9, 21), (21, 40)]] {
        let root = tempfile::tempdir().unwrap();
        let manifest = weights(root.path(), &split).display().to_string();
        let mut front =
            Front::start_with(root.path(), &manifest, &split, None, &METAL, RUNTIME_METAL);
        let expected = reference_with(root.path(), &prompt(), 12, &METAL);
        assert!(!expected.0.is_empty());
        assert_eq!(front.run(&prompt(), 12), expected, "{split:?}");
        let sampled_args = [
            "--backend",
            "metal",
            "--max-context",
            CONTEXT,
            "--temperature",
            "0.9",
            "--top-p",
            "0.95",
            "--seed",
            "7",
        ];
        let sampled = reference_with(root.path(), &prompt(), 12, &sampled_args);
        let got = front.run_sampled(
            &prompt(),
            12,
            &json!({"temperature": 0.9, "top_p": 0.95, "seed": 7}),
        );
        assert_eq!(got, sampled, "{split:?}");
    }
}

/// A Metal stage announces its runtime, its device class from the GPU
/// family, its profile, and the CUDA wire.
#[test]
fn a_metal_stage_announces_its_runtime_class_and_wire() {
    assert_ne!(RUNTIME, RUNTIME_METAL);
    let split = [(0, 20), (20, 40)];
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &split).display().to_string();
    let front = Front::start_with(
        root.path(),
        &manifest,
        &split,
        Some(1),
        &METAL,
        RUNTIME_METAL,
    );
    let hello = &front.stages[0].hello;
    let class = hello["device"]["class"].as_str().unwrap();
    assert!(class.starts_with("metal-apple"), "{class}");
    assert_eq!(
        hello["profile"],
        format!("metal-device-{}", class.trim_start_matches("metal-"))
    );
    assert_eq!(hello["wire"], json!({"residual": "f32", "cache": "bf16"}));
    assert_eq!(hello["device"]["experts"], "unified");
}
