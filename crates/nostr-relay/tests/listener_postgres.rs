//! The relay survives losing its Postgres notification listener (#9947).
//! Destructive: run only through `scripts/test-postgres.sh` against its
//! disposable database, one test at a time (the cancel test seeds unsigned
//! rows while it runs).

use std::{
    io::ErrorKind,
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nostr_relay::{
    domain::{Event, Filter},
    gateway::{Gateway, GatewayConfig},
    store::{LISTENER_APPLICATION_NAME, Store},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio_postgres::{Client, NoTls};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

#[test]
fn the_relay_keeps_serving_and_catches_up_when_its_listener_is_killed() {
    let Some(database_url) = disposable_database() else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let gateway_one = runtime
        .block_on(Gateway::start(test_config(database_url.clone())))
        .unwrap();
    let gateway_two = runtime
        .block_on(Gateway::start(test_config(database_url.clone())))
        .unwrap();
    let (address_one, address_two) = (gateway_one.local_addr(), gateway_two.local_addr());
    let (stop_one, stop_two) = (gateway_one.shutdown_handle(), gateway_two.shutdown_handle());
    let server_one = runtime.spawn(gateway_one.run());
    let server_two = runtime.spawn(gateway_two.run());
    let (admin, connection) = runtime
        .block_on(tokio_postgres::connect(&database_url, NoTls))
        .unwrap();
    let driver = runtime.spawn(connection);

    // The subscriber is on process two; every event comes from process one
    // or straight from the database, so process two sees each one only
    // through its notifications or its catch-up reads.
    let mut subscriber = connect_client(address_two);
    send_json(&mut subscriber, json!(["REQ", "all", {"kinds": [1]}]));
    assert_eq!(read_json(&mut subscriber)[0], "EOSE");
    let mut publisher = connect_client(address_one);
    let first = signed_event(41, "before any fault");
    publish(&mut publisher, &first);
    assert_event(&mut subscriber, &first);

    for round in 0..3_u8 {
        wait_for_listeners(&runtime, &admin, 2);
        // Committed without a notification: only catch-up by sequence can
        // deliver it.
        let missed = signed_event(50 + round, "committed while nobody listened");
        runtime.block_on(insert_without_notify(&admin, &missed));
        let killed = runtime.block_on(terminate_listeners(&admin));
        assert_eq!(killed, 2, "both processes had a listener to lose");
        // Published while the listeners are down or reconnecting.
        let during = signed_event(60 + round, "published during the outage");
        publish(&mut publisher, &during);
        assert_event(&mut subscriber, &missed);
        assert_event(&mut subscriber, &during);
    }

    // The replaced listeners carry live notifications again, and nothing
    // was delivered twice.
    wait_for_listeners(&runtime, &admin, 2);
    let last = signed_event(70, "after the listeners came back");
    publish(&mut publisher, &last);
    assert_event(&mut subscriber, &last);
    assert_no_message(&mut subscriber);

    // Both processes still accept new connections and serve history.
    for address in [address_one, address_two] {
        let mut reader = connect_client(address);
        send_json(&mut reader, json!(["REQ", "h", {"ids": [last.id]}]));
        let event = read_json(&mut reader);
        assert_eq!(event[0], "EVENT");
        assert_eq!(event[2]["id"], last.id);
        assert_eq!(read_json(&mut reader)[0], "EOSE");
    }

    drop((publisher, subscriber));
    stop_one.shutdown();
    stop_two.shutdown();
    // A process that had stopped itself would return an error here.
    runtime.block_on(server_one).unwrap().unwrap();
    runtime.block_on(server_two).unwrap().unwrap();
    drop(admin);
    runtime.block_on(driver).unwrap().unwrap();
}

/// A cancelled history read ends its own request only: the statement that
/// follows it on the same connection always runs (#9947).
#[test]
fn a_cancelled_read_never_fails_the_next_statement() {
    let Some(database_url) = disposable_database() else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (store, _) = Store::connect_with_report(&database_url).await.unwrap();
        store.latest_ingest_seq().await.unwrap();
        // Enough rows that a read is still running when some cancels land.
        let (admin, connection) = tokio_postgres::connect(&database_url, NoTls).await.unwrap();
        let driver = tokio::spawn(connection);
        admin
            .batch_execute(
                "INSERT INTO nostr_event (id, pubkey, created_at, kind, tags, content, sig) \
                 SELECT md5('id' || i) || md5('di' || i), md5('pk') || md5('kp'), i, 2, '[]', \
                        repeat('x', 200), repeat(md5('sig'), 4) \
                 FROM generate_series(1, 20000) AS i ON CONFLICT DO NOTHING",
            )
            .await
            .unwrap();
        let filter = Filter {
            kinds: Some(vec![2]),
            ..Filter::default()
        };
        // The seeded rows are unsigned, so the follow-up reads another kind.
        let follow_up = Filter {
            kinds: Some(vec![7]),
            ..Filter::default()
        };
        for round in 0..200_u64 {
            let (cancel, cancelled) = watch::channel(false);
            let read = store.query_filter_cancellable(&filter, now(), 500, i64::MAX, cancelled);
            let fire = async {
                tokio::time::sleep(Duration::from_micros(round % 7 * 150)).await;
                let _ = cancel.send(true);
            };
            let (result, ()) = tokio::join!(read, fire);
            drop(result);
            store
                .latest_ingest_seq()
                .await
                .unwrap_or_else(|error| panic!("round {round}: {error}"));
            store
                .query_filter(&follow_up, now(), 1)
                .await
                .unwrap_or_else(|error| panic!("round {round}: {error}"));
        }
        admin
            .batch_execute("DELETE FROM nostr_event WHERE kind = 2")
            .await
            .unwrap();
        drop(admin);
        driver.await.unwrap().unwrap();
    });
}

fn disposable_database() -> Option<String> {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return None;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the listener suite requires a disposable database guard");
        return None;
    }
    Some(database_url)
}

fn test_config(database_url: String) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.limits.events_per_minute_ip = 10_000;
    config.limits.events_per_minute_pubkey = 10_000;
    config.limits.req_per_minute_ip = 10_000;
    config.limits.max_connections_per_ip = 100;
    config
}

async fn terminate_listeners(admin: &Client) -> usize {
    admin
        .query(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE application_name = $1 AND pid <> pg_backend_pid()",
            &[&LISTENER_APPLICATION_NAME],
        )
        .await
        .unwrap()
        .iter()
        .filter(|row| row.get::<_, bool>(0))
        .count()
}

fn wait_for_listeners(runtime: &tokio::runtime::Runtime, admin: &Client, expected: i64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let count: i64 = runtime
            .block_on(admin.query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE application_name = $1",
                &[&LISTENER_APPLICATION_NAME],
            ))
            .unwrap()
            .get(0);
        if count == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "listeners did not come back: {count} of {expected}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

async fn insert_without_notify(admin: &Client, event: &Event) -> i64 {
    let created_at = i64::try_from(event.created_at).unwrap();
    let kind = i32::from(event.kind);
    let tags = serde_json::to_string(&event.tags).unwrap();
    admin
        .query_one(
            r#"
            INSERT INTO nostr_event (
                id, pubkey, created_at, kind, tags, content, sig,
                replacement_identifier, expires_at
            )
            VALUES ($1, $2, $3, $4, $5::text::jsonb, $6, $7, NULL, NULL)
            RETURNING ingest_seq
            "#,
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

fn connect_client(address: SocketAddr) -> WebSocket<TcpStream> {
    let stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client(format!("ws://{address}/"), stream).unwrap().0
}

fn publish(websocket: &mut WebSocket<TcpStream>, event: &Event) {
    send_json(websocket, json!(["EVENT", event]));
    let response = read_json(websocket);
    assert_eq!(response[0], "OK", "{response}");
    assert_eq!(response[1], event.id);
    assert_eq!(response[2], true, "{response}");
}

#[track_caller]
fn assert_event(websocket: &mut WebSocket<TcpStream>, event: &Event) {
    let message = read_json(websocket);
    assert_eq!(message[0], "EVENT", "{message}");
    assert_eq!(message[1], "all");
    assert_eq!(message[2]["id"], event.id, "expected {}", event.content);
}

fn assert_no_message(websocket: &mut WebSocket<TcpStream>) {
    websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    match websocket.read() {
        Err(tokio_tungstenite::tungstenite::Error::Io(error))
            if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
        other => panic!("expected no WebSocket message, got {other:?}"),
    }
}

fn send_json(websocket: &mut WebSocket<TcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

#[track_caller]
fn read_json(websocket: &mut WebSocket<TcpStream>) -> Value {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(text.as_str()).unwrap();
                // Unauthenticated clients are offered a challenge; ignore it.
                if value[0] != "AUTH" {
                    return value;
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected WebSocket message: {other:?}"),
        }
    }
}

fn signed_event(secret_byte: u8, content: &str) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at: now(),
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
