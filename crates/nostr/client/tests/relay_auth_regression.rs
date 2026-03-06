#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use futures_util::{SinkExt, StreamExt};
use nostr::Event;
use nostr::nip42::validate_auth_event;
use nostr_client::{RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage, Subscription};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

fn sample_event() -> Event {
    Event {
        id: "ab".repeat(32),
        pubkey: "cd".repeat(32),
        created_at: 1_762_800_000,
        kind: 42,
        tags: vec![vec!["h".to_string(), "oa-main".to_string()]],
        content: "hello from relay".to_string(),
        sig: "ef".repeat(64),
    }
}

async fn spawn_auth_resubscribe_relay(
    challenge: &'static str,
) -> (
    String,
    oneshot::Receiver<Event>,
    oneshot::Receiver<Value>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock relay listener");
    let addr = listener.local_addr().expect("resolve listener addr");
    let (auth_tx, auth_rx) = oneshot::channel();
    let (req_tx, req_rx) = oneshot::channel();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept websocket client");
        let mut ws = accept_async(stream)
            .await
            .expect("upgrade websocket connection");

        let first = ws
            .next()
            .await
            .expect("first frame")
            .expect("valid first websocket frame");
        let Message::Text(first_text) = first else {
            panic!("expected initial REQ frame");
        };
        let first_value: Value =
            serde_json::from_str(first_text.as_ref()).expect("parse initial REQ frame");
        let frame = first_value.as_array().expect("REQ frame array");
        assert_eq!(frame.first().and_then(Value::as_str), Some("REQ"));
        let subscription_id = frame[1].as_str().expect("REQ subscription id").to_string();

        let auth_frame = json!(["AUTH", challenge]);
        ws.send(Message::Text(auth_frame.to_string()))
            .await
            .expect("send auth challenge");

        let auth = ws
            .next()
            .await
            .expect("auth response frame")
            .expect("valid auth response frame");
        let Message::Text(auth_text) = auth else {
            panic!("expected AUTH response frame");
        };
        let auth_value: Value =
            serde_json::from_str(auth_text.as_ref()).expect("parse auth response");
        let auth_payload = auth_value.as_array().expect("auth response array");
        assert_eq!(auth_payload.first().and_then(Value::as_str), Some("AUTH"));
        let auth_event: Event =
            serde_json::from_value(auth_payload[1].clone()).expect("parse auth event");
        auth_tx.send(auth_event).expect("deliver auth event");

        let req = ws
            .next()
            .await
            .expect("resent REQ frame")
            .expect("valid resent REQ frame");
        let Message::Text(req_text) = req else {
            panic!("expected resent REQ frame");
        };
        let req_value: Value =
            serde_json::from_str(req_text.as_ref()).expect("parse resent REQ frame");
        req_tx.send(req_value.clone()).expect("deliver resent REQ");

        let payload = json!(["EVENT", subscription_id, sample_event()]);
        ws.send(Message::Text(payload.to_string()))
            .await
            .expect("send event after auth");
    });

    (format!("ws://{}", addr), auth_rx, req_rx, handle)
}

async fn spawn_auth_failure_relay(
    challenge: &'static str,
) -> (String, oneshot::Receiver<Option<Value>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock relay listener");
    let addr = listener.local_addr().expect("resolve listener addr");
    let (frame_tx, frame_rx) = oneshot::channel();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept websocket client");
        let mut ws = accept_async(stream)
            .await
            .expect("upgrade websocket connection");

        let first = ws
            .next()
            .await
            .expect("first frame")
            .expect("valid first websocket frame");
        let Message::Text(first_text) = first else {
            panic!("expected initial REQ frame");
        };
        let first_value: Value =
            serde_json::from_str(first_text.as_ref()).expect("parse initial REQ frame");
        let frame = first_value.as_array().expect("REQ frame array");
        assert_eq!(frame.first().and_then(Value::as_str), Some("REQ"));

        let auth_frame = json!(["AUTH", challenge]);
        ws.send(Message::Text(auth_frame.to_string()))
            .await
            .expect("send auth challenge");

        let observed = tokio::time::timeout(Duration::from_millis(250), ws.next())
            .await
            .ok()
            .and_then(|message| message)
            .and_then(|message| message.ok())
            .and_then(|message| match message {
                Message::Text(text) => serde_json::from_str(text.as_ref()).ok(),
                _ => None,
            });
        frame_tx.send(observed).expect("deliver observed frame");
    });

    (format!("ws://{}", addr), frame_rx, handle)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_success_resends_active_subscription_and_delivers_event() {
    let challenge = "relay-auth-challenge";
    let (relay_url, auth_rx, req_rx, relay_handle) = spawn_auth_resubscribe_relay(challenge).await;
    let relay = RelayConnection::with_config(
        relay_url.as_str(),
        RelayConfig {
            connect_timeout: Duration::from_secs(2),
            nip42_identity: Some(RelayAuthIdentity {
                private_key_hex: "11".repeat(32),
            }),
        },
    )
    .expect("relay connection");
    relay.connect().await.expect("connect relay");

    let (subscription, mut event_rx) = Subscription::with_channel(
        "chat-sync".to_string(),
        vec![json!({"kinds":[42], "#h":["oa-main"]})],
    );
    relay.subscribe(subscription).await.expect("subscribe");

    let auth_message = tokio::time::timeout(Duration::from_secs(2), relay.recv())
        .await
        .expect("wait for auth message")
        .expect("relay recv result")
        .expect("auth relay message");
    assert!(matches!(auth_message, RelayMessage::Auth(value) if value == challenge));

    let auth_event = tokio::time::timeout(Duration::from_secs(2), auth_rx)
        .await
        .expect("wait for auth event")
        .expect("auth event delivered");
    validate_auth_event(&auth_event, relay_url.as_str(), challenge, None)
        .expect("validate auth event");

    let req_value = tokio::time::timeout(Duration::from_secs(2), req_rx)
        .await
        .expect("wait for resent REQ")
        .expect("resent REQ delivered");
    let req_frame = req_value.as_array().expect("resent REQ array");
    assert_eq!(req_frame.first().and_then(Value::as_str), Some("REQ"));
    assert_eq!(req_frame.get(1).and_then(Value::as_str), Some("chat-sync"));

    let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("wait for relayed event")
        .expect("subscription event");
    assert_eq!(event.content, "hello from relay");

    let _ = relay.disconnect().await;
    relay_handle.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_auth_identity_does_not_emit_auth_or_resent_req_frames() {
    let challenge = "relay-auth-failure";
    let (relay_url, observed_rx, relay_handle) = spawn_auth_failure_relay(challenge).await;
    let relay = RelayConnection::with_config(
        relay_url.as_str(),
        RelayConfig {
            connect_timeout: Duration::from_secs(2),
            nip42_identity: Some(RelayAuthIdentity {
                private_key_hex: "not-hex".to_string(),
            }),
        },
    )
    .expect("relay connection");
    relay.connect().await.expect("connect relay");
    relay
        .subscribe(Subscription::new(
            "chat-sync".to_string(),
            vec![json!({"kinds":[42], "#h":["oa-main"]})],
        ))
        .await
        .expect("subscribe");

    let auth_message = tokio::time::timeout(Duration::from_secs(2), relay.recv())
        .await
        .expect("wait for auth message")
        .expect("relay recv result")
        .expect("auth relay message");
    assert!(matches!(auth_message, RelayMessage::Auth(value) if value == challenge));

    let observed = tokio::time::timeout(Duration::from_secs(2), observed_rx)
        .await
        .expect("wait for observed client frame")
        .expect("observed frame delivered");
    assert!(observed.is_none());

    let _ = relay.disconnect().await;
    relay_handle.abort();
}
