//! Reproducible M4 load proof. This is ignored by ordinary test runs and is
//! enabled by `scripts/test-postgres.sh` against a disposable database.

use std::{
    net::{SocketAddr, TcpStream},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nostr_relay::{
    domain::Event,
    gateway::{Gateway, GatewayConfig},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

const PUBLISHERS: usize = 4;
const EVENT_COUNT: usize = 2_000;
const CONNECT_SAMPLES: usize = 250;
const REQ_SAMPLES: usize = 100;
const HISTORY_LIMIT: usize = 10;
const RUNS: usize = 5;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "release-mode load proof; run scripts/test-postgres.sh"]
async fn m4_release_load_proof() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: load proof requires a disposable database guard");
        return;
    }

    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.db_connections = PUBLISHERS;
    config.shutdown_grace = Duration::from_secs(5);
    config.limits.max_subscriptions = 256;
    config.limits.max_filters = 4;
    config.limits.max_limit = 100;
    config.limits.max_query_cost = 1_000_000;
    config.limits.events_per_minute_ip = 100_000;
    config.limits.events_per_minute_pubkey = 100_000;
    config.limits.req_per_minute_ip = 100_000;
    config.limits.max_connections_per_ip = 1_000;
    config.limits.send_queue_capacity = 2_048;

    let gateway = Gateway::start(config).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());
    let result = tokio::task::spawn_blocking(move || run_load_series(address))
        .await
        .unwrap();
    println!("M4_BENCHMARK_JSON={result}");

    stop.shutdown();
    timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[derive(Debug, Clone, Copy)]
struct RunMetrics {
    events_per_second: f64,
    connect_p99_ms: f64,
    req_to_eose_p99_ms: f64,
}

fn run_load_series(address: SocketAddr) -> Value {
    let results = (0..RUNS)
        .map(|run| run_load(address, run))
        .collect::<Vec<_>>();
    let event_rates = results
        .iter()
        .map(|result| result.events_per_second)
        .collect::<Vec<_>>();
    let connect_p99 = results
        .iter()
        .map(|result| result.connect_p99_ms)
        .collect::<Vec<_>>();
    let req_p99 = results
        .iter()
        .map(|result| result.req_to_eose_p99_ms)
        .collect::<Vec<_>>();
    json!({
        "runs": RUNS,
        "events_per_run": EVENT_COUNT,
        "total_events": RUNS * EVENT_COUNT,
        "publishers": PUBLISHERS,
        "events_per_second_median": rounded(median(&event_rates)),
        "events_per_second_range": range(&event_rates),
        "connect_samples_per_run": CONNECT_SAMPLES,
        "connect_p99_ms_median": rounded(median(&connect_p99)),
        "connect_p99_ms_range": range(&connect_p99),
        "req_samples_per_run": REQ_SAMPLES,
        "req_history_events": HISTORY_LIMIT,
        "req_to_eose_p99_ms_median": rounded(median(&req_p99)),
        "req_to_eose_p99_ms_range": range(&req_p99),
    })
}

fn run_load(address: SocketAddr, run: usize) -> RunMetrics {
    let timestamp = now();
    let events = (0..PUBLISHERS)
        .map(|publisher| {
            (0..EVENT_COUNT / PUBLISHERS)
                .map(|sequence| {
                    signed_event(
                        u8::try_from(40 + publisher).unwrap(),
                        timestamp,
                        &format!("load-{run}-{publisher}-{sequence}"),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let barrier = Arc::new(Barrier::new(PUBLISHERS + 1));
    let publishers = events
        .into_iter()
        .map(|events| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut websocket = connect_client(address);
                barrier.wait();
                for event in events {
                    send_json(&mut websocket, json!(["EVENT", event]));
                    let response = read_json(&mut websocket);
                    assert_eq!(response[0], "OK");
                    assert_eq!(response[2], true);
                }
                let _ = websocket.close(None);
            })
        })
        .collect::<Vec<_>>();
    let ingest_started = Instant::now();
    barrier.wait();
    for publisher in publishers {
        publisher.join().unwrap();
    }
    let ingest_elapsed = ingest_started.elapsed();
    let events_per_second = EVENT_COUNT as f64 / ingest_elapsed.as_secs_f64();

    // Let the notification cursor drain so connection and query latency are
    // measured at steady state rather than behind the just-finished writes.
    thread::sleep(Duration::from_secs(1));

    let mut connect_latencies = Vec::with_capacity(CONNECT_SAMPLES);
    for _ in 0..CONNECT_SAMPLES {
        let started = Instant::now();
        let mut websocket = connect_client(address);
        connect_latencies.push(started.elapsed());
        let _ = websocket.close(None);
    }

    let mut requester = connect_client(address);
    let mut req_latencies = Vec::with_capacity(REQ_SAMPLES);
    for _ in 0..REQ_SAMPLES {
        let started = Instant::now();
        send_json(
            &mut requester,
            json!(["REQ", "benchmark", {"kinds": [1], "limit": HISTORY_LIMIT}]),
        );
        loop {
            let message = read_json(&mut requester);
            if message[0] == "EOSE" {
                break;
            }
            assert_eq!(message[0], "EVENT");
        }
        req_latencies.push(started.elapsed());
    }
    let _ = requester.close(None);

    RunMetrics {
        events_per_second,
        connect_p99_ms: milliseconds(percentile_99(&mut connect_latencies)),
        req_to_eose_p99_ms: milliseconds(percentile_99(&mut req_latencies)),
    }
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn range(samples: &[f64]) -> [f64; 2] {
    let minimum = samples.iter().copied().reduce(f64::min).unwrap();
    let maximum = samples.iter().copied().reduce(f64::max).unwrap();
    [rounded(minimum), rounded(maximum)]
}

fn percentile_99(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    let rank = (samples.len() * 99).div_ceil(100).saturating_sub(1);
    samples[rank]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn connect_client(address: SocketAddr) -> WebSocket<TcpStream> {
    let stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    client(format!("ws://{address}/"), stream).unwrap().0
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
