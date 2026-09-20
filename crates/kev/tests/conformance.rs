//! End-to-end conformance: backbone plus LoRA plus pointer head, on CPU,
//! against the Python reference's golden probabilities and mechanism probes.
//!
//! Every committed variant runs the same battery: `fixtures/` for
//! `kev-0.5b`, `fixtures/variants/<id>/` for the rest. Artifacts resolve
//! per `tests/common`; variants whose weights are absent skip.

mod common;

use kev::{Record, SystemOneRequest, to_answers, to_record};
use serde_json::Value;

use common::{fixture, model, names, variants};

/// Max absolute difference between two probability tables.
fn max_delta(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(pa, pb)| pa.iter().zip(pb).map(|(x, y)| (x - y).abs()))
        .fold(0.0, f64::max)
}

#[test]
fn golden_probabilities_reproduce() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let mut worst = 0.0f64;
        let mut worst_name = String::new();
        for name in names(&variant, "requests") {
            let body = fixture(&variant, &format!("requests/{name}.json"));
            let golden = fixture(&variant, &format!("golden/{name}.json"));
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
            let answers = to_answers(&got, &meta);
            assert_eq!(
                serde_json::to_value(&answers).unwrap(),
                golden["answers"],
                "{}: {name}: answers",
                variant.id,
            );
        }
        eprintln!("{}: max probs delta {worst:.3e} ({worst_name})", variant.id);
        assert!(
            worst < 1e-3,
            "{}: probs drifted {worst:.3e} on {worst_name}",
            variant.id
        );
    }
}

#[test]
fn packed_matches_separate() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let probe = fixture(&variant, "probes/packed_vs_separate.json");
        let packed: Vec<Vec<f64>> = serde_json::from_value(probe["packed"].clone()).unwrap();
        let separate: Vec<Vec<f64>> = serde_json::from_value(probe["separate"].clone()).unwrap();

        // Rust-side packed forward.
        let body = fixture(&variant, "requests/support.json");
        let request: SystemOneRequest = serde_json::from_value(body["request"].clone()).unwrap();
        let (record, _) = to_record(&request).unwrap();
        let enc = model.encode(&record, 8192, 8192).unwrap();
        let got_packed = model.probs(&enc).unwrap();
        assert!(
            max_delta(&got_packed, &packed) < 1e-3,
            "{}: packed probs drifted from the reference",
            variant.id,
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
        eprintln!(
            "{}: packed-vs-separate delta {delta:.3e} (reference {reference_delta:.3e})",
            variant.id
        );
        assert!(
            delta < 1e-3,
            "{}: packed vs separate drifted {delta:.3e}",
            variant.id
        );
        assert!(
            max_delta(&got_separate, &separate) < 1e-3,
            "{}: separate probs drifted from the reference",
            variant.id,
        );
    }
}

#[test]
fn isolation_probe_holds() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let probe = fixture(&variant, "probes/isolation.json");
        for cond in ["state_in_sibling", "absent", "state_in_state"] {
            let body = fixture(&variant, &format!("encodings/isolation_{cond}.json"));
            let record: Record = serde_json::from_value(body["record"].clone()).unwrap();
            let enc = model.encode(&record, 8192, 8192).unwrap();
            let probs = model.probs(&enc).unwrap();
            let p_secret = probs[1][0]; // distribution of the code question
            let reference = probe[cond][0].as_f64().unwrap();
            eprintln!(
                "{}: isolation {cond}: {p_secret:.4} (reference {reference:.4})",
                variant.id
            );
            assert!(
                (p_secret - reference).abs() < 0.05,
                "{}: isolation {cond}: {p_secret} vs {reference}",
                variant.id,
            );
            // The qualitative bound follows the reference's own value for
            // this checkpoint: strong only when the reference is strong,
            // weak only when the reference is weak.
            match cond {
                "state_in_state" if reference > 0.9 => {
                    assert!(p_secret > 0.9, "{}: {cond}: {p_secret}", variant.id)
                }
                "state_in_state" => {}
                _ if reference < 0.2 => assert!(
                    p_secret < 0.2,
                    "{}: sibling secret leaked: {p_secret}",
                    variant.id
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn permutation_probe_reproduces() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let probe = fixture(&variant, "probes/permutation.json");
        let qid = probe["question"].as_str().unwrap().to_string();
        let body = fixture(&variant, "requests/support.json");
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
                    "{}: {qid} under order {order:?}: {key} {p} vs {want_p}",
                    variant.id,
                );
                if *p > best {
                    best = *p;
                    argmax = key.clone();
                }
            }
            argmaxes.push(argmax);
        }
        eprintln!("{}: permutation argmaxes: {argmaxes:?}", variant.id);
        // The fixture's own runs all picked the same key; ours must too.
        assert!(
            argmaxes.iter().all(|a| a == &argmaxes[0]),
            "{}: argmax flipped under reordering: {argmaxes:?}",
            variant.id,
        );
    }
}

#[test]
fn forgery_cannot_add_options() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let probe = fixture(&variant, "probes/forgery.json");
        let body = fixture(&variant, "encodings/forgery.json");
        let record: Record = serde_json::from_value(body["record"].clone()).unwrap();
        let enc = model.encode(&record, 8192, 8192).unwrap();
        let want = probe["n_options_expected"].as_u64().unwrap() as usize;
        assert_eq!(
            enc.opt_idx.iter().map(Vec::len).collect::<Vec<_>>(),
            serde_json::from_value::<Vec<usize>>(probe["opt_idx_widths"].clone()).unwrap(),
        );
        assert_eq!(
            enc.opt_idx[0].len(),
            want,
            "{}: forged delimiters changed option count",
            variant.id,
        );
    }
}

/// A variant's isolation flag must round-trip from `head_meta.json` into
/// the loaded model, and its encodings must agree with the flag.
#[test]
fn option_isolation_flag_matches_reference() {
    for variant in variants() {
        let Some(model) = model(&variant) else {
            eprintln!("{}: skipping, no artifacts", variant.id);
            continue;
        };
        let manifest = fixture(&variant, "manifest.json");
        let want = manifest["head_meta"]["option_isolation"]
            .as_bool()
            .unwrap_or(false);
        assert_eq!(
            model.option_isolation, want,
            "{}: option_isolation",
            variant.id,
        );
        let body = fixture(&variant, "encodings/isolation_absent.json");
        let record: Record = serde_json::from_value(body["record"].clone()).unwrap();
        let enc = model.encode(&record, 8192, 8192).unwrap();
        assert_eq!(
            enc.option_isolation, want,
            "{}: encoding isolation flag",
            variant.id,
        );
    }
}
