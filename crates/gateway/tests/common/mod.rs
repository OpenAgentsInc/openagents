//! Shared harness for the gateway's relay-facing integration tests:
//! an in-process fanout relay that also stores addressable events, a
//! stub backend, a deployed gateway, and caller-side helpers speaking
//! `nostr::decision` — no mock of the protocol itself.
//!
//! Every test stands up its own listeners and directories; nothing
//! shares state but the shapes being checked.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{SinkExt, StreamExt};
use nostr::decision::{self, Seal};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::nip44;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tenancy::{Binding, Expected, Lane, Manifest, Quota, Registry, Tenant, keys};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, accept_async, connect_async, tungstenite,
};

use gateway::config::{Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

pub const AUTH_KIND: u16 = 22_242;
pub const WORKER_BYTE: u8 = 0x77;
pub const CALLER_BYTE: u8 = 0x42;

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub fn hex_secret(byte: u8) -> String {
    (0..32).map(|_| format!("{byte:02x}")).collect()
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&hex_secret(byte)).unwrap()
}

pub fn secret(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).unwrap()
}

pub fn xonly(byte: u8) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::new(), &secret(byte))
        .x_only_public_key()
        .0
}

/// A valid-looking artifact pin for test bindings.
pub fn artifact(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

// ---------- the fanout relay ----------

/// One subscription's filter: kinds plus single-letter tag matches.
pub struct Filter {
    kinds: Vec<u16>,
    authors: Vec<String>,
    tags: Vec<(String, Vec<String>)>,
}

impl Filter {
    fn parse(value: &Value) -> Self {
        let kinds = value
            .get("kinds")
            .and_then(Value::as_array)
            .map(|kinds| {
                kinds
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|k| k as u16)
                    .collect()
            })
            .unwrap_or_default();
        let authors = value
            .get("authors")
            .and_then(Value::as_array)
            .map(|authors| {
                authors
                    .iter()
                    .filter_map(|author| author.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let tags = value
            .as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(name, values)| {
                        let name = name.strip_prefix('#')?.to_string();
                        let values = values
                            .as_array()?
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_string))
                            .collect();
                        Some((name, values))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            kinds,
            authors,
            tags,
        }
    }

    /// NIP-01 matching: kind and author lists admit any member, and
    /// every tag condition needs one matching tag value.
    fn matches(&self, event: &Event) -> bool {
        (self.kinds.is_empty() || self.kinds.contains(&event.kind))
            && (self.authors.is_empty() || self.authors.contains(&event.pubkey))
            && self.tags.iter().all(|(name, values)| {
                event
                    .tag_values(name.as_str())
                    .any(|value| values.iter().any(|wanted| wanted == value))
            })
    }
}

pub struct Sub {
    id: String,
    filter: Filter,
}

/// One connection's outgoing queue and subscriptions.
pub struct Conn {
    out: mpsc::UnboundedSender<String>,
    subs: Vec<Sub>,
}

/// The relay's connection table.
pub type Conns = Arc<Mutex<Vec<Conn>>>;

/// The relay's event store: addressable kinds replace on
/// `(pubkey, kind, d)`; everything else appends.
type Store = Arc<Mutex<Vec<Event>>>;

/// Stand the relay up on a real port; return its `ws://` URL and the
/// connection table tests watch for the worker's subscription.
pub async fn relay() -> (String, Conns) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let conns: Conns = Arc::new(Mutex::new(Vec::new()));
    let store: Store = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&conns);
    let stored = Arc::clone(&store);
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let shared = Arc::clone(&shared);
            let stored = Arc::clone(&stored);
            tokio::spawn(async move {
                let _ = relay_conn(stream, shared, stored).await;
            });
        }
    });
    (url, conns)
}

/// One connection's protocol: challenge, auth, subscribe. Addressable
/// events store and replace; every stored match answers a REQ before
/// `EOSE`, then live events fan out.
async fn relay_conn(
    stream: TcpStream,
    conns: Conns,
    store: Store,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut socket = accept_async(stream).await?;
    let (out, mut inbox) = mpsc::unbounded_channel::<String>();
    let index = {
        let mut conns = conns.lock().unwrap();
        conns.push(Conn {
            out,
            subs: Vec::new(),
        });
        conns.len() - 1
    };
    socket
        .send(tungstenite::Message::Text(
            json!(["AUTH", "the-test-challenge"]).to_string().into(),
        ))
        .await?;
    loop {
        tokio::select! {
            outbound = inbox.recv() => {
                let Some(text) = outbound else { return Ok(()) };
                socket.send(tungstenite::Message::Text(text.into())).await?;
            }
            inbound = socket.next() => {
                let Some(Ok(tungstenite::Message::Text(text))) = inbound else {
                    // The connection is gone; drop its subscriptions so
                    // fanout stops owing it frames. The row stays —
                    // indices are other connections' identities.
                    if let Some(conn) = conns.lock().unwrap().get_mut(index) {
                        conn.subs.clear();
                    }
                    return Ok(());
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                match value[0].as_str() {
                    Some("AUTH") | Some("EVENT") => {
                        let id = value[1]["id"].as_str().unwrap_or_default().to_string();
                        socket
                            .send(tungstenite::Message::Text(
                                json!(["OK", id, true, ""]).to_string().into(),
                            ))
                            .await?;
                        if value[0].as_str() == Some("EVENT") {
                            if let Ok(event) = serde_json::from_value::<Event>(value[1].clone()) {
                                remember(&store, event);
                            }
                            fanout(&conns, &value[1]);
                        }
                    }
                    Some("REQ") => {
                        let id = value[1].as_str().unwrap_or_default().to_string();
                        if let Some(filter) = value.get(2).map(Filter::parse) {
                            let stored: Vec<Event> = store
                                .lock()
                                .unwrap()
                                .iter()
                                .filter(|event| filter.matches(event))
                                .cloned()
                                .collect();
                            for event in stored {
                                socket
                                    .send(tungstenite::Message::Text(
                                        json!(["EVENT", id, event]).to_string().into(),
                                    ))
                                    .await?;
                            }
                            conns.lock().unwrap()[index].subs.push(Sub {
                                id: id.clone(),
                                filter,
                            });
                        }
                        socket
                            .send(tungstenite::Message::Text(
                                json!(["EOSE", id]).to_string().into(),
                            ))
                            .await?;
                    }
                    Some("CLOSE") => {
                        let id = value[1].as_str().unwrap_or_default();
                        conns.lock().unwrap()[index]
                            .subs
                            .retain(|sub| sub.id != id);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Store one event. Addressable kinds replace the record that shares
/// their `(pubkey, kind, d)` — the relay's own staleness rule.
fn remember(store: &Store, event: Event) {
    let addressable = (10_000..20_000).contains(&event.kind) || event.kind >= 30_000;
    let mut events = store.lock().unwrap();
    if addressable {
        let address = (
            event.pubkey.clone(),
            event.kind,
            event.tag_values("d").next().map(str::to_string),
        );
        events.retain(|kept| {
            (
                kept.pubkey.clone(),
                kept.kind,
                kept.tag_values("d").next().map(str::to_string),
            ) != address
        });
    }
    events.push(event);
}

/// Deliver one stored event to every subscription it matches —
/// including the publisher's own, as a relay does.
fn fanout(conns: &Conns, event: &Value) {
    let Ok(parsed) = serde_json::from_value::<Event>(event.clone()) else {
        return;
    };
    for conn in conns.lock().unwrap().iter() {
        for sub in &conn.subs {
            if sub.filter.matches(&parsed) {
                let _ = conn.out.send(json!(["EVENT", sub.id, event]).to_string());
            }
        }
    }
}

/// Wait until some connection holds a subscription — the worker's —
/// before a caller publishes, since live fanout drops what nobody
/// asked for.
pub async fn subscribed(conns: &Conns) {
    for _ in 0..250 {
        if conns
            .lock()
            .unwrap()
            .iter()
            .any(|conn| !conn.subs.is_empty())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the worker never subscribed");
}

// ---------- the stub backend and the real gateway ----------

/// What a stub backend publishes and answers.
struct Backend {
    model: String,
    signature: String,
    answer_status: StatusCode,
    answer_body: Value,
    delay_ms: u64,
    forwards: Arc<AtomicUsize>,
}

async fn backend_models(State(backend): State<Arc<Backend>>) -> Json<Value> {
    Json(json!({
        "models": [{
            "id": backend.model,
            "name": backend.model,
            "artifact_identity": {"digest": backend.signature},
            "execution": {},
        }],
    }))
}

async fn backend_systemone(State(backend): State<Arc<Backend>>, _body: Bytes) -> Response {
    backend.forwards.fetch_add(1, Ordering::SeqCst);
    if backend.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backend.delay_ms)).await;
    }
    let mut body = backend.answer_body.clone();
    body["model"] = json!(backend.model);
    (backend.answer_status, Json(body)).into_response()
}

/// Stand a stub backend up; `signature` is the artifact digest its
/// model card publishes.
pub async fn backend(
    signature: &str,
    answer_status: StatusCode,
    answer_body: Value,
    delay_ms: u64,
) -> (String, Arc<AtomicUsize>) {
    let backend = Arc::new(Backend {
        model: "kev-0.6b".to_string(),
        signature: signature.to_string(),
        answer_status,
        answer_body,
        delay_ms,
        forwards: Arc::new(AtomicUsize::new(0)),
    });
    let forwards = backend.forwards.clone();
    let router = axum::Router::new()
        .route("/v1/models", get(backend_models))
        .route("/v1/systemone", post(backend_systemone))
        .with_state(backend);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{address}"), forwards)
}

/// The test answer a backend gives: a typed question answered.
pub fn answer() -> Value {
    json!({
        "model": "kev-0.6b",
        "answers": {"q1": {"type": "noul", "noul": 0.9}},
        "usage": {"input_tokens": 10, "output_tokens": 2},
    })
}

/// The manifest: a shared door anonymous callers may name, `acme` with
/// a dedicated door and the given quota.
pub fn manifest(signature: &str, requests_per_day: Option<u64>) -> Manifest {
    let mut shared = BTreeMap::new();
    shared.insert(
        "shared-kev".to_string(),
        Binding {
            lane: Lane::Shared,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: signature.to_string(),
                execution: BTreeMap::new(),
            },
            capacity: None,
            promotion: None,
            scope: vec![],
        },
    );
    let mut acme_doors = BTreeMap::new();
    acme_doors.insert(
        "acme-kev".to_string(),
        Binding {
            lane: Lane::Dedicated,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: signature.to_string(),
                execution: BTreeMap::new(),
            },
            capacity: None,
            promotion: None,
            scope: vec![],
        },
    );
    let mut tenants = BTreeMap::new();
    tenants.insert(
        "acme".to_string(),
        Tenant {
            credential: "key-ref:acme".to_string(),
            principals: vec![],
            doors: acme_doors,
            quota: requests_per_day.map(|limit| Quota {
                requests_per_day: Some(limit),
                questions_per_day: None,
                input_bytes_per_day: None,
                concurrency: None,
                policy: Some(tenancy::quota::POLICY_V1.to_string()),
            }),
        },
    );
    Manifest {
        v: tenancy::SCHEMA.to_string(),
        sequence: 0,
        supersedes: None,
        shared,
        tenants,
        digest: String::new(),
    }
}

/// A deployed gateway: registry on disk, one key per tenant, the
/// service on a real port.
pub struct Deployment {
    pub tokens: BTreeMap<String, String>,
    pub address: String,
    /// The registry directory a publisher reads the shared set from.
    pub registry: std::path::PathBuf,
    _dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

pub async fn deploy(manifest: Manifest, endpoint: &str) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let doors = ["shared-kev", "acme-kev"]
        .into_iter()
        .map(|door| {
            (
                door.to_string(),
                Door {
                    endpoint: endpoint.to_string(),
                    classify: None,
                    classify_item_concurrency: 1,
                },
            )
        })
        .collect();
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        max_options: 4096,
        doors,
    })
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        address,
        registry: dir.path().to_path_buf(),
        _dir: dir,
        _state: state,
    }
}

// ---------- the caller ----------

pub async fn send(socket: &mut Socket, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

pub async fn read_json(socket: &mut Socket) -> Value {
    loop {
        let message = socket.next().await.unwrap().unwrap();
        if let tungstenite::Message::Text(text) = message
            && let Ok(value) = serde_json::from_str::<Value>(&text)
        {
            return value;
        }
    }
}

/// Connect, answer the AUTH challenge, return the socket.
pub async fn authenticated_socket(url: &str, key_byte: u8) -> Socket {
    let (mut socket, _) = connect_async(url).await.unwrap();
    let challenge = read_json(&mut socket).await;
    assert_eq!(challenge[0], "AUTH");
    let auth = signer(key_byte).sign(
        unix_now(),
        AUTH_KIND,
        vec![
            Tag::new(vec!["relay".into(), url.to_string()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap().into(),
            ]),
        ],
        String::new(),
    );
    let id = auth.id.clone();
    send(&mut socket, json!(["AUTH", auth])).await;
    let ok = read_json(&mut socket).await;
    assert_eq!(ok, json!(["OK", id, true, ""]));
    socket
}

/// A caller's sealing material for one event to `worker`.
pub fn seal(byte: u8, created_at: u64, worker: &XOnlyPublicKey) -> Seal<'static> {
    let signer: &'static RelaySigner = Box::leak(Box::new(signer(byte)));
    Seal {
        signer,
        conversation: nip44::conversation_key(&secret(byte), worker),
        nonce: secp256k1::rand::random(),
        created_at,
    }
}

/// A decision request body for `door`.
pub fn body(request: &str, attempt: u32, door: &str) -> decision::RequestBody {
    let mut questions = serde_json::Map::new();
    questions.insert(
        "q1".to_string(),
        json!({"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"}),
    );
    decision::RequestBody::new(
        request,
        attempt,
        door,
        json!("A caller's private text."),
        questions,
    )
}

/// Sign and encrypt a request event for the worker.
pub fn request_event(
    byte: u8,
    worker_pub: &str,
    body: &decision::RequestBody,
    created_at: u64,
) -> Event {
    let worker_key: XOnlyPublicKey = worker_pub.parse().unwrap();
    decision::request_event(seal(byte, created_at, &worker_key), body, worker_pub).unwrap()
}
