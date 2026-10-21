#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! The G1 v2 harness (#12405) end to end on the synthetic model: a run
//! records logits, an oracle dump built from that run passes a strict
//! policy, and a perturbed oracle fails it. With CUDA, the router
//! observations match too and every negative control fails.

mod common;

use std::{collections::BTreeMap, path::Path};

use common::{BIN, LIMIT, run_bounded, weights};
use psionic_models::deepseek_v41_qualify::{
    DecisionRule, DistributionLimits, POLICY_SCHEMA, Policy,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CASE: &str = "c1";
const PROMPT: [u32; 7] = [3, 5, 0, 6, 4, 7, 2];
// Below 8: the CPU synthetic model has eight tokens.
const GENERATED: [u32; 4] = [1, 6, 2, 5];

/// A dump in the oracle's layout: `manifest.json`, `CASE/run.json`, and one
/// safetensors file of `(name, dtype, shape, bytes)`.
fn oracle(dir: &Path, tensors: &[(String, &str, Vec<usize>, Vec<u8>)]) {
    std::fs::create_dir_all(dir.join(CASE)).unwrap();
    let mut header = serde_json::Map::new();
    let mut data = Vec::new();
    let mut entries = Vec::new();
    for (name, dtype, shape, bytes) in tensors {
        let begin = data.len();
        data.extend_from_slice(bytes);
        let code = if *dtype == "float32" { "F32" } else { "I32" };
        header.insert(
            name.clone(),
            json!({"dtype": code, "shape": shape, "data_offsets": [begin, data.len()]}),
        );
        entries.push(
            json!({"name": name, "file": format!("{CASE}/tensors-rank0.safetensors"),
            "shape": shape, "dtype": dtype, "bytes": bytes.len(),
            "sha256": hex::encode(Sha256::digest(bytes))}),
        );
    }
    let header = serde_json::to_vec(&Value::Object(header)).unwrap();
    let mut file = (header.len() as u64).to_le_bytes().to_vec();
    file.extend(header);
    file.extend(data);
    std::fs::write(dir.join(CASE).join("tensors-rank0.safetensors"), file).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&json!({"tensors": entries})).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join(CASE).join("run.json"),
        serde_json::to_vec(&json!({
            "prompt_tokens": PROMPT.len(), "prompt_token_ids": PROMPT,
            "generated_ids": GENERATED, "extra_step_argmax": 2, "n_gen": GENERATED.len(),
            "near_ties": [], "selection_margins": {},
        }))
        .unwrap(),
    )
    .unwrap();
}

fn qualify(root: &Path, manifest: &str, dump: &Path, out: &str, extra: &[&str]) -> Value {
    let mut args = vec![
        "qualify".to_owned(),
        "--weights".into(),
        root.display().to_string(),
        "--manifest".into(),
        manifest.into(),
        "--oracle".into(),
        dump.display().to_string(),
        "--out".into(),
        root.join(out).display().to_string(),
        "--cases".into(),
        CASE.into(),
        "--max-context".into(),
        "64".into(),
        "--step-secs".into(),
        "120".into(),
        // The test binary has no build bundle beside it.
        "--development".into(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_owned()));
    let output = run_bounded(std::process::Command::new(BIN).args(&args), LIMIT);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&std::fs::read(root.join(out).join("report.json")).unwrap()).unwrap()
}

fn strict_policy(root: &Path) -> String {
    let rule = DecisionRule {
        epsilon: Some(1e-6),
        max_substitutions: 0,
        max_substituted_fraction: Some(0.0),
    };
    let policy = Policy {
        schema: POLICY_SCHEMA.into(),
        version: "test".into(),
        distribution: DistributionLimits {
            total_variation: Some(1e-6),
            jensen_shannon: Some(1e-9),
            reference_logprob_error: Some(1e-5),
            case_mean_total_variation: Some(1e-6),
            tail_total_variation: Some(1e-6),
            tail_fraction: Some(0.0),
        },
        top1: rule,
        router: rule,
        index: rule,
        router_layer_mismatch: None,
    };
    let path = root.join("policy.json");
    std::fs::write(&path, serde_json::to_vec(&policy).unwrap()).unwrap();
    path.display().to_string()
}

/// The recorded logits by label and the router IDs by layer, read back from
/// a run's record folder in step order.
fn recorded(root: &Path, out: &str, layers: usize) -> (BTreeMap<String, Vec<u8>>, Vec<Vec<u8>>) {
    let record = root.join(out).join("record");
    let mut logits = BTreeMap::new();
    let mut routers = vec![Vec::new(); layers];
    for step in 0..=GENERATED.len() {
        let stem = format!("{CASE}-0-{step}");
        let label = if step == 0 {
            "last".to_owned()
        } else {
            format!("g{step}")
        };
        logits.insert(
            label,
            std::fs::read(record.join(format!("{stem}.logits.f32"))).unwrap(),
        );
        for (layer, ids) in routers.iter_mut().enumerate() {
            for stage in 0..2 {
                let path = record.join(format!("{stem}.stage{stage}.L{layer}.ExpertIds.u32"));
                if let Ok(bytes) = std::fs::read(path) {
                    ids.extend(bytes);
                }
            }
        }
    }
    (logits, routers)
}

fn dump_from(dir: &Path, logits: &BTreeMap<String, Vec<u8>>, routers: &[Vec<u8>], perturb: bool) {
    let mut tensors = Vec::new();
    for (label, bytes) in logits {
        let mut bytes = bytes.clone();
        if perturb && label == "g2" {
            // Make the least likely token the most likely.
            let mut values: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let max = values.iter().copied().fold(f32::MIN, f32::max);
            let low = values
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, _)| i)
                .unwrap();
            values[low] = max + 8.0;
            bytes = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        }
        for mode in ["decode", "replay"] {
            tensors.push((
                format!("{CASE}.{mode}.{label}.logits"),
                "float32",
                vec![bytes.len() / 4],
                bytes.clone(),
            ));
        }
    }
    for (layer, bytes) in routers.iter().enumerate() {
        if !bytes.is_empty() {
            tensors.push((
                format!("{CASE}.decode.router_ids.layers.{layer}"),
                "int32",
                vec![bytes.len() / 4],
                bytes.clone(),
            ));
        }
    }
    oracle(dir, &tensors);
}

#[test]
fn the_harness_passes_its_own_run_and_fails_a_changed_one_on_the_cpu() {
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &[(0, 20), (20, 40)])
        .display()
        .to_string();
    let empty = root.path().join("empty");
    oracle(&empty, &[]);
    let cpu = ["--backend", "cpu"];
    let first = qualify(root.path(), &manifest, &empty, "first", &cpu);
    // No oracle logits: nothing compared, so nothing passes.
    assert_eq!(first["passed"], json!(false));
    assert_eq!(first["launch"], "development");
    assert_eq!(first["stage_bundle_sha256"], "");
    assert_eq!(first["expected_bundle_sha256"], json!(null));
    let (logits, _) = recorded(root.path(), "first", 0);
    assert_eq!(logits.len(), GENERATED.len() + 1);
    let policy = strict_policy(root.path());
    let same = root.path().join("same");
    dump_from(&same, &logits, &[], false);
    let report = qualify(
        root.path(),
        &manifest,
        &same,
        "same",
        &["--backend", "cpu", "--policy", &policy],
    );
    let case = &report["cases"][0];
    assert_eq!(
        case["replay_equals_forced_decode"],
        json!(true),
        "{report:#}"
    );
    for mode in case["modes"].as_array().unwrap() {
        assert_eq!(mode["passed"], json!(true), "{mode:#}");
        assert_eq!(
            mode["positions"].as_array().unwrap().len(),
            GENERATED.len() + 1
        );
    }
    let chunked = qualify(
        root.path(),
        &manifest,
        &same,
        "chunked",
        &[
            "--backend",
            "cpu",
            "--policy",
            &policy,
            "--prefill-chunk",
            "3",
            "--max-step-tokens",
            "3",
        ],
    );
    assert_eq!(chunked["schedule"]["prefill_chunk"], 3);
    assert_eq!(
        chunked["cases"][0]["replay_equals_forced_decode"], true,
        "{chunked:#}"
    );
    for mode in chunked["cases"][0]["modes"].as_array().unwrap() {
        assert_eq!(mode["passed"], true, "{mode:#}");
    }
    let changed = root.path().join("changed");
    dump_from(&changed, &logits, &[], true);
    let report = qualify(
        root.path(),
        &manifest,
        &changed,
        "changed",
        &["--backend", "cpu", "--policy", &policy],
    );
    assert_eq!(report["passed"], json!(false));
    let failures = report["cases"][0]["modes"][0]["failures"].to_string();
    assert!(failures.contains("g2: top-1"), "{failures}");
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_observations_match_and_every_negative_control_fails() {
    let backend = psionic_backend_cuda::CudaBackend::new();
    if backend.selected_device().is_none() || !backend.quantized_kernels_available() {
        assert!(
            std::env::var_os("PSIONIC_REQUIRE_CUDA").is_none(),
            "CUDA required"
        );
        eprintln!("skipped: no CUDA device or V4.1 kernels");
        return;
    }
    drop(backend);
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &[(0, 20), (20, 40)])
        .display()
        .to_string();
    let cuda = ["--backend", "cuda", "--gpus", "0,0", "--observe"];
    let empty = root.path().join("empty");
    oracle(&empty, &[]);
    qualify(root.path(), &manifest, &empty, "first", &cuda);
    let (logits, routers) = recorded(root.path(), "first", 40);
    assert!(routers.iter().any(|r| !r.is_empty()));
    let policy = strict_policy(root.path());
    let same = root.path().join("same");
    dump_from(&same, &logits, &routers, false);
    let mut args = cuda.to_vec();
    args.extend(["--policy", &policy]);
    let report = qualify(root.path(), &manifest, &same, "same", &args);
    assert_eq!(report["passed"], json!(true), "{report:#}");
    let mut chunked = args.clone();
    chunked.extend(["--prefill-chunk", "3", "--max-step-tokens", "3"]);
    let report = qualify(root.path(), &manifest, &same, "chunked-cuda", &chunked);
    assert_eq!(report["passed"], json!(true), "{report:#}");
    let mut passed = Vec::new();
    for control in [
        "wrong-weights",
        "wrong-routing",
        "wrong-rope",
        "stale-cache",
    ] {
        let mut args = args.clone();
        args.extend(["--control", control]);
        let report = qualify(
            root.path(),
            &manifest,
            &same,
            &format!("control-{control}"),
            &args,
        );
        assert_eq!(report["control"], json!(control));
        let case = &report["cases"][0];
        eprintln!(
            "{control}: passed {} max tv {} router {}",
            report["passed"], case["modes"][0]["max_total_variation"], case["router"]["failed"]
        );
        if report["passed"] == json!(true) {
            passed.push(control);
        }
    }
    assert!(passed.is_empty(), "controls that passed: {passed:?}");
}

#[test]
fn answers_run_greedily_from_the_oracle_prompt_to_the_cap() {
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &[(0, 20), (20, 40)])
        .display()
        .to_string();
    let dump = root.path().join("dump");
    oracle(&dump, &[]);
    let mut args = vec![
        "qualify".to_owned(),
        "--weights".into(),
        root.path().display().to_string(),
        "--manifest".into(),
        manifest,
        "--oracle".into(),
        dump.display().to_string(),
        "--out".into(),
        root.path().join("answers").display().to_string(),
        "--cases".into(),
        CASE.into(),
        "--max-context".into(),
        "64".into(),
        "--backend".into(),
        "cpu".into(),
        "--development".into(),
        "--answers".into(),
        "5".into(),
    ];
    args.push("--step-secs".into());
    args.push("120".into());
    let output = run_bounded(std::process::Command::new(BIN).args(&args), LIMIT);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(
        &std::fs::read(root.path().join("answers").join("answers.json")).unwrap(),
    )
    .unwrap();
    let answer = &report["answers"][0];
    let n = answer["tokens"].as_u64().unwrap();
    assert!((1..=5).contains(&n), "{answer:#}");
    assert_eq!(answer["ids"].as_array().unwrap().len() as u64, n);
    assert_eq!(report["launch"], "development");
}
