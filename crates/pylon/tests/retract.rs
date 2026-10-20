#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `check retract`: a checker withdraws its own failing verdict about a
//! pylon with a NIP-09 deletion request, and nothing else.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nostr::decision::Refusal;
use nostr::pylon::Verdict;
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
        model_id: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_checker_withdraws_only_its_own_failing_verdicts() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    // The command reads the checker key from the pylon home; this test
    // binary runs only this test, so setting the variable races nothing.
    unsafe { std::env::set_var("OPENAGENTS_PYLON_HOME", home.path()) };
    let checker_key = Identity::load_or_create(&home.path().join("checker.key")).unwrap();
    let pylon_key = Identity::generate();
    let checker = Checker {
        relay: url.clone(),
        checker: checker_key.clone(),
        buyer: Identity::generate(),
        home: home.path().to_path_buf(),
        wait: Duration::from_secs(10),
    };
    let mut config = Config::new(&url, "test-retract", home.path().join("pylon"));
    config.allow = Some(BTreeSet::from([checker.buyer.pubkey().to_string()]));
    config.rate_per_minute = 600;
    let provider = Provider::deciding(
        config,
        pylon_key.clone(),
        None,
        Arc::new(Tilted {
            identity: served(),
            tilt: 1.4,
            liar: false,
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
    let failed = check_decide::round(
        &checker,
        pylon_key.pubkey(),
        &reference,
        &verify::CLEF_FLASH,
    )
    .await
    .unwrap();
    assert_eq!(failed.verdict, Verdict::Fail);
    let label = failed.label.clone().unwrap();
    let words: Vec<String> = [
        "check",
        "retract",
        "--relay",
        &url,
        "--pylon",
        pylon_key.pubkey(),
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let code = tokio::task::spawn_blocking(move || pylon::cli::run(true, &words))
        .await
        .unwrap();
    assert_eq!(code, 0);
    let stored = hub.lock().await.stored.clone();
    let deletion = stored
        .iter()
        .find(|e| e.kind == 5)
        .expect("a deletion request");
    assert_eq!(deletion.pubkey, checker_key.pubkey());
    let named: Vec<&str> = deletion
        .tags
        .iter()
        .filter(|t| t.name() == Some("e"))
        .filter_map(|t| t.value())
        .collect();
    assert_eq!(named, vec![label.as_str()]);
    let _ = stop_tx.send(());
    let _ = running.await;
}
