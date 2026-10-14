#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `check decide` end to end against an in-process relay: a pylon serving
//! decisions, a checker sending fresh items as ordinary decision jobs, a
//! recompute on the checker's reference, the buyer's receipt, and the
//! checker's NIP-32 verdict.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nostr::decision::Refusal;
use nostr::pylon::{Verdict, parse_check, parse_receipt};
use pylon::check::Checker;
use pylon::check_decide;
use pylon::decide::{Decider, Identity as Served};
use pylon::engine::Pending;
use pylon::identity::Identity;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};
use pylon::verify;
use serde_json::{Map, Value, json};

/// A decider whose probabilities fall off by `tilt` per option, and whose
/// choice is the first option and score the expected level (when `liar`,
/// the last option and one level too high).
struct Tilted {
    identity: Served,
    tilt: f64,
    liar: bool,
}

fn profile(n: usize, tilt: f64) -> Vec<f64> {
    let raw: Vec<f64> = (0..n).map(|i| (-(i as f64) * tilt).exp()).collect();
    let z: f64 = raw.iter().sum();
    raw.into_iter().map(|x| x / z).collect()
}

impl Decider for Tilted {
    fn identity(&self) -> Served {
        self.identity.clone()
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn decide<'a>(
        &'a self,
        _state: &'a Value,
        questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>> {
        Box::pin(async move {
            let mut answers = Map::new();
            for (id, q) in questions {
                let answer = match q["type"].as_str() {
                    Some("choice") => {
                        let keys: Vec<String> =
                            q["criteria"].as_object().unwrap().keys().cloned().collect();
                        let p = profile(keys.len(), self.tilt);
                        let pick = if self.liar { keys.len() - 1 } else { 0 };
                        let probs: Map<String, Value> = keys
                            .iter()
                            .zip(&p)
                            .map(|(k, v)| (k.clone(), json!(v)))
                            .collect();
                        json!({"type": "choice", "choice": keys[pick], "confidence": p[0], "probabilities": probs})
                    }
                    Some("score") => {
                        let n = q["criteria"].as_array().unwrap().len();
                        let p = profile(n, self.tilt);
                        let probs: Map<String, Value> = p
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (i.to_string(), json!(v)))
                            .collect();
                        let score: f64 =
                            p.iter().enumerate().map(|(i, v)| i as f64 * v).sum::<f64>()
                                + if self.liar { 1.0 } else { 0.0 };
                        json!({"type": "score", "score": score, "confidence": p[0], "probabilities": probs})
                    }
                    _ => json!({"type": "noul", "noul": 1.0 / (1.0 + self.tilt)}),
                };
                answers.insert(id.clone(), answer);
            }
            Ok(json!({"model": self.identity.model, "answers": answers,
                      "usage": {"input_tokens": 1, "output_tokens": 0}}))
        })
    }
}

fn served() -> Served {
    Served {
        model: "clef-flash".into(),
        artifact_digest: Some(format!("sha256:{}", "cd".repeat(32))),
    }
}

async fn check_against(pylon_tilt: f64, liar: bool) -> check_decide::DecisionCheck {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let pylon_key = Identity::generate();
    let checker = Checker {
        relay: url.clone(),
        checker: Identity::generate(),
        buyer: Identity::generate(),
        home: home.path().to_path_buf(),
        wait: Duration::from_secs(10),
    };
    let mut config = Config::new(&url, "test-decide", home.path().join("pylon"));
    config.allow = Some(BTreeSet::from([checker.buyer.pubkey().to_string()]));
    config.rate_per_minute = 600;
    let provider = Provider::deciding(
        config,
        pylon_key.clone(),
        None,
        Arc::new(Tilted {
            identity: served(),
            tilt: pylon_tilt,
            liar,
        }),
        Arc::new(Dedicated),
    )
    .unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stop_rx.await;
    }));
    for _ in 0..100 {
        if hub.lock().await.stored.iter().any(|e| e.kind == 30_200) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let reference = Tilted {
        identity: served(),
        tilt: 0.8,
        liar: false,
    };
    let result = check_decide::round(
        &checker,
        pylon_key.pubkey(),
        &reference,
        &verify::CLEF_FLASH,
    )
    .await
    .unwrap();
    // The receipt and the label are on the relay and verify.
    let stored = hub.lock().await.stored.clone();
    let receipt = stored
        .iter()
        .find(|e| Some(&e.id) == result.receipt.as_ref())
        .expect("the round's receipt");
    let body = parse_receipt(receipt, None).unwrap();
    assert_eq!(body.provider, pylon_key.pubkey());
    assert_eq!(body.buyer, checker.buyer.pubkey());
    let label = stored
        .iter()
        .find(|e| Some(&e.id) == result.label.as_ref())
        .expect("the round's verdict");
    let check = parse_check(label).unwrap();
    assert_eq!(check.checker, checker.checker.pubkey());
    assert_eq!(check.receipt, receipt.id);
    assert_eq!(check.verdict, result.verdict);
    assert!(
        check
            .method
            .starts_with("decision-recompute clef-flash@sha256:")
    );
    let _ = stop_tx.send(());
    let _ = running.await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pylon_serving_the_same_model_passes() {
    let result = check_against(0.8, false).await;
    assert_eq!(result.verdict, Verdict::Pass, "{result:?}");
    assert_eq!(result.round.verified, 8);
    assert!(result.round.mean < 1e-9);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pylon_serving_another_model_fails() {
    let result = check_against(1.4, false).await;
    assert_eq!(result.verdict, Verdict::Fail, "{result:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pylon_whose_choice_disagrees_with_its_probabilities_fails() {
    let result = check_against(0.8, true).await;
    // Unless a round drew no choice question, the liar is struck.
    if result.items.iter().any(|i| i.problem.is_some()) {
        assert_eq!(result.verdict, Verdict::Fail, "{result:?}");
    }
}
