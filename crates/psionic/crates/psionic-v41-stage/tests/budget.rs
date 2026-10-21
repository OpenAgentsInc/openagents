#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! The request budget a V4.1 group serves, from the real wire sizes.

use psionic_models::{
    deepseek_v41_cpu::transformer::TransformerConfig, deepseek_v41_stage::V41StageGeometry,
    deepseek_v41_stage_cpu::wire_geometry, stage_state::WireDtype,
};
use psionic_v41_stage::{Budget, budget, budget_sized, synthetic_manifest};

fn split(stages: u32) -> Vec<std::ops::Range<u32>> {
    let width = 40 / stages;
    (0..stages).map(|i| i * width..(i + 1) * width).collect()
}

/// The CUDA stage's wire: F32 residuals and BF16 cache rows.
fn cuda() -> V41StageGeometry {
    V41StageGeometry::flash().with_dtypes(WireDtype::F32, WireDtype::Bf16)
}

fn cpu() -> V41StageGeometry {
    wire_geometry(&TransformerConfig::flash().unwrap())
}

#[test]
fn the_real_wire_bounds_the_prompt_below_the_default_context() {
    let mut seen = Vec::new();
    for (name, geometry) in [("cuda", cuda()), ("cpu", cpu())] {
        for stages in [2, 4, 8] {
            let manifest = synthetic_manifest(&split(stages)).unwrap();
            let b = budget(&manifest, &geometry, 8192, None);
            eprintln!("{name}, {stages} stages: {b:?}");
            assert_eq!(b.max_context_tokens, 8192);
            // An unchunked F32 residual alone overflows 256 MiB at 3,277
            // tokens, so the wire, not the context, bounds the prompt.
            assert!(b.max_prefill_tokens < 3277, "{name} {stages}: {b:?}");
            assert_eq!(b.max_input_tokens, b.max_prefill_tokens);
            seen.push((name, stages, b.max_input_tokens));
        }
    }
    // The four- and eight-stage CUDA groups the plans place.
    let cuda4 = seen.iter().find(|s| s.0 == "cuda" && s.1 == 4).unwrap().2;
    let cuda8 = seen.iter().find(|s| s.0 == "cuda" && s.1 == 8).unwrap().2;
    assert_eq!((cuda4, cuda8), (CUDA_FOUR, CUDA_EIGHT));
}

/// The prompt bound of a CUDA group split 0-10, 10-20, 20-30, 30-40.
const CUDA_FOUR: u64 = 2872;
/// The prompt bound of a CUDA group split into eight five-layer stages.
const CUDA_EIGHT: u64 = 2872;

#[test]
fn the_context_and_the_operator_cap_lower_the_prompt_bound() {
    let manifest = synthetic_manifest(&split(4)).unwrap();
    let b = budget(&manifest, &cuda(), 2048, None);
    assert_eq!(b.max_input_tokens, 2047);
    let b = budget(&manifest, &cuda(), 8192, Some(1100));
    assert_eq!(b.max_input_tokens, 1100);
    assert!(b.max_prefill_tokens > 1100);
}

#[test]
fn a_request_is_checked_against_the_prompt_and_context_bounds() {
    let b = Budget {
        max_context_tokens: 100,
        max_input_tokens: 60,
        max_prefill_tokens: 60,
        max_step_tokens: 60,
        prefill_chunk_tokens: None,
    };
    assert!(b.check(60, 40).is_ok());
    let over = b.check(61, 1).unwrap_err();
    assert_eq!((over.limit, over.actual), (60, 61));
    let long = b.check(60, 41).unwrap_err();
    assert_eq!((long.limit, long.actual), (100, 101));
    // Even with no output limit, a request needs room for one token.
    assert!(b.check(60, 0).is_ok());
}

#[test]
fn chunks_fit_every_boundary_at_odd_positions_and_long_contexts() {
    use psionic_models::{deepseek_v41_stage::stage_contract, stage_state::CodecLimits};
    for geometry in [cuda(), cpu()] {
        for stages in [2, 4, 8] {
            let manifest = synthetic_manifest(&split(stages)).unwrap();
            let b = budget_sized(&manifest, &geometry, 32768, 2048, None, Some(2048));
            assert_eq!(
                (b.max_context_tokens, b.max_step_tokens, b.max_input_tokens),
                (32768, 2048, 32767)
            );
            assert_eq!(b.prefill_chunk_tokens, Some(2048));
            assert!(b.check(19989, 4096).is_ok());
            let limits = CodecLimits::default();
            for stage in manifest.stages.iter().skip(1) {
                let contract = stage_contract(&geometry, stage.stage, stage.layers());
                let max = contract.max_chunk_tokens(&limits, 128);
                for position in 0..129 {
                    let (bytes, largest) = contract.step_bytes(128, max, position);
                    assert!(
                        bytes <= limits.max_message_bytes && largest <= limits.max_section_bytes
                    );
                }
            }
        }
    }
}

#[test]
fn the_cloud_chunk_profile_matches_measured_memory_and_reserved_runtime() {
    let profile = psionic_v41_stage::chunk_memory_profile().unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../pylon/placements/deepseek-v41");
    let path = dir
        .parent()
        .unwrap()
        .join("deepseek-v41-chunked-32k-memory.json");
    if std::env::var_os("PSIONIC_REGENERATE_CHUNK_MEMORY").is_some() {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&profile).unwrap()),
        )
        .unwrap();
    }
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(retained, profile);
    for name in ["four-single", "two-dual", "one-quad"] {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap())
                .unwrap();
        for (stage, measured) in manifest["stages"]
            .as_array()
            .unwrap()
            .iter()
            .zip(profile["stages"].as_array().unwrap())
        {
            assert_eq!(stage["first_layer"], measured["first_layer"]);
            assert_eq!(stage["end_layer"], measured["end_layer"]);
            assert!(
                stage["gpu_runtime_bytes"].as_u64().unwrap()
                    >= measured["runtime_bytes"].as_u64().unwrap()
            );
        }
    }
}
