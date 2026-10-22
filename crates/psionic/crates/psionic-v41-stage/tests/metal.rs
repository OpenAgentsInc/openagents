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

/// A Metal bundle (#12420) is the worker and a BUILD.json naming its hash
/// and its Metal kernel source's hash; the worker announces the bundle hash
/// only while both match.
#[test]
fn a_metal_bundle_names_the_worker_and_its_kernels() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("psionic-v41-stage");
    std::fs::copy(common::BIN, &exe).unwrap();
    let info = |exe: &std::path::Path| -> serde_json::Value {
        let out = common::run_bounded(
            std::process::Command::new(exe).arg("build-info"),
            common::LIMIT,
        );
        assert!(out.status.success());
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let before = info(&exe);
    let kernels = before["metal_kernels_sha256"].as_str().unwrap().to_owned();
    assert_eq!(kernels.len(), 64);
    let record = |kernels: &str| {
        json!({
            "schema": psionic_v41_stage::BUILD_SCHEMA,
            "backend": "metal",
            "sha256": before["sha256"],
            "libraries": {},
            "metal_kernels_sha256": kernels,
        })
        .to_string()
    };
    let good = record(&kernels);
    std::fs::write(dir.path().join("BUILD.json"), &good).unwrap();
    let after = info(&exe);
    assert_eq!(after["bundle"], "verified");
    assert_eq!(
        after["bundle_sha256"],
        hex::encode(Sha256::digest(good.as_bytes()))
    );
    // Another kernel source is another build.
    std::fs::write(dir.path().join("BUILD.json"), record(&"0".repeat(64))).unwrap();
    let wrong = info(&exe);
    assert_eq!(wrong["bundle_sha256"], serde_json::Value::Null);
    assert!(
        wrong["bundle"]
            .as_str()
            .unwrap()
            .contains("Metal kernels hash to")
    );
}
