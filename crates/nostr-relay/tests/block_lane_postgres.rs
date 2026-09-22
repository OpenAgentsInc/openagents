//! Configured Block-lane proofs against a live Postgres relay.
//!
//! `channel_window_contract` drives NIP-CW `POST /query` over real HTTP:
//! NIP-98 authentication, cursor-paged top-level rows, relay-signed
//! `kind:39006` bounds and `kind:39005` summaries, aux closure, a refused
//! half cursor, and access-scoped membership on a closed group.
//!
//! `push_executor_contract` drives a configured NIP-PL executor: a real
//! NIP-44-encrypted lease is accepted, a matching stored event posts the
//! fixed reconnect constant — and nothing else — to a stub push gateway.
//!
//! The suite is destructive and runs only against a disposable database.

use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener as StdTcpListener, TcpStream as StdTcpStream},
    sync::mpsc,
    time::Duration,
};

use nostr::{
    channel_window::{self, Cursor},
    nip44,
    push_lease::APNS_BODY,
};
use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig, PushExecutor},
};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
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

    // Relay one serves the channel window. Relay two shares the database and
    // carries the configured NIP-PL executor pointed at a stub push gateway.
    let gateway_one = Gateway::start(test_config(database_url.clone()))
        .await
        .unwrap();
    let address_one = gateway_one.local_addr();
    let stop_one = gateway_one.shutdown_handle();
    let server_one = tokio::spawn(gateway_one.run());

    let (wakes, recorded) = mpsc::channel();
    let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let gateway_port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || accept_wakes(listener, wakes));

    let mut push_config = test_config(database_url);
    push_config.push = Some(PushExecutor {
        secret: SecretKey::from_byte_array([77; 32]).unwrap(),
        pubkey: pubkey(77),
        origin: "ws://relay.test".to_owned(),
        gateway: format!("http://127.0.0.1:{gateway_port}/wake"),
        app_profile: "app.test/ios".to_owned(),
        transport: "apns".to_owned(),
    });
    let gateway_two = Gateway::start(push_config).await.unwrap();
    let address_two = gateway_two.local_addr();
    let stop_two = gateway_two.shutdown_handle();
    let server_two = tokio::spawn(gateway_two.run());

    tokio::task::spawn_blocking(move || {
        channel_window_contract(address_one);
        push_executor_contract(address_two, recorded);
    })
    .await
    .unwrap();

    stop_one.shutdown();
    stop_two.shutdown();
    timeout(Duration::from_secs(5), server_one)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(5), server_two)
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
/// cursor paging, and closed-group access scoping.
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

    // A channel with no group is served: an empty page is still a window
    // because it carries signed bounds.
    let absent_body = json!({"#h": ["cw-absent"], "top_level": true}).to_string();
    let absent_response = query_request(address, &absent_body, 22);
    let absent_page: Vec<Event> = serde_json::from_slice(&absent_response).unwrap();
    assert_eq!(absent_page.len(), 1);
    assert_eq!(absent_page[0].kind, channel_window::BOUNDS_KIND);

    // A closed group hides the window from a nonmember and serves a member.
    let closed = rpc_request(
        address,
        "creategroup",
        json!([
            "cw-closed",
            "Closed Room",
            "members only",
            "",
            true,
            pubkey(30),
            [9]
        ]),
    );
    assert_eq!(closed["result"], true, "{closed}");
    let sealed = signed_event(30, now(), 9, vec![h("cw-closed")], "members only");
    send_json(&mut admin, json!(["EVENT", sealed]));
    assert_eq!(read_json(&mut admin)[2], true);

    let closed_body = json!({"#h": ["cw-closed"], "top_level": true}).to_string();
    let outsider_response = query_request(address, &closed_body, 22);
    let outsider_page: Vec<Event> = serde_json::from_slice(&outsider_response).unwrap();
    assert!(
        outsider_page.is_empty(),
        "a closed channel serves nothing to a nonmember",
    );
    let member_response = query_request(address, &closed_body, 30);
    let member_page: Vec<Event> = serde_json::from_slice(&member_response).unwrap();
    assert!(member_page.iter().any(|event| event.id == sealed.id));
    channel_window::accept_window(&member_page, "cw-closed", None, &pubkey(90)).unwrap();

    admin.close(None).unwrap();
}

/// NIP-PL executor end to end: NIP-11 advertises the descriptor, a real
/// NIP-44 lease commits, a matching event posts the fixed reconnect
/// constant, and a nonmatching event posts nothing.
fn push_executor_contract(address: SocketAddr, wakes: mpsc::Receiver<String>) {
    let document = nip11(address);
    assert!(
        document["supported_extensions"]
            .as_array()
            .unwrap()
            .contains(&json!("nip-pl")),
        "a configured executor advertises nip-pl: {document}"
    );
    let push = &document["push"];
    assert_eq!(push["origin"], "ws://relay.test");
    assert_eq!(push["keys"][0]["pubkey"], pubkey(77));
    assert_eq!(push["app_profiles"][0]["transport"], "apns");

    let mut author = connect_client(address);
    let challenge = expect_auth_challenge(&mut author);
    authenticate(&mut author, 21, &challenge);

    // The lease plaintext is encrypted to the advertised executor key.
    let plaintext = json!({
        "v": 1,
        "origin": "ws://relay.test",
        "app_profile": "app.test/ios",
        "transport": "apns",
        "endpoint": "device-token-1",
        "generation": 1,
        "active": true,
        "subscriptions": [
            {"filter": {"kinds": [1], "#p": [pubkey(21)]}, "class": "default"}
        ],
    })
    .to_string();
    let author_secret = SecretKey::from_byte_array([21; 32]).unwrap();
    let content = nip44::encrypt(
        &plaintext,
        &nip44::conversation_key(&author_secret, &xonly(77)),
        [9; 32],
    )
    .unwrap();
    let lease = signed_event(
        21,
        now(),
        30_350,
        vec![
            Tag::new(vec!["d".into(), "installation-one".into()]),
            Tag::new(vec!["expiration".into(), (now() + 3_600).to_string()]),
            Tag::new(vec!["exec".into(), "current".into()]),
        ],
        &content,
    );
    send_json(&mut author, json!(["EVENT", lease]));
    assert_eq!(
        read_json(&mut author)[2],
        true,
        "a decryptable conforming lease is accepted",
    );

    // A lease whose origin does not bind the advertised tenant is refused.
    let mut mismatched = lease.clone();
    let wrong_origin = plaintext.replacen("ws://relay.test", "ws://other.test", 1);
    mismatched.content = nip44::encrypt(
        &wrong_origin,
        &nip44::conversation_key(&author_secret, &xonly(77)),
        [10; 32],
    )
    .unwrap();
    mismatched.tags[0] = Tag::new(vec!["d".into(), "installation-two".into()]);
    resign(&mut mismatched, 21);
    send_json(&mut author, json!(["EVENT", mismatched]));
    let refusal = read_json(&mut author);
    assert_eq!(refusal[2], false);
    assert!(
        refusal[3]
            .as_str()
            .unwrap()
            .starts_with("invalid: origin mismatch"),
        "{refusal}"
    );

    // A matching stored event wakes the endpoint through the stub gateway.
    let mut publisher = connect_client(address);
    let challenge = expect_auth_challenge(&mut publisher);
    authenticate(&mut publisher, 23, &challenge);
    let nonmatching = signed_event(23, now(), 1, vec![], "no p tag");
    send_json(&mut publisher, json!(["EVENT", nonmatching]));
    assert_eq!(read_json(&mut publisher)[2], true);
    let matching = signed_event(
        23,
        now(),
        1,
        vec![Tag::new(vec!["p".into(), pubkey(21)])],
        "a mention of the lease author",
    );
    send_json(&mut publisher, json!(["EVENT", matching]));
    assert_eq!(read_json(&mut publisher)[2], true);

    let request = wakes
        .recv_timeout(Duration::from_secs(10))
        .expect("the matching event wakes the installation");
    assert!(request.starts_with("POST /wake HTTP/1.1"), "{request}");
    assert!(
        request.contains("X-Push-Endpoint: device-token-1"),
        "{request}"
    );
    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
    assert_eq!(body, APNS_BODY);
    assert!(
        !request.contains(&matching.id),
        "the wake never carries event data: {request}"
    );
    assert!(
        wakes.recv_timeout(Duration::from_secs(2)).is_err(),
        "the nonmatching event posts no wake",
    );

    author.close(None).unwrap();
    publisher.close(None).unwrap();
}

fn accept_wakes(listener: StdTcpListener, wakes: mpsc::Sender<String>) {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 8_192];
                while let Ok(read) = stream.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        let head_end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
                        if let Ok(head) = std::str::from_utf8(&request[..head_end]) {
                            let declared = head
                                .split("\r\n")
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length: ")
                                        .and_then(|rest| rest.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if request.len() >= head_end + 4 + declared {
                                break;
                            }
                        }
                    }
                }
                let _ = wakes.send(String::from_utf8_lossy(&request).into_owned());
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return,
        }
    }
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

fn nip11(address: SocketAddr) -> Value {
    let mut stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .write_all(
            b"GET / HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    serde_json::from_slice(http_body(&response)).unwrap()
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

fn resign(event: &mut Event, secret_byte: u8) {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let digest = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&digest, &keypair).to_string();
}

fn pubkey(secret_byte: u8) -> String {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0
        .to_string()
}

fn xonly(secret_byte: u8) -> XOnlyPublicKey {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0
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
