//! The hosted decision service end to end, on this machine: a local relay,
//! the real decision worker's open lane in front of a scripted TypeSafe
//! door, and a Jev client resolved on a computer with no TypeSafe key.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use gateway::relay_worker::{Worker, WorkerConfig};
use jev_hosted::{Door, Via};
use nostr::domain::Event;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

const DOOR_KEY: &str = "ts-fixture-door-key";

/// One subscription: kinds and `#p`.
struct Sub {
    id: String,
    kinds: Vec<u64>,
    p: Vec<String>,
}

type Conns = Arc<Mutex<Vec<(mpsc::UnboundedSender<String>, Vec<Sub>)>>>;

/// A relay that authenticates every connection, stores nothing, and fans
/// each event out to the subscriptions open when it lands, as the
/// production relay treats the ephemeral decision kinds.
async fn relay() -> (String, Conns) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let conns: Conns = Arc::default();
    let shared = conns.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let conns = shared.clone();
            tokio::spawn(async move {
                let Ok(mut socket) = accept_async(stream).await else {
                    return;
                };
                let (out, mut inbox) = mpsc::unbounded_channel::<String>();
                let index = {
                    let mut conns = conns.lock().unwrap();
                    conns.push((out, Vec::new()));
                    conns.len() - 1
                };
                let _ = socket
                    .send(Message::Text(
                        json!(["AUTH", "challenge"]).to_string().into(),
                    ))
                    .await;
                loop {
                    tokio::select! {
                        outbound = inbox.recv() => {
                            let Some(text) = outbound else { return };
                            if socket.send(Message::Text(text.into())).await.is_err() { return; }
                        }
                        inbound = socket.next() => {
                            let Some(Ok(Message::Text(text))) = inbound else {
                                conns.lock().unwrap()[index].1.clear();
                                return;
                            };
                            let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                            let reply = match value[0].as_str() {
                                Some("AUTH") | Some("EVENT") => {
                                    if value[0] == "EVENT" {
                                        fanout(&conns, &value[1]);
                                    }
                                    json!(["OK", value[1]["id"], true, ""])
                                }
                                Some("REQ") => {
                                    let filter = &value[2];
                                    let list = |name: &str| filter[name].as_array().cloned().unwrap_or_default();
                                    conns.lock().unwrap()[index].1.push(Sub {
                                        id: value[1].as_str().unwrap_or_default().into(),
                                        kinds: list("kinds").iter().filter_map(Value::as_u64).collect(),
                                        p: list("#p").iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                                    });
                                    json!(["EOSE", value[1]])
                                }
                                Some("CLOSE") => {
                                    let id = value[1].as_str().unwrap_or_default().to_string();
                                    conns.lock().unwrap()[index].1.retain(|sub| sub.id != id);
                                    continue;
                                }
                                _ => continue,
                            };
                            if socket.send(Message::Text(reply.to_string().into())).await.is_err() { return; }
                        }
                    }
                }
            });
        }
    });
    (url, conns)
}

fn fanout(conns: &Conns, event: &Value) {
    let Ok(parsed) = serde_json::from_value::<Event>(event.clone()) else {
        return;
    };
    for (out, subs) in conns.lock().unwrap().iter() {
        for sub in subs {
            let kind = sub.kinds.is_empty() || sub.kinds.contains(&u64::from(parsed.kind));
            let p =
                sub.p.is_empty() || parsed.tag_values("p").any(|p| sub.p.iter().any(|w| w == p));
            if kind && p {
                let _ = out.send(json!(["EVENT", sub.id, event]).to_string());
            }
        }
    }
}

async fn subscribed(conns: &Conns) {
    for _ in 0..250 {
        if conns
            .lock()
            .unwrap()
            .iter()
            .any(|(_, subs)| !subs.is_empty())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the worker never subscribed");
}

/// A scripted TypeSafe door that answers only the worker's key.
async fn door() -> (String, Arc<AtomicUsize>) {
    let answered = Arc::new(AtomicUsize::new(0));
    let counter = answered.clone();
    let router = axum::Router::new().route(
        "/v1/systemone",
        axum::routing::post(move |headers: HeaderMap, body: Bytes| {
            let counter = counter.clone();
            async move {
                if headers.get("authorization").and_then(|v| v.to_str().ok())
                    != Some(&format!("Bearer {DOOR_KEY}"))
                {
                    return (StatusCode::UNAUTHORIZED, "no key").into_response();
                }
                counter.fetch_add(1, Ordering::SeqCst);
                let request: Value = serde_json::from_slice(&body).unwrap();
                let answers: serde_json::Map<String, Value> = request["questions"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|id| (id.clone(), json!({"type": "noul", "noul": 0.75})))
                    .collect();
                axum::Json(json!({
                    "model": request["model"],
                    "answers": answers,
                    "usage": {"input_tokens": 40, "output_tokens": 1},
                }))
                .into_response()
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{address}"), answered)
}

/// The worker's secret and public key.
const WORKER_SECRET: &str = "7777777777777777777777777777777777777777777777777777777777777777";

async fn hosted_rig(per_key_day: u32) -> (String, String, Arc<AtomicUsize>, tempfile::TempDir) {
    const KEY_ENV: &str = "JEV_HOSTED_FIXTURE_DOOR_KEY";
    // SAFETY: only this test binary reads this variable, and every test
    // sets the same value.
    unsafe { std::env::set_var(KEY_ENV, DOOR_KEY) };
    let (relay, conns) = relay().await;
    let (upstream, answered) = door().await;
    let jobs = tempfile::tempdir().unwrap();
    let config: WorkerConfig = serde_json::from_value(json!({
        "relay": relay,
        "worker_secret": WORKER_SECRET,
        "upstream": upstream,
        "jobs_dir": jobs.path(),
        "probe_secs": 0,
        "open": {
            "key_env": KEY_ENV,
            "models": ["jev-1.13.0"],
            "quota": {"per_key_day": per_key_day, "per_key_minute": per_key_day, "total_day": 100},
        },
        "service": {"door": "https://api.typesafe.ai", "version": "decision-worker/fixture"},
    }))
    .unwrap();
    let worker = Worker::open(config).unwrap();
    let pubkey = worker.pubkey().to_string();
    let url = relay.clone();
    tokio::spawn(async move {
        let (socket, _) = connect_async(&url).await.unwrap();
        let _ = worker.serve(socket).await;
    });
    subscribed(&conns).await;
    (relay, pubkey, answered, jobs)
}

fn pinned() -> Door<'static> {
    Door {
        url: jev_hosted::DOOR,
        model: "jev-1.13.0",
    }
}

fn question() -> jev::Questions {
    jev::Questions::new().with("tests_pass", jev::Noul::new("Do the tests pass?"))
}

#[tokio::test]
async fn a_computer_with_no_key_is_answered_by_the_hosted_worker_until_its_quota_ends() {
    let (relay, worker, answered, _jobs) = hosted_rig(2).await;
    let home = tempfile::tempdir().unwrap();
    let vars: HashMap<&str, String> = HashMap::from([
        (jev_hosted::RELAY_VAR, relay.clone()),
        (jev_hosted::WORKER_VAR, worker.clone()),
        (jev_hosted::DECISIONS_VAR, "jev".to_string()),
    ]);
    let env = |name: &str| vars.get(name).cloned();
    let resolved = jev_hosted::resolve(&env, home.path(), &pinned(), &|config| config).unwrap();
    assert!(
        matches!(resolved.via, Via::Hosted { .. }),
        "{:?}",
        resolved.via
    );
    let client = resolved.client;
    assert_eq!(client.base_url(), "https://api.typesafe.ai");
    assert!(client.service().unwrap().contains(&worker));

    for _ in 0..2 {
        let response = client
            .system_one(jev::SystemOneRequest::new(
                "cargo test: 3 passed",
                question(),
            ))
            .await
            .expect("the hosted worker answers");
        assert_eq!(response.model, "jev-1.13.0");
        assert!((response.noul("tests_pass").unwrap().noul - 0.75).abs() < 1e-9);
        assert_eq!(response.usage.input_tokens, Some(40));
        assert_eq!(
            response.service(),
            Some(json!({"door": "https://api.typesafe.ai", "version": "decision-worker/fixture"}))
        );
    }
    assert_eq!(answered.load(Ordering::SeqCst), 2);

    let refused = client
        .system_one(
            jev::SystemOneRequest::new("cargo test: 3 passed", question()).retry(
                jev::RetryPolicy {
                    max_retries: 0,
                    ..jev::RetryPolicy::default()
                },
            ),
        )
        .await
        .expect_err("the key's day is used");
    let why = jev_hosted::unavailable(&refused).expect("a brake's refusal says Jev is unavailable");
    assert!(why.starts_with("Jev refused: busy"), "{why}");
    assert!(!why.contains("today") && !why.contains("quota"), "{why}");
    assert_eq!(
        answered.load(Ordering::SeqCst),
        2,
        "a refused job reaches no door"
    );
}

#[tokio::test]
async fn an_unreachable_service_says_so() {
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = closed.local_addr().unwrap();
    drop(closed);
    let home = tempfile::tempdir().unwrap();
    let vars: HashMap<&str, String> = HashMap::from([
        (jev_hosted::RELAY_VAR, format!("ws://{address}")),
        (jev_hosted::DECISIONS_VAR, "jev".to_string()),
    ]);
    let env = |name: &str| vars.get(name).cloned();
    let client = jev_hosted::resolve(&env, home.path(), &pinned(), &|config| {
        config.retry(jev::RetryPolicy {
            max_retries: 0,
            ..jev::RetryPolicy::default()
        })
    })
    .unwrap()
    .client;
    let error = client
        .system_one(jev::SystemOneRequest::new("state", question()))
        .await
        .expect_err("nothing answers");
    let why = jev_hosted::unavailable(&error).expect("unreachable is unavailable");
    assert!(why.starts_with("Jev is unreachable"), "{why}");
}

/// A TypeSafe door that answers every call 402: a key with no credits.
async fn no_credits_door() -> (String, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    let router = axum::Router::new().route(
        "/v1/systemone",
        axum::routing::post(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::PAYMENT_REQUIRED,
                    axum::Json(json!({"error": {"code": "payment_required",
                        "message": "Your organization has no available TypeSafe API credits."}})),
                )
                    .into_response()
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{address}"), asked)
}

#[tokio::test]
async fn a_local_key_with_no_credits_is_answered_by_the_hosted_worker() {
    let (relay, worker, answered, _jobs) = hosted_rig(10).await;
    let (typesafe, asked) = no_credits_door().await;
    let home = tempfile::tempdir().unwrap();
    let vars: HashMap<&str, String> = HashMap::from([
        (jev_hosted::RELAY_VAR, relay.clone()),
        (jev_hosted::WORKER_VAR, worker.clone()),
        (jev_hosted::DECISIONS_VAR, "jev".to_string()),
    ]);
    let env = |name: &str| vars.get(name).cloned();
    let backups = jev_hosted::local_doors(&env, home.path());
    assert_eq!(backups.len(), 1, "no gateway or OpenRouter key here");
    assert!(
        !home.path().join(jev_hosted::KEY_FILE).exists(),
        "the hosted door makes its key only when first asked"
    );
    let client = jev_hosted::with_backups(
        "ts-out-of-credit".to_string(),
        backups,
        &Door {
            url: &typesafe,
            model: "jev-1.13.0",
        },
        &|config| config,
    )
    .unwrap();
    assert_eq!(client.doors(), Some(format!("doors {typesafe} → {relay}")));

    for _ in 0..3 {
        let response = client
            .system_one(jev::SystemOneRequest::new(
                "cargo test: 3 passed",
                question(),
            ))
            .await
            .expect("the hosted worker answers for the key that cannot pay");
        assert!((response.noul("tests_pass").unwrap().noul - 0.75).abs() < 1e-9);
        let service = response.service().unwrap();
        assert_eq!(service["door"], "https://api.typesafe.ai");
        assert!(service["exchange"].as_str().unwrap().contains(&worker));
        // The record names the hosted service as what answered.
        let record = jev_hosted::decision_record(
            "d",
            "judge",
            &client,
            json!({"model": "jev-1.13.0"}),
            Ok(&response),
            1,
        );
        assert_eq!(record.via.as_deref(), Some("hosted"));
    }
    // The 402 benched the person's own door: asked once, then skipped.
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert_eq!(answered.load(Ordering::SeqCst), 3);
    assert!(home.path().join(jev_hosted::KEY_FILE).exists());
}
