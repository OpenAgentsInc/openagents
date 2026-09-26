//! Configured Block-lane proofs against a live Postgres relay.
//!
//! `channel_window_contract` drives NIP-CW `POST /query` over real HTTP:
//! NIP-98 authentication, cursor-paged top-level rows, relay-signed
//! `kind:39006` bounds and `kind:39005` summaries, aux closure, a refused
//! half cursor, and access scoping across group privacy changes.
//!
//! NIP-PL configuration is refused until the complete executor is implemented.
//!
//! The suite is destructive and runs only against a disposable database.

use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream as StdTcpStream},
    time::Duration,
};

use nostr::channel_window::{self, Cursor};
use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig, PushExecutor},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn block_lane_contract_against_postgres() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: live gateway suite requires a disposable database guard");
        return;
    }

    // The channel role runs; the incomplete push executor must remain inert.
    let gateway_one = Gateway::start(test_config(database_url.clone()))
        .await
        .unwrap();
    let address_one = gateway_one.local_addr();
    let stop_one = gateway_one.shutdown_handle();
    let server_one = tokio::spawn(gateway_one.run());

    let mut push_config = test_config(database_url);
    push_config.push = Some(PushExecutor {
        secret: SecretKey::from_byte_array([77; 32]).unwrap(),
        pubkey: pubkey(77),
        origin: "ws://relay.test".to_owned(),
        gateway: "http://127.0.0.1:1/wake".into(),
        app_profile: "app.test/ios".to_owned(),
        transport: "apns".to_owned(),
    });
    assert!(
        Gateway::start(push_config).await.is_err(),
        "incomplete push delivery must not start"
    );
    tokio::task::spawn_blocking(move || channel_window_contract(address_one))
        .await
        .unwrap();

    stop_one.shutdown();
    timeout(Duration::from_secs(5), server_one)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

fn test_config(database_url: String) -> GatewayConfig {
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
    config.management_pubkey = Some(pubkey(91));
    config.limits.max_frame_bytes = 131_072;
    config.limits.max_subscriptions = 8;
    config.limits.max_filters = 4;
    config.limits.max_limit = 10;
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.limits.req_per_minute_ip = 100;
    config.limits.max_connections_per_ip = 10;
    config.limits.send_queue_capacity = 64;
    config
}

/// NIP-CW `POST /query` end to end: real stored channel events, real
/// NIP-98 authorization, real signed bounds and summary overlays, real
/// cursor paging, and private-group access scoping independent of joining policy.
fn channel_window_contract(address: SocketAddr) {
    let created = rpc_request(
        address,
        "creategroup",
        json!([
            "cw-room",
            "Window Room",
            "NIP-CW live room",
            "",
            false,
            pubkey(30),
            [7, 9]
        ]),
    );
    assert_eq!(created["result"], true, "{created}");

    let mut admin = connect_client(address);
    let challenge = expect_auth_challenge(&mut admin);
    authenticate(&mut admin, 30, &challenge);
    let h = |group: &str| Tag::new(vec!["h".into(), group.to_owned()]);
    let root_one = signed_event(30, now() - 30, 9, vec![h("cw-room")], "first");
    let root_two = signed_event(30, now() - 20, 9, vec![h("cw-room")], "second");
    let root_three = signed_event(30, now() - 10, 9, vec![h("cw-room")], "third");
    let reply = signed_event(
        30,
        now() - 15,
        9,
        vec![
            h("cw-room"),
            Tag::new(vec![
                "e".into(),
                root_three.id.clone(),
                String::new(),
                "reply".into(),
            ]),
        ],
        "a reply, not a row",
    );
    let reaction = signed_event(
        30,
        now() - 12,
        7,
        vec![
            h("cw-room"),
            Tag::new(vec!["e".into(), root_three.id.clone()]),
            Tag::new(vec!["p".into(), pubkey(30)]),
        ],
        "+",
    );
    for event in [&root_one, &root_two, &root_three, &reply, &reaction] {
        send_json(&mut admin, json!(["EVENT", event]));
        assert_eq!(read_json(&mut admin)[2], true, "channel seed refused");
    }

    // Head page: two newest top-level rows, aux, summary, signed bounds.
    // `kinds` restricts rows to kind 9: the unmarked `e` tag makes the
    // kind-7 reaction depth 0, so without the restriction it is a row.
    let head_body = json!({
        "#h": ["cw-room"],
        "top_level": true,
        "kinds": [9],
        "limit": 2,
        "include_summaries": true,
        "include_aux": true,
    })
    .to_string();
    let head_response = query_request(address, &head_body, 22);
    let head_page: Vec<Event> = serde_json::from_slice(&head_response).unwrap();
    let rows = head_page
        .iter()
        .filter(|event| event.kind == 9)
        .collect::<Vec<_>>();
    assert_eq!(
        rows.iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        vec![root_three.id.as_str(), root_two.id.as_str()],
        "top-level rows, newest first; the reply is not a row",
    );
    assert!(
        head_page.iter().any(|event| event.id == reaction.id),
        "aux closure carries the reaction",
    );
    let summary = head_page
        .iter()
        .find(|event| event.kind == channel_window::SUMMARY_KIND)
        .expect("a row with a reply gets a summary");
    assert_eq!(summary.pubkey, pubkey(90), "the relay signs summaries");
    assert_eq!(summary.tag_values("e").next(), Some(root_three.id.as_str()));
    let summary_content: Value = serde_json::from_str(&summary.content).unwrap();
    assert_eq!(summary_content["reply_count"], 1);
    assert_eq!(summary_content["descendant_count"], 1);
    let bounds = head_page
        .iter()
        .find(|event| event.kind == channel_window::BOUNDS_KIND)
        .expect("every served page carries bounds");
    assert_eq!(bounds.pubkey, pubkey(90));
    bounds.validate_crypto().unwrap();
    assert_eq!(
        bounds.tag_values("d").next(),
        Some("cw-room:head"),
        "bounds bind the channel and the request cursor",
    );
    channel_window::accept_window(&head_page, "cw-room", None, &pubkey(90)).unwrap();
    let bounds_content: Value = serde_json::from_str(&bounds.content).unwrap();
    assert_eq!(bounds_content["has_more"], true);
    let cursor = Cursor {
        created_at: bounds_content["next_cursor"]["created_at"]
            .as_u64()
            .unwrap(),
        id: bounds_content["next_cursor"]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    };

    // Continuation: the composite cursor reaches the remaining row.
    let next_body = json!({
        "#h": ["cw-room"],
        "top_level": true,
        "kinds": [9],
        "limit": 2,
        "until": cursor.created_at,
        "before_id": cursor.id,
    })
    .to_string();
    let next_response = query_request(address, &next_body, 22);
    let next_page: Vec<Event> = serde_json::from_slice(&next_response).unwrap();
    let rows = next_page
        .iter()
        .filter(|event| event.kind == 9)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, root_one.id);
    let bounds = next_page
        .iter()
        .find(|event| event.kind == channel_window::BOUNDS_KIND)
        .unwrap();
    assert_eq!(
        bounds.tag_values("d").next(),
        Some(format!("cw-room:{}:{}", cursor.created_at, cursor.id).as_str()),
    );
    channel_window::accept_window(&next_page, "cw-room", Some(&cursor), &pubkey(90)).unwrap();
    let bounds_content: Value = serde_json::from_str(&bounds.content).unwrap();
    assert_eq!(bounds_content["has_more"], false);
    assert!(bounds_content["next_cursor"].is_null());

    // A half cursor is refused rather than silently degraded.
    let half = json!({"#h": ["cw-room"], "top_level": true, "until": 1}).to_string();
    let head = format!(
        "POST /query HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/json\r\nAuthorization: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        nip98(&half, 22),
        half.len()
    );
    let response = raw_http(address, &head, half.as_bytes());
    assert!(
        response.starts_with(b"HTTP/1.1 400 Bad Request"),
        "{response:?}"
    );

    // No authorization is refused.
    let head = format!(
        "POST /query HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        head_body.len()
    );
    let response = raw_http(address, &head, head_body.as_bytes());
    assert!(
        response.starts_with(b"HTTP/1.1 401 Unauthorized"),
        "{response:?}"
    );

    // An authorization for a different method is refused.
    let wrong_method = signed_event(
        22,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/query".into()]),
            Tag::new(vec!["method".into(), "GET".into()]),
            Tag::new(vec![
                "payload".into(),
                hex(&Sha256::digest(head_body.as_bytes())),
            ]),
        ],
        "",
    );
    let head = format!(
        "POST /query HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/json\r\nAuthorization: Nostr {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        base64(&serde_json::to_vec(&wrong_method).unwrap()),
        head_body.len()
    );
    let response = raw_http(address, &head, head_body.as_bytes());
    assert!(
        response.starts_with(b"HTTP/1.1 401 Unauthorized"),
        "{response:?}"
    );

    // A nonexistent channel has no authority to sign a bounds overlay.
    let absent_body = json!({"#h": ["cw-absent"], "top_level": true}).to_string();
    let absent_response = query_request(address, &absent_body, 22);
    let absent_page: Vec<Event> = serde_json::from_slice(&absent_response).unwrap();
    assert!(absent_page.is_empty());

    // Closed controls joining, not reading: an otherwise public group is served.
    let closed = rpc_request(
        address,
        "creategroup",
        json!([
            "cw-closed",
            "Closed Room",
            "public reading, closed joining",
            "",
            true,
            pubkey(30),
            [9]
        ]),
    );
    assert_eq!(closed["result"], true, "{closed}");
    let sealed = signed_event(30, now(), 9, vec![h("cw-closed")], "channel row");
    send_json(&mut admin, json!(["EVENT", sealed]));
    assert_eq!(read_json(&mut admin)[2], true);

    let closed_body = json!({"#h": ["cw-closed"], "top_level": true}).to_string();
    let outsider_response = query_request(address, &closed_body, 22);
    let outsider_page: Vec<Event> = serde_json::from_slice(&outsider_response).unwrap();
    assert!(outsider_page.iter().any(|event| event.id == sealed.id));
    channel_window::accept_window(&outsider_page, "cw-closed", None, &pubkey(90)).unwrap();

    // The current metadata controls both queries. Opening joins while making
    // reads private must remove the rows and every existence-revealing overlay.
    let private = signed_event(
        30,
        now(),
        9_002,
        vec![h("cw-closed"), Tag::new(vec!["private".into()])],
        "make private with open joining",
    );
    send_json(&mut admin, json!(["EVENT", private]));
    assert_eq!(read_json(&mut admin)[2], true, "privacy mutation stores");
    let outsider_response = query_request(address, &closed_body, 22);
    let outsider_page: Vec<Event> = serde_json::from_slice(&outsider_response).unwrap();
    assert!(
        outsider_page.is_empty(),
        "an open private group has no bounds"
    );
    let member_response = query_request(address, &closed_body, 30);
    let member_page: Vec<Event> = serde_json::from_slice(&member_response).unwrap();
    assert!(member_page.iter().any(|event| event.id == sealed.id));
    channel_window::accept_window(&member_page, "cw-closed", None, &pubkey(90)).unwrap();

    // A hidden group's existence is not disclosed by a window overlay either.
    let hidden = signed_event(
        30,
        now(),
        9_002,
        vec![h("cw-closed"), Tag::new(vec!["hidden".into()])],
        "hide metadata",
    );
    send_json(&mut admin, json!(["EVENT", hidden]));
    assert_eq!(read_json(&mut admin)[2], true, "hidden mutation stores");
    let outsider_response = query_request(address, &closed_body, 22);
    let outsider_page: Vec<Event> = serde_json::from_slice(&outsider_response).unwrap();
    assert!(outsider_page.is_empty(), "a hidden group has no bounds");
    let member_response = query_request(address, &closed_body, 30);
    let member_page: Vec<Event> = serde_json::from_slice(&member_response).unwrap();
    assert!(member_page.iter().any(|event| event.id == sealed.id));

    let public = signed_event(
        30,
        now(),
        9_002,
        vec![h("cw-closed"), Tag::new(vec!["closed".into()])],
        "restore public reading with closed joining",
    );
    send_json(&mut admin, json!(["EVENT", public]));
    assert_eq!(read_json(&mut admin)[2], true, "public mutation stores");
    let outsider_response = query_request(address, &closed_body, 22);
    let outsider_page: Vec<Event> = serde_json::from_slice(&outsider_response).unwrap();
    assert!(outsider_page.iter().any(|event| event.id == sealed.id));
    channel_window::accept_window(&outsider_page, "cw-closed", None, &pubkey(90)).unwrap();

    admin.close(None).unwrap();
}

fn nip98(body: &str, secret: u8) -> String {
    let event = signed_event(
        secret,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/query".into()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec![
                "payload".into(),
                hex(&Sha256::digest(body.as_bytes())),
            ]),
        ],
        "",
    );
    format!("Nostr {}", base64(&serde_json::to_vec(&event).unwrap()))
}

/// POST `/query` with a NIP-98 signature from `secret` and return the
/// decoded body of the `200 OK` response.
fn query_request(address: SocketAddr, body: &str, secret: u8) -> Vec<u8> {
    let head = format!(
        "POST /query HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/json\r\nAuthorization: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        nip98(body, secret),
        body.len()
    );
    let response = raw_http(address, &head, body.as_bytes());
    assert!(
        response.starts_with(b"HTTP/1.1 200 OK"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    http_body(&response).to_vec()
}

fn rpc_request(address: SocketAddr, method: &str, params: Value) -> Value {
    let body = json!({ "method": method, "params": params }).to_string();
    let payload_hash = hex(&Sha256::digest(body.as_bytes()));
    let event = signed_event(
        91,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/manage".into()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["payload".into(), payload_hash]),
        ],
        "",
    );
    let authorization = base64(&serde_json::to_vec(&event).unwrap());
    let mut stream = StdTcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /manage HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/nostr+json+rpc\r\nAuthorization: Nostr {authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

fn raw_http(address: SocketAddr, head: &str, body: &[u8]) -> Vec<u8> {
    let mut stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap_or_else(|error| {
        if error.kind() == ErrorKind::ConnectionReset {
            response.len()
        } else {
            panic!("http read failed: {error}")
        }
    });
    response
}

fn http_body(response: &[u8]) -> &[u8] {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    &response[boundary + 4..]
}

fn connect_client(address: SocketAddr) -> WebSocket<StdTcpStream> {
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

fn expect_auth_challenge(websocket: &mut WebSocket<StdTcpStream>) -> String {
    let message = read_json(websocket);
    assert_eq!(message[0], "AUTH");
    message[1].as_str().unwrap().to_owned()
}

fn authenticate(websocket: &mut WebSocket<StdTcpStream>, secret: u8, challenge: &str) -> Event {
    let event = signed_event(
        secret,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec!["challenge".into(), challenge.to_owned()]),
        ],
        "",
    );
    send_json(websocket, json!(["AUTH", event]));
    let response = read_json(websocket);
    assert_eq!(response[0], "OK");
    assert_eq!(response[2], true);
    event
}

fn send_json(websocket: &mut WebSocket<StdTcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

#[track_caller]
fn read_json(websocket: &mut WebSocket<StdTcpStream>) -> Value {
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from((first & 3) << 4 | second >> 4)],
        ));
        output.push(if chunk.len() > 1 {
            char::from(TABLE[usize::from((second & 15) << 2 | third >> 6)])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(TABLE[usize::from(third & 63)])
        } else {
            '='
        });
    }
    output
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
