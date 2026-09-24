//! A live relay restart for the program-extension proof.
//!
//! Stored events survive a new gateway on the same database. Ephemeral
//! events do not. A private reminder stays invisible to a second client.
//! The suite is destructive and runs only against a disposable database.

use std::{
    net::{SocketAddr, TcpStream as StdTcpStream},
    time::Duration,
};

use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restarted_relay_keeps_durable_events_and_drops_ephemeral_ones() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL to a disposable database");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the restart proof requires a disposable database");
        return;
    }

    let config = config(database_url);
    let gateway = Gateway::start(config.clone()).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());
    assert_information_document(address).await;

    let published = tokio::task::spawn_blocking(move || publish(address))
        .await
        .unwrap();

    stop.shutdown();
    timeout(Duration::from_secs(8), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    let gateway = Gateway::start(config).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());
    assert_information_document(address).await;
    tokio::task::spawn_blocking(move || recover(address, &published))
        .await
        .unwrap();
    stop.shutdown();
    timeout(Duration::from_secs(8), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

struct Published {
    durable_id: String,
    ephemeral_id: String,
    reminder_id: String,
    author: String,
}

fn config(database_url: String) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some("ws://relay.test".to_owned());
    config.auth_required = true;
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.relay_signer = Some(RelaySigner::from_secret_hex(&hex(&[90; 32])).unwrap());
    config.identity.pubkey = config
        .relay_signer
        .as_ref()
        .map(|signer| signer.pubkey().to_owned());
    config.limits.max_frame_bytes = 65_536;
    config.limits.max_subscriptions = 8;
    config.limits.max_filters = 4;
    config.limits.max_limit = 20;
    config.limits.max_query_cost = 10_000;
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.limits.req_per_minute_ip = 200;
    config.limits.max_connections_per_ip = 8;
    config.limits.send_queue_capacity = 64;
    config
}

fn publish(address: SocketAddr) -> Published {
    let mut author = connect(address);
    authenticate(&mut author, 4);
    let durable = signed_event(4, now(), 1, Vec::new(), "durable note");
    send(&mut author, json!(["EVENT", durable]));
    assert_ok(&mut author, &durable.id);
    let ephemeral = signed_event(4, now(), 21_000, Vec::new(), "ephemeral note");
    send(&mut author, json!(["EVENT", ephemeral]));
    assert_ok(&mut author, &ephemeral.id);
    let reminder = signed_event(
        4,
        now(),
        30_300,
        vec![
            Tag::new(vec!["d".into(), "reminder-1".into()]),
            Tag::new(vec!["not_before".into(), now().to_string()]),
        ],
        &fake_nip44(),
    );
    send(&mut author, json!(["EVENT", reminder]));
    assert_ok(&mut author, &reminder.id);

    let mut stranger = connect(address);
    authenticate(&mut stranger, 8);
    send(
        &mut stranger,
        json!(["REQ", "hidden", {"kinds": [30300], "authors": [pubkey(4)]}]),
    );
    let hidden = read(&mut stranger);
    assert_eq!(hidden[0], "CLOSED");
    assert!(
        hidden[2]
            .as_str()
            .unwrap()
            .starts_with("restricted: to read these private events, filter authors"),
        "{hidden}"
    );

    Published {
        durable_id: durable.id,
        ephemeral_id: ephemeral.id,
        reminder_id: reminder.id,
        author: pubkey(4),
    }
}

fn recover(address: SocketAddr, published: &Published) {
    let mut first = connect(address);
    authenticate(&mut first, 4);
    let mut second = connect(address);
    authenticate(&mut second, 8);

    let handles = [
        std::thread::spawn({
            let id = published.durable_id.clone();
            move || {
                let mut client = connect(address);
                authenticate(&mut client, 5);
                expect_event(&mut client, &id);
            }
        }),
        std::thread::spawn({
            let id = published.durable_id.clone();
            move || {
                let mut client = connect(address);
                authenticate(&mut client, 6);
                expect_event(&mut client, &id);
            }
        }),
    ];
    for handle in handles {
        handle.join().unwrap();
    }

    send(
        &mut first,
        json!(["REQ", "gone", {"ids": [published.ephemeral_id]}]),
    );
    assert_eq!(read(&mut first)[0], "EOSE");
    send(
        &mut second,
        json!(["REQ", "private", {"kinds": [30300], "authors": [published.author]}]),
    );
    let hidden = read(&mut second);
    assert_eq!(hidden[0], "CLOSED");
    assert!(
        hidden[2]
            .as_str()
            .unwrap()
            .starts_with("restricted: to read these private events, filter authors"),
        "{hidden}"
    );
    send(
        &mut first,
        json!(["REQ", "own", {"ids": [published.reminder_id]}]),
    );
    let found = read(&mut first);
    assert_eq!(found[0], "EVENT");
    assert_eq!(found[2]["id"], published.reminder_id);
}

fn expect_event(websocket: &mut WebSocket<StdTcpStream>, id: &str) {
    send(websocket, json!(["REQ", "kept", {"ids": [id]}]));
    let event = read(websocket);
    assert_eq!(event[0], "EVENT");
    assert_eq!(event[2]["id"], id);
    assert_eq!(read(websocket)[0], "EOSE");
}

async fn assert_information_document(address: SocketAddr) {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            b"GET / HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let body = String::from_utf8(response).unwrap();
    let document: Value = serde_json::from_str(body.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let extensions = document["supported_extensions"].as_array().unwrap();
    assert!(extensions.contains(&json!("nip-cw")));
    assert!(!extensions.contains(&json!("nip-pl")));
    assert!(!extensions.contains(&json!("nip-gs")));
}

fn connect(address: SocketAddr) -> WebSocket<StdTcpStream> {
    let stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (websocket, _) = client(format!("ws://{address}/"), stream).unwrap();
    websocket
}

fn authenticate(websocket: &mut WebSocket<StdTcpStream>, secret: u8) {
    let challenge = read(websocket);
    assert_eq!(challenge[0], "AUTH");
    let event = signed_event(
        secret,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap().to_owned(),
            ]),
        ],
        "",
    );
    send(websocket, json!(["AUTH", event]));
    let response = read(websocket);
    assert_eq!(response[0], "OK");
    assert_eq!(response[2], true);
}

fn assert_ok(websocket: &mut WebSocket<StdTcpStream>, id: &str) {
    let response = read(websocket);
    assert_eq!(response[0], "OK", "{response}");
    assert_eq!(response[1], id);
    assert_eq!(response[2], true, "{response}");
}

fn send(websocket: &mut WebSocket<StdTcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

fn read(websocket: &mut WebSocket<StdTcpStream>) -> Value {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected WebSocket message: {other:?}"),
        }
    }
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn pubkey(secret_byte: u8) -> String {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0
        .to_string()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn fake_nip44() -> String {
    let mut bytes = [0_u8; 99];
    bytes[0] = 0x02;
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        output.push(char::from(
            TABLE[usize::from((second & 0x0f) << 2 | third >> 6)],
        ));
        output.push(char::from(TABLE[usize::from(third & 0x3f)]));
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
