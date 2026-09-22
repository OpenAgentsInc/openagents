//! The program, extension, and job lifecycle across real processes.
//!
//! `interoperability.rs` composes the contracts in one process. This file
//! runs the same lifecycle the way it actually happens: the relay is the
//! built `nostr-relay` binary over a disposable Postgres, the worker is
//! the built `coder-worker`, and the customers are a `coder -p` child, an
//! in-process turn through `RelayDoor`, and scripted sockets. Every hop
//! between them is a real one.
//!
//! Run it through `scripts/test-postgres.sh`, which provisions the
//! disposable database and sets the destructive guard. With neither set —
//! or without a `nostr-relay` binary beside the test's own — each test
//! reports itself skipped rather than proving nothing quietly.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::capability::Trust;
use coder::generate::Door;
use coder::program::Program;
use coder::program_authority::Grant;
use coder::questions;
use coder::relay::{FEEDBACK_KIND, Identity, REQUEST_KIND, RESULT_KIND, RelayDoor};
use coder::runstate::{Claim, Mark, Refusal, State, Store};
use coder::runtime::{Host, Inputs, Runtime};
use coder::survey::Survey;
use coder::trace::Recorder;
use coder::turn;
use coder::{Agent, Task};
use nostr::cap;
use nostr::contracts::{ArtifactRef, RefusalCode, digest_bytes};
use nostr::domain::{Event, EventClass, Tag};
use nostr::ext::{self, Install, InstallStep};
use nostr::nip44;
use nostr::prg;
use nostr::run;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

/// Fixed test keys. The relay, the publisher, a discovering reader, the
/// worker, the job customer, and a stranger each hold one.
const RELAY: u8 = 0x09;
const PUBLISHER: u8 = 0x31;
const READER: u8 = 0x32;
const WORKER: u8 = 0x33;
const CUSTOMER: u8 = 0x34;
const STRANGER: u8 = 0x35;

/// The environment that would otherwise let one machine's credentials
/// decide which door a spawned process opens.
const CREDENTIALS: [&str; 12] = [
    "TYPESAFE_API_KEY",
    "CODER_DOOR_KEY",
    "CODER_AI_GATEWAY_KEY",
    "CODER_DOOR_URL",
    "CODER_MODEL",
    "CODER_WORKER",
    "CODER_WORKER_MODEL",
    "CODER_EXECUTOR",
    "CODER_WORKER_ALLOW",
    "CODER_PROGRAMS",
    "CODER_PROGRAM_EFFECTS",
    "CODER_NSEC",
];

const READ_TIMEOUT: Duration = Duration::from_secs(15);
const LOG_WAIT: Duration = Duration::from_secs(30);

fn database() -> Option<String> {
    let url = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL").ok()?;
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        return None;
    }
    Some(url)
}

/// The directory the built binaries land in, holding `nostr-relay`,
/// `coder-worker`, and `coder` once `scripts/test-postgres.sh` builds them.
fn binary_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_coder-worker"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn relay_binary() -> Option<PathBuf> {
    let binary = binary_dir().join("nostr-relay");
    binary.exists().then_some(binary)
}

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

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The relay binary as a child process, configured the way the issue's
/// acceptance asks: author authentication required, the OpenAgents
/// discovery profiles advertised, and a NIP-42 challenge on connect.
struct Relay {
    child: Option<Child>,
    address: SocketAddr,
    url: String,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Relay {
    fn spawn(database_url: &str) -> Self {
        let binary = relay_binary().expect("the nostr-relay binary is built");
        // The relay checks an AUTH event's `relay` tag against
        // `NOSTR_RELAY_URL` exactly, so the bound port has to be known
        // before spawn: reserve one, let it go, and name it in both.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let url = format!("ws://127.0.0.1:{port}");
        let mut child = Command::new(binary)
            .env("DATABASE_URL", database_url)
            .env("NOSTR_RELAY_BIND_ADDR", "127.0.0.1")
            .env("NOSTR_RELAY_PORT", port.to_string())
            .env("NOSTR_RELAY_DB_CONNECTIONS", "4")
            .env("NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP", "10000")
            .env("NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY", "10000")
            .env("NOSTR_RELAY_RATE_REQ_PER_MIN_IP", "10000")
            .env("NOSTR_RELAY_MAX_CONNECTIONS_PER_IP", "100")
            .env("NOSTR_RELAY_URL", &url)
            .env("NOSTR_RELAY_SECRET_KEY", hex(&[RELAY; 32]))
            .env("NOSTR_RELAY_AUTH_REQUIRED", "true")
            .env("NOSTR_RELAY_OPENAGENTS_PROFILES", "true")
            .env("NOSTR_RELAY_LOG_LEVEL", "debug")
            .env_remove("PORT")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the relay binary runs");
        let stdout = child.stdout.take().unwrap();
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let writer = Arc::clone(&lines);
        let (first_send, first) = mpsc::channel::<String>();
        thread::spawn(move || {
            let mut first_send = Some(first_send);
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if let Some(send) = first_send.take() {
                            let _ = send.send(line.clone());
                        }
                        writer.lock().unwrap().push(line);
                    }
                    Err(_) => break,
                }
            }
        });
        let line = first
            .recv_timeout(LOG_WAIT)
            .expect("the relay reports its address");
        let startup: Value = serde_json::from_str(&line).unwrap();
        let address: SocketAddr = startup["address"].as_str().unwrap().parse().unwrap();
        Self {
            child: Some(child),
            address,
            url,
            lines,
        }
    }

    fn seen(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    fn kill(&mut self) {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

type Socket = WebSocket<TcpStream>;

/// Opens a socket and answers the relay's NIP-42 challenge as `identity`.
fn connect(relay: &Relay, identity: &Identity) -> Socket {
    let stream = TcpStream::connect(relay.address).unwrap();
    stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(READ_TIMEOUT)).unwrap();
    let (mut socket, _) = client(format!("{}/", relay.url), stream).unwrap();
    let challenge = read(&mut socket);
    assert_eq!(challenge[0], "AUTH", "{challenge}");
    let auth = identity.signer().sign(
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), relay.url.clone()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap().to_owned(),
            ]),
        ],
        String::new(),
    );
    send(&mut socket, json!(["AUTH", auth]));
    let accepted = read(&mut socket);
    assert_eq!(accepted[0], "OK");
    assert_eq!(accepted[2], true, "{accepted}");
    socket
}

fn send(socket: &mut Socket, value: Value) {
    socket.send(Message::text(value.to_string())).unwrap();
}

fn read(socket: &mut Socket) -> Value {
    loop {
        match socket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected relay frame: {other:?}"),
        }
    }
}

/// Publishes `event` and returns the relay's `OK`. Frames that answer an
/// open subscription — its `EOSE`, a live `EVENT` — are not the verdict.
fn publish(socket: &mut Socket, event: &Event) -> Value {
    send(socket, json!(["EVENT", event]));
    loop {
        let frame = read(socket);
        if frame[0] == "OK" && frame[1] == event.id {
            return frame;
        }
        assert!(
            frame[0] != "CLOSED" && frame[0] != "NOTICE",
            "relay closed or noticed during publish: {frame}"
        );
    }
}

/// Runs `filter` to `EOSE` on a fresh subscription and returns what the
/// relay stored for it, with the completeness hint the EOSE carried.
fn query(socket: &mut Socket, subscription: &str, filter: Value) -> (Vec<Event>, Value) {
    send(socket, json!(["REQ", subscription, filter]));
    let mut events = Vec::new();
    loop {
        let frame = read(socket);
        match frame[0].as_str().unwrap_or_default() {
            "EVENT" => {
                assert_eq!(frame[1], subscription);
                events.push(serde_json::from_value(frame[2].clone()).unwrap());
            }
            "EOSE" => {
                send(socket, json!(["CLOSE", subscription]));
                return (events, frame);
            }
            other => panic!("unexpected frame during a query: {other}"),
        }
    }
}

fn only(query_result: (Vec<Event>, Value)) -> Event {
    let (events, _) = query_result;
    assert_eq!(events.len(), 1, "{events:?}");
    events.into_iter().next().unwrap()
}

/// The worker binary with its stderr collected for assertions.
struct Worker {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Worker {
    fn start(url: &str, variables: &[(&str, String)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_coder-worker"));
        for name in CREDENTIALS {
            command.env_remove(name);
        }
        command
            .env("CODER_WORKER_SECRET", hex(&[WORKER; 32]))
            .env("CODER_RELAY", url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (name, value) in variables {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("the worker binary runs");
        let stderr = child.stderr.take().unwrap();
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let writer = Arc::clone(&lines);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => writer.lock().unwrap().push(line),
                    Err(_) => break,
                }
            }
        });
        Self { child, lines }
    }

    /// Reads the collected log until a line contains `needle`.
    fn log_until(&self, needle: &str) -> String {
        let deadline = Instant::now() + LOG_WAIT;
        loop {
            if let Some(line) = self
                .lines
                .lock()
                .unwrap()
                .iter()
                .find(|line| line.contains(needle))
                .cloned()
            {
                return line;
            }
            assert!(Instant::now() < deadline, "no {needle:?} in the worker log");
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn seen(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    fn kill(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A NIP-CJ request the way the terminal writes one: NIP-44 to the worker,
/// `p`-tagged so the relay routes it, on the job-request kind.
fn job_request(payload: &Value) -> Event {
    let conversation = nip44::conversation_key(
        identity(CUSTOMER).secret(),
        &xonly(identity(WORKER).pubkey()),
    );
    let ciphertext = nip44::encrypt(
        &payload.to_string(),
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    identity(CUSTOMER).signer().sign(
        now(),
        REQUEST_KIND,
        vec![Tag::new(vec![
            "p".into(),
            identity(WORKER).pubkey().to_string(),
        ])],
        ciphertext,
    )
}

/// The decrypted body of a worker answer bound to `request`, read by the
/// request's author — the only key the worker encrypted it to.
fn job_answer(event: &Event, request: &Event, customer: &Identity) -> Value {
    event.validate_crypto().unwrap();
    assert_eq!(event.pubkey, identity(WORKER).pubkey());
    assert!(
        event.tag_values("e").any(|id| id == request.id),
        "{event:?}"
    );
    assert!(event.tag_values("p").any(|key| key == request.pubkey));
    let conversation =
        nip44::conversation_key(customer.secret(), &xonly(identity(WORKER).pubkey()));
    match nip44::decrypt(&event.content, &conversation) {
        Ok(plaintext) => serde_json::from_str(&plaintext).unwrap(),
        Err(why) => panic!("cannot decrypt answer {event:?}: {why}"),
    }
}

/// Reads the subscription until the result for `request` arrives.
fn await_result(socket: &mut Socket, request: &Event, customer: &Identity) -> Value {
    let deadline = Instant::now() + LOG_WAIT;
    loop {
        assert!(Instant::now() < deadline, "no result for {}", request.id);
        let frame = read(socket);
        assert!(
            frame[0] != "CLOSED" && frame[0] != "NOTICE",
            "relay closed or noticed the answers subscription: {frame}"
        );
        if frame[0] != "EVENT" {
            continue;
        }
        let event: Event = serde_json::from_value(frame[2].clone()).unwrap();
        if matches!(event.kind, RESULT_KIND | FEEDBACK_KIND)
            && event.tag_values("e").any(|id| id == request.id)
        {
            return job_answer(&event, request, customer);
        }
    }
}

fn schema_ref() -> Value {
    json!({
        "digest": "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0",
        "size": 17,
        "media_type": "application/schema+json"
    })
}

/// The program definition the publisher signs into kind 30182: a native
/// `query` step feeding a WebAssembly `module` step.
fn program_definition(publisher: &str) -> Value {
    json!({
        "v": 1,
        "requires": [],
        "id": format!("{publisher}:openagents/interop-demo"),
        "summary": "a native lookup feeding a wasm guest",
        "input": schema_ref(),
        "output": schema_ref(),
        "steps": [
            {
                "name": "native",
                "kind": "query",
                "after": [],
                "input": {"from": "input", "pointer": ""},
                "output": schema_ref(),
                "bounds": {},
                "on_error": "stop"
            },
            {
                "name": "guest",
                "kind": "module",
                "after": ["native"],
                "input": {"from": "step:native", "pointer": ""},
                "output": schema_ref(),
                "bounds": {},
                "on_error": "stop"
            }
        ],
        "result": {"from": "step:guest", "pointer": "/value"},
        "bounds": {}
    })
}

/// The package manifest the release pins, listing the program definition
/// and the guest bytes by digest.
fn package_manifest(
    publisher: &str,
    definition_digest: &str,
    definition_size: u64,
    wasm_digest: &str,
    wasm_size: u64,
) -> Value {
    json!({
        "v": "openagents.package.v1",
        "requires": [],
        "package": format!("{publisher}:interop-pkg"),
        "version": "1.0.0",
        "license": "MIT",
        "provenance": {"source": "local", "receipts": [], "unknowns": []},
        "components": [
            {
                "slug": "interop-demo",
                "kind": "program",
                "definition": {"digest": definition_digest, "size": definition_size, "media_type": "application/json"},
                "descriptor": {"digest": definition_digest, "size": definition_size, "media_type": "application/json"}
            },
            {
                "slug": "pure",
                "kind": "plugin",
                "definition": {"digest": wasm_digest, "size": wasm_size, "media_type": "application/wasm"},
                "descriptor": {"digest": wasm_digest, "size": wasm_size, "media_type": "application/wasm"}
            },
            {
                "slug": "notes",
                "kind": "skill",
                "definition": {"digest": definition_digest, "size": definition_size, "media_type": "application/json"},
                "descriptor": {"digest": definition_digest, "size": definition_size, "media_type": "application/json"}
            },
            {
                "slug": "echo",
                "kind": "operation",
                "definition": {"digest": wasm_digest, "size": wasm_size, "media_type": "application/wasm"},
                "descriptor": {"digest": wasm_digest, "size": wasm_size, "media_type": "application/wasm"}
            }
        ],
        "files": [
            {"path": "defs/interop-demo.json", "digest": definition_digest, "size": definition_size, "media_type": "application/json"},
            {"path": "guest/pure.wasm", "digest": wasm_digest, "size": wasm_size, "media_type": "application/wasm"}
        ],
        "dependencies": []
    })
}

fn event_ref(event: &Event) -> Value {
    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
}

fn artifact_of(bytes: &[u8], media_type: &str) -> Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len() as u64,
        "media_type": media_type
    })
}

fn locator(identity: &Identity, artifact: &ArtifactRef, url: &str) -> Event {
    let digest = artifact.digest.strip_prefix("sha256:").unwrap();
    identity.signer().sign(
        now(),
        ext::LOCATOR_KIND,
        vec![
            Tag::new(vec!["url".into(), url.to_string()]),
            Tag::new(vec!["x".into(), digest.to_string()]),
            Tag::new(vec!["m".into(), artifact.media_type.clone()]),
            Tag::new(vec!["size".into(), artifact.size.to_string()]),
        ],
        "the package bytes this digest pins".to_string(),
    )
}

/// A capability manifest body, the shape `capabilities/` carries.
fn capability_definition(publisher: &str) -> Value {
    json!({
        "v": 1,
        "requires": [],
        "id": format!("{publisher}:openagents/interop-cap"),
        "profile": "native",
        "summary": "the executor the package's delegate steps would name",
        "input": schema_ref(),
        "output": schema_ref(),
        "effects": {"reads": ["workspace"], "writes": [], "network": [], "process": false, "delegates": false, "spend": false},
        "minimum": {},
        "support": {"bounds": {"wall_ms": "unknown"}, "cancellation": "unsupported", "idempotency": "none", "evidence": []},
        "binding_contract": {"operation": "interop", "interface": "host.v1"}
    })
}

/// Publish a package, discover it by its tags, verify and stage its lock,
/// and run its program — every hop over the wire between two authenticated
/// clients of a real relay process.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_package_publishes_discovers_installs_and_runs_over_the_wire() {
    let Some(database_url) = database() else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if relay_binary().is_none() {
        eprintln!("skipped: the nostr-relay binary is not built");
        return;
    }
    let mut relay = Relay::spawn(&database_url);
    let publisher = identity(PUBLISHER);
    let reader = identity(READER);
    let mut publish_socket = connect(&relay, &publisher);
    let mut read_socket = connect(&relay, &reader);

    // Publish: the capability, the program, the release, the manifest and
    // wasm locators — each its own event, each admitted by the relay's
    // OpenAgents profile admission.
    let definition = program_definition(publisher.pubkey());
    let definition_bytes = definition.to_string().into_bytes();
    let program_event = publisher.signer().sign(
        now(),
        prg::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "interop-demo".into()]),
            Tag::new(vec!["t".into(), prg::PROGRAM_MARKER.into()]),
            Tag::new(vec!["t".into(), "oa:step:query".into()]),
            Tag::new(vec!["t".into(), "oa:step:module".into()]),
        ],
        definition.to_string(),
    );
    let capability_event = publisher.signer().sign(
        now(),
        cap::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "interop-cap".into()]),
            Tag::new(vec!["t".into(), cap::CAP_MARKER.into()]),
        ],
        capability_definition(publisher.pubkey()).to_string(),
    );
    let wasm = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugin/fixtures/pure.wasm"
    ))
    .unwrap();
    let manifest = package_manifest(
        publisher.pubkey(),
        &digest_bytes(&definition_bytes),
        definition_bytes.len() as u64,
        &digest_bytes(&wasm),
        wasm.len() as u64,
    );
    let manifest_bytes = manifest.to_string().into_bytes();
    let release = publisher.signer().sign(
        now(),
        ext::RELEASE_KIND,
        vec![Tag::new(vec!["t".into(), "oa:ext:release:v1".into()])],
        json!({
            "v": 1,
            "requires": [],
            "type": "release",
            "package": format!("{}:interop-pkg", publisher.pubkey()),
            "version": "1.0.0",
            "manifest": artifact_of(&manifest_bytes, "application/json")
        })
        .to_string(),
    );
    let listing = publisher.signer().sign(
        now(),
        ext::LISTING_KIND,
        vec![
            Tag::new(vec!["d".into(), "interop-pkg".into()]),
            Tag::new(vec!["t".into(), "oa:ext:listing:v1".into()]),
        ],
        json!({
            "v": 1,
            "requires": [],
            "type": "listing",
            "package": format!("{}:interop-pkg", publisher.pubkey()),
            "state": "published",
            "release": event_ref(&release),
            "title": "interop demo package",
            "description": "the package the wire proof installs"
        })
        .to_string(),
    );
    for event in [&capability_event, &program_event, &release, &listing] {
        let verdict = publish(&mut publish_socket, event);
        assert_eq!(verdict[2], true, "kind {} refused: {verdict}", event.kind);
    }
    let manifest_locator = locator(
        &publisher,
        &ArtifactRef {
            digest: digest_bytes(&manifest_bytes),
            size: manifest_bytes.len() as u64,
            media_type: "application/json".to_string(),
            schema: None,
            event: None,
            sources: Vec::new(),
        },
        &format!(
            "https://packages.interop.local/manifest/{}",
            digest_bytes(&manifest_bytes)
        ),
    );
    let wasm_locator = locator(
        &publisher,
        &ArtifactRef {
            digest: digest_bytes(&wasm),
            size: wasm.len() as u64,
            media_type: "application/wasm".to_string(),
            schema: None,
            event: None,
            sources: Vec::new(),
        },
        &format!(
            "https://packages.interop.local/guest/{}",
            digest_bytes(&wasm)
        ),
    );
    for event in [&manifest_locator, &wasm_locator] {
        let verdict = publish(&mut publish_socket, event);
        assert_eq!(verdict[2], true, "locator refused: {verdict}");
    }

    // The relay refuses the malformed and the unsupported at admission:
    // a profile body that is not the schema, and a version nobody speaks.
    let mut malformed = definition.clone();
    malformed["steps"][0]["kind"] = json!("teleport");
    let malformed_event = publisher.signer().sign(
        now(),
        prg::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "interop-bad".into()]),
            Tag::new(vec!["t".into(), prg::PROGRAM_MARKER.into()]),
        ],
        malformed.to_string(),
    );
    let verdict = publish(&mut publish_socket, &malformed_event);
    assert_eq!(verdict[2], false);
    assert!(
        verdict[3].as_str().unwrap().starts_with("invalid:"),
        "{verdict}"
    );
    let mut unsupported = definition.clone();
    unsupported["v"] = json!(99);
    let unsupported_event = publisher.signer().sign(
        now(),
        prg::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "interop-old".into()]),
            Tag::new(vec!["t".into(), prg::PROGRAM_MARKER.into()]),
        ],
        unsupported.to_string(),
    );
    let verdict = publish(&mut publish_socket, &unsupported_event);
    assert_eq!(verdict[2], false);

    // Discover: the reader finds the program by its step tag, the
    // capability by its marker, the release through the listing's
    // EventRef, and each byte by its locator digest.
    let (found, eose) = query(
        &mut read_socket,
        "programs",
        json!({"kinds": [prg::DISCOVERY_KIND], "#t": ["oa:step:module"]}),
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].id, program_event.id);
    if eose.as_array().unwrap().len() == 3 {
        assert_eq!(eose[2], json!(["finish"]), "{eose}");
    }
    let definition =
        prg::parse_definition(&serde_json::from_str::<Value>(&found[0].content).unwrap()).unwrap();
    prg::check_discovery_tags(&found[0].tags, &definition).unwrap();
    let discovered_listing = only(query(
        &mut read_socket,
        "listings",
        json!({"kinds": [ext::LISTING_KIND], "#d": ["interop-pkg"]}),
    ));
    let listed: Value = serde_json::from_str(&discovered_listing.content).unwrap();
    assert_eq!(listed["release"]["id"], release.id);
    let discovered_release = only(query(
        &mut read_socket,
        "releases",
        json!({"kinds": [ext::RELEASE_KIND], "ids": [release.id]}),
    ));
    ext::parse_record(&discovered_release).unwrap();
    let discovered_capability = only(query(
        &mut read_socket,
        "capabilities",
        json!({"kinds": [cap::DISCOVERY_KIND], "#d": ["interop-cap"]}),
    ));
    cap::parse_definition(&serde_json::from_str::<Value>(&discovered_capability.content).unwrap())
        .unwrap();

    // Verify: the manifest the release pins parses, every locator matches
    // the artifact it names, the byte closure checks, and the install
    // transitions stage the exact lock.
    let parsed_manifest = ext::parse_manifest(&manifest).unwrap();
    let manifest_artifact = ArtifactRef {
        digest: digest_bytes(&manifest_bytes),
        size: manifest_bytes.len() as u64,
        media_type: "application/json".to_string(),
        schema: None,
        event: None,
        sources: Vec::new(),
    };
    let wasm_artifact = ArtifactRef {
        digest: digest_bytes(&wasm),
        size: wasm.len() as u64,
        media_type: "application/wasm".to_string(),
        schema: None,
        event: None,
        sources: Vec::new(),
    };
    ext::locator_matches(&manifest_locator, &manifest_artifact).unwrap();
    ext::locator_matches(&wasm_locator, &wasm_artifact).unwrap();
    let mut files = BTreeMap::new();
    files.insert(digest_bytes(&definition_bytes), definition_bytes.clone());
    files.insert(digest_bytes(&manifest_bytes), manifest_bytes.clone());
    files.insert(digest_bytes(&wasm), wasm.clone());
    ext::verify_closure(&ext::Closure {
        manifest: &parsed_manifest,
        files: &files,
        staged: &[
            "defs/interop-demo.json".to_string(),
            "guest/pure.wasm".to_string(),
        ],
        dependencies: &BTreeMap::from([(release.id.clone(), parsed_manifest.clone())]),
        root: &release.id,
        byte_limit: 1 << 20,
    })
    .unwrap();
    let lock = digest_bytes(release.id.as_bytes());
    let installed = ext::transition(
        &Install::Absent,
        InstallStep::Stage {
            lock: lock.clone(),
            verified: true,
        },
    )
    .unwrap();
    assert!(matches!(installed, Install::Staged { .. }));
    let mut store = BTreeMap::new();
    store.insert(lock.clone(), manifest_bytes.clone());
    let head = format!("sha256:{}", "9".repeat(64));
    assert_eq!(
        ext::preserve_active_pin(&lock, &head),
        lock.as_str(),
        "an active run keeps its pin when a newer head exists"
    );
    let edges = BTreeMap::from([(lock.clone(), Vec::new())]);
    assert_eq!(
        prg::pin_closure(&lock, &edges, &store).unwrap()[&lock],
        manifest_bytes
    );

    // Run: the wire-fetched definition binds against this host and the
    // runtime executes both steps — the `query` reads the request source,
    // the `module` runs the located wasm — into a recorded run.
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("questions")).unwrap();
    let bound = json!({
        "definition": serde_json::from_str::<Value>(&found[0].content).unwrap(),
        "binding": {
            "name": "interop demo",
            "inputs": {},
            "outputs": {},
            "steps": {
                "native": {"source": "request", "bounds": {"max_results": 4}},
                "guest": {
                    "module": {
                        "bytes_base64": plugin::encode_base64(&wasm),
                        "profile": "pure",
                        "operation": "echo",
                        "input": {"topic": "interop"}
                    },
                    "bounds": {"fuel": 50_000_000_u64}
                }
            }
        }
    });
    let program = Program::parse(bound.to_string().as_bytes()).unwrap();
    let logs = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(logs.path(), "stub", "stub", "interop").unwrap();
    let runtime = Runtime::over(
        Survey::read_with(Some(root.path()), root.path(), &Trust::everything()),
        questions::Registry::open(&[root.path().join("questions")]),
        Host::with_repository(),
    );
    let inputs = Inputs {
        request: "read the notes".to_string(),
        tasks: vec![Task::reading("note alpha", "a.rs").expecting("alpha")],
        executor: "none".to_string(),
    };
    let finished = runtime
        .run(&program, &inputs, &Grant::all(), Some(&mut recorder))
        .await;
    assert_eq!(finished.program.as_deref(), Some("interop-demo"));
    assert!(finished.stopped.is_none(), "{:?}", finished.stopped);
    assert_eq!(finished.steps.len(), 2, "{:?}", finished.steps);
    drop(recorder);
    let trace = std::fs::read_dir(logs.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let recorded = atif::log::read(&trace).unwrap();
    let calls = recorded
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .collect::<Vec<_>>();
    assert!(
        calls.iter().any(|call| call.name == "program_authority"),
        "{calls:?}"
    );
    // The module step ran as a step, not a recorded call: the run's step
    // list carries its name and its guest output.
    let guest = finished
        .steps
        .iter()
        .find(|step| step.name == "guest")
        .expect("the module step ran");
    let guest_out: Value = serde_json::from_str(&guest.output).unwrap();
    assert_eq!(guest_out["status"], "ok", "{guest_out}");

    // Authority bounds: no package text grants anything, an unauthorized
    // phase refuses, and an archive the contract does not unpack refuses.
    assert!(ext::grants_after_migration().is_empty());
    assert_eq!(
        ext::authorize(&ext::Authorization {
            surface: ext::Surface::Human,
            phase: ext::Phase::Install,
            side: ext::Side::Script,
            authorized: false,
        })
        .unwrap_err()
        .code,
        RefusalCode::NotAdmitted
    );
    assert_eq!(
        ext::expand_archive("application/zip").unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );

    // Revocation over the wire: the package author publishes a kind-3185
    // revocation naming the release, the reader discovers it through the
    // `e` tag, and the knowledge fold admits it. Offline, a missing
    // checkpoint refuses strict admission — unless the pin was explicit,
    // which is the offline handling the issue asks after.
    let revocation = publisher.signer().sign(
        now(),
        ext::REVOCATION_KIND,
        vec![
            Tag::new(vec!["e".into(), release.id.clone()]),
            Tag::new(vec!["t".into(), "oa:ext:revocation:v1".into()]),
        ],
        json!({
            "v": 1,
            "requires": [],
            "type": "revocation",
            "package": format!("{}:interop-pkg", publisher.pubkey()),
            "release": {
                "id": release.id,
                "pubkey": publisher.pubkey(),
                "kind": ext::RELEASE_KIND,
            },
            "reason": "interop demo revocation",
            "effective_at": now(),
        })
        .to_string(),
    );
    assert_eq!(publish(&mut publish_socket, &revocation)[2], true);
    let discovered_revocation = only(query(
        &mut read_socket,
        "revocations",
        json!({"kinds": [ext::REVOCATION_KIND], "#e": [release.id]}),
    ));
    assert_eq!(discovered_revocation.id, revocation.id);
    ext::parse_record(&discovered_revocation).unwrap();
    let knowledge = ext::absorb(
        &ext::RevocationKnowledge {
            revision: 0,
            revocations: BTreeSet::new(),
        },
        1,
        std::slice::from_ref(&revocation.id),
    )
    .unwrap();
    assert!(knowledge.revocations.contains(&revocation.id));
    let mut offline = ext::Freshness {
        as_of: now(),
        valid_until: now() + 60,
        now: now(),
        skew: 60,
        present: false,
        empty_answer: false,
        strict: true,
        explicit_pin: false,
    };
    assert_eq!(
        ext::fresh(&offline).unwrap_err().code,
        RefusalCode::Stale,
        "strict admission refuses when no checkpoint answers offline"
    );
    offline.explicit_pin = true;
    ext::fresh(&offline).unwrap();
    assert_eq!(
        ext::preserve_active_pin(&lock, &format!("sha256:{}", "8".repeat(64))),
        lock.as_str(),
        "a revoked head does not move an active run's pin"
    );

    relay.kill();
}

/// A remote job, the traffic loss the issue names, and the restart: the
/// worker and relay are killed between the request and its answer, the
/// controller's own journal is what recovery reads, and nothing replays
/// the effect that was never known to finish.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_job_recovers_by_journal_and_never_replays_unknown() {
    let Some(database_url) = database() else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if relay_binary().is_none() {
        eprintln!("skipped: the nostr-relay binary is not built");
        return;
    }
    let mut relay = Relay::spawn(&database_url);
    let customer = identity(CUSTOMER);
    let stranger = identity(STRANGER);
    let mut worker = Worker::start(
        &relay.url,
        &[("CODER_WORKER_ALLOW", customer.pubkey().to_string())],
    );
    worker.log_until("waiting for jobs");

    // A working job over the live path: the customer subscribes before it
    // publishes, the worker answers, the result decrypts to the request.
    let mut customer_socket = connect(&relay, &customer);
    send(
        &mut customer_socket,
        json!(["REQ", "answers", {"kinds": [RESULT_KIND, FEEDBACK_KIND], "#p": [customer.pubkey()]}]),
    );
    let baseline = job_request(&json!({"v": 2, "task": "ping"}));
    assert_eq!(publish(&mut customer_socket, &baseline)[2], true);
    let answer = await_result(&mut customer_socket, &baseline, &customer);
    assert_eq!(answer["type"], "result", "{answer}");
    // The relay's own log is the third witness: it admitted the request
    // going out and the result coming back. The log collector runs beside
    // the socket, so give it a moment to land.
    let deadline = Instant::now() + LOG_WAIT;
    loop {
        let relay_log = relay.seen().join("\n");
        if relay_log.contains("\"kind\":25900") && relay_log.contains("\"kind\":26900") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "relay never admitted the pair: {relay_log}"
        );
        thread::sleep(Duration::from_millis(25));
    }

    // A customer the worker does not admit gets a typed refusal, over the
    // same wire, never silence.
    let mut stranger_socket = connect(&relay, &stranger);
    send(
        &mut stranger_socket,
        json!(["REQ", "stranger-answers", {"kinds": [RESULT_KIND, FEEDBACK_KIND], "#p": [stranger.pubkey()]}]),
    );
    let conversation =
        nip44::conversation_key(stranger.secret(), &xonly(identity(WORKER).pubkey()));
    let denied = stranger.signer().sign(
        now(),
        REQUEST_KIND,
        vec![Tag::new(vec![
            "p".into(),
            identity(WORKER).pubkey().to_string(),
        ])],
        nip44::encrypt(
            &json!({"v": 2, "task": "ping"}).to_string(),
            &conversation,
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .unwrap(),
    );
    assert_eq!(publish(&mut stranger_socket, &denied)[2], true);
    let refusal = await_result(&mut stranger_socket, &denied, &stranger);
    assert_eq!(refusal["code"], "not_admitted", "{refusal}");

    // Malformed and unversioned traffic get typed answers the same way.
    let mut unreadable = job_request(&json!({"v": 2, "task": "ping"}));
    unreadable.content = "not ciphertext".to_string();
    let unreadable = customer.signer().sign(
        unreadable.created_at,
        unreadable.kind,
        unreadable.tags.clone(),
        unreadable.content,
    );
    assert_eq!(publish(&mut customer_socket, &unreadable)[2], true);
    let refusal = await_result(&mut customer_socket, &unreadable, &customer);
    assert_eq!(refusal["code"], "malformed", "{refusal}");
    let unversioned = job_request(&json!({"v": 9, "task": "ping"}));
    assert_eq!(publish(&mut customer_socket, &unversioned)[2], true);
    let refusal = await_result(&mut customer_socket, &unversioned, &customer);
    assert_eq!(refusal["code"], "unsupported_version", "{refusal}");

    // The controller's journal claims the run and marks the dispatch; the
    // run record goes on the wire as a private durable event.
    let journal = tempfile::tempdir().unwrap();
    {
        let mut store = Store::open(journal.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-lost",
                base: "base",
                program: "interop-demo",
                questions: &[],
                sources: &[],
                owner: 0,
            })
            .unwrap();
        store
            .advance("run-lost", Mark::run(State::Dispatched))
            .unwrap();
    }
    // The database is shared across runs, so this run's record carries a
    // unique `h` and every read scopes to it rather than a global count.
    let head = hex(&secp256k1::rand::random::<[u8; 32]>());
    let record = customer.signer().sign(
        now(),
        run::RECORD_KIND,
        vec![
            Tag::new(vec!["p".into(), identity(WORKER).pubkey().to_string()]),
            Tag::new(vec!["h".into(), head.clone()]),
            Tag::new(vec!["t".into(), run::MARKER.into()]),
        ],
        "Y2lwaGVydGV4dA==".to_string(),
    );
    assert_eq!(publish(&mut customer_socket, &record)[2], true);

    // The loss the issue names: the request goes out and both processes
    // die before its acknowledgment or result traffic can arrive.
    let lost = job_request(&json!({"v": 2, "task": "the lost job"}));
    send(&mut customer_socket, json!(["EVENT", &lost]));
    relay.kill();
    worker.kill();

    // Controller restart: the store reopens, recovery marks the dispatched
    // run unknown, and the contracts say an unknown effect is neither an
    // answer nor a thing to retry.
    let mut store = Store::open(journal.path()).unwrap();
    let recovered = store.recover().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].state, State::Unknown);
    assert!(!run::accepted("unknown", "not_run", "pending"));
    assert!(!run::replay_offline(&[]).unwrap().executed);
    assert_eq!(
        prg::allow_retry(true).unwrap_err().code,
        RefusalCode::CannotEnforce
    );
    let durability = run::crash(run::Boundary::Effect, "reservation-lost");
    assert_eq!(durability.outcome.as_deref(), Some("unknown"));

    // Both processes come back on the same database. The durable run
    // record is still queryable — that is the status/journal recovery —
    // and no result for the lost request was ever stored or delivered.
    let mut relay = Relay::spawn(&database_url);
    let mut worker = Worker::start(
        &relay.url,
        &[("CODER_WORKER_ALLOW", customer.pubkey().to_string())],
    );
    worker.log_until("waiting for jobs");
    let mut recovered_socket = connect(&relay, &customer);
    let journals = query(
        &mut recovered_socket,
        "journal",
        json!({"kinds": [run::RECORD_KIND], "#h": [head]}),
    );
    assert_eq!(journals.0.len(), 1);
    assert_eq!(journals.0[0].id, record.id);
    let nothing = query(
        &mut recovered_socket,
        "lost-answers",
        json!({"kinds": [RESULT_KIND, FEEDBACK_KIND], "#e": [lost.id]}),
    );
    assert!(nothing.0.is_empty(), "an ephemeral answer is never stored");

    // A fresh job after the restart is a new request with a new id; the
    // worker's log never shows the lost id twice, and the store refuses a
    // second owner for the recovered run.
    send(
        &mut recovered_socket,
        json!(["REQ", "answers-2", {"kinds": [RESULT_KIND, FEEDBACK_KIND], "#p": [customer.pubkey()]}]),
    );
    let next = job_request(&json!({"v": 2, "task": "after the restart"}));
    assert_eq!(publish(&mut recovered_socket, &next)[2], true);
    let answer = await_result(&mut recovered_socket, &next, &customer);
    assert_eq!(answer["type"], "result", "{answer}");
    let mentions = worker
        .seen()
        .iter()
        .filter(|line| line.contains(&lost.id[..16]))
        .count();
    assert!(mentions <= 1, "{:?}", worker.seen());
    let mut second = Store::open(journal.path()).unwrap();
    assert!(matches!(
        second.claim(&Claim {
            run: "run-lost",
            base: "base",
            program: "interop-demo",
            questions: &[],
            sources: &[],
            owner: 0,
        }),
        Err(Refusal::Claimed { .. })
    ));

    relay.kill();
    worker.kill();
}

/// Filters, private read surfaces, retention, and forks — the parts of the
/// contract that are only real when a relay is between the writer and the
/// reader.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filters_privacy_retention_and_forks_hold_on_the_wire() {
    let Some(database_url) = database() else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if relay_binary().is_none() {
        eprintln!("skipped: the nostr-relay binary is not built");
        return;
    }
    let mut relay = Relay::spawn(&database_url);
    let publisher = identity(PUBLISHER);
    let customer = identity(CUSTOMER);
    let stranger = identity(STRANGER);
    let mut publisher_socket = connect(&relay, &publisher);
    let mut customer_socket = connect(&relay, &customer);
    let mut stranger_socket = connect(&relay, &stranger);

    // Filters: each single-letter index the query contract names.
    let definition = program_definition(publisher.pubkey());
    let first = publisher.signer().sign(
        now() - 10,
        prg::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "fork-demo".into()]),
            Tag::new(vec!["t".into(), prg::PROGRAM_MARKER.into()]),
        ],
        definition.to_string(),
    );
    let mut forked = definition.clone();
    forked["summary"] = json!("the forked revision");
    let second = publisher.signer().sign(
        now(),
        prg::DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), "fork-demo".into()]),
            Tag::new(vec!["t".into(), prg::PROGRAM_MARKER.into()]),
        ],
        forked.to_string(),
    );
    publish(&mut publisher_socket, &first);
    publish(&mut publisher_socket, &second);
    let (winner, _) = query(
        &mut customer_socket,
        "fork",
        json!({"kinds": [prg::DISCOVERY_KIND], "#d": ["fork-demo"]}),
    );
    assert_eq!(winner.len(), 1, "the newer fork replaces: {winner:?}");
    assert_eq!(winner[0].id, second.id);
    let by_author = query(
        &mut customer_socket,
        "by-author",
        json!({"kinds": [prg::DISCOVERY_KIND], "authors": [publisher.pubkey()], "since": now() - 60}),
    );
    assert!(!by_author.0.is_empty());
    let by_tag = query(
        &mut customer_socket,
        "by-tag",
        json!({"kinds": [prg::DISCOVERY_KIND], "#t": [prg::PROGRAM_MARKER], "limit": 5}),
    );
    assert!(!by_tag.0.is_empty());
    let miss = query(
        &mut customer_socket,
        "miss",
        json!({"kinds": [prg::DISCOVERY_KIND], "#d": ["nobody-wrote-this"]}),
    );
    assert!(miss.0.is_empty());

    // Retention: an already-expired event is refused at admission and an
    // ephemeral kind is fanned out but never stored.
    let expired = customer.signer().sign(
        now(),
        1,
        vec![Tag::new(vec![
            "expiration".into(),
            (now() - 60).to_string(),
        ])],
        "already gone".to_string(),
    );
    let verdict = publish(&mut customer_socket, &expired);
    assert_eq!(verdict[2], false, "{verdict}");
    assert_eq!(EventClass::from_kind(REQUEST_KIND), EventClass::Ephemeral);
    let ephemeral = customer.signer().sign(
        now(),
        REQUEST_KIND,
        vec![Tag::new(vec!["p".into(), "cd".repeat(32)])],
        "an ephemeral".to_string(),
    );
    publish(&mut customer_socket, &ephemeral);
    let stored = query(
        &mut customer_socket,
        "ephemeral",
        json!({"kinds": [REQUEST_KIND]}),
    );
    assert!(stored.0.is_empty(), "ephemeral kinds are not stored");

    // Private surfaces: a run record reads to its author and its `p`
    // recipient only, and a stranger cannot even publish one in another's
    // name — the relay enforces author authentication for the kind. The
    // database is shared across runs, so each read scopes to this run's
    // `h` rather than asserting a global count.
    let head = hex(&secp256k1::rand::random::<[u8; 32]>());
    let record = customer.signer().sign(
        now(),
        run::RECORD_KIND,
        vec![
            Tag::new(vec!["p".into(), publisher.pubkey().to_string()]),
            Tag::new(vec!["h".into(), head.clone()]),
            Tag::new(vec!["t".into(), run::MARKER.into()]),
        ],
        "Y2lwaGVydGV4dA==".to_string(),
    );
    assert_eq!(publish(&mut customer_socket, &record)[2], true);
    let filter = json!({"kinds": [run::RECORD_KIND], "#h": [head]});
    assert_eq!(
        only(query(&mut customer_socket, "author-read", filter.clone())).id,
        record.id,
        "the author reads its own record"
    );
    assert!(
        query(&mut stranger_socket, "stranger-read", filter.clone())
            .0
            .is_empty(),
        "a stranger reads nothing under the same filter"
    );
    let mut worker_socket = connect(&relay, &identity(WORKER));
    assert!(
        query(&mut worker_socket, "unrelated-read", filter.clone())
            .0
            .is_empty()
    );
    let mut publisher_socket2 = connect(&relay, &publisher);
    assert_eq!(
        only(query(&mut publisher_socket2, "recipient-read", filter)).id,
        record.id,
        "the p recipient reads the record"
    );
    let forged_head = hex(&secp256k1::rand::random::<[u8; 32]>());
    let forged = stranger.signer().sign(
        now(),
        run::RECORD_KIND,
        vec![
            Tag::new(vec!["p".into(), customer.pubkey().to_string()]),
            Tag::new(vec!["h".into(), forged_head.clone()]),
            Tag::new(vec!["t".into(), run::MARKER.into()]),
        ],
        "Y2lwaGVydGV4dA==".to_string(),
    );
    // The stranger's own record p-tags the customer, so it lands and the
    // customer — the named recipient — reads it. The publisher, who is
    // neither author nor recipient, does not.
    assert_eq!(publish(&mut stranger_socket, &forged)[2], true);
    let forged_filter = json!({"kinds": [run::RECORD_KIND], "#h": [forged_head]});
    assert_eq!(
        only(query(
            &mut customer_socket,
            "as-recipient",
            forged_filter.clone()
        ))
        .id,
        forged.id,
        "the named recipient reads the record"
    );
    assert!(
        query(&mut publisher_socket2, "as-outsider", forged_filter)
            .0
            .is_empty(),
        "an outsider reads neither party's private record"
    );

    // Private capability policy: only its author reads it.
    let mailbox = "01".repeat(32);
    let policy = publisher.signer().sign(
        now(),
        cap::PREFERENCE_KIND,
        vec![
            Tag::new(vec!["t".into(), cap::PRIVATE_POLICY_MARKER.into()]),
            Tag::new(vec!["p".into(), publisher.pubkey().to_string()]),
            Tag::new(vec!["d".into(), mailbox]),
        ],
        "Y2lwaGVydGV4dA==".to_string(),
    );
    assert_eq!(publish(&mut publisher_socket, &policy)[2], true);
    let policy_filter = json!({"kinds": [cap::PREFERENCE_KIND]});
    assert_eq!(
        query(&mut publisher_socket, "own-policy", policy_filter.clone())
            .0
            .len(),
        1
    );
    assert!(
        query(&mut stranger_socket, "other-policy", policy_filter)
            .0
            .is_empty()
    );

    // Fencing across controller processes: the journal refuses a second
    // claim, and generation fencing refuses a stale handoff.
    let fencing_dir = tempfile::tempdir().unwrap();
    let mut first_owner = Store::open(fencing_dir.path()).unwrap();
    first_owner
        .claim(&Claim {
            run: "run-fenced",
            base: "base",
            program: "interop-demo",
            questions: &[],
            sources: &[],
            owner: 0,
        })
        .unwrap();
    let mut second_owner = Store::open(fencing_dir.path()).unwrap();
    assert!(matches!(
        second_owner.claim(&Claim {
            run: "run-fenced",
            base: "base",
            program: "interop-demo",
            questions: &[],
            sources: &[],
            owner: 0,
        }),
        Err(Refusal::Claimed { .. })
    ));
    assert!(run::accept_generation(2, 1).is_err());
    let fenced = run::Dispatcher {
        id: "worker-a".into(),
        reachable: true,
        acked_generation: Some(1),
    };
    assert!(run::handoff(true, false, 1, &[fenced]).unwrap().is_empty());

    relay.kill();
}

/// The claims each side makes, checked against what it was configured to
/// do: the relay's NIP-11 document against its environment, the worker's
/// startup log against its flags, and a `coder -p` child and an in-process
/// turn both reaching the worker through the same relay and leaving their
/// traces.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advertised_roles_match_configuration_and_turns_record() {
    let Some(database_url) = database() else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if relay_binary().is_none() {
        eprintln!("skipped: the nostr-relay binary is not built");
        return;
    }
    let mut relay = Relay::spawn(&database_url);
    let customer = identity(CUSTOMER);
    let mut worker = Worker::start(
        &relay.url,
        &[
            ("CODER_WORKER_ALLOW", customer.pubkey().to_string()),
            ("CODER_WORKER_JOBS", "2".to_string()),
        ],
    );

    // The worker's startup claims, from the same stderr an operator reads.
    assert_eq!(
        worker.log_until("worker"),
        format!("worker  {}", identity(WORKER).pubkey())
    );
    let door = worker.log_until("door    ");
    assert!(door.contains("stub"), "{door}");
    let jobs = worker.log_until("jobs    ");
    assert!(jobs.contains("2 at once"), "{jobs}");
    let admits = worker.log_until("admits  ");
    assert!(admits.contains("1 customer"), "{admits}");
    worker.log_until("waiting for jobs");

    // The relay's NIP-11 against what it was spawned with: author
    // authentication is required, the OpenAgents profiles it was given
    // are the extensions it advertises, and NIP-42 and NIP-67 are on the
    // supported list.
    let mut http = TcpStream::connect(relay.address).unwrap();
    use std::io::{Read, Write};
    http.write_all(
        b"GET / HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n",
    )
    .unwrap();
    let mut response = Vec::new();
    http.read_to_end(&mut response).unwrap();
    let body = String::from_utf8(response).unwrap();
    let document: Value = serde_json::from_str(body.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(document["limitation"]["auth_required"], true);
    let nips = document["supported_nips"].as_array().unwrap();
    assert!(nips.contains(&json!(42)), "{nips:?}");
    assert!(nips.contains(&json!(67)), "{nips:?}");
    let extensions = document["supported_extensions"].as_array().unwrap();
    for profile in ["nip-cap-v1", "nip-prg-v1", "nip-ext-v1", "nip-run-v1"] {
        assert!(extensions.contains(&json!(profile)), "{extensions:?}");
    }

    // A `coder -p` child is its own process on the same wire: its turn
    // reaches the worker through the relay, and the trace it leaves is
    // the headless record the issue asks for.
    let traces = tempfile::tempdir().unwrap();
    let trace_path = traces.path().join("headless.jsonl");
    let home = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder"));
    for name in CREDENTIALS {
        command.env_remove(name);
    }
    command
        .env("HOME", home.path())
        .env("CODER_SECRET_KEY", hex(&[CUSTOMER; 32]))
        .env("CODER_WORKER", identity(WORKER).pubkey())
        .env("CODER_RELAY", &relay.url)
        .env_remove("CODER_SECRET_KEY_FILE")
        .args(["--trace"])
        .arg(&trace_path)
        .args(["-p", "say counted"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "coder -p failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let headless = atif::log::read(&trace_path).unwrap();
    let headless_messages: Vec<&str> = headless
        .steps
        .iter()
        .map(|step| step.message.as_str())
        .collect();
    assert!(
        headless_messages
            .iter()
            .any(|message| message.contains("say counted")),
        "the headless trace records the prompt: {headless_messages:?}"
    );
    assert!(
        headless_messages
            .iter()
            .any(|message| message.contains("stub door")),
        "the headless trace records the worker's answer: {headless_messages:?}"
    );

    // The terminal side runs the same turn in-process through the same
    // door type, and its trace is the second record.
    let terminal_logs = tempfile::tempdir().unwrap();
    let recorder = Recorder::open(terminal_logs.path(), "stub", "relay", "interop").unwrap();
    let door = Door::Relay(Box::new(RelayDoor::new(
        relay.url.clone(),
        xonly(identity(WORKER).pubkey()),
        identity(CUSTOMER),
    )));
    let mut agent = Agent::new(None, door).with_trace(Some(recorder));
    let finished = turn::run(&mut agent, "say counted".to_string(), &mut |_| {})
        .await
        .expect("the terminal turn finishes over the relay");
    assert!(!finished.reply.is_empty());
    drop(agent);
    let terminal_trace = std::fs::read_dir(terminal_logs.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let terminal = atif::log::read(&terminal_trace).unwrap();
    let terminal_messages: Vec<&str> = terminal
        .steps
        .iter()
        .map(|step| step.message.as_str())
        .collect();
    assert!(
        terminal_messages
            .iter()
            .any(|message| message.contains("say counted")),
        "the terminal trace records the prompt: {terminal_messages:?}"
    );
    assert!(
        terminal_messages
            .iter()
            .any(|message| message.contains("stub door")),
        "the terminal trace records the worker's answer: {terminal_messages:?}"
    );

    // The stub worker's answers are unmetered: the turn's cost record says
    // so rather than inventing a number.
    relay.kill();
    worker.kill();
}
