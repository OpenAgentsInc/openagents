#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The checked-in calibration behind `verify::CLEF_FLASH`: every honest
//! pair we measured passes every simulated round, and every substitute
//! passes almost none. If a Psionic change moves these numbers, this test
//! says so before a checker strikes honest pylons.
//!
//! Data: `crates/pylon/data/calibration/` (Psionic on Metal and CPU, on
//! seeded items) and `crates/psionic/fixtures/clef/e2e/` (CPU, CUDA f16
//! and f32, llama.cpp, and the full-precision Hugging Face reference on
//! 40 hand-made requests). See `docs/research/decisionmodels/calibration.md`.

use std::path::{Path, PathBuf};

use pylon::calibrate::{self, Answered};
use pylon::verify::{self, CLEF_FLASH};
use serde_json::Value;

fn data(name: &str) -> Vec<Answered> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/calibration")
        .join(name);
    calibrate::load(&path).unwrap()
}

fn fixture(name: &str) -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../psionic/fixtures/clef/e2e")
        .join(name);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn probs(answer: &Value) -> Vec<f64> {
    match answer["type"].as_str() {
        Some("noul") => {
            let p = answer["noul"].as_f64().unwrap();
            vec![p, 1.0 - p]
        }
        _ => answer["probabilities"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_f64().unwrap())
            .collect(),
    }
}

/// Per-question distances between two fixture runs of the e2e requests.
fn fixture_distances(a: &str, b: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for (x, y) in fixture(a).iter().zip(fixture(b)) {
        if x["status"] != 200 || y["status"] != 200 {
            continue;
        }
        for (id, qa) in x["body"]["answers"].as_object().unwrap() {
            let Some(qb) = y["body"]["answers"].get(id) else {
                continue;
            };
            let (p, q) = (probs(qa), probs(qb));
            if p.len() == q.len() {
                out.push(verify::distance(&p, &q));
            }
        }
    }
    out
}

fn run_distances(a: &str, b: &str) -> Vec<f64> {
    let b = data(b);
    let by_id: std::collections::BTreeMap<&str, &Answered> =
        b.iter().map(|x| (x.id.as_str(), x)).collect();
    data(a)
        .iter()
        .filter_map(|x| {
            let y = by_id.get(x.id.as_str())?;
            Some(verify::distance(
                x.probabilities.as_ref()?,
                y.probabilities.as_ref()?,
            ))
        })
        .collect()
}

fn shares(distances: &[f64]) -> (f64, f64) {
    assert!(distances.len() >= 30, "only {} items", distances.len());
    verify::simulate(distances, &CLEF_FLASH, 20_000, 7)
}

#[test]
fn honest_pairs_pass_every_round() {
    for (name, d) in [
        (
            "metal repeat",
            run_distances("metal-a.jsonl", "metal-b.jsonl"),
        ),
        (
            "metal f16 accumulate",
            run_distances("metal-a.jsonl", "metal-f16.jsonl"),
        ),
        ("cpu vs metal", run_distances("metal-a.jsonl", "cpu.jsonl")),
        (
            "cuda f32 vs cpu",
            fixture_distances("psionic-cpu-q4_k_m.jsonl", "psionic-cuda-f32-q4_k_m.jsonl"),
        ),
        (
            "cuda f16 vs cpu",
            fixture_distances("psionic-cpu-q4_k_m.jsonl", "psionic-cuda-f16-q4_k_m.jsonl"),
        ),
    ] {
        let (pass, strike) = shares(&d);
        assert!(
            pass >= 0.9999 && strike == 0.0,
            "{name}: {pass} pass, {strike} strike"
        );
        let worst = d.iter().copied().fold(0.0, f64::max);
        assert!(
            worst * 4.0 < CLEF_FLASH.item,
            "{name}: an honest item at {worst}"
        );
    }
}

#[test]
fn substitutes_pass_almost_no_round() {
    for (name, d, most) in [
        (
            "q8_0 of the same weights",
            run_distances("metal-a.jsonl", "metal-q8.jsonl"),
            0.05,
        ),
        (
            "full-precision reference (an upstream API)",
            fixture_distances("psionic-cpu-q4_k_m.jsonl", "reference-hf-f32.jsonl"),
            0.02,
        ),
        (
            "llama.cpp on the same file",
            fixture_distances("psionic-cpu-q4_k_m.jsonl", "llama-b11538-cpu-q4_k_m.jsonl"),
            // Another runtime on the pinned file: its prompt differs from
            // the reference encoder on 21 of 37 requests.
            0.25,
        ),
    ] {
        let (pass, strike) = shares(&d);
        eprintln!(
            "{name}: {:?} pass {pass} strike {strike}",
            verify::spread(&d)
        );
        assert!(pass <= most, "{name}: {pass} of rounds pass");
    }
}

#[test]
fn model_free_cheats_strike() {
    // Answers made without the model, from the Metal run's items: uniform,
    // always the first option, and the reference's own top choice as a
    // one-hot (which needs the model, and still fails).
    let reference = data("metal-a.jsonl");
    let mut uniform = Vec::new();
    let mut first = Vec::new();
    let mut oracle = Vec::new();
    for r in &reference {
        let p = r.probabilities.as_ref().unwrap();
        let n = p.len();
        uniform.push(verify::distance(p, &vec![1.0 / n as f64; n]));
        let mut f = vec![0.0; n];
        f[0] = 1.0;
        first.push(verify::distance(p, &f));
        let best = (0..n).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap();
        let mut o = vec![0.0; n];
        o[best] = 1.0;
        oracle.push(verify::distance(p, &o));
    }
    for (name, d) in [
        ("uniform", uniform),
        ("first option", first),
        ("oracle top choice", oracle),
    ] {
        let (pass, strike) = shares(&d);
        assert!(
            pass == 0.0 && strike > 0.99,
            "{name}: {pass} pass, {strike} strike"
        );
    }
}

#[test]
fn a_substitute_fails_the_self_check_rule() {
    // The self-check's rule (at least 99 % of simulated rounds pass, none
    // strike) applied to the Q8_0 run against the Q4_K_M reference.
    let c = calibrate::compare(&data("metal-a.jsonl"), &data("metal-q8.jsonl"), &CLEF_FLASH);
    assert!(
        c.round_pass < calibrate::SELFCHECK_PASS && c.round_strike > 0.0,
        "{c:?}"
    );
    // And the CPU run against the Metal reference passes it.
    let c = calibrate::compare(&data("metal-a.jsonl"), &data("cpu.jsonl"), &CLEF_FLASH);
    assert!(
        c.round_pass >= calibrate::SELFCHECK_PASS && c.round_strike == 0.0,
        "{c:?}"
    );
}
