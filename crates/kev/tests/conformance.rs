//! End-to-end conformance: backbone plus LoRA plus pointer head, on CPU,
//! against the Python reference's golden probabilities and mechanism probes.
//!
//! Needs the artifact bundle and the base checkpoint. Resolution order:
//! `KEV_ARTIFACT_DIR` / `KEV_BASE_DIR`, then `../../../kev-artifacts/`
//! relative to the crate. Tests skip when the files are absent.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use candle_core::Device;
use kev::{DecisionModel, Record, SystemOneRequest, to_record};
use serde_json::Value;

fn dir(env: &str, fallback: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(env) {
        let dir = PathBuf::from(dir);
        return dir.exists().then_some(dir);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fallback);
    dir.exists().then_some(dir)
}

fn device() -> Device {
    if std::env::var("KEV_TEST_DEVICE").as_deref() == Ok("metal") {
        Device::new_metal(0).expect("metal device")
    } else {
        Device::Cpu
    }
}

fn model() -> Option<&'static DecisionModel> {
    static MODEL: OnceLock<Option<DecisionModel>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let adapter = dir("KEV_ARTIFACT_DIR", "../../../kev-artifacts/kev-0.5b")?;
            let base = dir("KEV_BASE_DIR", "../../../kev-artifacts/qwen25-0.5b")?;
            DecisionModel::load(&base, &adapter, device()).ok()
        })
        .as_ref()
}

fn fixture(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(rel);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn names(dir_name: &str) -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(dir_name);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir:?}: {e}"))
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Max absolute difference between two probability tables.
fn max_delta(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(pa, pb)| pa.iter().zip(pb).map(|(x, y)| (x - y).abs()))
        .fold(0.0, f64::max)
}

#[test]
fn golden_probabilities_reproduce() {
    let Some(model) = model() else {
        eprintln!("skipping: no artifact bundle (set KEV_ARTIFACT_DIR / KEV_BASE_DIR)");
        return;
    };
    let mut worst = 0.0f64;
    let mut worst_name = String::new();
    for name in names("requests") {
        let body = fixture(&format!("requests/{name}.json"));
        let golden = fixture(&format!("golden/{name}.json"));
        let want: Vec<Vec<f64>> = serde_json::from_value(golden["probs"].clone()).unwrap();
        let request: SystemOneRequest =
            serde_json::from_value(body["request"].clone()).expect("request");
        let (record, meta) = to_record(&request).expect("to_record");
        let enc = model.encode(&record, 8192, 8192).expect("encode");
        let got = model.probs(&enc).expect("probs");
        let delta = max_delta(&got, &want);
        if delta > worst {
            worst = delta;
            worst_name = name.clone();
        }
        // The rounded answers must be identical to the reference's.
        let answers = kev::to_answers(&got, &meta);
        assert_eq!(
            serde_json::to_value(&answers).unwrap(),
            golden["answers"],
            "{name}: answers"
        );
    }
    eprintln!("max probs delta {worst:.3e} ({worst_name})");
    assert!(worst < 1e-3, "probs drifted {worst:.3e} on {worst_name}");
}

#[test]
fn packed_matches_separate() {
    let Some(model) = model() else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    let probe = fixture("probes/packed_vs_separate.json");
    let packed: Vec<Vec<f64>> = serde_json::from_value(probe["packed"].clone()).unwrap();
    let separate: Vec<Vec<f64>> = serde_json::from_value(probe["separate"].clone()).unwrap();

    // Rust-side packed forward.
    let body = fixture("requests/support.json");
    let request: SystemOneRequest = serde_json::from_value(body["request"].clone()).unwrap();
    let (record, _) = to_record(&request).unwrap();
    let enc = model.encode(&record, 8192, 8192).unwrap();
    let got_packed = model.probs(&enc).unwrap();
    assert!(
        max_delta(&got_packed, &packed) < 1e-3,
        "packed probs drifted from the reference"
    );

    // Rust-side separate forwards, one record per question.
    let mut got_separate = Vec::new();
    for (qid, q) in &request.questions {
        let mut one = request.clone();
        one.questions.retain(|id, _| id == qid);
        let _ = q;
        let (r1, _) = to_record(&one).unwrap();
        let e1 = model.encode(&r1, 8192, 8192).unwrap();
        got_separate.push(model.probs(&e1).unwrap()[0].clone());
    }
    let delta = max_delta(&got_packed, &got_separate);
    let reference_delta = probe["max_abs_delta"].as_f64().unwrap();
    eprintln!("packed-vs-separate delta {delta:.3e} (reference {reference_delta:.3e})");
    assert!(delta < 1e-3, "packed vs separate drifted {delta:.3e}");
    assert!(
        max_delta(&got_separate, &separate) < 1e-3,
        "separate probs drifted from the reference"
    );
}

#[test]
fn isolation_probe_holds() {
    let Some(model) = model() else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    let probe = fixture("probes/isolation.json");
    for cond in ["state_in_sibling", "absent", "state_in_state"] {
        let body = fixture(&format!("encodings/isolation_{cond}.json"));
        let record: Record = serde_json::from_value(body["record"].clone()).unwrap();
        let enc = model.encode(&record, 8192, 8192).unwrap();
        let probs = model.probs(&enc).unwrap();
        let p_secret = probs[1][0]; // distribution of the code question
        let reference = probe[cond][0].as_f64().unwrap();
        eprintln!("isolation {cond}: {p_secret:.4} (reference {reference:.4})");
        assert!(
            (p_secret - reference).abs() < 0.05,
            "isolation {cond}: {p_secret} vs {reference}"
        );
        match cond {
            "state_in_state" => assert!(p_secret > 0.9),
            _ => assert!(p_secret < 0.2, "sibling secret leaked: {p_secret}"),
        }
    }
}

#[test]
fn permutation_probe_reproduces() {
    let Some(model) = model() else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    let probe = fixture("probes/permutation.json");
    let qid = probe["question"].as_str().unwrap().to_string();
    let body = fixture("requests/support.json");
    let request: SystemOneRequest = serde_json::from_value(body["request"].clone()).unwrap();
    let mut argmaxes = Vec::new();
    for run in probe["runs"].as_array().unwrap() {
        // Reorder the choice criteria to the run's recorded order.
        let order: Vec<String> = serde_json::from_value(run["order"].clone()).unwrap();
        let mut one = request.clone();
        let Some(kev::Question::Choice { criteria, .. }) = one.questions.get_mut(&qid) else {
            panic!("{qid} is not a choice question");
        };
        let old = std::mem::take(criteria);
        for key in &order {
            criteria.insert(key.clone(), old.get(key).cloned().unwrap_or(Value::Null));
        }
        let (record, _) = to_record(&one).unwrap();
        let enc = model.encode(&record, 8192, 8192).unwrap();
        let probs = model.probs(&enc).unwrap()[0].clone();
        let want = run["probs_by_key"].as_object().unwrap();
        let mut argmax = String::new();
        let mut best = f64::NEG_INFINITY;
        for (key, p) in order.iter().zip(&probs) {
            let want_p = want[key].as_f64().unwrap();
            assert!(
                (p - want_p).abs() < 1e-3,
                "{qid} under order {order:?}: {key} {p} vs {want_p}"
            );
            if *p > best {
                best = *p;
                argmax = key.clone();
            }
        }
        argmaxes.push(argmax);
    }
    eprintln!("permutation argmaxes: {argmaxes:?}");
    // The fixture's own runs all picked the same key; ours must too.
    assert!(
        argmaxes.iter().all(|a| a == &argmaxes[0]),
        "argmax flipped under reordering: {argmaxes:?}"
    );
}

#[test]
fn forgery_cannot_add_options() {
    let Some(model) = model() else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    let probe = fixture("probes/forgery.json");
    let body = fixture("encodings/forgery.json");
    let record: Record = serde_json::from_value(body["record"].clone()).unwrap();
    let enc = model.encode(&record, 8192, 8192).unwrap();
    let want = probe["n_options_expected"].as_u64().unwrap() as usize;
    assert_eq!(
        enc.opt_idx.iter().map(Vec::len).collect::<Vec<_>>(),
        serde_json::from_value::<Vec<usize>>(probe["opt_idx_widths"].clone()).unwrap(),
    );
    assert_eq!(enc.opt_idx[0].len(), want, "forged delimiters changed option count");
}
