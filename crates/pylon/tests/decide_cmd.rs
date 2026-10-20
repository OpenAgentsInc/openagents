#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `openagents pylon decide` end to end against an in-process relay: one
//! free decision job from this computer's buyer key to a decision pylon.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use pylon::decide::{Fixed, Identity as Served};
use pylon::identity::Identity;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decide_sends_one_decision_job_and_prints_the_answers() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    // The command reads its buyer key from the pylon home; this test binary
    // runs only this test, so setting the variable races nothing.
    unsafe { std::env::set_var("OPENAGENTS_PYLON_HOME", home.path()) };
    let buyer = Identity::load_or_create(&home.path().join("buyer.key")).unwrap();
    let pylon_key = Identity::generate();
    let mut config = Config::new(&url, "test-decide", home.path().join("pylon"));
    config.allow = Some(BTreeSet::from([buyer.pubkey().to_string()]));
    let provider = Provider::deciding(
        config,
        pylon_key.clone(),
        None,
        Arc::new(Fixed {
            identity: Served {
                model: "clef-flash".into(),
                artifact_digest: Some(format!("sha256:{}", "cd".repeat(32))),
                model_id: None,
            },
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
    let run =
        |words: Vec<String>| tokio::task::spawn_blocking(move || pylon::cli::run(true, &words));
    let words = |extra: &[&str]| {
        let mut w: Vec<String> = ["decide", "--relay", &url, "--wait", "10"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        w.extend(extra.iter().map(|s| (*s).to_string()));
        w
    };
    let questions = r#"{"urgent":{"type":"noul","instructions":"Is this urgent?"}}"#;
    // Named and chosen: both reach the one pylon.
    let named = run(words(&[
        "--pylon",
        pylon_key.pubkey(),
        "--questions",
        questions,
    ]));
    assert_eq!(named.await.unwrap(), 0);
    let chosen = run(words(&["--state", r#"{"x":1}"#, "--questions", questions]));
    assert_eq!(chosen.await.unwrap(), 0);
    // No questions: invalid usage.
    assert_ne!(run(words(&[])).await.unwrap(), 0);
    let _ = stop_tx.send(());
    let _ = running.await;
}
