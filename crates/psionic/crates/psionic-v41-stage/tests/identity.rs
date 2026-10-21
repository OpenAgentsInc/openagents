#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! What a stage announces about the build and arithmetic it runs, and the
//! settings it refuses before loading anything (#12407, audit findings F2
//! and F3, and GPU numbering under an inherited `CUDA_VISIBLE_DEVICES`).

mod common;

use std::process::Command;

use common::{BIN, Front, LIMIT, run_bounded, weights};
use psionic_models::deepseek_v41_cpu::quantization::Arithmetic;
use psionic_v41_stage::{Backend, CUDA_PROFILE, parse_profile, profile_name, select_device};
use sha2::{Digest, Sha256};

#[test]
fn every_backend_and_arithmetic_pair_is_implemented_or_refused() {
    let cases: [(Backend, &str, Result<Option<Arithmetic>, &str>); 10] = [
        (Backend::Cpu, "f32", Ok(Some(Arithmetic::Float32))),
        (Backend::Cpu, "bf16", Ok(Some(Arithmetic::ReferenceBf16))),
        (
            Backend::Cpu,
            "bf16-tp8",
            Ok(Some(Arithmetic::ReferenceBf16Tp8)),
        ),
        (
            Backend::Cpu,
            "device",
            Err("is the CUDA stage's arithmetic"),
        ),
        (
            Backend::Cpu,
            "fp8",
            Err("is not one of f32, bf16, bf16-tp8"),
        ),
        (Backend::Cuda, "device", Ok(None)),
        (
            Backend::Cuda,
            "f32",
            Err("doesn't implement --arithmetic f32"),
        ),
        (
            Backend::Cuda,
            "bf16",
            Err("doesn't implement --arithmetic bf16"),
        ),
        (
            Backend::Cuda,
            "bf16-tp8",
            Err("doesn't implement --arithmetic bf16-tp8"),
        ),
        (Backend::Cuda, "fp8", Err("is not `device`")),
    ];
    for (backend, value, want) in cases {
        match (parse_profile(backend, value), want) {
            (Ok(got), Ok(want)) => assert_eq!(got, want, "{backend:?} {value}"),
            (Err(refused), Err(want)) => {
                assert!(refused.0.contains(want), "{backend:?} {value}: {refused}");
            }
            (got, want) => panic!("{backend:?} {value}: got {got:?}, want {want:?}"),
        }
    }
    assert_eq!(CUDA_PROFILE, "device");
    assert_eq!(
        profile_name(Backend::Cpu, Some(Arithmetic::ReferenceBf16Tp8)),
        "cpu-bf16-tp8"
    );
    assert!(profile_name(Backend::Cuda, None).starts_with("cuda-device"));
}

#[test]
fn gpu_counts_the_devices_this_process_may_see() {
    // No inherited mask: the physical index.
    assert_eq!(select_device(None, 0).unwrap(), "0");
    assert_eq!(select_device(None, 3).unwrap(), "3");
    // An inherited mask: an index into it.
    assert_eq!(select_device(Some("2,3"), 0).unwrap(), "2");
    assert_eq!(select_device(Some("2,3"), 1).unwrap(), "3");
    assert_eq!(select_device(Some(" 5 , 7 "), 1).unwrap(), "7");
    assert_eq!(
        select_device(Some("GPU-aaaa,GPU-bbbb"), 1).unwrap(),
        "GPU-bbbb"
    );
    let refused = select_device(Some("2,3"), 2).unwrap_err().0;
    assert!(
        refused.contains("outside CUDA_VISIBLE_DEVICES=2,3"),
        "{refused}"
    );
    assert!(select_device(Some(""), 0).is_err());
    assert!(select_device(Some("-1"), 0).is_err());
    assert!(select_device(Some("0,,1"), 0).is_err());
}

fn refusal(args: &[&str]) -> String {
    let out = run_bounded(Command::new(BIN).args(args), LIMIT);
    assert_eq!(out.status.code(), Some(2), "{args:?}");
    String::from_utf8(out.stderr).unwrap()
}

/// The arithmetic is checked before the weights: these directories don't
/// exist, so a refusal about them would mean the order is wrong.
#[test]
fn an_arithmetic_the_backend_does_not_run_is_refused_before_loading() {
    let stage = |backend: &str, arithmetic: &str| {
        refusal(&[
            "--weights",
            "/nonexistent/weights",
            "--layers",
            "0-20",
            "--manifest",
            "sha256:00",
            "--stage",
            "s0",
            "--generation",
            "1",
            "--front",
            "127.0.0.1:9",
            "--token",
            "t",
            "--backend",
            backend,
            "--arithmetic",
            arithmetic,
        ])
    };
    assert!(stage("cpu", "device").contains("CUDA stage's arithmetic"));
    assert!(stage("cpu", "fp8").contains("is not one of"));
    let reference = |backend: &str, arithmetic: &str| {
        refusal(&[
            "reference",
            "--weights",
            "/nonexistent/weights",
            "--backend",
            backend,
            "--arithmetic",
            arithmetic,
        ])
    };
    assert!(reference("cpu", "device").contains("CUDA stage's arithmetic"));
    assert!(reference("cpu", "fp8").contains("is not one of"));
    if cfg!(feature = "cuda") {
        for arithmetic in ["f32", "bf16", "bf16-tp8"] {
            let refused = stage("cuda", arithmetic);
            assert!(refused.contains("doesn't implement"), "{refused}");
            let refused = reference("cuda", arithmetic);
            assert!(refused.contains("doesn't implement"), "{refused}");
        }
        assert!(stage("cuda", "fp8").contains("is not `device`"));
        assert!(reference("cuda", "fp8").contains("is not `device`"));
    }
}

#[test]
fn hello_and_build_info_name_the_executable_and_its_profile() {
    let sha = hex::encode(Sha256::digest(std::fs::read(BIN).unwrap()));
    let out = run_bounded(Command::new(BIN).arg("build-info"), LIMIT);
    assert!(out.status.success());
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(info["sha256"], sha);
    assert_eq!(info["cuda"], cfg!(feature = "cuda"));

    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest = weights(root.path(), &split);
    let front = Front::start_with(
        root.path(),
        &manifest.display().to_string(),
        &split,
        Some(0),
        &["--arithmetic", "f32"],
        "psionic-v41-stage/1",
    );
    let hello = &front.stages[0].hello;
    assert_eq!(hello["build"]["sha256"], sha);
    assert_eq!(hello["profile"], "cpu-f32");
}
