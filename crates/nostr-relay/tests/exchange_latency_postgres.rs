//! Relay exchange latency against a live Postgres reached over a delayed link.
//!
//! A Coder observer read is two relay hops of private `3188` artifacts: the
//! phone's sealed request reaches the host through the relay, and the host's
//! sealed reply comes back the same way. Production reaches its Cloud SQL
//! database over a network link, so every sequential statement in an
//! admission costs a database round trip. This benchmark puts a proxy that
//! delays each direction by `NOSTR_RELAY_BENCH_DB_DELAY_US` (default 1000;
//! Tokio's millisecond timer makes that about 1 to 2 ms each way, and 0 skips
//! the proxy) between the gateway and Postgres, and times:
//!
//! - request to the relay's `OK`;
//! - request to its delivery on the host's standing subscription; and
//! - request to the host's reply on the phone's standing subscription.
//!
//! It is destructive and runs only against the disposable database made by
//! `scripts/test-postgres.sh`, or by hand with the same guard:
//!
//! ```sh
//! NOSTR_RELAY_TEST_DATABASE_URL="host=<socket dir> user=<user> dbname=<db>" \
//!   NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
//!   cargo test --release -p nostr-relay --test exchange_latency_postgres -- --nocapture
//! ```

use std::{
    net::{SocketAddr, TcpStream as StdTcpStream},
    time::{Duration, Instant},
};

use nostr_relay::{
    domain::{Event, Tag},
    gateway::{Gateway, GatewayConfig},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixStream},
    sync::mpsc,
};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

const PHONE: u8 = 31;
const HOST: u8 = 32;
const MAILBOX_BYTE: &str = "cd";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observer_exchange_latency_over_a_delayed_database_link() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the latency benchmark requires a disposable database guard");
        return;
    }
    let delay = Duration::from_micros(
        std::env::var("NOSTR_RELAY_BENCH_DB_DELAY_US")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1_000),
    );
    let exchanges = std::env::var("NOSTR_RELAY_BENCH_EXCHANGES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(40_usize);

    let proxied_url = if delay.is_zero() {
        database_url
    } else {
        start_delay_proxy(&database_url, delay).await
    };
    let gateway = Gateway::start(bench_config(proxied_url)).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());

    let report = tokio::task::spawn_blocking(move || run_exchanges(address, exchanges))
        .await
        .unwrap();
    println!(
        "{}",
        json!({
            "db_one_way_delay_us": delay.as_micros(),
            "exchanges": exchanges,
            "request_to_ok_ms": summary(&report.ok),
            "request_to_host_ms": summary(&report.delivered),
            "request_to_reply_ms": summary(&report.reply),
        })
    );
    assert_eq!(report.reply.len(), exchanges);

    stop.shutdown();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

struct Report {
    ok: Vec<Duration>,
    delivered: Vec<Duration>,
    reply: Vec<Duration>,
}

fn run_exchanges(address: SocketAddr, exchanges: usize) -> Report {
    let mailbox = MAILBOX_BYTE.repeat(32);
    let mut phone = connect(address, PHONE);
    let mut host = connect(address, HOST);
    subscribe(&mut phone, "replies", &pubkey(PHONE));
    subscribe(&mut host, "requests", &pubkey(HOST));

    let (sender, receiver) = std::sync::mpsc::channel::<(String, Instant)>();
    let host_thread = std::thread::spawn(move || {
        let mut delivered = Vec::new();
        let mut counter = 1_000_000_u64;
        while delivered.len() < exchanges {
            let message = read_json(&mut host);
            if message[0] != "EVENT" {
                continue;
            }
            delivered.push(Instant::now());
            let request_id = message[2]["id"].as_str().unwrap().to_owned();
            counter += 1;
            let reply = sealed(HOST, PHONE, &mailbox, counter, &request_id);
            send_json(&mut host, json!(["EVENT", reply]));
            let _ = sender.send((request_id, Instant::now()));
        }
        delivered
    });

    let mailbox = MAILBOX_BYTE.repeat(32);
    let mut ok = Vec::new();
    let mut sent_at = Vec::new();
    let mut reply = Vec::new();
    for index in 0..exchanges {
        let request = sealed(PHONE, HOST, &mailbox, index as u64, "request");
        let started = Instant::now();
        sent_at.push(started);
        send_json(&mut phone, json!(["EVENT", request]));
        let mut got_ok = false;
        let mut got_reply = false;
        while !(got_ok && got_reply) {
            let message = read_json(&mut phone);
            match message[0].as_str() {
                Some("OK") if message[1] == request.id => {
                    assert_eq!(message[2], true, "{message}");
                    ok.push(started.elapsed());
                    got_ok = true;
                }
                Some("EVENT") => {
                    reply.push(started.elapsed());
                    got_reply = true;
                }
                _ => {}
            }
        }
        let _ = receiver.recv().unwrap();
        // Space the exchanges like reads a person makes, not a flood.
        std::thread::sleep(Duration::from_millis(5));
    }
    let delivered_at = host_thread.join().unwrap();
    let delivered = delivered_at
        .iter()
        .zip(&sent_at)
        .map(|(at, sent)| at.duration_since(*sent))
        .collect();
    Report {
        ok,
        delivered,
        reply,
    }
}

fn sealed(from: u8, to: u8, mailbox: &str, counter: u64, note: &str) -> Event {
    let author = SecretKey::from_byte_array([from; 32]).unwrap();
    let recipient = pubkey(to).parse().unwrap();
    let inline = json!({"note": note, "n": counter});
    let bytes = nostr::contracts::jcs(&inline).unwrap();
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],
        "artifact":{"digest":nostr::contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":"openagents.fixture.v1"},
        "inline":inline,"issued_at":now(),"retain_until":now()+3600});
    let mut nonce = [0_u8; 32];
    nonce[..8].copy_from_slice(&counter.to_be_bytes());
    nonce[8] = from;
    nostr::private_artifact::seal(&body, &author, &recipient, mailbox, now(), nonce).unwrap()
}

fn summary(samples: &[Duration]) -> Value {
    let mut millis = samples
        .iter()
        .map(|sample| sample.as_secs_f64() * 1_000.0)
        .collect::<Vec<_>>();
    millis.sort_by(f64::total_cmp);
    let at = |fraction: f64| {
        let index = ((millis.len() as f64 - 1.0) * fraction).round() as usize;
        (millis[index] * 100.0).round() / 100.0
    };
    json!({"p50": at(0.5), "p90": at(0.9), "min": at(0.0), "max": at(1.0)})
}

/// Forward TCP connections to the database, delaying every chunk in each
/// direction by `delay` while keeping byte order.
async fn start_delay_proxy(database_url: &str, delay: Duration) -> String {
    let mut socket_dir = None;
    let mut port = "5432".to_owned();
    let mut rest = Vec::new();
    for part in database_url.split_whitespace() {
        match part.split_once('=') {
            Some(("host", value)) => socket_dir = Some(value.to_owned()),
            Some(("port", value)) => port = value.to_owned(),
            _ => rest.push(part.to_owned()),
        }
    }
    let target = socket_dir.expect("NOSTR_RELAY_TEST_DATABASE_URL names a host");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((client, _)) = listener.accept().await else {
                return;
            };
            let _ = client.set_nodelay(true);
            let target = target.clone();
            let port = port.clone();
            tokio::spawn(async move {
                if target.starts_with('/') {
                    let server = UnixStream::connect(format!("{target}/.s.PGSQL.{port}"))
                        .await
                        .unwrap();
                    let (server_read, server_write) = server.into_split();
                    let (client_read, client_write) = client.into_split();
                    tokio::join!(
                        pump(client_read, server_write, delay),
                        pump(server_read, client_write, delay)
                    );
                } else {
                    let server = TcpStream::connect(format!("{target}:{port}"))
                        .await
                        .unwrap();
                    let _ = server.set_nodelay(true);
                    let (server_read, server_write) = server.into_split();
                    let (client_read, client_write) = client.into_split();
                    tokio::join!(
                        pump(client_read, server_write, delay),
                        pump(server_read, client_write, delay)
                    );
                }
            });
        }
    });
    format!("host=127.0.0.1 port={} {}", local.port(), rest.join(" "))
}

async fn pump(
    mut reader: impl AsyncReadExt + Unpin + Send + 'static,
    mut writer: impl AsyncWriteExt + Unpin + Send + 'static,
    delay: Duration,
) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<(tokio::time::Instant, Vec<u8>)>();
    let reading = async move {
        let mut buffer = vec![0_u8; 65_536];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let due = tokio::time::Instant::now() + delay;
                    if sender.send((due, buffer[..read].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    };
    let writing = async move {
        while let Some((due, bytes)) = receiver.recv().await {
            tokio::time::sleep_until(due).await;
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    };
    tokio::join!(reading, writing);
}

fn bench_config(database_url: String) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some("ws://relay.test".to_owned());
    config.auth_required = true;
    config.db_connections = 4;
    config.shutdown_grace = Duration::from_secs(2);
    config.limits.max_frame_bytes = 131_072;
    config.limits.events_per_minute_ip = 100_000;
    config.limits.events_per_minute_pubkey = 100_000;
    config.limits.req_per_minute_ip = 1_000;
    config.limits.send_queue_capacity = 256;
    config
}

fn connect(address: SocketAddr, secret: u8) -> WebSocket<StdTcpStream> {
    let stream = StdTcpStream::connect(address).unwrap();
    stream.set_nodelay(true).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let (mut websocket, _) = client(format!("ws://{address}/"), stream).unwrap();
    let challenge = read_json(&mut websocket);
    assert_eq!(challenge[0], "AUTH");
    let auth = signed_event(
        secret,
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap().to_owned(),
            ]),
        ],
    );
    send_json(&mut websocket, json!(["AUTH", auth]));
    let response = read_json(&mut websocket);
    assert_eq!(response[2], true, "{response}");
    websocket
}

fn subscribe(websocket: &mut WebSocket<StdTcpStream>, id: &str, recipient: &str) {
    send_json(
        websocket,
        json!(["REQ", id, {"kinds":[3188], "#p":[recipient], "limit":0}]),
    );
    loop {
        let message = read_json(websocket);
        if message[0] == "EOSE" {
            return;
        }
    }
}

fn signed_event(secret_byte: u8, kind: u16, tags: Vec<Tag>) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at: now(),
        kind,
        tags,
        content: String::new(),
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

fn send_json(websocket: &mut WebSocket<StdTcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

fn read_json(websocket: &mut WebSocket<StdTcpStream>) -> Value {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected WebSocket message: {other:?}"),
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
