//! Conformance against the Python reference's golden fixtures. The
//! weight-backed cases skip unless `LAYA_BUNDLE_DIR` names a directory of
//! checkpoint folders (the layout `~/work/laya-artifacts` ships); the
//! tokenizer cases need the same variable.

use std::path::{Path, PathBuf};

use laya::api::SystemOneRequest;
use laya::decision::DecisionModel;
use serde_json::{Value, json};

fn bundle() -> Option<PathBuf> {
    std::env::var("LAYA_BUNDLE_DIR").ok().map(PathBuf::from)
}

fn load(dir: &Path) -> DecisionModel {
    DecisionModel::load(dir, candle_core::Device::Cpu).expect("checkpoint loads")
}

/// Collect `|a - b|` for every answer number in the fixture: probabilities,
/// scores, confidences, and act probabilities.
fn diffs<'a>(actual: &'a Value, expected: &'a Value, path: &str, out: &mut Vec<(String, f64)>) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for (key, evalue) in e {
                let avalue = a
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}.{key}: missing from actual"));
                diffs(avalue, evalue, &format!("{path}.{key}"), out);
            }
        }
        (Value::Number(a), Value::Number(e)) => {
            let a = a.as_f64().unwrap();
            let e = e.as_f64().unwrap();
            out.push((path.to_string(), (a - e).abs()));
        }
        (a, e) => assert_eq!(a, e, "{path}: actual differs from fixture"),
    }
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// The requests `fixtures/requests-<checkpoint>.json` declares, run
/// through both implementations. Tolerance is the reference's own four-
/// decimal reporting grid plus fp32 implementation noise.
#[test]
fn checkpoints_match_the_python_reference() {
    let Some(bundle) = bundle() else {
        eprintln!("LAYA_BUNDLE_DIR unset; skipping weight-backed conformance");
        return;
    };
    for checkpoint in ["english", "multilingual", "typed-decisions"] {
        let model = load(&bundle.join(checkpoint));
        let requests = fixture(&format!("requests-{checkpoint}.json"));
        let mut worst = (String::new(), 0.0f64);
        for case in requests.as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let request: SystemOneRequest =
                serde_json::from_value(case["request"].clone()).unwrap();
            let out = model.system_one(&request).unwrap();
            assert_eq!(
                out["model"], case["response"]["model"],
                "{checkpoint}/{name}: model identity"
            );
            assert_eq!(
                out["usage"], case["response"]["usage"],
                "{checkpoint}/{name}: token accounting"
            );
            let mut deltas = Vec::new();
            diffs(
                &out["answers"],
                &case["response"]["answers"],
                &format!("{checkpoint}/{name}"),
                &mut deltas,
            );
            // Two grids: reported probabilities/scores at the reference's
            // own four-decimal rounding, and entropy-derived confidence at
            // a wider bound — saturated tails differ at ~1e-4 between fp32
            // attention implementations and entropy amplifies them.
            for (path, delta) in &deltas {
                if *delta > worst.1 {
                    worst = (path.clone(), *delta);
                }
                let tol = if path.contains("confidence") || path.contains("act_probability") {
                    0.01
                } else {
                    0.001
                };
                assert!(
                    *delta <= tol,
                    "{path}: differs from fixture by {delta} (over {tol})"
                );
            }
        }
        eprintln!("{checkpoint}: max |delta| {:.6} at {}", worst.1, worst.0);
    }
}

/// Encode parity without running the encoder: `build_sequence` on each
/// checkpoint's real tokenizer reproduces the fixture's ids and marker
/// positions exactly.
#[test]
fn sequence_encoding_matches_the_reference() {
    let Some(bundle) = bundle() else {
        eprintln!("LAYA_BUNDLE_DIR unset; skipping tokenizer conformance");
        return;
    };
    for checkpoint in ["english", "multilingual", "typed-decisions"] {
        let dir = bundle.join(checkpoint);
        let tokenizer =
            tokenizers::Tokenizer::from_file(dir.join("tokenizer/tokenizer.json")).unwrap();
        let specials =
            laya::config::Specials::load(&tokenizer, &dir.join("tokenizer/tokenizer_config.json"))
                .unwrap();
        let cfg: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("rl_agent_config.json")).unwrap(),
        )
        .unwrap();
        let cases = fixture(&format!("sequences-{checkpoint}.json"));
        for case in cases.as_array().unwrap() {
            let question: laya::api::Question =
                serde_json::from_value(case["question"].clone()).unwrap();
            let request: SystemOneRequest = serde_json::from_value(json!({
                "state": case["state"], "model": "m", "questions": {"q": question}
            }))
            .unwrap();
            let (internal, _meta) = request.to_internal().unwrap().remove(0);
            let state = laya::encode::serialize_state(&request.state);
            let encoding = laya::encode::build_sequence(
                &tokenizer,
                &specials,
                &state,
                &internal,
                None,
                cfg["max_len"].as_u64().unwrap() as usize,
                cfg["head_max_len"].as_u64().unwrap() as usize,
                false,
            )
            .unwrap();
            let expected_ids: Vec<u32> = serde_json::from_value(case["ids"].clone()).unwrap();
            let expected_markers: Vec<usize> =
                serde_json::from_value(case["markers"].clone()).unwrap();
            assert_eq!(
                encoding.ids, expected_ids,
                "{checkpoint}/{}: ids differ",
                case["name"]
            );
            assert_eq!(
                encoding.markers, expected_markers,
                "{checkpoint}/{}: markers differ",
                case["name"]
            );
        }
    }
}
