//! M4 actual-process proof. Run only through `scripts/test-postgres.sh`
//! against its dedicated disposable database.

use std::{
    io::{BufRead, BufReader},
    net::{SocketAddr, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nostr_relay::domain::Event;
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn m4_two_process_gap_and_chaos_contract() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: M4 process proof requires a disposable database guard");
        return;
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut relay_one = RelayProcess::spawn(&database_url);
    let mut relay_two = RelayProcess::spawn(&database_url);
    let (admin, connection) = runtime
        .block_on(tokio_postgres::connect(&database_url, NoTls))
        .unwrap();
    let driver = runtime.spawn(connection);

    let mut subscriber = connect_client(relay_two.address);
    send_json(&mut subscriber, json!(["REQ", "all", {"kinds": [1]}]));
    assert_eq!(read_json(&mut subscriber), json!(["EOSE", "all"]));

    let mut publisher = connect_client(relay_one.address);
    let first = signed_event(31, now(), "cross-process");
    publish(&mut publisher, &first);
    assert_event(&mut subscriber, &first);

    let missed = signed_event(32, now(), "notification intentionally omitted");
    let missed_seq = runtime.block_on(insert_without_notify(&admin, &missed));
    let trigger = signed_event(33, now(), "gap trigger");
    publish(&mut publisher, &trigger);
    let caught_up = read_json(&mut subscriber);
    assert_eq!(caught_up[0], "EVENT");
    assert_eq!(caught_up[2]["id"], missed.id);
    let delivered_trigger = read_json(&mut subscriber);
    assert_eq!(delivered_trigger[0], "EVENT");
    assert_eq!(delivered_trigger[2]["id"], trigger.id);
    assert!(missed_seq > 0);

    drop(publisher);
    let killed = relay_one.kill_and_wait();
    assert!(
        !killed.success(),
        "kill-one proof must terminate one process"
    );

    let mut survivor_publisher = connect_client(relay_two.address);
    let survivor = signed_event(34, now(), "survivor remains current");
    publish(&mut survivor_publisher, &survivor);
    assert_event(&mut subscriber, &survivor);

    runtime.block_on(inject_unbounded_gap(&admin));
    match subscriber.read() {
        Ok(Message::Close(_))
        | Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed)
        | Err(tokio_tungstenite::tungstenite::Error::AlreadyClosed)
        | Err(tokio_tungstenite::tungstenite::Error::Protocol(_)) => {}
        other => panic!("relay did not fail closed on an unbounded gap: {other:?}"),
    }
    let failed = relay_two.wait_for_exit(EXIT_TIMEOUT);
    assert!(
        !failed.success(),
        "a process that cannot prove its notification gap must exit non-zero"
    );

    drop(survivor_publisher);
    drop(admin);
    runtime.block_on(driver).unwrap().unwrap();
}

struct RelayProcess {
    child: Option<Child>,
    address: SocketAddr,
}

impl RelayProcess {
    fn spawn(database_url: &str) -> Self {
        let relay_secret = hex(&[9; 32]);
        let mut child = Command::new(env!("CARGO_BIN_EXE_nostr-relay"))
            .env("DATABASE_URL", database_url)
            .env("NOSTR_RELAY_BIND_ADDR", "127.0.0.1")
            .env("NOSTR_RELAY_PORT", "0")
            .env("NOSTR_RELAY_DB_CONNECTIONS", "2")
            .env("NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP", "10000")
            .env("NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY", "10000")
            .env("NOSTR_RELAY_RATE_REQ_PER_MIN_IP", "10000")
            .env("NOSTR_RELAY_MAX_CONNECTIONS_PER_IP", "100")
            .env("NOSTR_RELAY_URL", "ws://127.0.0.1")
            .env("NOSTR_RELAY_SECRET_KEY", relay_secret)
            .env_remove("PORT")
            .env_remove("NOSTR_RELAY_AUTH_REQUIRED")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        assert!(
            !line.is_empty(),
            "relay exited before reporting its address"
        );
        let startup: Value = serde_json::from_str(&line).unwrap();
        let address = startup["address"].as_str().unwrap().parse().unwrap();
        Self {
            child: Some(child),
            address,
        }
    }

    fn kill_and_wait(&mut self) -> std::process::ExitStatus {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap()
    }

    fn wait_for_exit(&mut self, duration: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + duration;
        let child = self.child.as_mut().unwrap();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                self.child = None;
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "relay did not exit before timeout"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for RelayProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn insert_without_notify(admin: &Client, event: &Event) -> i64 {
    let statement = admin
        .prepare(
            r#"
            INSERT INTO nostr_event (
                id, pubkey, created_at, kind, tags, content, sig,
                replacement_identifier, expires_at
            )
            VALUES ($1, $2, $3, $4, $5::text::jsonb, $6, $7, NULL, NULL)
            RETURNING ingest_seq
            "#,
        )
        .await
        .unwrap();
    let created_at = i64::try_from(event.created_at).unwrap();
    let kind = i32::from(event.kind);
    let tags = serde_json::to_string(&event.tags).unwrap();
    admin
        .query_one(
            &statement,
            &[
                &event.id,
                &event.pubkey,
                &created_at,
                &kind,
                &tags,
                &event.content,
                &event.sig,
            ],
        )
        .await
        .unwrap()
        .get(0)
}

async fn inject_unbounded_gap(admin: &Client) {
    let latest_statement = admin
        .prepare("SELECT COALESCE(MAX(ingest_seq), 0) FROM nostr_event")
        .await
        .unwrap();
    let latest: i64 = admin
        .query_one(&latest_statement, &[])
        .await
        .unwrap()
        .get(0);
    let payload = (latest + 10_000).to_string();
    let statement = admin
        .prepare("SELECT pg_notify('nostr_event', $1)")
        .await
        .unwrap();
    admin.query_one(&statement, &[&payload]).await.unwrap();
}

fn connect_client(address: SocketAddr) -> WebSocket<TcpStream> {
    let stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut websocket = client(format!("ws://{address}/"), stream).unwrap().0;
    let challenge = read_json(&mut websocket);
    assert_eq!(challenge[0], "AUTH");
    websocket
}

fn publish(websocket: &mut WebSocket<TcpStream>, event: &Event) {
    let response = publish_response(websocket, event);
    assert_eq!(response[0], "OK");
    assert_eq!(response[1], event.id);
    assert_eq!(response[2], true);
}

fn publish_response(websocket: &mut WebSocket<TcpStream>, event: &Event) -> Value {
    send_json(websocket, json!(["EVENT", event]));
    read_json(websocket)
}

fn assert_event(websocket: &mut WebSocket<TcpStream>, event: &Event) {
    let message = read_json(websocket);
    assert_eq!(message[0], "EVENT");
    assert_eq!(message[1], "all");
    assert_eq!(message[2]["id"], event.id);
}

fn send_json(websocket: &mut WebSocket<TcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

fn read_json(websocket: &mut WebSocket<TcpStream>) -> Value {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected WebSocket message: {other:?}"),
        }
    }
}

fn signed_event(secret_byte: u8, created_at: u64, content: &str) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind: 1,
        tags: Vec::new(),
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}
