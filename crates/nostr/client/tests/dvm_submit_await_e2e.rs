#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use futures_util::{SinkExt, StreamExt};
use nostr::Event;
use nostr_client::{ClientError, DvmClient, PoolConfig, RelayPool};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

fn request_event(event_id: &str) -> Event {
    Event {
        id: event_id.to_string(),
        pubkey: "buyer-pubkey".to_string(),
        created_at: 1_760_000_000,
        kind: 5050,
        tags: vec![vec![
            "i".to_string(),
            "text".to_string(),
            "hello".to_string(),
        ]],
        content: "generate".to_string(),
        sig: "11".repeat(64),
    }
}

fn result_event(event_id: &str, request_event_id: &str) -> Event {
    Event {
        id: event_id.to_string(),
        pubkey: "provider-pubkey".to_string(),
        created_at: 1_760_000_001,
        kind: 6050,
        tags: vec![
            vec!["e".to_string(), request_event_id.to_string()],
            vec!["p".to_string(), "buyer-pubkey".to_string()],
            vec!["status".to_string(), "success".to_string()],
        ],
        content: "{\"output\":\"ok\"}".to_string(),
        sig: "22".repeat(64),
    }
}

async fn spawn_mock_relay(send_result: bool) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock relay listener");
    let addr = listener.local_addr().expect("resolve listener addr");

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept websocket client");
        let mut ws = accept_async(stream)
            .await
            .expect("upgrade websocket connection");

        let mut seen_request_event_id: Option<String> = None;
        loop {
            let Some(message) = ws.next().await else {
                break;
            };
            let Ok(message) = message else {
                break;
            };
            let Message::Text(text) = message else {
                continue;
            };
            let value: Value = serde_json::from_str(text.as_ref()).expect("parse relay frame");
            let Some(frame) = value.as_array() else {
                continue;
            };
            let Some(kind) = frame.first().and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "EVENT" => {
                    let published: Event =
                        serde_json::from_value(frame[1].clone()).expect("parse published event");
                    seen_request_event_id = Some(published.id);
                }
                "REQ" => {
                    if !send_result {
                        continue;
                    }
                    let subscription_id =
                        frame[1].as_str().expect("REQ subscription id").to_string();
                    let request_event_id = seen_request_event_id
                        .clone()
                        .or_else(|| extract_request_id_from_filters(frame))
                        .unwrap_or_else(|| "unknown-request".to_string());
                    let result = result_event("result-1", &request_event_id);
                    let payload = serde_json::json!(["EVENT", subscription_id, result]);
                    ws.send(Message::Text(payload.to_string()))
                        .await
                        .expect("send result event");
                }
                "CLOSE" => break,
                _ => {}
            }
        }
    });

    (format!("ws://{}", addr), handle)
}

fn extract_request_id_from_filters(frame: &[Value]) -> Option<String> {
    frame.iter().skip(2).find_map(|filter| {
        filter
            .as_object()
            .and_then(|object| object.get("#e"))
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_and_await_result_round_trip_succeeds() {
    let (relay_url, relay_handle) = spawn_mock_relay(true).await;
    let pool = Arc::new(RelayPool::new(PoolConfig::default()));
    pool.add_relay(relay_url.as_str())
        .await
        .expect("add relay to pool");
    pool.connect_all().await.expect("connect relay pool");

    let client = DvmClient::new(Arc::clone(&pool));
    let request = request_event("request-1");
    let result = client
        .submit_job_request_and_await_result(&request, Duration::from_secs(2))
        .await
        .expect("receive correlated result");

    assert_eq!(result.kind, 6050);
    assert!(
        result
            .tags
            .iter()
            .any(|tag| tag.first().is_some_and(|value| value == "e")
                && tag.get(1).is_some_and(|value| value == "request-1"))
    );

    pool.disconnect_all().await.expect("disconnect relay pool");
    relay_handle.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_and_await_result_times_out_when_no_result_arrives() {
    let (relay_url, relay_handle) = spawn_mock_relay(false).await;
    let pool = Arc::new(RelayPool::new(PoolConfig::default()));
    pool.add_relay(relay_url.as_str())
        .await
        .expect("add relay to pool");
    pool.connect_all().await.expect("connect relay pool");

    let client = DvmClient::new(Arc::clone(&pool));
    let request = request_event("request-timeout");
    let error = client
        .submit_job_request_and_await_result(&request, Duration::from_millis(350))
        .await
        .expect_err("expected timeout when relay never sends result");

    match error {
        ClientError::Timeout(message) => {
            assert!(message.contains("timed out waiting for result event"));
            assert!(message.contains("request-timeout"));
        }
        other => panic!("expected timeout error, got {other:?}"),
    }

    pool.disconnect_all().await.expect("disconnect relay pool");
    relay_handle.abort();
}
