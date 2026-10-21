#![cfg(all(unix, feature = "cuda"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! `--backend cuda` stage processes over loopback (#12398): W7's
//! device-resident stage behind W9's executor, on the synthetic model in
//! checkpoint form. They need a CUDA device and the V4.1 kernels; without
//! them each test says so and passes, unless `PSIONIC_REQUIRE_CUDA` is set.

mod common;

use common::{Front, reference_with, weights};
use psionic_backend_cuda::CudaBackend;
use psionic_v41_stage::{RUNTIME, RUNTIME_CUDA};
use serde_json::json;

fn cuda() -> bool {
    let backend = CudaBackend::new();
    let ready = backend.selected_device().is_some() && backend.quantized_kernels_available();
    assert!(
        ready || std::env::var_os("PSIONIC_REQUIRE_CUDA").is_none(),
        "CUDA required but the device or the kernels are unavailable"
    );
    if !ready {
        eprintln!("skipped: no CUDA device or V4.1 kernels");
    }
    ready
}

fn prompt() -> serde_json::Value {
    json!([{"role": "user", "content": "t3 t5 t0 t6 t4 t7 t21 t40"}])
}

const CONTEXT: &str = "64";

#[test]
fn cuda_groups_answer_like_the_unsplit_cuda_stage() {
    if !cuda() {
        return;
    }
    for split in [vec![(0, 13), (13, 40)], vec![(0, 9), (9, 21), (21, 40)]] {
        let root = tempfile::tempdir().unwrap();
        let manifest = weights(root.path(), &split);
        let manifest = manifest.display().to_string();
        let mut front = Front::start_with(
            root.path(),
            &manifest,
            &split,
            None,
            &["--backend", "cuda", "--max-context", CONTEXT],
            RUNTIME_CUDA,
        );
        let expected = reference_with(
            root.path(),
            &prompt(),
            12,
            &["--backend", "cuda", "--max-context", CONTEXT],
        );
        assert!(!expected.0.is_empty());
        assert_eq!(front.run(&prompt(), 12), expected, "{split:?}");
        // Sampled on the device with the front's settings.
        let sampled = reference_with(
            root.path(),
            &prompt(),
            12,
            &[
                "--backend",
                "cuda",
                "--max-context",
                CONTEXT,
                "--temperature",
                "0.9",
                "--top-p",
                "0.95",
                "--seed",
                "7",
            ],
        );
        let got = front.run_sampled(
            &prompt(),
            12,
            &json!({"temperature": 0.9, "top_p": 0.95, "seed": 7}),
        );
        assert_eq!(got, sampled, "{split:?}");
    }
}

/// Each backend announces its own runtime, so Pylon's front never makes a
/// group of mixed backends ready: it needs every stage's runtime to match.
#[test]
fn a_cuda_stage_announces_a_runtime_a_cpu_group_refuses() {
    if !cuda() {
        return;
    }
    assert_ne!(RUNTIME, RUNTIME_CUDA);
    let split = [(0, 20), (20, 40)];
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &split).display().to_string();
    let cpu = Front::start_with(root.path(), &manifest, &split, Some(0), &[], RUNTIME);
    let gpu = Front::start_with(
        root.path(),
        &manifest,
        &split,
        Some(1),
        &["--backend", "cuda", "--max-context", CONTEXT],
        RUNTIME_CUDA,
    );
    assert_ne!(
        cpu.stages[0].hello["runtime"],
        gpu.stages[0].hello["runtime"]
    );
}
