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

/// `chain` (W2, W3) runs the group's stages as threads of one process and
/// answers as the unsplit CUDA stage does; its memory ledger adds up to the
/// stage's device bytes.
#[test]
fn a_chained_group_answers_like_the_unsplit_cuda_stage_and_reports_its_memory() {
    if !cuda() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 13), (13, 40)];
    weights(root.path(), &split);
    let expected = reference_with(
        root.path(),
        &prompt(),
        12,
        &["--backend", "cuda", "--max-context", CONTEXT],
    );
    let out = common::run_bounded(
        std::process::Command::new(common::BIN)
            .args([
                "chain",
                "--weights",
                &root.path().display().to_string(),
                "--split",
                "0-13,13-40",
                "--gpus",
                "0,0",
                "--max-context",
                CONTEXT,
                "--max-step-tokens",
                "16",
                "--prompt",
                &prompt().to_string(),
                "--max-tokens",
                "12",
                "--engram-lookups",
                "8",
            ])
            .env("CUDA_VISIBLE_DEVICES", common::visible_devices()),
        common::LIMIT,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<u32> = report["answer"]["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    assert_eq!(ids, expected.0);
    let stages = report["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 2);
    for stage in stages {
        let ledger = &stage["ledger"];
        let device = &stage["device_bytes"];
        assert_eq!(
            ledger["device_total"].as_u64().unwrap(),
            device["weights"].as_u64().unwrap() + device["state"].as_u64().unwrap(),
            "{stage}"
        );
        assert_eq!(ledger["max_step_tokens"], 16);
        // Layer 1's Engram lives on stage 0; layer 14's on stage 1.
        assert_eq!(stage["engram"].as_array().unwrap().len(), 1, "{stage}");
        assert!(stage["per_decode_token"]["parameters"].as_u64().unwrap() > 0);
        assert!(stage["run"]["step_ms"].as_array().unwrap().len() >= 12);
    }
}

/// The whole model on one GPU with the single-machine tiers (#12463,
/// #12466): routed experts read in place with a small device cache and the
/// host lane for misses, Engram rows read from the shards. Every round
/// answers as the unsplit CUDA stage does, a saved routing profile seeds
/// the next load, and the report carries the cache's counts.
#[test]
fn one_gpu_with_an_expert_cache_answers_like_the_unsplit_stage() {
    if !cuda() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    weights(root.path(), &[(0, 40)]);
    let expected = reference_with(
        root.path(),
        &prompt(),
        12,
        &["--backend", "cuda", "--max-context", CONTEXT],
    );
    let profile = root.path().join("profile.json");
    let run = |extra: &[&str]| -> serde_json::Value {
        let weights = root.path().display().to_string();
        let mut args = vec![
            "chain",
            "--weights",
            &weights,
            "--split",
            "0-40",
            "--gpus",
            "0",
            "--max-context",
            CONTEXT,
            "--max-step-tokens",
            "16",
            "--prompt",
        ];
        let prompt = prompt().to_string();
        args.push(&prompt);
        args.extend([
            "--max-tokens",
            "12",
            "--experts",
            "file",
            "--expert-cache",
            "slots:4",
            "--engram",
            "file",
            "--repeat-answer",
            "2",
            "--host-threads",
            "4",
        ]);
        args.extend(extra);
        let out = common::run_bounded(
            std::process::Command::new(common::BIN)
                .args(&args)
                .env("CUDA_VISIBLE_DEVICES", common::visible_devices()),
            common::LIMIT,
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let profile_arg = profile.display().to_string();
    for report in [
        run(&["--save-expert-profile", &profile_arg]),
        run(&["--expert-profile", &profile_arg, "--miss-lane", "host"]),
    ] {
        let ids = |answer: &serde_json::Value| -> Vec<u32> {
            answer["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect()
        };
        let answer = &report["answer"];
        assert_eq!(ids(answer), expected.0);
        assert_eq!(ids(&answer["earlier_rounds"][0]), expected.0);
        let stage = &report["stages"][0];
        assert_eq!(stage["experts"], "file");
        let cache = &stage["run"]["expert_cache"];
        assert_eq!(cache["slots_per_layer"], 4, "{cache}");
        assert_eq!(cache["miss_lane"], "host", "{cache}");
        assert!(cache["hits"].as_u64().unwrap() > 0, "{cache}");
        assert!(cache["host"].as_u64().unwrap() > 0, "{cache}");
        assert!(stage["run"]["engram_staging"]["rows"].as_u64().unwrap() > 0);
        assert!(
            stage["engram"].as_array().unwrap().is_empty(),
            "no pinned tables"
        );
        assert!(profile.is_file());
    }
}

/// Layer-major prefill (#12464) answers exactly as the ordinary step, in a
/// split group and on one GPU.
#[test]
fn layer_major_prefill_answers_like_the_unsplit_stage() {
    if !cuda() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    weights(root.path(), &[(0, 13), (13, 40)]);
    let expected = reference_with(
        root.path(),
        &prompt(),
        12,
        &["--backend", "cuda", "--max-context", CONTEXT],
    );
    for (split, gpus) in [("0-13,13-40", "0,0"), ("0-40", "0")] {
        let out = common::run_bounded(
            std::process::Command::new(common::BIN)
                .args([
                    "chain",
                    "--weights",
                    &root.path().display().to_string(),
                    "--split",
                    split,
                    "--gpus",
                    gpus,
                    "--max-context",
                    CONTEXT,
                    "--max-step-tokens",
                    "16",
                    "--prompt",
                    &prompt().to_string(),
                    "--max-tokens",
                    "12",
                    "--prefill",
                    "layer-major",
                ])
                .env("CUDA_VISIBLE_DEVICES", common::visible_devices()),
            common::LIMIT,
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let ids: Vec<u32> = report["answer"]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(ids, expected.0, "{split}");
    }
}

/// A whole-model serving worker accepts --dspark over the existing link,
/// with its own profile, and keeps ordinary greedy tokens across requests.
#[test]
fn dspark_serving_matches_plain_decode_across_changed_prefixes() {
    if !cuda() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 40)];
    let manifest = weights(root.path(), &split).display().to_string();
    let mut front = Front::start_with(
        root.path(),
        &manifest,
        &split,
        None,
        &["--backend", "cuda", "--dspark", "--max-context", CONTEXT],
        RUNTIME_CUDA,
    );
    assert!(
        front.stages[0].hello["profile"]
            .as_str()
            .unwrap()
            .starts_with("consumer-exact-dspark-")
    );
    let first = prompt();
    let changed = json!([{"role": "user", "content": "t41 t13 t0 t6 t4 t7 t21 t40"}]);
    for prompt in [&first, &changed, &first] {
        let expected = reference_with(
            root.path(),
            prompt,
            24,
            &["--backend", "cuda", "--max-context", CONTEXT],
        );
        assert_eq!(front.run(prompt, 24), expected);
    }
}
