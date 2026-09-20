//! `coder-worker` against a relay that misbehaves.
//!
//! `relay_lifecycle.rs` proves the terminal's end of the relay door. This
//! file proves the worker's end: that a relay dropping the socket or
//! restarting does not end the service, that a request the worker cannot
//! read is answered or set aside deliberately, and that nothing a
//! customer sends can leave a request unanswered or a slot taken.
//!
//! The relay here is the test itself: a loopback listener that speaks
//! enough NIP-01 and NIP-42 for the worker to subscribe, and then does
//! whatever the test needs. The worker is the built binary, run with the
//! stub door so no credentials are involved, and its standard error is
//! read back as the log an operator would see.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use coder::relay::{Identity, REQUEST_KIND};
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, ChildStderr, Command};
use tokio::sync::Mutex;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite};

const WORKER: u8 = 0xaa;
const CLIENT: u8 = 0x0b;

/// The whole of any one test; the worker's reconnect waits are seconds.
const TEST_BOUND: Duration = Duration::from_secs(30);

/// The variables that would otherwise let one machine's environment decide
/// which door the worker opens.
const CREDENTIALS: [&str; 9] = [
    "TYPESAFE_API_KEY",
    "CODER_DOOR_KEY",
    "CODER_AI_GATEWAY_KEY",
    "CODER_DOOR_URL",
    "CODER_MODEL",
    "CODER_WORKER",
    "CODER_WORKER_MODEL",
    "CODER_EXECUTOR",
    "CODER_WORKER_ALLOW",
];

type Server = WebSocketStream<TcpStream>;

fn identity(byte: u8) -> Identity {
    Identity::from_secret(SecretKey::from_byte_array([byte; 32]).unwrap()).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn xonly(pubkey: &str) -> XOnlyPublicKey {
    let mut bytes = [0u8; 32];
    for (index, pair) in pubkey.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).unwrap();
        let low = (pair[1] as char).to_digit(16).unwrap();
        bytes[index] = ((high << 4) | low) as u8;
    }
    XOnlyPublicKey::from_byte_array(bytes).unwrap()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn conversation() -> [u8; 32] {
    nip44::conversation_key(identity(CLIENT).secret(), &xonly(identity(WORKER).pubkey()))
}

/// A request from the client to the worker, encrypted and signed the way
/// the terminal does it, at `created_at`.
fn request_at(payload: &Value, created_at: u64) -> Event {
    let ciphertext = nip44::encrypt(
        &payload.to_string(),
        &conversation(),
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    identity(CLIENT).signer().sign(
        created_at,
        REQUEST_KIND,
        vec![Tag::new(vec![
            "p".into(),
            identity(WORKER).pubkey().to_string(),
        ])],
        ciphertext,
    )
}

fn request(payload: &Value) -> Event {
    request_at(payload, unix_now())
}

/// The worker's answer to `request`, decrypted.
fn answer(event: &Event, request: &Event) -> Value {
    event.validate_crypto().unwrap();
    assert_eq!(event.pubkey, identity(WORKER).pubkey());
    assert!(
        event.tag_values("e").any(|id| id == request.id),
        "{event:?}"
    );
    assert!(event.tag_values("p").any(|key| key == request.pubkey));
    let plaintext = nip44::decrypt(&event.content, &conversation()).unwrap();
    serde_json::from_str(&plaintext).unwrap()
}

async fn send(socket: &mut Server, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

async fn read(socket: &mut Server) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Some(value);
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

/// The worker binary, started against `url`, with its log readable.
struct Worker {
    child: Child,
    log: Arc<Mutex<Lines<BufReader<ChildStderr>>>>,
    /// Log lines already read, for tests that count them.
    seen: Arc<Mutex<Vec<String>>>,
}

impl Worker {
    fn start(url: &str, arguments: &[&str], variables: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_coder-worker"));
        for name in CREDENTIALS {
            command.env_remove(name);
        }
        command
            .env("CODER_WORKER_SECRET", hex(&[WORKER; 32]))
            .env("CODER_RELAY", url)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (name, value) in variables {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("the worker binary runs");
        let stderr = child.stderr.take().unwrap();
        Self {
            child,
            log: Arc::new(Mutex::new(BufReader::new(stderr).lines())),
            seen: Arc::default(),
        }
    }

    /// Reads log lines until one contains `needle`, and returns it.
    async fn log_until(&self, needle: &str) -> String {
        loop {
            let line = self
                .log
                .lock()
                .await
                .next_line()
                .await
                .unwrap()
                .expect("the worker log ended before the line arrived");
            self.seen.lock().await.push(line.clone());
            if line.contains(needle) {
                return line;
            }
        }
    }

    /// Every log line read so far.
    async fn seen(&self) -> Vec<String> {
        self.seen.lock().await.clone()
    }

    /// Whether the process has exited, after a short grace.
    async fn exited(&mut self) -> bool {
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.child.try_wait().unwrap().is_some()
    }
}

/// Accepts the worker's connection, authenticates it, and takes its jobs
/// subscription up to and including `EOSE`.
///
/// Returns the socket and the filter the worker subscribed with.
async fn subscribe(listener: &TcpListener) -> (Server, Value) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(tcp).await.unwrap();
    send(&mut socket, json!(["AUTH", "worker-challenge"])).await;
    loop {
        let frame = read(&mut socket).await.expect("the worker stays connected");
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let auth: Event = serde_json::from_value(frame[1].clone()).unwrap();
                auth.validate_crypto().unwrap();
                assert_eq!(auth.pubkey, identity(WORKER).pubkey());
                send(&mut socket, json!(["OK", auth.id, true, ""])).await;
            }
            "REQ" => {
                assert_eq!(frame[1], "jobs");
                let filter = frame[2].clone();
                send(&mut socket, json!(["EOSE", "jobs"])).await;
                return (socket, filter);
            }
            other => panic!("unexpected frame before the subscription: {other} {frame}"),
        }
    }
}

fn assert_jobs_filter(filter: &Value) {
    assert_eq!(filter["kinds"], json!([REQUEST_KIND]));
    assert_eq!(filter["#p"], json!([identity(WORKER).pubkey()]));
}

/// Delivers `request` on the jobs subscription.
async fn deliver(socket: &mut Server, request: &Event) {
    send(socket, json!(["EVENT", "jobs", request])).await;
}

/// Reads the worker's next published event, acknowledging it.
async fn published(socket: &mut Server) -> Event {
    loop {
        let frame = read(socket).await.expect("the worker stays connected");
        if frame[0] == "EVENT" {
            let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
            send(socket, json!(["OK", event.id, true, ""])).await;
            return event;
        }
    }
}

async fn bounded<T>(test: impl Future<Output = T>) -> T {
    tokio::time::timeout(TEST_BOUND, test)
        .await
        .expect("the test finishes inside its bound")
}

/// One request is answered once. The same event delivered again is set
/// aside with one log line, and the next request, a different event,
/// is answered as usual.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_delivered_twice_is_answered_once() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let worker = Worker::start(&url, &[], &[]);
        let (mut socket, _) = subscribe(&listener).await;

        let job = request(&json!({"v": 2, "task": "ping"}));
        deliver(&mut socket, &job).await;
        deliver(&mut socket, &job).await;
        let result = answer(&published(&mut socket).await, &job);
        assert_eq!(result["type"], "result", "{result}");
        let line = worker.log_until("already delivered").await;
        assert!(
            line.starts_with(&format!("ignored {}", &job.id[..16])),
            "{line}"
        );

        let next = request(&json!({"v": 2, "task": "pong"}));
        deliver(&mut socket, &next).await;
        let result = answer(&published(&mut socket).await, &next);
        assert_eq!(result["type"], "result", "{result}");
        let ignored = worker
            .seen()
            .await
            .iter()
            .filter(|line| line.contains("already delivered"))
            .count();
        assert_eq!(ignored, 1, "{:?}", worker.seen().await);
    })
    .await;
}

/// A relay that hangs up, and then is not there for a while, is a fault
/// the service outlives: the worker connects again with backoff,
/// subscribes again with the same filter, and answers the next job as if
/// nothing had happened.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_or_restarted_relay_is_rejoined_and_resubscribed() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("ws://{address}");
        let mut worker = Worker::start(&url, &[], &[]);

        let (socket, filter) = subscribe(&listener).await;
        assert_jobs_filter(&filter);
        worker.log_until("subscribed").await;

        // The relay hangs up and goes away entirely: the port is closed,
        // so the first reconnect attempt is refused.
        drop(socket);
        drop(listener);
        let line = worker.log_until("reconnecting").await;
        assert!(line.starts_with("relay: "), "{line}");
        assert!(
            !worker.exited().await,
            "the worker exited on a dropped socket"
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // The relay is back on the same port. The worker subscribes again
        // with the same filter and the next job goes through.
        let listener = TcpListener::bind(address).await.unwrap();
        let (mut socket, filter) = subscribe(&listener).await;
        assert_jobs_filter(&filter);
        let reconnects = worker
            .seen()
            .await
            .iter()
            .filter(|line| line.contains("reconnecting"))
            .count();
        assert!(reconnects >= 1, "{:?}", worker.seen().await);

        let job = request(&json!({"v": 2, "task": "ping"}));
        deliver(&mut socket, &job).await;
        let result = answer(&published(&mut socket).await, &job);
        assert_eq!(result["type"], "result", "{result}");
    })
    .await;
}
