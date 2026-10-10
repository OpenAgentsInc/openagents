#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `POST /v1/systemone` farms a decision to a connected Pylon over Nostr
//! (#11225): the Pylon's beacon advertises `pylon/decision`, the gateway
//! sends it a NIP-DEC job on the relay, checks the answer and the served
//! identity, and names the Pylon in the answer. With the Pylon gone, the
//! gateway fails over (here to no door, so the call is `unavailable`).

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::HeaderMap;
use gateway::decision_dispatch::{Decisions, Dispatch};
use pylon::decide::{Fixed, Identity as Served};
use pylon::identity::Identity;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};
use serde_json::{Value, json};

async fn body(response: axum::response::Response) -> (u16, Value, Option<String>) {
    let status = response.status().as_u16();
    let door = response
        .headers()
        .get("x-decision-door")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap(), door)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connected_pylon_answers_and_the_gateway_fails_over_when_it_stops() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    let decisions = Decisions {
        relay: Some(url.clone()),
        vertex: None,
        no_jev: true,
        shadow: 0.0,
        deadline_ms: 8_000,
        pylon_ms: 4_000,
        ..Decisions::default()
    };
    let dispatch = Dispatch::open(decisions, registry.path()).unwrap();

    // A pylon that answers decisions only, admitting the gateway's key.
    let home = tempfile::tempdir().unwrap();
    let pylon_key = Identity::generate();
    let mut config = Config::new(&url, "test-clef", home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([dispatch.pubkey().to_string()]));
    config.rate_per_minute = 60;
    let served = Served {
        model: "clef-flash".into(),
        artifact_digest: Some(format!("sha256:{}", "ab".repeat(32))),
    };
    let provider = Provider::deciding(
        config,
        pylon_key.clone(),
        None,
        Arc::new(Fixed {
            identity: served.clone(),
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
    let beacon = hub
        .lock()
        .await
        .stored
        .iter()
        .find(|e| e.kind == 30_200)
        .cloned()
        .expect("a beacon");
    let beacon = nostr::pylon::parse_beacon(&beacon).unwrap();
    let service = beacon.serves(nostr::pylon::Lane::CjDecision).unwrap();
    assert_eq!(service.model, served.advertised());
    assert_eq!(
        service.capability,
        format!("{}:pylon/decision", pylon_key.pubkey())
    );
    assert!(beacon.serves(nostr::pylon::Lane::CjConversation).is_none());

    let request = Bytes::from(
        json!({
            "model": "jev-latest",
            "state": "I was billed twice for my subscription",
            "questions": {
                "topic": {"type": "choice", "instructions": "Which team?",
                          "criteria": {"billing": "money", "technical": "bugs"}},
                "refund": {"type": "noul", "instructions": "Money back?"},
                "sev": {"type": "score", "criteria": ["low", "mid", "high"]}
            }
        })
        .to_string(),
    );
    let (status, answer, door) = body(
        dispatch
            .handle(&HeaderMap::new(), &request)
            .await
            .expect("the route answers jev-latest"),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(door.as_deref(), Some("pylon:test-clef"));
    assert_eq!(answer["model"], "clef-flash");
    assert_eq!(answer["service"]["door"], "pylon:test-clef");
    assert_eq!(answer["service"]["provider"], pylon_key.pubkey());
    assert_eq!(answer["service"]["identity"], served.advertised());
    assert_eq!(answer["answers"]["topic"]["choice"], "billing");
    assert!(answer["latency_ms"].as_u64().is_some());
    // No decision content reached the relay in the clear.
    for event in &hub.lock().await.stored {
        assert!(!event.content.contains("billed twice"));
    }
    // The evidence line names the pylon, the model, and the time (written
    // off the runtime, so it may land a moment after the answer).
    let mut line = Value::Null;
    for _ in 0..100 {
        let log = std::fs::read_dir(registry.path().join("decisions"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.ends_with(".jsonl") && !name.starts_with("shadow")
            });
        if let Some(first) = log
            .and_then(|log| std::fs::read_to_string(log.path()).ok())
            .and_then(|text| text.lines().next().map(str::to_owned))
        {
            line = serde_json::from_str(&first).unwrap();
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(line["door"], "pylon:test-clef");
    assert_eq!(line["pylon"], pylon_key.pubkey());
    assert_eq!(line["model"], "clef-flash");

    // A model this route does not answer goes to the registry's doors.
    let other =
        Bytes::from(json!({"model": "kev-latest", "state": "s", "questions": {}}).to_string());
    assert!(dispatch.handle(&HeaderMap::new(), &other).await.is_none());

    // The pylon stops: its beacon still looks fresh, but the job goes
    // unanswered, and with no other door the call is unavailable.
    let _ = stop_tx.send(());
    let _ = running.await;
    let (status, refused, _) =
        body(dispatch.handle(&HeaderMap::new(), &request).await.unwrap()).await;
    assert_eq!(status, 503, "{refused}");
    assert_eq!(refused["error"]["code"], "unavailable");
}
