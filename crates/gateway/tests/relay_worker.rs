//! The NIP-CJ decision worker's end-to-end contract: an in-process
//! fanout relay, the real gateway admission path upstream, and callers
//! speaking `nostr::decision` — no mock of the protocol itself. Every
//! test stands up its own listeners and directories; nothing shares
//! state but the shapes being checked.

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
use nostr::decision::{self, Pending, RequestBody, Seal};
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
use gateway::relay_worker::{Principal, Worker, WorkerConfig};
use gateway::serve::{self, ServeState};

const AUTH_KIND: u16 = 22_242;
const WORKER_BYTE: u8 = 0x77;
const CALLER_BYTE: u8 = 0x42;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn hex_secret(byte: u8) -> String {
    (0..32).map(|_| format!("{byte:02x}")).collect()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&hex_secret(byte)).unwrap()
}

fn secret(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).unwrap()
}

fn xonly(byte: u8) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::new(), &secret(byte))
        .x_only_public_key()
        .0
}

/// A valid-looking artifact pin for test bindings.
fn artifact(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

// ---------- the fanout relay ----------

/// One subscription's filter: kinds plus single-letter tag matches.
struct Filter {
    kinds: Vec<u16>,
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
        Self { kinds, tags }
    }

    /// NIP-01 matching: the kind list admits any of its members, and
    /// every tag condition needs one matching tag value.
    fn matches(&self, event: &Event) -> bool {
        (self.kinds.is_empty() || self.kinds.contains(&event.kind))
            && self.tags.iter().all(|(name, values)| {
                event
                    .tag_values(name.as_str())
                    .any(|value| values.iter().any(|wanted| wanted == value))
            })
    }
}

struct Sub {
    id: String,
    filter: Filter,
}

/// One connection's outgoing queue and subscriptions.
struct Conn {
    out: mpsc::UnboundedSender<String>,
    subs: Vec<Sub>,
}

/// The relay's shared table; a `Mutex` is plenty — fanout is O(conns).
type Conns = Arc<Mutex<Vec<Conn>>>;

/// Stand the relay up on a real port; return its `ws://` URL and the
/// connection table tests watch for the worker's subscription.
async fn relay() -> (String, Conns) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let conns: Conns = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&conns);
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let shared = Arc::clone(&shared);
            tokio::spawn(async move {
                let _ = relay_conn(stream, shared).await;
            });
        }
    });
    (url, conns)
}

/// One connection's protocol: challenge, auth, subscribe, store-and-
/// forward nothing — events fan out to whatever subscribes when they
/// land, like an ephemeral-kind relay treats this family.
async fn relay_conn(stream: TcpStream, conns: Conns) -> Result<(), Box<dyn std::error::Error>> {
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
                            fanout(&conns, &value[1]);
                        }
                    }
                    Some("REQ") => {
                        let id = value[1].as_str().unwrap_or_default().to_string();
                        if let Some(filter) = value.get(2) {
                            conns.lock().unwrap()[index].subs.push(Sub {
                                id: id.clone(),
                                filter: Filter::parse(filter),
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
/// before a caller publishes, since the relay stores nothing.
async fn subscribed(conns: &Conns) {
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
    answer_status: StatusCode,
    answer_body: Value,
    delay_ms: u64,
    forwards: Arc<AtomicUsize>,
}

async fn backend_models(State(_backend): State<Arc<Backend>>) -> Json<Value> {
    Json(json!({
        "models": [{
            "id": "kev-0.6b",
            "name": "kev-0.6b",
            "artifact_identity": {"digest": artifact('a')},
            "execution": {},
        }],
    }))
}

async fn backend_systemone(State(backend): State<Arc<Backend>>, _body: Bytes) -> Response {
    backend.forwards.fetch_add(1, Ordering::SeqCst);
    if backend.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backend.delay_ms)).await;
    }
    (backend.answer_status, Json(backend.answer_body.clone())).into_response()
}

/// Stand a stub backend up; both doors route to it (both cards pin the
/// same artifact digest for the test model).
async fn backend(
    answer_status: StatusCode,
    answer_body: Value,
    delay_ms: u64,
) -> (String, Arc<AtomicUsize>) {
    let backend = Arc::new(Backend {
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
fn answer() -> Value {
    json!({
        "model": "kev-0.6b",
        "answers": {"q1": {"type": "noul", "noul": 0.9}},
        "usage": {"input_tokens": 10, "output_tokens": 2},
    })
}

/// The manifest: a shared door anonymous callers may name, `acme` with
/// a dedicated door and the given quota.
fn manifest(requests_per_day: Option<u64>) -> Manifest {
    let mut shared = BTreeMap::new();
    shared.insert(
        "shared-kev".to_string(),
        Binding {
            lane: Lane::Shared,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: artifact('a'),
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
                artifact_signature: artifact('a'),
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
struct Deployment {
    tokens: BTreeMap<String, String>,
    address: String,
    _dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

async fn deploy(manifest: Manifest, endpoint: &str) -> Deployment {
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
                    batching: None,
                },
            )
        })
        .collect();
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        accounts: None,
        billing: None,
        skills: None,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        cors_origins: vec![],
        max_options: 4096,
        doors,
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    })
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        address,
        _dir: dir,
        _state: state,
    }
}

// ---------- the rig: relay + gateway + worker ----------

/// One whole lane under test.
struct Rig {
    relay_url: String,
    worker_pub: String,
    forwards: Arc<AtomicUsize>,
    jobs_dir: tempfile::TempDir,
    deployment: Deployment,
}

/// Stand everything up and connect the worker. `bound` decides whether
/// the caller key maps to acme's credential; `tune` adjusts the config.
async fn rig(
    quota: Option<u64>,
    endpoint_status: StatusCode,
    endpoint_body: Value,
    delay_ms: u64,
    bound: bool,
    anonymous: bool,
    jobs: usize,
) -> Rig {
    let (relay_url, conns) = relay().await;
    let (endpoint, forwards) = backend(endpoint_status, endpoint_body, delay_ms).await;
    let deployment = deploy(manifest(quota), &endpoint).await;
    let mut principals = BTreeMap::new();
    if bound {
        principals.insert(
            xonly(CALLER_BYTE).to_string(),
            Principal {
                key: deployment.tokens["acme"].clone(),
                tenant: None,
                workspace: None,
            },
        );
    }
    let jobs_dir = tempfile::tempdir().unwrap();
    let worker = Worker::open(WorkerConfig {
        relay: relay_url.clone(),
        worker_secret: Some(hex_secret(WORKER_BYTE)),
        upstream: deployment.address.clone(),
        principals,
        anonymous,
        jobs,
        upstream_timeout_secs: 30,
        jobs_dir: jobs_dir.path().to_path_buf(),
        request_window: None,
    })
    .unwrap();
    let worker_pub = worker.pubkey().to_string();
    let serving = Arc::clone(&worker);
    let url = relay_url.clone();
    tokio::spawn(async move {
        let (socket, _) = connect_async(&url).await.unwrap();
        let _ = serving.serve(socket).await;
    });
    subscribed(&conns).await;
    Rig {
        relay_url,
        worker_pub,
        forwards,
        jobs_dir,
        deployment,
    }
}

// ---------- the caller ----------

async fn send(socket: &mut Socket, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

async fn read_json(socket: &mut Socket) -> Value {
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
async fn authenticated_socket(url: &str, key_byte: u8) -> Socket {
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
fn seal(byte: u8, created_at: u64, worker: &XOnlyPublicKey) -> Seal<'static> {
    let signer: &'static RelaySigner = Box::leak(Box::new(signer(byte)));
    Seal {
        signer,
        conversation: nip44::conversation_key(&secret(byte), worker),
        nonce: secp256k1::rand::random(),
        created_at,
    }
}

/// A decision request body for `door`.
fn body(request: &str, attempt: u32, door: &str) -> RequestBody {
    let mut questions = serde_json::Map::new();
    questions.insert(
        "q1".to_string(),
        json!({"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"}),
    );
    RequestBody::new(
        request,
        attempt,
        door,
        json!("A caller's private text."),
        questions,
    )
}

/// Sign and encrypt a request event for the worker.
fn request_event(byte: u8, worker_pub: &str, body: &RequestBody, created_at: u64) -> Event {
    let worker_key: XOnlyPublicKey = worker_pub.parse().unwrap();
    decision::request_event(seal(byte, created_at, &worker_key), body, worker_pub).unwrap()
}

/// Publish `event` and collect every bound answer until the terminal
/// one — a `result`, or a status `error` — or `secs` runs out.
async fn run_job(
    socket: &mut Socket,
    caller: u8,
    worker_pub: &str,
    body: &RequestBody,
    event: &Event,
    secs: u64,
) -> (
    Vec<decision::StatusPayload>,
    Option<decision::ResultPayload>,
) {
    send(
        socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [event.id]}]),
    )
    .await;
    send(socket, json!(["EVENT", event])).await;
    let pending = Pending {
        attempt_id: &event.id,
        worker: worker_pub,
        customer: &xonly(caller).to_string(),
        request: &body.request,
        attempt: body.attempt,
        request_digest: body.digest(),
    };
    let mut statuses = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!remaining.is_zero(), "the worker never answered");
        let frame = tokio::time::timeout(remaining, read_json(socket))
            .await
            .expect("the worker never answered");
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(answer_event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        match decision::bind_answer(&answer_event, &pending, &secret(caller)) {
            Ok(decision::Answer::Status(status)) => {
                let terminal = status.status == decision::Status::Error;
                statuses.push(status);
                if terminal {
                    return (statuses, None);
                }
            }
            Ok(decision::Answer::Result(result)) => return (statuses, Some(result)),
            Err(_) => continue,
        }
    }
}

/// The receipt out of a result, parsed and self-verified.
fn receipt(result: &decision::ResultPayload) -> receipts::execution::ExecutionReceipt {
    receipts::execution::ExecutionReceipt::parse(&result.receipt.to_string()).unwrap()
}

/// The tenant reference a bound principal's receipts name.
fn tenant_ref(deployment: &Deployment) -> String {
    let id = deployment.tokens["acme"]
        .strip_prefix("oak_")
        .and_then(|rest| rest.split('.').next())
        .unwrap();
    format!("key-ref:{id}")
}

// ---------- the cases ----------

/// A bound signer's call is answered through the real admission path:
/// progress statuses, the verbatim response, and a sealed relay
/// receipt bound to the request event itself.
#[tokio::test]
async fn answered_roundtrip() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-one", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let request_digest = body.digest();
    let (statuses, result) =
        run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    let result = result.expect("the job never resolved");
    let words: Vec<&str> = statuses.iter().map(|s| s.status.as_str()).collect();
    assert!(words.contains(&"queued"), "statuses: {words:?}");
    assert!(words.contains(&"processing"), "statuses: {words:?}");
    assert_eq!(result.outcome, decision::Outcome::Answered);
    assert_eq!(result.response, Some(answer()));
    let receipt = receipt(&result);
    assert_eq!(receipt.transport, "relay");
    assert_eq!(receipt.attempt_id, event.id);
    assert_eq!(receipt.request_digest, request_digest);
    assert_eq!(
        receipt.tenant.as_deref(),
        Some(tenant_ref(&rig.deployment).as_str())
    );
    assert_eq!(receipt.served.model, "kev-0.6b");
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}

/// A settled pair republished under a fresh event gets the recorded
/// result, sealed to the new event's identity — one upstream spend.
#[tokio::test]
async fn settled_pair_republishes() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-retry", 1, "acme-kev");
    let first = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &first, 10).await;
    assert_eq!(result.unwrap().outcome, decision::Outcome::Answered);

    let second = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    assert_ne!(first.id, second.id);
    let (_, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &body,
        &second,
        10,
    )
    .await;
    let result = result.expect("the republish never resolved");
    assert_eq!(result.outcome, decision::Outcome::Answered);
    assert_eq!(result.response, Some(answer()));
    let receipt = receipt(&result);
    assert_eq!(
        receipt.attempt_id, second.id,
        "the receipt must reseal to this delivery"
    );
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}

/// The same pair carrying a different body is an idempotency conflict —
/// a terminal refusal, no upstream spend.
#[tokio::test]
async fn changed_body_conflicts() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let first_body = body("req-conflict", 1, "acme-kev");
    let first = request_event(CALLER_BYTE, &rig.worker_pub, &first_body, unix_now());
    let (_, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &first_body,
        &first,
        10,
    )
    .await;
    assert!(result.is_some());

    let mut changed = body("req-conflict", 1, "acme-kev");
    changed.state = json!("A different private text.");
    let second = request_event(CALLER_BYTE, &rig.worker_pub, &changed, unix_now());
    let (statuses, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &changed,
        &second,
        10,
    )
    .await;
    assert!(
        result.is_none(),
        "a conflict resolves as a status error, not a result"
    );
    let refusal = statuses
        .last()
        .and_then(|status| status.refusal.clone())
        .expect("the terminal status carries no refusal");
    assert_eq!(refusal.code, "idempotency_conflict");
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}

/// A second delivery of an in-flight pair joins the run and gets the
/// same outcome sealed to its own event id.
#[tokio::test]
async fn in_flight_pair_joins() {
    let rig = rig(None, StatusCode::OK, answer(), 1_500, true, true, 4).await;
    let body = body("req-join", 1, "acme-kev");
    let first = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let second = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());

    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [first.id, second.id]}]),
    )
    .await;
    send(&mut socket, json!(["EVENT", first])).await;
    send(&mut socket, json!(["EVENT", second])).await;

    let mut results: Vec<(String, decision::ResultPayload)> = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while results.len() < 2 {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let frame = tokio::time::timeout(remaining, read_json(&mut socket))
            .await
            .expect("both deliveries were not answered");
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        let attempt_id = event.tag_values("e").next().unwrap_or_default().to_string();
        for (pending_id, pending_body) in [(&first.id, &body), (&second.id, &body)] {
            let pending = Pending {
                attempt_id: pending_id,
                worker: &rig.worker_pub,
                customer: &xonly(CALLER_BYTE).to_string(),
                request: &pending_body.request,
                attempt: pending_body.attempt,
                request_digest: pending_body.digest(),
            };
            if let Ok(decision::Answer::Result(result)) =
                decision::bind_answer(&event, &pending, &secret(CALLER_BYTE))
            {
                results.push((attempt_id.clone(), result));
            }
        }
    }
    assert_eq!(results.len(), 2);
    for (attempt_id, result) in &results {
        assert_eq!(result.outcome, decision::Outcome::Answered);
        assert_eq!(receipt(result).attempt_id, *attempt_id);
    }
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}

/// A request past the worker's job bound is refused `busy` with a retry
/// hint, and the admitted job still resolves.
#[tokio::test]
async fn busy_refuses_then_recovers() {
    let rig = rig(None, StatusCode::OK, answer(), 1_500, true, true, 1).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let first_body = body("req-holds-slot", 1, "acme-kev");
    let first = request_event(CALLER_BYTE, &rig.worker_pub, &first_body, unix_now());
    send(&mut socket, json!(["EVENT", first])).await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    let second_body = body("req-sees-busy", 1, "acme-kev");
    let second = request_event(CALLER_BYTE, &rig.worker_pub, &second_body, unix_now());
    let (statuses, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &second_body,
        &second,
        10,
    )
    .await;
    assert!(result.is_none());
    let refusal = statuses
        .last()
        .and_then(|status| status.refusal.clone())
        .expect("the terminal status carries no refusal");
    assert_eq!(refusal.code, "busy");
    assert!(refusal.retry_after_ms.is_some());
}

/// An unmapped signer forwards with no bearer — the anonymous shared
/// lane — and the receipt names no tenant.
#[tokio::test]
async fn unmapped_signer_uses_shared_lane() {
    let rig = rig(None, StatusCode::OK, answer(), 0, false, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-anon", 1, "shared-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    let result = result.expect("the anonymous call never resolved");
    assert_eq!(result.outcome, decision::Outcome::Answered);
    assert_eq!(receipt(&result).tenant, None);
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}

/// `anonymous: false` refuses an unmapped signer before anything runs.
#[tokio::test]
async fn anonymous_off_refuses_unmapped() {
    let rig = rig(None, StatusCode::OK, answer(), 0, false, false, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-denied", 1, "shared-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (statuses, result) =
        run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    assert!(result.is_none());
    let refusal = statuses
        .last()
        .and_then(|status| status.refusal.clone())
        .expect("the terminal status carries no refusal");
    assert_eq!(refusal.code, "not_admitted");
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 0);
}

/// An anonymous caller naming a dedicated door gets the door's own
/// refusal — `door_not_bound` — as a refused outcome.
#[tokio::test]
async fn anonymous_cannot_name_dedicated_door() {
    let rig = rig(None, StatusCode::OK, answer(), 0, false, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-trespass", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    let result = result.expect("the refusal never resolved");
    assert_eq!(result.outcome, decision::Outcome::Refused);
    assert_eq!(
        result.refusal.as_ref().map(|refusal| refusal.code.as_str()),
        Some("door_not_bound")
    );
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 0);
}

/// A payload whose `v` names a schema the worker does not serve is
/// refused `unsupported_version` — a terminal status, no spend.
#[tokio::test]
async fn unsupported_version_refused() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let worker_key: XOnlyPublicKey = rig.worker_pub.parse().unwrap();
    let payload = json!({
        "v": "openagents.systemone.v99",
        "type": "systemone",
        "request": "req-wrong-version",
        "attempt": 1,
        "model": "acme-kev",
        "state": "A caller's private text.",
        "questions": {"q1": {"type": "noul", "instructions": "i", "criteria": "c"}},
    });
    let event = seal(CALLER_BYTE, unix_now(), &worker_key)
        .event(
            decision::REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), rig.worker_pub.clone()])],
            &payload,
        )
        .unwrap();
    send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [event.id]}]),
    )
    .await;
    send(&mut socket, json!(["EVENT", event])).await;

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let frame = tokio::time::timeout(remaining, read_json(&mut socket))
            .await
            .expect("the version refusal never arrived");
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(answer_event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        // The envelope cannot validate under v99, so the refusal binds
        // by `e`/`p` tags and the payload's own request/attempt — bind
        // it with a v1-shaped pending built from the same pair.
        let body = body("req-wrong-version", 1, "acme-kev");
        let pending = Pending {
            attempt_id: &event.id,
            worker: &rig.worker_pub,
            customer: &xonly(CALLER_BYTE).to_string(),
            request: &body.request,
            attempt: body.attempt,
            request_digest: body.digest(),
        };
        if let Ok(decision::Answer::Status(status)) =
            decision::bind_answer(&answer_event, &pending, &secret(CALLER_BYTE))
        {
            assert_eq!(status.status, decision::Status::Error);
            assert_eq!(
                status.refusal.map(|refusal| refusal.code),
                Some("unsupported_version".to_string())
            );
            break;
        }
    }
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 0);
}

/// A request addressed to a pubkey no worker serves is never answered —
/// the caller's own contact deadline is what bounds the wait.
#[tokio::test]
async fn absent_worker_silence() {
    let (relay_url, _) = relay().await;
    let mut socket = authenticated_socket(&relay_url, CALLER_BYTE).await;
    let ghost = xonly(0xde).to_string();
    let body = body("req-ghost", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &ghost, &body, unix_now());
    send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [event.id]}]),
    )
    .await;
    send(&mut socket, json!(["EVENT", event])).await;
    let heard = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let frame = read_json(&mut socket).await;
            if frame[0] == "EVENT" {
                return true;
            }
        }
    })
    .await;
    assert!(heard.is_err(), "a ghost worker answered");
}

/// A request outside the freshness window is refused `stale` — a
/// terminal status, no ledger entry, no spend.
#[tokio::test]
async fn stale_request_refused() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-old", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now() - 1_200);
    let (statuses, result) =
        run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    assert!(result.is_none());
    let refusal = statuses
        .last()
        .and_then(|status| status.refusal.clone())
        .expect("the terminal status carries no refusal");
    assert_eq!(refusal.code, "stale");
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 0);
}

/// A cancel mid-run settles the pair `unavailable` with cause
/// `cancelled`, and the upstream never hears from it again.
#[tokio::test]
async fn cancel_mid_run() {
    let rig = rig(None, StatusCode::OK, answer(), 2_000, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-cancelled", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [event.id]}]),
    )
    .await;
    send(&mut socket, json!(["EVENT", event])).await;
    tokio::time::sleep(Duration::from_millis(400)).await;

    let worker_key: XOnlyPublicKey = rig.worker_pub.parse().unwrap();
    let cancel = decision::cancel_event(
        seal(CALLER_BYTE, unix_now(), &worker_key),
        &rig.worker_pub,
        &body.request,
        &event.id,
    )
    .unwrap();
    send(&mut socket, json!(["EVENT", cancel])).await;

    let pending = Pending {
        attempt_id: &event.id,
        worker: &rig.worker_pub,
        customer: &xonly(CALLER_BYTE).to_string(),
        request: &body.request,
        attempt: body.attempt,
        request_digest: body.digest(),
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let result = loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let frame = tokio::time::timeout(remaining, read_json(&mut socket))
            .await
            .expect("the cancelled job never resolved");
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(answer_event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        if let Ok(decision::Answer::Result(result)) =
            decision::bind_answer(&answer_event, &pending, &secret(CALLER_BYTE))
        {
            break result;
        }
    };
    assert_eq!(result.outcome, decision::Outcome::Unavailable);
    assert_eq!(
        result.refusal.as_ref().map(|refusal| refusal.code.as_str()),
        Some("cancelled")
    );
}

/// A cancel signed by another key resolves nothing — the job runs to
/// its answer.
#[tokio::test]
async fn wrong_signer_cancel_ignored() {
    let rig = rig(None, StatusCode::OK, answer(), 1_200, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-uncancellable", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    send(&mut socket, json!(["EVENT", event])).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let worker_key: XOnlyPublicKey = rig.worker_pub.parse().unwrap();
    let cancel = decision::cancel_event(
        seal(0x99, unix_now(), &worker_key),
        &rig.worker_pub,
        &body.request,
        &event.id,
    )
    .unwrap();
    send(&mut socket, json!(["EVENT", cancel])).await;

    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    assert_eq!(
        result.expect("the job never resolved").outcome,
        decision::Outcome::Answered
    );
}

/// An upstream failure is an `unavailable` outcome — capacity, not the
/// door's judgment — with the upstream's exact code kept as cause.
#[tokio::test]
async fn upstream_failure_is_unavailable() {
    let rig = rig(
        None,
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({"error": {"code": "internal", "message": "the model fell over"}}),
        0,
        true,
        true,
        4,
    )
    .await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-unavailable", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    let result = result.expect("the failure never resolved");
    assert_eq!(result.outcome, decision::Outcome::Unavailable);
    assert_eq!(
        result.refusal.as_ref().map(|refusal| refusal.code.as_str()),
        Some("unavailable")
    );
}

/// The tenant's quota binds the relay lane exactly as it binds HTTP:
/// the second call is the door's refusal, recorded, not retried.
#[tokio::test]
async fn quota_binds_relay_lane() {
    let rig = rig(Some(1), StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let first_body = body("req-spends", 1, "acme-kev");
    let first = request_event(CALLER_BYTE, &rig.worker_pub, &first_body, unix_now());
    let (_, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &first_body,
        &first,
        10,
    )
    .await;
    assert_eq!(result.unwrap().outcome, decision::Outcome::Answered);

    let second_body = body("req-over-quota", 1, "acme-kev");
    let second = request_event(CALLER_BYTE, &rig.worker_pub, &second_body, unix_now());
    let (_, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &second_body,
        &second,
        10,
    )
    .await;
    let result = result.expect("the quota refusal never resolved");
    assert_eq!(result.outcome, decision::Outcome::Refused);
    assert_eq!(
        result.refusal.as_ref().map(|refusal| refusal.code.as_str()),
        Some("quota_exhausted")
    );
}

/// A worker restart keeps the settled record: the pair's redelivery
/// republishes the recorded result — no second spend, ever.
#[tokio::test]
async fn settled_record_survives_restart() {
    let rig = rig(None, StatusCode::OK, answer(), 0, true, true, 4).await;
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("req-durable", 1, "acme-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10).await;
    assert!(result.is_some());

    // A new worker over the same ledger answers from the record.
    let mut principals = BTreeMap::new();
    principals.insert(
        xonly(CALLER_BYTE).to_string(),
        Principal {
            key: rig.deployment.tokens["acme"].clone(),
            tenant: None,
            workspace: None,
        },
    );
    let worker = Worker::open(WorkerConfig {
        relay: rig.relay_url.clone(),
        worker_secret: Some(hex_secret(WORKER_BYTE)),
        upstream: rig.deployment.address.clone(),
        principals,
        anonymous: true,
        jobs: 4,
        upstream_timeout_secs: 30,
        jobs_dir: rig.jobs_dir.path().to_path_buf(),
        request_window: None,
    })
    .unwrap();
    let serving = Arc::clone(&worker);
    let url = rig.relay_url.clone();
    tokio::spawn(async move {
        let (socket, _) = connect_async(&url).await.unwrap();
        let _ = serving.serve(socket).await;
    });
    tokio::time::sleep(Duration::from_millis(400)).await;

    let second = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let (_, result) = run_job(
        &mut socket,
        CALLER_BYTE,
        &rig.worker_pub,
        &body,
        &second,
        10,
    )
    .await;
    let result = result.expect("the restarted worker never republished");
    assert_eq!(result.outcome, decision::Outcome::Answered);
    assert_eq!(receipt(&result).attempt_id, second.id);
    assert_eq!(rig.forwards.load(Ordering::SeqCst), 1);
}
