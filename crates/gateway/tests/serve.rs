//! The gateway's end-to-end contract: real HTTP both ways, a stubbed
//! backend publishing a model card, and a real registry, key store, and
//! ledger on disk. Each test stands up its own directory and listeners —
//! nothing shares state but the shape of the claims being checked.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use hmac::Mac as _;
use serde_json::{Value, json};
use tenancy::quota::Ledger;
use tenancy::{Binding, Capacity, Expected, Lane, Manifest, Quota, Registry, Tenant, keys};

use gateway::config::{Config, Door, SCHEMA};
use gateway::money::{Money, Priced};
use gateway::serve::{self, ServeState};

/// A valid-looking artifact pin for test bindings.
fn artifact(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

/// The manifest the tests share: a shared door every caller may name,
/// `acme` with a dedicated door and a quota, and `globex` bound to
/// nothing but the shared set.
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
                artifact_signature: artifact('b'),
                execution: BTreeMap::new(),
            },
            capacity: Some(Capacity {
                concurrency: Some(2),
                requests_per_minute: None,
            }),
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
    tenants.insert(
        "globex".to_string(),
        Tenant {
            credential: "key-ref:globex".to_string(),
            principals: vec![],
            doors: BTreeMap::new(),
            quota: None,
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

/// A stub's per-request responder: the forwarded body to its answer.
type Responder = dyn Fn(&Value) -> (StatusCode, Value) + Send + Sync;

/// What a stub backend publishes and answers.
struct Backend {
    /// The card's model id.
    model: String,
    /// The digest `artifact_identity.digest` reports.
    digest: String,
    /// The status `POST /v1/systemone` answers with.
    answer_status: StatusCode,
    /// The body it answers with.
    answer_body: Value,
    /// How long the answer takes — the seam an in-flight retry needs.
    delay_ms: u64,
    /// How many forwards arrived — a refusal must not spend one.
    forwards: Arc<AtomicUsize>,
    /// Forwards the stub is holding right now.
    in_flight: Arc<AtomicUsize>,
    /// The highest `in_flight` observed — the record a concurrency
    /// bound is checked against.
    peak: Arc<AtomicUsize>,
    /// Every forwarded body, so a test can read the questions the
    /// facade actually sent.
    bodies: Arc<Mutex<Vec<Value>>>,
    /// The card's `batching` field, when the stub declares one.
    batching: Option<Value>,
    /// A per-request answer, when the test needs input-dependent
    /// replies — keyed off the request's state.
    respond: Option<Arc<Responder>>,
}

async fn backend_models(State(backend): State<Arc<Backend>>) -> Json<Value> {
    let mut card = json!({
        "id": backend.model,
        "name": backend.model,
        "artifact_identity": {"digest": backend.digest},
        "execution": {},
    });
    if let Some(batching) = &backend.batching {
        card["batching"] = batching.clone();
    }
    Json(json!({ "models": [card] }))
}

async fn backend_systemone(State(backend): State<Arc<Backend>>, body: Bytes) -> Response {
    backend.forwards.fetch_add(1, Ordering::SeqCst);
    let held = backend.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
    backend.peak.fetch_max(held, Ordering::SeqCst);
    let parsed: Value = serde_json::from_slice(&body).unwrap_or_default();
    backend.bodies.lock().unwrap().push(parsed.clone());
    if backend.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backend.delay_ms)).await;
    }
    let response = if let Some(respond) = &backend.respond {
        let (status, body) = respond(&parsed);
        (status, Json(body)).into_response()
    } else {
        (backend.answer_status, Json(backend.answer_body.clone())).into_response()
    };
    backend.in_flight.fetch_sub(1, Ordering::SeqCst);
    response
}

/// Stand a stub backend up on a real port.
async fn backend(backend: Backend) -> (String, Arc<AtomicUsize>) {
    let forwards = backend.forwards.clone();
    let router = axum::Router::new()
        .route("/v1/models", get(backend_models))
        .route("/v1/systemone", post(backend_systemone))
        .with_state(Arc::new(backend));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{address}"), forwards)
}

/// The card an honest backend publishes for `kev-0.6b`.
fn honest(digest: String, body: Value) -> Backend {
    Backend {
        model: "kev-0.6b".to_string(),
        digest,
        answer_status: StatusCode::OK,
        answer_body: body,
        delay_ms: 0,
        forwards: Arc::new(AtomicUsize::new(0)),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        bodies: Arc::new(Mutex::new(Vec::new())),
        batching: None,
        respond: None,
    }
}

/// A registry directory, a key per tenant, and the running gateway.
struct Deployment {
    /// The issued tokens, by tenant.
    tokens: BTreeMap<String, String>,
    /// The registry directory — receipts and ledger live inside.
    dir: tempfile::TempDir,
    /// The gateway's address.
    address: String,
    /// The state, kept alive so the ledger lock stays held.
    _state: Arc<ServeState>,
}

/// Stand the whole thing up: registry, keys, stub backend, gateway.
async fn deploy(manifest: Manifest, endpoints: BTreeMap<String, String>) -> Deployment {
    deploy_doors(
        manifest,
        endpoints
            .into_iter()
            .map(|(door, endpoint)| {
                (
                    door,
                    Door {
                        endpoint,
                        classify: None,
                        classify_item_concurrency: 1,
                        batching: None,
                    },
                )
            })
            .collect(),
    )
    .await
}

/// The same deployment, with each door's full declaration supplied —
/// classify bounds included.
async fn deploy_doors(manifest: Manifest, doors: BTreeMap<String, Door>) -> Deployment {
    deploy_tuned(manifest, doors, |_| {}).await
}

/// A deployment whose config the caller adjusts first — the seam for
/// bounds a test needs to tighten.
async fn deploy_tuned(
    manifest: Manifest,
    doors: BTreeMap<String, Door>,
    tune: impl FnOnce(&mut Config),
) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let mut config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
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
        max_options: 4096,
        doors,
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    tune(&mut config);
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        dir,
        address,
        _state: state,
    }
}

/// The receipts file's sealed lines — each parses and self-verifies.
fn receipt_log(dir: &tempfile::TempDir) -> Vec<receipts::execution::ExecutionReceipt> {
    std::fs::read_to_string(dir.path().join("receipts.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| receipts::execution::ExecutionReceipt::parse(line).unwrap())
        .collect()
}

/// A decision request body.
fn call(model: &str) -> Value {
    json!({
        "model": model,
        "state": "A caller's private text.",
        "questions": {
            "q1": {"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"},
        },
    })
}

/// POST a call, with or without a key.
async fn send_call(
    deployment: &Deployment,
    body: &Value,
    token: Option<&str>,
) -> reqwest::Response {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/systemone", deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request.send().await.unwrap()
}

#[tokio::test]
async fn workspace_mode_enforces_membership_on_decisions_and_discovery() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({"answers":{"q1":0.9}}))).await;
    let deployment = deploy_tuned(
        manifest(None),
        [(
            "acme-kev".into(),
            Door {
                endpoint,
                classify: None,
                classify_item_concurrency: 1,
                batching: None,
            },
        )]
        .into_iter()
        .collect(),
        |config| config.require_workspace_membership = true,
    )
    .await;
    let registry = Registry::open(deployment.dir.path()).unwrap();
    let key = keys::authenticate(
        deployment.dir.path(),
        registry.manifest(),
        &deployment.tokens["acme"],
    )
    .unwrap();
    let accounts = tenancy::Accounts::install(deployment.dir.path()).unwrap();
    let owner = accounts.create_account("owner", &[]).unwrap();
    let member = accounts
        .create_account("member", &[format!("key:{}", key.key_id)])
        .unwrap();
    let ws = accounts
        .create_workspace(
            &owner.id,
            "team",
            tenancy::WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    let other = accounts
        .create_workspace(
            &member.id,
            "other",
            tenancy::WorkspaceKind::Organization,
            "globex",
            None,
        )
        .unwrap();
    let invite = accounts
        .invite(&owner.id, &ws.id, tenancy::Role::Member, 60)
        .unwrap();
    accounts.accept(&member.id, &invite.token).unwrap();
    let client = reqwest::Client::new();
    let post = |workspace: &str| {
        client
            .post(format!("{}/v1/systemone", deployment.address))
            .bearer_auth(&deployment.tokens["acme"])
            .header("x-workspace-id", workspace)
            .json(&call("acme-kev"))
    };
    assert_eq!(
        send_call(&deployment, &call("acme-kev"), None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send_call(
            &deployment,
            &call("acme-kev"),
            Some(&deployment.tokens["acme"])
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(&other.id).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(post(&ws.id).send().await.unwrap().status(), StatusCode::OK);
    let models = || {
        client
            .get(format!("{}/v1/models", deployment.address))
            .bearer_auth(&deployment.tokens["acme"])
            .header("x-workspace-id", &ws.id)
    };
    assert_eq!(models().send().await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        post(&ws.id)
            .header("x-workspace-id", &other.id)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    accounts
        .remove_member(&owner.id, &ws.id, &member.id)
        .unwrap();
    assert_eq!(
        post(&ws.id).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        models().send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let classification = client
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .header("x-workspace-id", &ws.id)
        .json(&classify_batch(1))
        .send()
        .await
        .unwrap();
    assert_eq!(classification.status(), StatusCode::FORBIDDEN);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    // Missing storage fails closed instead of reverting to legacy admission.
    std::fs::remove_file(deployment.dir.path().join("accounts.json")).unwrap();
    assert_eq!(
        post(&ws.id).send().await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn an_authorized_call_answers_and_leaves_a_receipt() {
    let (endpoint, forwards) =
        backend(honest(artifact('b'), json!({"answers": {"q1": 0.9}}))).await;
    let deployment = deploy(
        manifest(None),
        [("acme-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "answered");
    assert!(response.headers().get("x-receipt").is_some());
    assert!(response.headers().get("x-request-id").is_some());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["answers"]["q1"], 0.9);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);

    // The sealed receipt names who called, which registry admitted it,
    // and what artifact actually answered.
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Answered);
    assert!(receipt.tenant.is_some());
    assert_eq!(receipt.served.model, "kev-0.6b");
    assert_eq!(receipt.served.artifact_signature, artifact('b'));
    assert!(receipt.registry.is_some());
}

#[tokio::test]
async fn an_unbound_door_and_an_unknown_key_are_refused() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [
            ("shared-kev".to_string(), endpoint.clone()),
            ("acme-kev".to_string(), endpoint),
        ]
        .into_iter()
        .collect(),
    )
    .await;

    // globex holds no binding for acme's dedicated door.
    let response = send_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.tokens["globex"]),
    )
    .await;
    assert_eq!(response.status(), 403);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "door_not_bound");

    // A made-up credential authenticates nothing.
    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some("oak_deadbeef.deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
    )
    .await;
    assert_eq!(response.status(), 401);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unauthenticated");

    // Neither refusal reached a backend.
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    assert_eq!(receipt_log(&deployment.dir).len(), 2);
}

#[tokio::test]
async fn an_anonymous_call_reaches_only_the_shared_lane() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {"q1": 1}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(&deployment, &call("shared-kev"), None).await;
    assert_eq!(response.status(), 200);

    let response = send_call(&deployment, &call("acme-kev"), None).await;
    assert_eq!(response.status(), 403);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);

    // The anonymous call's receipt carries no tenant.
    let first = &receipt_log(&deployment.dir)[0];
    assert_eq!(first.tenant, None);
}

#[tokio::test]
async fn quota_exhaustion_is_refused_and_counted() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(Some(1)),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let first = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(first.status(), 200);

    let second = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(second.status(), 429);
    assert!(second.headers().get("retry-after").is_some());
    let body: Value = second.json().await.unwrap();
    assert_eq!(body["error"]["code"], "quota_exhausted");
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_share_the_reservation_and_resolved_or_changed_pairs_refuse() {
    // A slow backend holds the first attempt open so a retry can arrive
    // while its reservation still stands.
    let (endpoint, forwards) = backend(Backend {
        delay_ms: 300,
        ..honest(artifact('a'), json!({"answers": {}}))
    })
    .await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;
    let client = reqwest::Client::new();
    let send = |body: Value| {
        client
            .post(format!("{}/v1/systemone", deployment.address))
            .bearer_auth(&deployment.tokens["acme"])
            .header("idempotency-key", "req-duplicate")
            .header("x-attempt", "1")
            .json(&body)
    };

    // The same attempt in flight twice: both dispatch under the one
    // reservation — a retry is not a second spend.
    let first = send(call("shared-kev"));
    let second = send(call("shared-kev"));
    let (first, second) = tokio::join!(first.send(), second.send());
    assert_eq!(first.unwrap().status(), 200);
    assert_eq!(second.unwrap().status(), 200);
    assert_eq!(forwards.load(Ordering::SeqCst), 2);

    // Settled now: the same pair cannot be taken again, and a changed
    // body under it is refused rather than merged.
    let third = send(call("shared-kev")).send().await.unwrap();
    assert_eq!(third.status(), 409);
    let mut changed = call("shared-kev");
    changed["state"] = json!("Different text entirely.");
    let fourth = send(changed).send().await.unwrap();
    assert_eq!(fourth.status(), 409);
    assert_eq!(forwards.load(Ordering::SeqCst), 2);

    // A second gateway cannot open the same directory — the ledger
    // takes one writer.
    assert!(matches!(
        Ledger::open(deployment.dir.path()),
        Err(tenancy::quota::LedgerTrouble::Locked(_))
    ));
}

#[tokio::test]
async fn a_mismatched_backend_is_refused_before_dispatch() {
    // The backend publishes the wrong artifact for what the binding pins.
    let (endpoint, forwards) = backend(honest(artifact('z'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_mismatch");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);

    // The attempt is recorded unattempted and the reservation released —
    // the budget is not charged for work that never dispatched.
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unattempted);
}

#[tokio::test]
async fn a_dead_backend_is_unattempted_and_a_failed_forward_is_unavailable() {
    // Nothing listens on the endpoint — the identity check itself fails,
    // so the attempt is unattempted and uncharged.
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), "http://127.0.0.1:1".to_string())]
            .into_iter()
            .collect(),
    )
    .await;
    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "door_unavailable");
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unattempted);

    // A backend that answers but refuses with a 5xx is `unavailable` —
    // the attempt dispatched, and quota-v1 counts it.
    let (endpoint, _) = backend(Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        answer_body: json!({"error": {"code": "busy"}}),
        ..honest(artifact('a'), json!({}))
    })
    .await;
    let second = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;
    let response = send_call(&second, &call("shared-kev"), Some(&second.tokens["acme"])).await;
    assert_eq!(response.status(), 503);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "unavailable");
    let [receipt] = receipt_log(&second.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unavailable);
    assert_eq!(receipt.cause.as_deref(), Some("unavailable"));
}

#[tokio::test]
async fn a_backend_refusal_passes_through_typed() {
    let (endpoint, _) = backend(Backend {
        answer_status: StatusCode::UNPROCESSABLE_ENTITY,
        answer_body: json!({"error": {"code": "too_many_options", "message": "…"}}),
        ..honest(artifact('a'), json!({}))
    })
    .await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 422);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "refused");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "too_many_options");
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Refused);
    assert_eq!(receipt.cause.as_deref(), Some("too_many_options"));
}

#[tokio::test]
async fn a_registry_update_names_its_revision_and_rebinds_the_next_call() {
    let (endpoint, _) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    // First call under sequence 0.
    send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;

    // The operator rebinds the door to artifact B — sequence 1. The
    // backend still publishes A, so the next call refuses as a mismatch
    // under the new revision, and each receipt names the revision its
    // call was admitted under.
    let mut updated = manifest(None);
    updated.sequence = 1;
    updated.supersedes = Some(
        Registry::open(deployment.dir.path())
            .unwrap()
            .digest()
            .to_string(),
    );
    updated
        .shared
        .get_mut("shared-kev")
        .unwrap()
        .artifact
        .artifact_signature = artifact('b');
    Registry::update(deployment.dir.path(), updated).unwrap();

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_mismatch");

    let records = receipt_log(&deployment.dir);
    assert_eq!(records[0].registry.as_ref().unwrap().sequence, 0);
    assert_eq!(records[1].registry.as_ref().unwrap().sequence, 1);
    assert_ne!(
        records[0].registry.as_ref().unwrap().digest,
        records[1].registry.as_ref().unwrap().digest
    );
}

#[tokio::test]
async fn an_oversized_envelope_is_refused_before_a_door_is_consulted() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    // 300 questions over a 256-question bound — refused at 422 with no
    // forward and no reservation taken.
    let mut questions = serde_json::Map::new();
    for index in 0..300 {
        questions.insert(
            format!("q{index}"),
            json!({"type": "noul", "instructions": "…", "criteria": "…"}),
        );
    }
    let response = send_call(
        &deployment,
        &json!({"model": "shared-kev", "state": "…", "questions": questions}),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 422);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "too_many_questions");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn discovery_lists_the_callers_doors() {
    let (endpoint, _) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [
            ("shared-kev".to_string(), endpoint.clone()),
            ("acme-kev".to_string(), endpoint),
        ]
        .into_iter()
        .collect(),
    )
    .await;

    // acme sees its dedicated door plus the shared set.
    let body: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = body["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|card| card["id"].as_str())
        .collect();
    assert_eq!(ids, ["acme-kev", "shared-kev"]);
    for card in body["models"].as_array().unwrap() {
        assert_eq!(card["admission"]["scope"], json!([]));
        assert!(card["admission"]["record"].is_null());
        assert!(
            card["admission"]["registry_digest"]
                .as_str()
                .is_some_and(|v| !v.is_empty())
        );
        assert_eq!(card["admission"]["registry_sequence"], 0);
    }

    // globex sees only the shared door — another tenant's dedicated
    // door is invisible, not merely unreachable.
    let body: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["globex"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["models"].as_array().unwrap().len(), 1);
}

/// A door that serves classify: the product limits plus the configured
/// item concurrency the scheduler fans out under.
fn classify_door(endpoint: String, item_concurrency: u64) -> Door {
    Door {
        endpoint,
        classify: Some(gateway::classify::BackendLimits::product()),
        classify_item_concurrency: item_concurrency,
        batching: None,
    }
}

fn classify_call() -> Value {
    json!({
        "v":"openagents.classify.v1", "model":"acme-kev", "capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1", "name":"test-policy",
          "select":{"single_label":{"ties":"first-declared","no_match":{"kind":"null"}}}},
        "inputs":[{"id":"second","text":"one"},{"id":"first","text":"two"}],
        "mode":"single-label", "labels":[{"id":"a"},{"id":"b"}]
    })
}

/// Compare a public response fixture with a real HTTP result. Only elapsed
/// times and run-specific references are normalized. Model identities,
/// scores, outcomes, and usage stay intact; receipt integrity has separate tests.
fn classification_response_fixture(name: &str, body: &Value) {
    fn normalize(value: &mut Value, identities: &mut BTreeMap<String, String>) {
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    // Declared budgets are contract values, not measurements.
                    if matches!(key.as_str(), "policy" | "bounds") {
                        continue;
                    }
                    if key == "latency_ms" {
                        assert!(value.as_u64().is_some());
                        *value = json!(0);
                    } else if key == "queue_ms" {
                        // Per item a measured wait; under `timing` the
                        // call's aggregate. Either way the value is a
                        // measurement, not a contract — zero it.
                        if let Some(fields) = value.as_object_mut() {
                            for field in fields.values_mut() {
                                assert!(field.as_u64().is_some());
                                *field = json!(0);
                            }
                        } else {
                            assert!(value.as_u64().is_some());
                            *value = json!(0);
                        }
                    } else if matches!(key.as_str(), "attempt_id" | "receipt" | "usage_ref") {
                        if let Some(identity) = value.as_str() {
                            let next = identities.len();
                            let replacement =
                                identities.entry(identity.to_string()).or_insert_with(|| {
                                    if key == "receipt" {
                                        assert!(
                                            identity.starts_with("sha256:") && identity.len() == 71
                                        );
                                        format!("sha256:{next:064x}")
                                    } else {
                                        format!("fixture-reference-{next}")
                                    }
                                });
                            *value = json!(replacement);
                        }
                    } else {
                        normalize(value, identities);
                    }
                }
            }
            Value::Array(values) => values
                .iter_mut()
                .for_each(|value| normalize(value, identities)),
            _ => {}
        }
    }
    let mut actual = body.clone();
    normalize(&mut actual, &mut BTreeMap::new());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/decision-models/fixtures/classify-v1/responses")
        .join(format!("{name}.json"));
    if std::env::var("UPDATE_CLASSIFY_FIXTURES").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&actual).unwrap()),
        )
        .unwrap();
    }
    let expected: Value = serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap();
    assert_eq!(actual, expected, "{}", path.display());
}

#[tokio::test]
async fn classify_preserves_input_order_and_records_verified_native_answers() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({
        "model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}},
        "usage":{"input_tokens":3,"output_tokens":1}
    }))).await;
    let deployment = deploy_doors(
        manifest(None),
        [("acme-kev".into(), classify_door(endpoint, 1))]
            .into_iter()
            .collect(),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(&classify_call())
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("single-label", &body);
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    assert_eq!(body["results"][0]["input"], "second");
    assert_eq!(body["results"][1]["input"], "first");
    assert_eq!(body["results"][0]["units"][0]["selected"], "a");
    assert_eq!(body["usage"]["input_tokens"], 6);
    assert_eq!(receipt_log(&deployment.dir).len(), 1);
}

#[tokio::test]
async fn classify_refuses_undeclared_limits_before_forwarding() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({}))).await;
    let deployment = deploy(
        manifest(None),
        [("acme-kev".into(), endpoint)].into_iter().collect(),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(&classify_call())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "unsupported_limits"
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

async fn classification_deployment(endpoint: String) -> Deployment {
    deploy_doors(
        manifest(None),
        [("acme-kev".into(), classify_door(endpoint, 1))]
            .into_iter()
            .collect(),
    )
    .await
}

/// The same deployment with the door's configured item concurrency and
/// any gateway tuning the test needs.
async fn classification_deployment_tuned(
    endpoint: String,
    item_concurrency: u64,
    tune: impl FnOnce(&mut Config),
) -> Deployment {
    deploy_tuned(
        manifest(None),
        [("acme-kev".into(), classify_door(endpoint, item_concurrency))]
            .into_iter()
            .collect(),
        tune,
    )
    .await
}

async fn send_classification(deployment: &Deployment, call: &Value) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(call)
        .send()
        .await
        .unwrap();
    (response.status(), response.json().await.unwrap())
}

#[tokio::test]
async fn classify_retains_partial_outcomes_and_does_not_invent_complete_usage() {
    let forwards = Arc::new(AtomicUsize::new(0));
    let counter = forwards.clone();
    let app = axum::Router::new()
        .route("/v1/models", get(|| async { Json(json!({"models":[{"id":"kev-0.6b", "artifact_identity":{"digest":artifact('b')}, "execution":{}}]})) }))
        .route("/v1/systemone", post(move || {
            let index = counter.fetch_add(1,Ordering::SeqCst);
            async move {
                if index == 0 { (StatusCode::OK,Json(json!({"model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}}, "usage":{"input_tokens":3,"output_tokens":1}}))) }
                else { (StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"unavailable"}))) }
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    let deployment = classification_deployment(endpoint).await;
    let mut call = classify_call();
    call["inputs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"third","text":"three"}));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("mixed", &body);
    assert_eq!(body["outcome"], "mixed");
    assert_eq!(
        body["outcomes"],
        json!({"answered":1,"unavailable":1,"unattempted":1,"refused":0})
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    assert_eq!(body["results"][2]["input"], "third");
    assert_eq!(body["usage"]["input_tokens_complete"], false);
    assert!(body["usage"].get("input_tokens").is_none());
    assert_eq!(body["results"][0]["usage"]["input_tokens"], 3);
    let ledger = std::fs::read_to_string(deployment.dir.path().join("quota-ledger.jsonl")).unwrap();
    let settled: Value = ledger
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|event| event["event"] == "settled")
        .expect("the attempt settled");
    assert_eq!(settled["units"]["questions"], 2);
    assert_eq!(settled["units"]["options"], 4);
}

#[tokio::test]
async fn classify_rejects_wrong_models_and_invalid_distributions() {
    for answer in [
        json!({"model":"other", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}}}),
        json!({"model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.8}}}}),
    ] {
        let (endpoint, _) = backend(honest(artifact('b'), answer)).await;
        let deployment = classification_deployment(endpoint).await;
        let (status, body) = send_classification(&deployment, &classify_call()).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["outcomes"]["answered"], 0);
        assert!(body["results"][0]["units"][0]["selected"].is_null());
    }
}

#[tokio::test]
async fn an_unfinished_chunked_response_is_refused_at_the_byte_limit() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buffer[..n]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 16384);
            }
            if index == 0 {
                assert!(request.starts_with(b"GET /v1/models"));
                let body=json!({"models":[{"id":"kev-0.6b","artifact_identity":{"digest":artifact('b')},"execution":{}}]}).to_string();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(body.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            } else {
                assert!(request.starts_with(b"POST /v1/systemone"));
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n400001\r\n")
                    .await
                    .unwrap();
                // Deliberately never send the terminating chunk or close the
                // connection. A read-to-end implementation waits for timeout.
                let bytes = vec![b'x'; 4_194_305];
                let _ = socket.write_all(&bytes).await;
                std::future::pending::<()>().await;
            }
        }
    });
    let deployment = classification_deployment(endpoint).await;
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        send_classification(&deployment, &classify_call()),
    )
    .await;
    server.abort();
    let (status, body) =
        result.expect("the byte bound refuses without waiting for response completion");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body["results"][0]["cause"]
            .as_str()
            .unwrap()
            .contains("exceeded 4194304 bytes"),
        "{body}"
    );
    assert_eq!(body["results"][1]["outcome"], "unattempted");
}

/// A stub that answers each forward from the request's state — the seam
/// the per-input modes need.
fn per_input_backend(
    respond: impl Fn(&Value) -> (StatusCode, Value) + Send + Sync + 'static,
) -> Backend {
    Backend {
        respond: Some(Arc::new(respond)),
        ..honest(artifact('b'), json!({}))
    }
}

#[tokio::test]
async fn classify_binary_sends_one_noul_and_selects_the_declared_subset() {
    // The noul rises with the input's own text: "keep" is over the
    // caller's 0.5 cut, "edge" lands on it, "drop" falls under.
    let stub = per_input_backend(|body| {
        let state = body["state"].as_str().unwrap_or_default();
        let noul = if state.contains("keep") {
            0.9
        } else if state.contains("edge") {
            0.5
        } else {
            0.1
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"noul","noul":noul}},
                   "usage":{"input_tokens":3,"output_tokens":1}}),
        )
    });
    let bodies = stub.bodies.clone();
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"filter",
          "select":{"binary":{"threshold":0.5}}},
        "inputs":[{"id":"a","text":"keep this"},{"id":"b","text":"drop this"},{"id":"c","text":"edge case"}],
        "mode":"binary","labels":[{"id":"keep","description":"Worth keeping"}]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "answered");
    assert_eq!(forwards.load(Ordering::SeqCst), 3);

    // Each forward asked exactly one noul question — one judgment per
    // input, the filter's label its only criterion.
    for sent in bodies.lock().unwrap().iter() {
        let questions = sent["questions"].as_object().unwrap();
        assert_eq!(questions.len(), 1);
        assert_eq!(questions["q0"]["type"], "noul");
        assert!(
            questions["q0"]["instructions"]
                .as_str()
                .unwrap()
                .contains("Worth keeping")
        );
    }

    // Per-input outcomes keep input order; the corpus subset names the
    // ids the threshold admitted — the boundary case included.
    assert_eq!(body["results"][0]["input"], "a");
    assert_eq!(body["results"][1]["input"], "b");
    assert_eq!(body["results"][2]["input"], "c");
    assert_eq!(body["results"][0]["units"][0]["selected"], "keep");
    assert!(body["results"][1]["units"][0]["selected"].is_null());
    assert_eq!(body["results"][2]["units"][0]["selected"], "keep");
    assert_eq!(body["results"][0]["units"][0]["raw"]["noul"], 0.9);
    assert_eq!(
        body["selections"],
        json!([{"mode":"binary","label":"keep","selected":["a","c"],"unevaluated":[]}])
    );
    assert_eq!(receipt_log(&deployment.dir).len(), 1);
}

#[tokio::test]
async fn classify_score_ranks_inputs_on_the_declared_rubric() {
    // Three inputs, three positions on one rubric: a scores highest,
    // then c, then b — the ranking is the call's, not input order.
    let rubric = json!({"0":"weak","1":"fair","2":"strong"});
    let rubric_for_answer = rubric.clone();
    let stub = per_input_backend(move |body| {
        let (score, level, probabilities) = match body["state"].as_str().unwrap_or_default() {
            "a-input" => (2.0, "2", json!({"0":0.0,"1":0.0,"2":1.0})),
            "b-input" => (0.4, "0", json!({"0":0.8,"1":0.2,"2":0.0})),
            _ => (1.2, "1", json!({"0":0.2,"1":0.6,"2":0.2})),
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"score","score":score,"confidence":0.9,
                                   "legend":rubric_for_answer,"selected":level,
                                   "probabilities":probabilities}},
                   "usage":{"input_tokens":4,"output_tokens":2}}),
        )
    });
    let bodies = stub.bodies.clone();
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending"}}},
        "inputs":[{"id":"a","text":"a-input"},{"id":"b","text":"b-input"},{"id":"c","text":"c-input"}],
        "mode":"score","levels":["weak","fair","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The forward carried the rubric as an ordered score criteria.
    let questions = bodies.lock().unwrap()[0]["questions"].clone();
    assert_eq!(questions["q0"]["type"], "score");
    assert_eq!(
        questions["q0"]["criteria"],
        json!(["weak", "fair", "strong"])
    );

    // Raw positions survive; `selected` is the categorical level the
    // estimator named; the corpus ranking orders by weighted position.
    assert_eq!(body["results"][0]["units"][0]["raw"]["score"], 2.0);
    assert_eq!(body["results"][0]["units"][0]["selected"], 2);
    assert_eq!(body["results"][2]["units"][0]["raw"]["score"], 1.2);
    assert_eq!(
        body["selections"],
        json!([{"mode":"score","ranking":["a","c","b"],"unevaluated":[]}])
    );
}

#[tokio::test]
async fn classify_score_ranking_keeps_ties_and_omits_invalid_rubrics() {
    let stub = per_input_backend(|body| {
        let invalid = body["state"] == "invalid";
        let answer = if invalid {
            json!({"type":"score","score":3.0,"confidence":1.0,
                   "legend":{"0":"weak","3":"outside"},
                   "probabilities":{"0":0.0,"3":1.0},"selected":"3"})
        } else {
            json!({"type":"score","score":0.5,"confidence":0.5,
                   "legend":{"0":"weak","1":"strong"},
                   "probabilities":{"0":0.5,"1":0.5}})
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b","answers":{"q0":answer}}),
        )
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"stable-ranking",
          "select":{"score":{"order":"ascending","top_n":1}}},
        "inputs":[{"id":"first","text":"valid"},{"id":"second","text":"valid"},
                  {"id":"bad","text":"invalid"}],
        "mode":"score","levels":["weak","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["selections"],
        json!([{
            "mode":"score","ranking":["first"],"unevaluated":["bad"]
        }])
    );
    assert_eq!(body["results"][0]["units"][0]["selected"], 1);
    assert_eq!(body["results"][1]["units"][0]["outcome"], "answered");
    assert_eq!(body["results"][2]["units"][0]["outcome"], "unavailable");
}

#[tokio::test]
async fn classify_refuses_invalid_rubrics_and_undeclared_rules() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({}))).await;
    let deployment = classification_deployment(endpoint).await;
    let base = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending"},"binary":{"threshold":0.5}}},
        "inputs":[{"id":"a","text":"x"}],
        "mode":"score","levels":["weak","strong"]
    });

    // A one-level rubric, an over-maximum rubric, a rubric beside a
    // label set, and a binary unit with two labels all refuse before
    // any forward.
    let mut under_minimum = base.clone();
    under_minimum["levels"] = json!(["only"]);
    let mut over_maximum = base.clone();
    over_maximum["levels"] = json!((0..=10).map(|n| format!("level {n}")).collect::<Vec<_>>());
    let mut with_labels = base.clone();
    with_labels["labels"] = json!([{"id":"x"}]);
    let mut wide_binary = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":base["policy"].clone(),
        "inputs":[{"id":"a","text":"x"}],
        "mode":"binary","labels":[{"id":"keep"},{"id":"drop"}]
    });
    for call in [
        under_minimum,
        over_maximum,
        with_labels,
        wide_binary.clone(),
    ] {
        let (status, body) = send_classification(&deployment, &call).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    // Eleven levels is a count refusal, the rest invalid envelopes.
    let mut levels_11 = base.clone();
    levels_11["levels"] = json!((0..11).map(|n| format!("level {n}")).collect::<Vec<_>>());
    let (_, body) = send_classification(&deployment, &levels_11).await;
    assert_eq!(body["error"]["code"], "too_many_levels");

    // A mode the policy does not declare is refused rather than
    // assigned a rule.
    let mut undeclared = base.clone();
    undeclared["policy"]["select"]["score"] = Value::Null;
    let (status, body) = send_classification(&deployment, &undeclared).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_request");
    wide_binary["policy"]["select"]["binary"] = Value::Null;
    let (status, _) = send_classification(&deployment, &wide_binary).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn classify_multi_label_counts_overlapping_labels_and_flags_uncertainty() {
    // Three inputs over a two-label set: one selects both labels, one
    // selects one with a weak second judgment, one selects nothing.
    // The counts overlap by design — each label's Noul stands alone.
    let stub = per_input_backend(|body| {
        let (x, y) = match body["state"].as_str().unwrap_or_default() {
            "both" => (0.9, 0.8),
            "one-weak" => (0.6, 0.4),
            _ => (0.2, 0.3),
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"noul","noul":x},
                              "q1":{"type":"noul","noul":y}},
                   "usage":{"input_tokens":5,"output_tokens":2}}),
        )
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"tags",
          "select":{"multi_label":{"threshold":0.5,"ties":"include-all",
                                   "no_match":"empty","uncertain_below":0.7}}},
        "inputs":[{"id":"a","text":"both"},{"id":"b","text":"one-weak"},{"id":"c","text":"none"}],
        "mode":"multi-label","labels":[{"id":"x"},{"id":"y"}]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("multi-label", &body);

    // Per-item: overlapping selection, a weak-label uncertainty flag,
    // and the empty selection's explicit no-match marker.
    assert_eq!(
        body["results"][0]["units"][0]["selected"],
        json!(["x", "y"])
    );
    assert_eq!(body["results"][0]["units"][0]["raw"]["x"]["noul"], 0.9);
    assert_eq!(body["results"][1]["units"][0]["selected"], json!(["x"]));
    assert_eq!(body["results"][1]["units"][0]["uncertain"], true);
    assert_eq!(body["results"][2]["units"][0]["selected"], json!([]));
    assert_eq!(body["results"][2]["units"][0]["no_match"], true);

    // The aggregate counts what the policy selected — overlapping
    // labels both count — names the no-match and the uncertain inputs,
    // and reports each label's declared zero.
    assert_eq!(
        body["aggregates"],
        json!([{
            "mode":"multi-label",
            "outcomes":{"answered":3,"refused":0,"unavailable":0,"unattempted":0},
            "no_match":1,
            "labels":{"x":2,"y":1},
            "uncertain":["b"],
        }])
    );
}

#[tokio::test]
async fn classify_binary_aggregate_counts_rejection_and_unevaluated_work() {
    // One input selected, one refused by the backend, one declined by
    // the threshold and flagged under the declared review cut.
    let stub = per_input_backend(|body| match body["state"].as_str().unwrap_or_default() {
        "keep" => (
            StatusCode::OK,
            json!({"model":"kev-0.6b","answers":{"q0":{"type":"noul","noul":0.9}}}),
        ),
        "refuse" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error":{"code":"too_many_options","message":"…"}}),
        ),
        _ => (
            StatusCode::OK,
            json!({"model":"kev-0.6b","answers":{"q0":{"type":"noul","noul":0.4}}}),
        ),
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"filter",
          "select":{"binary":{"threshold":0.5,"uncertain_below":0.75}}},
        "inputs":[{"id":"a","text":"keep"},{"id":"b","text":"refuse"},{"id":"c","text":"drop"}],
        "mode":"binary","labels":[{"id":"keep"}]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("binary-refusal", &body);
    assert_eq!(body["outcome"], "mixed");

    // The refused input is named under its outcome — never folded into
    // the label count or the no-match total. The declined input counts
    // as no-match, and its weak winning side flags it for review.
    assert_eq!(
        body["aggregates"],
        json!([{
            "mode":"binary",
            "outcomes":{"answered":2,"refused":1,"unavailable":0,"unattempted":0},
            "no_match":1,
            "labels":{"keep":1},
            "uncertain":["c"],
        }])
    );
    assert_eq!(
        body["selections"],
        json!([{"mode":"binary","label":"keep","selected":["a"],"unevaluated":["b"]}])
    );
    assert_eq!(body["results"][2]["units"][0]["uncertain"], true);
}

#[tokio::test]
async fn classify_single_label_no_match_label_counts_only_when_routed() {
    // A designated no-match label can win outright or be routed to by
    // the abstention cut — the tally keeps the two apart.
    let stub = per_input_backend(|body| {
        let (choice, probabilities) = match body["state"].as_str().unwrap_or_default() {
            "clear" => ("billing", json!({"billing":0.95,"other":0.05})),
            "genuine-other" => ("other", json!({"billing":0.1,"other":0.9})),
            _ => ("billing", json!({"billing":0.5,"other":0.5})),
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"choice","choice":choice,"confidence":0.8,
                                   "probabilities":probabilities}}}),
        )
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"route",
          "select":{"single_label":{"ties":"first-declared","min_probability":0.6,
                                    "no_match":{"kind":"label","label":"other"},
                                    "uncertain_below":0.9}}},
        "inputs":[{"id":"a","text":"clear"},{"id":"b","text":"genuine-other"},{"id":"c","text":"weak"}],
        "mode":"single-label","labels":[{"id":"billing"},{"id":"other"}]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The weak input's `selected` still shows the designated label,
    // marked as the no-match outcome; only the routed input counts
    // under `no_match` — the genuine "other" win counts under the label.
    assert_eq!(body["results"][2]["units"][0]["selected"], "other");
    assert_eq!(body["results"][2]["units"][0]["no_match"], true);
    assert_eq!(body["results"][2]["units"][0]["uncertain"], true);
    assert!(body["results"][0]["units"][0].get("no_match").is_none());
    assert_eq!(
        body["aggregates"],
        json!([{
            "mode":"single-label",
            "outcomes":{"answered":3,"refused":0,"unavailable":0,"unattempted":0},
            "no_match":1,
            "labels":{"billing":1,"other":1},
            "uncertain":["c"],
        }])
    );
}

#[tokio::test]
async fn classify_score_aggregate_tallies_levels_and_flags_the_uncertain() {
    // Two scored inputs, one concentrated and one diffuse: the level
    // tally counts both, and only the diffuse one flags for review.
    let stub = per_input_backend(|body| {
        let probabilities = match body["state"].as_str().unwrap_or_default() {
            "diffuse" => json!({"0":0.1,"1":0.1,"2":0.8}),
            _ => json!({"0":0.0,"1":0.0,"2":1.0}),
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"score","score":1.8,"confidence":0.7,
                                   "legend":{"0":"weak","1":"fair","2":"strong"},
                                   "selected":"2","probabilities":probabilities}}}),
        )
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending","uncertain_below":0.9}}},
        "inputs":[{"id":"a","text":"diffuse"},{"id":"b","text":"sharp"}],
        "mode":"score","levels":["weak","fair","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("score", &body);
    assert_eq!(body["results"][0]["units"][0]["uncertain"], true);
    assert!(body["results"][1]["units"][0].get("uncertain").is_none());
    assert_eq!(
        body["aggregates"],
        json!([{
            "mode":"score",
            "outcomes":{"answered":2,"refused":0,"unavailable":0,"unattempted":0},
            "no_match":0,
            "levels":{"0":0,"1":0,"2":2},
            "uncertain":["a"],
        }])
    );
    // The corpus ranking is unchanged — counts sit beside it.
    assert_eq!(
        body["selections"],
        json!([{"mode":"score","ranking":["a","b"],"unevaluated":[]}])
    );
}

#[tokio::test]
async fn classify_score_keeps_outcome_order_when_forwards_fail() {
    // One input answers, one is refused by the backend, one is
    // unavailable: the corpus ranking names only the scored input and
    // `unevaluated` carries the rest rather than dropping them.
    let stub = per_input_backend(|body| match body["state"].as_str().unwrap_or_default() {
        "a-input" => (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"score","score":0.75,"confidence":0.9,
                                   "legend":{"0":"weak","1":"strong"},
                                   "probabilities":{"0":0.25,"1":0.75}}}}),
        ),
        "b-input" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error":{"code":"too_many_options","message":"…"}}),
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":{"code":"busy"}}),
        ),
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"ascending"}}},
        "inputs":[{"id":"a","text":"a-input"},{"id":"b","text":"b-input"},{"id":"c","text":"c-input"}],
        "mode":"score","levels":["weak","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "mixed");
    assert_eq!(
        body["outcomes"],
        json!({"answered":1,"refused":1,"unavailable":1,"unattempted":0})
    );
    assert_eq!(body["results"][0]["input"], "a");
    assert_eq!(body["results"][1]["input"], "b");
    assert_eq!(body["results"][1]["outcome"], "refused");
    assert_eq!(body["results"][1]["cause"], "too_many_options");
    assert_eq!(body["results"][2]["input"], "c");
    assert_eq!(body["results"][2]["outcome"], "unavailable");
    assert_eq!(
        body["selections"],
        json!([{"mode":"score","ranking":["a"],"unevaluated":["b","c"]}])
    );
}

/// The classify body a concurrency test sends: `n` inputs named by
/// their position, each a single-label choice.
fn classify_batch(n: usize) -> Value {
    json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"test-policy",
          "select":{"single_label":{"ties":"first-declared","no_match":{"kind":"null"}}}},
        "inputs":(0..n).map(|i| json!({"id":format!("i{i}"),"text":format!("text {i}")})).collect::<Vec<_>>(),
        "mode":"single-label","labels":[{"id":"a"},{"id":"b"}]
    })
}

/// The choice answer the concurrency stubs all send.
fn choice_answer() -> Value {
    json!({"model":"kev-0.6b",
           "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,
                            "probabilities":{"a":0.8,"b":0.2}}},
           "usage":{"input_tokens":3,"output_tokens":1}})
}

/// POST a classify call under a specific credential.
async fn send_classification_as(
    deployment: &Deployment,
    call: &Value,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .json(call);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

#[tokio::test]
async fn classify_stays_serial_until_the_door_declares_item_concurrency() {
    // The default bound is one: four inputs over a slow stub never hold
    // two forwards at once, and each still answers in order.
    let stub = Backend {
        delay_ms: 60,
        ..honest(artifact('b'), choice_answer())
    };
    let peak = stub.peak.clone();
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |_| {}).await;
    let (status, body) = send_classification(&deployment, &classify_batch(4)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "answered");
    assert_eq!(forwards.load(Ordering::SeqCst), 4);
    assert_eq!(peak.load(Ordering::SeqCst), 1);
    for (index, item) in body["results"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["input"], format!("i{index}"));
    }
}

#[tokio::test]
async fn classify_fans_out_only_as_far_as_the_door_declares() {
    // Six inputs over a 60ms stub with item concurrency 3: the backend
    // never sees more than three forwards at once, and the results keep
    // input order however they completed.
    let stub = Backend {
        delay_ms: 60,
        ..honest(artifact('b'), choice_answer())
    };
    let peak = stub.peak.clone();
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 3, |_| {}).await;
    let (status, body) = send_classification(&deployment, &classify_batch(6)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(forwards.load(Ordering::SeqCst), 6);
    let observed = peak.load(Ordering::SeqCst);
    assert!(observed > 1 && observed <= 3, "peak {observed}");
    for (index, item) in body["results"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["input"], format!("i{index}"));
        assert_eq!(item["units"][0]["selected"], "a");
    }
    assert_eq!(body["usage"]["input_tokens"], 18);
}

#[tokio::test]
async fn classify_reassembles_input_order_when_completions_reorder() {
    // The first input is the slowest: under concurrency the later
    // inputs finish first, and the response still reports request order.
    let app = axum::Router::new()
        .route(
            "/v1/models",
            get(|| async {
                Json(json!({"models":[{"id":"kev-0.6b","artifact_identity":{"digest":artifact('b')},"execution":{}}]}))
            }),
        )
        .route(
            "/v1/systemone",
            post(|body: Bytes| async move {
                let parsed: Value = serde_json::from_slice(&body).unwrap_or_default();
                let delay = match parsed["state"].as_str().unwrap_or_default() {
                    "slow" => 200,
                    "mid" => 80,
                    _ => 20,
                };
                tokio::time::sleep(Duration::from_millis(delay)).await;
                (StatusCode::OK, Json(choice_answer()))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    let deployment = classification_deployment_tuned(endpoint, 3, |_| {}).await;
    let mut call = classify_batch(3);
    call["inputs"] = json!([
        {"id":"slow","text":"slow"},
        {"id":"mid","text":"mid"},
        {"id":"fast","text":"fast"},
    ]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "answered");
    let ids: Vec<&str> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["input"].as_str())
        .collect();
    assert_eq!(ids, ["slow", "mid", "fast"]);
    let slow = body["results"][0]["latency_ms"].as_u64().unwrap();
    let fast = body["results"][2]["latency_ms"].as_u64().unwrap();
    assert!(slow > fast, "slow {slow} should outlast fast {fast}");
}

#[tokio::test]
async fn classify_shares_the_door_bound_across_tenants() {
    // A shared door with a declared concurrency of two, configured for
    // an item bound of four: two tenants' calls together never hold more
    // than the binding's two forwards, and each call keeps its own order.
    let mut manifest = manifest(None);
    manifest.shared.get_mut("shared-kev").unwrap().capacity = Some(Capacity {
        concurrency: Some(2),
        requests_per_minute: None,
    });
    let stub = Backend {
        delay_ms: 80,
        ..honest(artifact('a'), choice_answer())
    };
    let peak = stub.peak.clone();
    let (endpoint, forwards) = backend(stub).await;
    let deployment = deploy_doors(
        manifest,
        [("shared-kev".into(), classify_door(endpoint, 4))]
            .into_iter()
            .collect(),
    )
    .await;
    let mut call = classify_batch(4);
    call["model"] = json!("shared-kev");
    call["capacity"] = json!("shared");
    let (acme, globex) = tokio::join!(
        send_classification_as(&deployment, &call, Some(&deployment.tokens["acme"])),
        send_classification_as(&deployment, &call, Some(&deployment.tokens["globex"])),
    );
    assert_eq!(acme.0, StatusCode::OK, "{}", acme.1);
    assert_eq!(globex.0, StatusCode::OK, "{}", globex.1);
    assert_eq!(acme.1["outcome"], "answered");
    assert_eq!(globex.1["outcome"], "answered");
    assert_eq!(forwards.load(Ordering::SeqCst), 8);
    let observed = peak.load(Ordering::SeqCst);
    assert!(observed <= 2, "peak {observed} exceeded the binding's 2");
    for body in [&acme.1, &globex.1] {
        for (index, item) in body["results"].as_array().unwrap().iter().enumerate() {
            assert_eq!(item["input"], format!("i{index}"));
        }
    }
}

#[tokio::test]
async fn classify_bounds_queued_inputs_globally_and_per_tenant() {
    for global in [2, 4] {
        let stub = Backend {
            delay_ms: 300,
            ..honest(artifact('a'), choice_answer())
        };
        let (endpoint, forwards) = backend(stub).await;
        let deployment = deploy_tuned(
            manifest(None),
            [("shared-kev".into(), classify_door(endpoint, 1))]
                .into_iter()
                .collect(),
            |config| {
                config.max_classify_inputs = global;
                config.max_classify_inputs_per_tenant = 2;
            },
        )
        .await;
        let registry = Registry::open(deployment.dir.path()).unwrap();
        let alias = keys::issue(deployment.dir.path(), registry.manifest(), "acme").unwrap();
        let mut call = classify_batch(2);
        call["model"] = json!("shared-kev");
        call["capacity"] = json!("shared");
        let address = deployment.address.clone();
        let token = deployment.tokens["acme"].clone();
        let body = call.clone();
        let first = tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("{address}/v1/classify"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while forwards.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let denied = reqwest::Client::new()
            .post(format!("{}/v1/classify", deployment.address))
            .bearer_auth(&alias.token)
            .json(&call)
            .send()
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(denied.headers()["retry-after"], "1");
        assert_eq!(
            denied.json::<Value>().await.unwrap()["error"]["code"],
            "classification_queue_full"
        );
        let other =
            send_classification_as(&deployment, &call, Some(&deployment.tokens["globex"])).await;
        assert_eq!(
            other.0,
            if global == 2 {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::OK
            },
            "{}",
            other.1
        );
        assert_eq!(first.await.unwrap()["outcome"], "answered");
        // The completed call releases both allowances, including for another key.
        assert_eq!(
            send_classification_as(&deployment, &call, Some(&alias.token))
                .await
                .0,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn classify_deadline_leaves_unattempted_items_and_partial_usage() {
    // A 300ms call over a 120ms stub at bound two: three batches of
    // forwards fit, the fourth never dispatches, and settlement counts
    // only what ran.
    let stub = Backend {
        delay_ms: 120,
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 2, |config| {
        config.forward_timeout_ms = 300;
    })
    .await;
    let (status, body) = send_classification(&deployment, &classify_batch(8)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dispatched = forwards.load(Ordering::SeqCst) as u64;
    let outcomes = &body["outcomes"];
    let incomplete =
        outcomes["unavailable"].as_u64().unwrap() + outcomes["unattempted"].as_u64().unwrap();
    assert!(
        incomplete > 0,
        "the deadline must leave work undone: {body}"
    );
    assert_eq!(
        outcomes["answered"].as_u64().unwrap() + outcomes["refused"].as_u64().unwrap() + incomplete,
        8
    );
    assert_eq!(dispatched, 8 - outcomes["unattempted"].as_u64().unwrap());
    let ledger = std::fs::read_to_string(deployment.dir.path().join("quota-ledger.jsonl")).unwrap();
    let settled: Value = ledger
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|event| event["event"] == "settled")
        .expect("the attempt settled");
    assert_eq!(settled["units"]["questions"], dispatched);
}

#[tokio::test]
async fn classify_a_dead_backend_halts_the_queue_without_inventing_work() {
    // Five inputs at bound two over a stub that dies on the third: the
    // dead forward is unavailable, its in-flight neighbour still reports
    // what it got, and everything queued reports unattempted.
    let stub = per_input_backend(|body| {
        if body["state"] == "text 2" {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error":{"code":"busy"}}),
            );
        }
        (StatusCode::OK, choice_answer())
    });
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 2, |_| {}).await;
    let (status, body) = send_classification(&deployment, &classify_batch(5)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "mixed");
    let outcomes = &body["outcomes"];
    assert_eq!(outcomes["unavailable"], 1);
    assert!(outcomes["unattempted"].as_u64().unwrap() >= 1, "{body}");
    assert_eq!(
        outcomes["answered"].as_u64().unwrap() + outcomes["unavailable"].as_u64().unwrap(),
        forwards.load(Ordering::SeqCst) as u64,
    );
    for (index, item) in body["results"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["input"], format!("i{index}"));
    }
}

#[tokio::test]
async fn classify_serial_and_concurrent_batches_are_measured() {
    // The fixture: eight inputs over a stub that holds each forward
    // 60ms, run once per configured bound. The measurement reports what
    // the runs did — completed items, batch elapsed, per-item latency,
    // and incomplete coverage — not a throughput claim.
    const INPUTS: usize = 8;
    async fn run(item_concurrency: u64) -> (Value, usize) {
        let stub = Backend {
            delay_ms: 60,
            ..honest(artifact('b'), choice_answer())
        };
        let peak = stub.peak.clone();
        let (endpoint, _) = backend(stub).await;
        let deployment = classification_deployment_tuned(endpoint, item_concurrency, |_| {}).await;
        let (_, body) = send_classification(&deployment, &classify_batch(INPUTS)).await;
        (body, peak.load(Ordering::SeqCst))
    }
    let (serial, serial_peak) = run(1).await;
    let (concurrent, concurrent_peak) = run(4).await;
    let report = |body: &Value, peak: usize| {
        let items = body["results"].as_array().unwrap();
        json!({
            "completed_items": items.iter().filter(|i| i["outcome"] == "answered").count(),
            "batch_ms": body["timing"]["latency_ms"],
            "per_item_ms": items.iter().map(|i| i["latency_ms"].clone()).collect::<Vec<_>>(),
            "per_item_queue_ms": items.iter().map(|i| i["queue_ms"].clone()).collect::<Vec<_>>(),
            "queue_ms": body["timing"]["queue_ms"],
            "incomplete": items.iter().filter(|i| i["outcome"] != "answered").count(),
            "peak_in_flight": peak,
        })
    };
    let measurement = json!({"serial": report(&serial, serial_peak),
                             "concurrent": report(&concurrent, concurrent_peak)});
    eprintln!("classify-schedule-measurement {measurement}");
    assert_eq!(measurement["serial"]["completed_items"], INPUTS);
    assert_eq!(measurement["concurrent"]["completed_items"], INPUTS);
    assert_eq!(measurement["serial"]["incomplete"], 0);
    assert_eq!(measurement["concurrent"]["incomplete"], 0);
    assert_eq!(serial_peak, 1);
    assert!(concurrent_peak > 1 && concurrent_peak <= 4);
    let serial_ms = serial["timing"]["latency_ms"].as_u64().unwrap();
    let concurrent_ms = concurrent["timing"]["latency_ms"].as_u64().unwrap();
    assert!(
        concurrent_ms < serial_ms,
        "concurrent {concurrent_ms} should beat serial {serial_ms}"
    );
    for body in [&serial, &concurrent] {
        for (index, item) in body["results"].as_array().unwrap().iter().enumerate() {
            assert_eq!(item["input"], format!("i{index}"));
        }
    }
}

#[tokio::test]
async fn classify_records_each_items_queue_time_and_the_calls() {
    // Six inputs over a 60ms stub at bound two: the first wave dispatches
    // at once and every later item waits. Each item reports its own
    // measured queue time, the response carries the aggregate, and the
    // sealed receipt keeps the longest wait as the call's queue time.
    let stub = Backend {
        delay_ms: 60,
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 2, |_| {}).await;
    let (status, body) = send_classification(&deployment, &classify_batch(6)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["results"].as_array().unwrap();
    let queued: Vec<u64> = items
        .iter()
        .map(|item| {
            item["queue_ms"]
                .as_u64()
                .unwrap_or_else(|| panic!("item carries no measured queue time: {item}"))
        })
        .collect();
    assert!(
        queued.iter().any(|wait| *wait > 0),
        "items behind the first wave must report a wait: {queued:?}"
    );
    let max = body["timing"]["queue_ms"]["max"].as_u64().unwrap();
    let total = body["timing"]["queue_ms"]["total"].as_u64().unwrap();
    assert_eq!(max, *queued.iter().max().unwrap());
    assert_eq!(total, queued.iter().sum::<u64>());
    let receipt = receipt_log(&deployment.dir)
        .into_iter()
        .next()
        .expect("the call sealed a receipt");
    assert_eq!(receipt.timing.queued_ms, Some(max));
}

#[tokio::test]
async fn classify_uses_its_own_deadline_not_the_forward_timeouts() {
    // A 300ms classification deadline over a 120ms stub at bound two —
    // while the forward timeout stays at ten seconds. The call resolves
    // near its own deadline with the tail unattempted; it does not wait
    // out the forward timeout for work that cannot fit.
    let stub = Backend {
        delay_ms: 120,
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 2, |config| {
        config.forward_timeout_ms = 10_000;
        config.classify_timeout_ms = Some(300);
    })
    .await;
    let (status, body) = send_classification(&deployment, &classify_batch(8)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["timing"]["latency_ms"].as_u64().unwrap() < 2_000,
        "the call's own deadline governs, not the forward timeout: {body}"
    );
    assert!(
        body["outcomes"]["unattempted"].as_u64().unwrap() > 0,
        "the short deadline must leave queued items: {body}"
    );
    assert!(forwards.load(Ordering::SeqCst) < 8);
}

#[tokio::test]
async fn classify_bounds_each_tenants_in_flight_forwards() {
    // A shared door declared for four concurrent forwards, calls allowed
    // a fan-out of four, and each tenant's in-flight share capped at
    // one. Two tenants' calls together never hold more than two
    // forwards — and the second tenant's smaller call still finishes
    // while the first tenant's larger one runs.
    let mut manifest = manifest(None);
    manifest.shared.get_mut("shared-kev").unwrap().capacity = Some(Capacity {
        concurrency: Some(4),
        requests_per_minute: None,
    });
    let stub = Backend {
        delay_ms: 150,
        ..honest(artifact('a'), choice_answer())
    };
    let peak = stub.peak.clone();
    let (endpoint, _) = backend(stub).await;
    let deployment = deploy_tuned(
        manifest,
        [("shared-kev".into(), classify_door(endpoint, 4))]
            .into_iter()
            .collect(),
        |config| config.max_tenant_classify_in_flight = Some(1),
    )
    .await;
    let mut big = classify_batch(4);
    big["model"] = json!("shared-kev");
    big["capacity"] = json!("shared");
    let mut small = big.clone();
    small["inputs"] = json!([
        {"id":"g0","text":"g0"},
        {"id":"g1","text":"g1"},
    ]);
    let acme_token = deployment.tokens["acme"].clone();
    let globex_token = deployment.tokens["globex"].clone();
    let acme = tokio::spawn({
        let address = deployment.address.clone();
        async move {
            reqwest::Client::new()
                .post(format!("{address}/v1/classify"))
                .bearer_auth(&acme_token)
                .json(&big)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    });
    // Let acme's first item dispatch before globex's call lands.
    tokio::time::sleep(Duration::from_millis(60)).await;
    let globex = send_classification_as(&deployment, &small, Some(&globex_token)).await;
    let acme = acme.await.unwrap();
    assert_eq!(globex.0, StatusCode::OK, "{}", globex.1);
    assert_eq!(globex.1["outcome"], "answered", "{}", globex.1);
    assert_eq!(acme["outcome"], "answered", "{acme}");
    assert!(
        peak.load(Ordering::SeqCst) <= 2,
        "two tenants at one share each may never exceed two forwards: {}",
        peak.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn classify_publishes_and_verifies_declared_adapter_batching() {
    // The operator declares the adapter loops calls; the backend card
    // agrees. Discovery reports the declaration under `adapter`, and a
    // card that disagrees is an identity fault, not a silent substitute.
    let stub = Backend {
        batching: Some(json!({"kind": "caller-loop"})),
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, _) = backend(stub).await;
    let door = Door {
        batching: Some(tenancy::backend::Batching {
            kind: tenancy::backend::BatchKind::CallerLoop,
            max_items: None,
        }),
        ..classify_door(endpoint, 2)
    };
    let deployment = deploy_doors(
        manifest(None),
        [("acme-kev".into(), door)].into_iter().collect(),
    )
    .await;
    let cards: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let card = &cards["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "acme-kev")
        .unwrap()["classification"];
    assert_eq!(
        card["execution"]["adapter"]["batching"]["kind"],
        "caller-loop"
    );
    let (status, body) = send_classification(&deployment, &classify_batch(2)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "answered");

    // A backend claiming native packing the declaration did not name is
    // refused before a byte of the request forwards.
    let mismatch_stub = Backend {
        batching: Some(json!({"kind": "native", "max_items": 4})),
        ..honest(artifact('b'), choice_answer())
    };
    let (mismatch_endpoint, mismatch_forwards) = backend(mismatch_stub).await;
    let mismatch = deploy_doors(
        manifest(None),
        [(
            "acme-kev".into(),
            Door {
                batching: Some(tenancy::backend::Batching {
                    kind: tenancy::backend::BatchKind::CallerLoop,
                    max_items: None,
                }),
                ..classify_door(mismatch_endpoint, 2)
            },
        )]
        .into_iter()
        .collect(),
    )
    .await;
    let (status, body) = send_classification(&mismatch, &classify_batch(2)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error"]["code"], "identity_mismatch");
    assert_eq!(mismatch_forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn discovery_reports_classify_limits_without_inventing_backend_support() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment_tuned(endpoint, 4, |config| {
        config.max_classify_inputs = 6;
        config.max_classify_inputs_per_tenant = 3;
        let limits = config
            .doors
            .get_mut("acme-kev")
            .unwrap()
            .classify
            .as_mut()
            .unwrap();
        limits.max_inputs = 5;
        limits.max_labels = 7;
        limits.max_input_bytes = 100;
        limits.max_forward_bytes = 4096;
    })
    .await;
    let cards: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let model = cards["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "acme-kev")
        .unwrap();
    let contract = &model["classification"];
    assert_eq!(contract["status"], "configured");
    assert_eq!(contract["limits"]["max_inputs"], 3);
    assert_eq!(contract["limits"]["max_labels"], 7);
    assert_eq!(contract["limits"]["max_input_bytes"], 100);
    assert_eq!(contract["limits"]["max_forward_bytes"], 4096);
    assert_eq!(contract["execution"]["max_item_concurrency"], 2);
    assert_eq!(contract["execution"]["model_packing"], false);
    assert!(contract["admission"]["context_tokens"].is_null());
    let missing = cards["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "shared-kev")
        .unwrap();
    assert_eq!(missing["classification"]["status"], "unavailable");
    assert!(missing["classification"].get("limits").is_none());
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    // The advertised per-tenant ceiling agrees with actual admission.
    assert_eq!(
        send_classification(&deployment, &classify_batch(4)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let mut oversized = classify_batch(1);
    oversized["inputs"][0]["text"] = json!("x".repeat(101));
    assert_eq!(
        send_classification(&deployment, &oversized).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

// ---------------------------------------------------------------------------
// Monetary admission: the opt-in mode that charges an authenticated
// workspace for every dispatched call. Every price here is a synthetic
// fixture — none of it is launch pricing.
// ---------------------------------------------------------------------------

/// The fixture hold one attempt reserves: the configured maximum usage
/// quoted under the fixture price — 1,000 input at 10 and 100 output at
/// 40 millionths of a unit.
const HOLD: u64 = 14_000;

/// The fixture charge the honest stub's usage report settles at —
/// 3 input at 10 and 1 output at 40 millionths.
const CHARGE: u64 = 70;

/// The door's fixture price and bound: `kev-0.6b` on the `dedicated`
/// lane under the synthetic `synthetic-fixture-v1` schedule.
fn fixture_priced() -> Priced {
    Priced {
        price: tenancy::money::Price {
            version: "synthetic-fixture-v1".to_string(),
            currency: "USD".to_string(),
            model: "kev-0.6b".to_string(),
            capacity: "dedicated".to_string(),
            policy: gateway::money::POLICY.to_string(),
            rates: [
                (
                    tenancy::money::Resource::InputTokens,
                    tenancy::money::Rate {
                        millionths: 10,
                        per_units: 1,
                    },
                ),
                (
                    tenancy::money::Resource::OutputTokens,
                    tenancy::money::Rate {
                        millionths: 40,
                        per_units: 1,
                    },
                ),
            ]
            .into(),
        },
        maximum_usage: [
            (tenancy::money::Resource::InputTokens, 1_000),
            (tenancy::money::Resource::OutputTokens, 100),
        ]
        .into(),
    }
}

/// One operator mutation against the fixture ledger.
fn ledger_apply(
    ledger: &mut tenancy::money::Ledger,
    workspace: &str,
    source: &str,
    operation: tenancy::money::Operation,
) {
    ledger
        .apply(tenancy::money::Mutation {
            workspace: workspace.to_string(),
            source: source.to_string(),
            audit: format!("fixture:{source}"),
            operation,
        })
        .unwrap();
}

/// Provision the workspace's account — create it in USD, then grant
/// `credit` millionths when given. Funding is the operator's act on the
/// ledger; the gateway itself never credits an account.
fn provision_account(ledger: &mut tenancy::money::Ledger, workspace: &str, credit: Option<u64>) {
    ledger_apply(
        ledger,
        workspace,
        "create",
        tenancy::money::Operation::Create {
            currency: "USD".to_string(),
            spend_limit: u64::MAX,
            topups_allowed: false,
        },
    );
    if let Some(amount) = credit {
        ledger_apply(
            ledger,
            workspace,
            "grant",
            tenancy::money::Operation::Credit {
                amount,
                credit_kind: tenancy::money::CreditKind::Grant,
            },
        );
    }
}

/// A monetary deployment: the running gateway plus the provisioning a
/// test needs to grow the scenario — another member, another workspace.
struct MoneyDeployment {
    /// The live gateway.
    deployment: Deployment,
    /// The organization workspace the acme key's account belongs to.
    workspace: String,
    /// The account the acme key's principal is bound to.
    member: String,
    /// The owning account — the workspace's inviter.
    owner: String,
    /// The money ledger's path, for log assertions.
    ledger: std::path::PathBuf,
}

/// Stand the whole monetary stack up: registry, a key per tenant, a
/// workspace the acme key's account joins, the operator-provisioned
/// ledger, and the gateway holding its lock.
async fn deploy_money(
    manifest: Manifest,
    doors: BTreeMap<String, Door>,
    priced: BTreeMap<String, Priced>,
    provision: impl FnOnce(&mut tenancy::money::Ledger, &str),
) -> MoneyDeployment {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in registry.manifest().tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    // The acme key's principal joins an organization workspace — the
    // membership a monetary charge binds to.
    let key = keys::authenticate(dir.path(), registry.manifest(), &tokens["acme"]).unwrap();
    let accounts = tenancy::Accounts::install(dir.path()).unwrap();
    let owner = accounts.create_account("owner", &[]).unwrap();
    let member = accounts
        .create_account("member", &[format!("key:{}", key.key_id)])
        .unwrap();
    let workspace = accounts
        .create_workspace(
            &owner.id,
            "team",
            tenancy::WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    let invite = accounts
        .invite(&owner.id, &workspace.id, tenancy::Role::Member, 3_600)
        .unwrap();
    accounts.accept(&member.id, &invite.token).unwrap();
    let ledger = dir.path().join("money.jsonl");
    {
        let mut opened = tenancy::money::Ledger::open(&ledger).unwrap();
        provision(&mut opened, &workspace.id);
    }
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: true,
        money: Some(Money {
            ledger: ledger.clone(),
            doors: priced,
        }),
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
        max_options: 4096,
        doors,
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    MoneyDeployment {
        deployment: Deployment {
            tokens,
            dir,
            address,
            _state: state,
        },
        workspace: workspace.id,
        member: member.id,
        owner: owner.id,
        ledger,
    }
}

/// POST a decision call under monetary admission — bearer key plus the
/// workspace header, with optional idempotency headers.
async fn send_money_call(
    deployment: &MoneyDeployment,
    body: &Value,
    token: Option<&str>,
    workspace: Option<&str>,
    key: Option<(&str, u32)>,
) -> reqwest::Response {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/systemone", deployment.deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(workspace) = workspace {
        request = request.header("x-workspace-id", workspace);
    }
    if let Some((idempotency, attempt)) = key {
        request = request
            .header("idempotency-key", idempotency)
            .header("x-attempt", attempt.to_string());
    }
    request.send().await.unwrap()
}

/// GET the caller's workspace balance.
async fn get_balance(
    deployment: &MoneyDeployment,
    token: Option<&str>,
    workspace: Option<&str>,
) -> (StatusCode, Value) {
    let mut request =
        reqwest::Client::new().get(format!("{}/v1/balance", deployment.deployment.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(workspace) = workspace {
        request = request.header("x-workspace-id", workspace);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

/// The priced `acme-kev` door over the given stub endpoint.
fn money_doors(endpoint: String) -> (BTreeMap<String, Door>, BTreeMap<String, Priced>) {
    (
        [(
            "acme-kev".into(),
            Door {
                endpoint,
                classify: None,
                classify_item_concurrency: 1,
                batching: None,
            },
        )]
        .into_iter()
        .collect(),
        [("acme-kev".into(), fixture_priced())]
            .into_iter()
            .collect(),
    )
}

/// The accounts store of a running deployment — provisioning a test
/// adds after the gateway is up still lands on the next request.
fn accounts(deployment: &MoneyDeployment) -> tenancy::Accounts {
    tenancy::Accounts::open(deployment.deployment.dir.path()).unwrap()
}

/// Add `principal`'s key to a fresh account that joins the workspace —
/// the shape a second key in the same workspace takes.
fn join_workspace(
    deployment: &MoneyDeployment,
    label: &str,
    principal: String,
    workspace: &str,
) -> String {
    let accounts = accounts(deployment);
    let account = accounts.create_account(label, &[principal]).unwrap();
    let invite = accounts
        .invite(&deployment.owner, workspace, tenancy::Role::Member, 3_600)
        .unwrap();
    accounts.accept(&account.id, &invite.token).unwrap();
    account.id
}

#[tokio::test]
async fn money_admission_reserves_then_settles_reported_usage() {
    let (endpoint, forwards) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(2 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-settlement"], "settled");
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    let (status, balance) =
        get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(status, StatusCode::OK, "{balance}");
    assert_eq!(balance["workspace"], deployment.workspace);
    assert_eq!(balance["balance"]["settled"], CHARGE);
    assert_eq!(balance["balance"]["reserved"], 0);
    assert_eq!(balance["balance"]["available"], 2 * HOLD - CHARGE);
    assert_eq!(balance["balance"]["currency"], "USD");
    assert_eq!(
        balance["prices"]["acme-kev"]["version"],
        "synthetic-fixture-v1"
    );
    // The ledger stays locked to the running gateway, and its log holds
    // the reserve and the settle under one attempt reference.
    assert!(tenancy::money::Ledger::open(&deployment.ledger).is_err());
    let log = std::fs::read_to_string(&deployment.ledger).unwrap();
    assert!(log.contains("\"reserve\""));
    assert!(log.contains("\"settle\""));
    assert!(log.contains("\"receipt\""));
}

#[tokio::test]
async fn money_refusals_reach_no_backend() {
    let (endpoint, forwards) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    let mut doors = BTreeMap::new();
    for (door, endpoint) in [
        ("acme-kev", endpoint.clone()),
        ("shared-kev", endpoint.clone()),
    ] {
        doors.insert(
            door.to_string(),
            Door {
                endpoint,
                classify: None,
                classify_item_concurrency: 1,
                batching: None,
            },
        );
    }
    // `acme-kev` is priced; `shared-kev` is deliberately not — a door
    // with no configured price must refuse, never invent one.
    let priced: BTreeMap<String, Priced> = [("acme-kev".into(), fixture_priced())]
        .into_iter()
        .collect();
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        // The workspace account exists but cannot cover one hold.
        provision_account(ledger, workspace, Some(CHARGE));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let workspace = deployment.workspace.clone();

    // No key, no workspace header, insufficient balance, an unpriced
    // door: four refusals, and the backend sees none of them.
    for (token, workspace, status, code) in [
        (None, Some(workspace.as_str()), 401, "unauthenticated"),
        (Some(token.as_str()), None, 400, "workspace_required"),
        (
            Some(token.as_str()),
            Some(workspace.as_str()),
            402,
            "insufficient_funds",
        ),
    ] {
        let response =
            send_money_call(&deployment, &call("acme-kev"), token, workspace, None).await;
        assert_eq!(response.status(), status);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], code);
    }
    let unpriced = send_money_call(
        &deployment,
        &call("shared-kev"),
        Some(&token),
        Some(&workspace),
        None,
    )
    .await;
    assert_eq!(unpriced.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = unpriced.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unpriced");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    // A workspace with membership but no provisioned account refuses the
    // same way — admission never creates one.
    let accounts = accounts(&deployment);
    let empty = accounts
        .create_workspace(
            &deployment.member,
            "empty",
            tenancy::WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&empty.id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn money_a_price_that_disagrees_with_the_binding_refuses() {
    let (endpoint, forwards) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    // The door is bound to `kev-0.6b` on `dedicated`; a price naming
    // another model or lane cannot govern its calls.
    for (model, capacity) in [("other-model", "dedicated"), ("kev-0.6b", "shared")] {
        let mut priced = fixture_priced();
        priced.price.model = model.to_string();
        priced.price.capacity = capacity.to_string();
        let (doors, _) = money_doors(endpoint.clone());
        let deployment = deploy_money(
            manifest(None),
            doors,
            [("acme-kev".into(), priced)].into_iter().collect(),
            |ledger, workspace| provision_account(ledger, workspace, Some(HOLD)),
        )
        .await;
        let response = send_money_call(
            &deployment,
            &call("acme-kev"),
            Some(&deployment.deployment.tokens["acme"]),
            Some(&deployment.workspace),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "price_invalid", "{body}");
        assert_eq!(forwards.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn money_completion_without_usage_stays_outstanding() {
    // The backend answers but reports nothing the price can read: the
    // call resolves, the full hold stays outstanding — never zero.
    let (endpoint, _) = backend(honest(artifact('b'), json!({"answers": {"q1": 0.9}}))).await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(2 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-settlement"], "outstanding");
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], HOLD);
    assert_eq!(balance["balance"]["settled"], 0);
    assert_eq!(balance["balance"]["available"], HOLD);
    // The ledger names the completion unknown — the hold is a liability
    // an operator reconciles, not a charge the gateway dropped.
    let log = std::fs::read_to_string(&deployment.ledger).unwrap();
    assert!(log.contains("\"unknown\""));
}

#[tokio::test]
async fn money_an_unavailable_backend_keeps_the_hold_outstanding() {
    // A dispatch that fails outright is unknown work: the reservation
    // stays outstanding rather than releasing on a timeout's say-so.
    let stub = Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        answer_body: json!({"error": {"code": "busy"}}),
        ..honest(artifact('b'), json!({}))
    };
    let (endpoint, forwards) = backend(stub).await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(2 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["x-settlement"], "outstanding");
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], HOLD);
}

#[tokio::test]
async fn money_concurrent_calls_never_overspend() {
    // Four calls, credit for two holds: exactly two dispatch, two
    // refuse at admission, and the backend sees exactly the funded two.
    let stub = Backend {
        delay_ms: 400,
        ..honest(
            artifact('b'),
            json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
        )
    };
    let (endpoint, forwards) = backend(stub).await;
    let (doors, priced) = money_doors(endpoint);
    // Widen the door's declared concurrency so admission refusal lands
    // on the money reserve, not the forward bound.
    let mut manifest = manifest(None);
    manifest
        .tenants
        .get_mut("acme")
        .unwrap()
        .doors
        .get_mut("acme-kev")
        .unwrap()
        .capacity = Some(Capacity {
        concurrency: Some(8),
        requests_per_minute: None,
    });
    let deployment = deploy_money(manifest, doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(2 * HOLD));
    })
    .await;
    let address = deployment.deployment.address.clone();
    let token = deployment.deployment.tokens["acme"].clone();
    let workspace = deployment.workspace.clone();
    let send = || {
        let (address, token, workspace) = (address.clone(), token.clone(), workspace.clone());
        tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("{address}/v1/systemone"))
                .bearer_auth(token)
                .header("x-workspace-id", workspace)
                .json(&call("acme-kev"))
                .send()
                .await
                .unwrap()
                .status()
        })
    };
    let mut tasks = Vec::new();
    for _ in 0..4 {
        tasks.push(send());
    }
    let mut ok = 0;
    let mut refused = 0;
    for task in tasks {
        match task.await.unwrap() {
            StatusCode::OK => ok += 1,
            StatusCode::PAYMENT_REQUIRED => refused += 1,
            status => panic!("unexpected {status}"),
        }
    }
    assert_eq!((ok, refused), (2, 2));
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&workspace)).await;
    assert_eq!(balance["balance"]["settled"], 2 * CHARGE);
    assert_eq!(balance["balance"]["reserved"], 0);
    assert_eq!(balance["balance"]["available"], 2 * HOLD - 2 * CHARGE);
}

#[tokio::test]
async fn money_a_retried_attempt_is_charged_once() {
    // Two concurrent sends of one (idempotency-key, attempt) pair
    // reserve once: one answers and the duplicate cannot dispatch.
    let stub = Backend {
        delay_ms: 150,
        ..honest(
            artifact('b'),
            json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
        )
    };
    let (endpoint, forwards) = backend(stub).await;
    let (doors, priced) = money_doors(endpoint);
    // Widen the door's declared concurrency so admission refusal lands
    // on the money reserve, not the forward bound.
    let mut manifest = manifest(None);
    manifest
        .tenants
        .get_mut("acme")
        .unwrap()
        .doors
        .get_mut("acme-kev")
        .unwrap()
        .capacity = Some(Capacity {
        concurrency: Some(8),
        requests_per_minute: None,
    });
    let deployment = deploy_money(manifest, doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(2 * HOLD));
    })
    .await;
    let address = deployment.deployment.address.clone();
    let token = deployment.deployment.tokens["acme"].clone();
    let workspace = deployment.workspace.clone();
    let send = || {
        let (address, token, workspace) = (address.clone(), token.clone(), workspace.clone());
        tokio::spawn(async move {
            let response = reqwest::Client::new()
                .post(format!("{address}/v1/systemone"))
                .bearer_auth(token)
                .header("x-workspace-id", workspace)
                .header("idempotency-key", "req-dup")
                .header("x-attempt", "1")
                .json(&call("acme-kev"))
                .send()
                .await
                .unwrap();
            let settlement = response
                .headers()
                .get("x-settlement")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            (response.status(), settlement)
        })
    };
    let (first, second) = (send(), send());
    let (first, second) = tokio::join!(first, second);
    let responses = [first.unwrap(), second.unwrap()];
    assert_eq!(
        responses
            .iter()
            .filter(|(status, _)| *status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        responses
            .iter()
            .filter(|(status, _)| *status == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(
        responses
            .iter()
            .find(|(status, _)| *status == StatusCode::OK)
            .unwrap()
            .1
            .as_deref(),
        Some("settled")
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    let token = deployment.deployment.tokens["acme"].clone();
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["settled"], CHARGE, "{balance}");
    let log = std::fs::read_to_string(&deployment.ledger).unwrap();
    assert_eq!(log.matches("\"kind\":\"reserve\"").count(), 1);
    // The resolved pair refuses a changed body and a replay alike.
    let mut changed = call("acme-kev");
    changed["state"] = json!("different content");
    let replay = send_money_call(
        &deployment,
        &changed,
        Some(&token),
        Some(&deployment.workspace),
        Some(("req-dup", 1)),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CONFLICT);
    let replay = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        Some(("req-dup", 1)),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CONFLICT);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["settled"], CHARGE);
}

#[tokio::test]
async fn money_the_workspace_spans_its_keys_and_survives_rotation() {
    let (endpoint, _) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(4 * HOLD));
    })
    .await;
    let dir = deployment.deployment.dir.path();
    // A second acme key joins the workspace through its own account —
    // the workspace's balance, not either key's, is what's charged.
    let registry = Registry::open(dir).unwrap();
    let second = keys::issue(dir, registry.manifest(), "acme").unwrap();
    let second_key = keys::authenticate(dir, registry.manifest(), &second.token).unwrap();
    join_workspace(
        &deployment,
        "member-two",
        format!("key:{}", second_key.key_id),
        &deployment.workspace,
    );
    for token in [
        deployment.deployment.tokens["acme"].clone(),
        second.token.clone(),
    ] {
        let response = send_money_call(
            &deployment,
            &call("acme-kev"),
            Some(&token),
            Some(&deployment.workspace),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let (_, balance) = get_balance(
        &deployment,
        Some(&second.token),
        Some(&deployment.workspace),
    )
    .await;
    assert_eq!(balance["balance"]["settled"], 2 * CHARGE);
    // Rotating the first key revokes it at once: the old secret stops
    // authenticating, and the workspace's balance is untouched.
    let first = keys::authenticate(
        dir,
        registry.manifest(),
        &deployment.deployment.tokens["acme"],
    )
    .unwrap();
    let rotated = keys::rotate(dir, &first.key_id).unwrap();
    let revoked = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.deployment.tokens["acme"]),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
    // The rotated credential needs its principal bound to an account —
    // then it spends from the same workspace balance, never a reset one.
    let rotated_key = keys::authenticate(dir, registry.manifest(), &rotated.token).unwrap();
    join_workspace(
        &deployment,
        "member-rotated",
        format!("key:{}", rotated_key.key_id),
        &deployment.workspace,
    );
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&rotated.token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let (_, balance) = get_balance(
        &deployment,
        Some(&rotated.token),
        Some(&deployment.workspace),
    )
    .await;
    assert_eq!(balance["balance"]["settled"], 3 * CHARGE);
    // A revoked key loses dispatch and balance reads together.
    keys::revoke(dir, &second_key.key_id).unwrap();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&second.token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (status, _) = get_balance(
        &deployment,
        Some(&second.token),
        Some(&deployment.workspace),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn money_balance_reads_stay_inside_the_callers_workspace() {
    let (endpoint, _) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    // The member's own workspace reads its exact position.
    let (status, balance) =
        get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(status, StatusCode::OK, "{balance}");
    assert_eq!(balance["balance"]["credited"], HOLD);
    // No header, no key, a workspace in another tenant, a workspace the
    // account never joined: each refuses rather than leaking a position.
    let accounts = accounts(&deployment);
    let foreign = accounts
        .create_workspace(
            &deployment.owner,
            "foreign",
            tenancy::WorkspaceKind::Organization,
            "globex",
            None,
        )
        .unwrap();
    let stranger = accounts.create_account("stranger", &[]).unwrap();
    let unjoined = accounts
        .create_workspace(
            &stranger.id,
            "unjoined",
            tenancy::WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    for (token, workspace, status) in [
        (Some(token.as_str()), None, 400),
        (None, Some(deployment.workspace.as_str()), 401),
        (Some(token.as_str()), Some(foreign.id.as_str()), 403),
        (Some(token.as_str()), Some(unjoined.id.as_str()), 403),
        (
            Some(deployment.deployment.tokens["globex"].as_str()),
            Some(deployment.workspace.as_str()),
            403,
        ),
    ] {
        assert_eq!(
            get_balance(&deployment, token, workspace).await.0,
            status,
            "{token:?} {workspace:?}"
        );
    }
    // No mutation route exists: the only writes are the operator's own
    // ledger entries.
    for method in ["post", "put", "delete"] {
        let response = reqwest::Client::new()
            .request(
                method.parse().unwrap(),
                format!("{}/v1/balance", deployment.deployment.address),
            )
            .bearer_auth(&token)
            .header("x-workspace-id", &deployment.workspace)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[tokio::test]
async fn money_recovery_keeps_an_outstanding_hold_outstanding() {
    // A hold the last writer left mid-flight is unknown liability after
    // reopen: the gateway serves over it, the balance still shows it
    // reserved, and nothing releases it silently.
    let (endpoint, _) = backend(honest(
        artifact('b'),
        json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
    ))
    .await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(3 * HOLD));
        ledger_apply(
            ledger,
            workspace,
            "stale:reserve",
            tenancy::money::Operation::Reserve {
                attempt: "stale#1".to_string(),
                request_digest: "sha256:fixture".to_string(),
                price: fixture_priced().price,
                maximum_usage: fixture_priced().maximum_usage,
            },
        );
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], HOLD, "{balance}");
    // New work still reserves and settles against what remains.
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], HOLD);
    assert_eq!(balance["balance"]["settled"], CHARGE);
}

#[tokio::test]
async fn money_absent_keeps_every_legacy_behavior() {
    // No `money` in the config: no ledger, no membership requirement,
    // no settlement header, and the balance route does not exist.
    let (endpoint, forwards) =
        backend(honest(artifact('b'), json!({"answers": {"q1": 0.9}}))).await;
    let deployment = deploy(
        manifest(None),
        [("acme-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;
    let response = send_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-settlement").is_none());
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
    let balance = reqwest::Client::new()
        .get(format!("{}/v1/balance", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap();
    assert_eq!(balance.status(), StatusCode::NOT_FOUND);
    assert!(!deployment.dir.path().join("money.jsonl").exists());
}

#[tokio::test]
async fn money_classification_reserves_once_and_settles_the_fan_out() {
    // One hold covers the whole call: each dispatched item reports its
    // usage, the aggregate settles, and one silent item would leave the
    // whole hold outstanding.
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let mut doors = BTreeMap::new();
    doors.insert(
        "acme-kev".to_string(),
        Door {
            endpoint,
            classify: Some(gateway::classify::BackendLimits::product()),
            classify_item_concurrency: 2,
            batching: None,
        },
    );
    let priced: BTreeMap<String, Priced> = [("acme-kev".into(), fixture_priced())]
        .into_iter()
        .collect();
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(3 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/classify", deployment.deployment.address))
        .bearer_auth(&token)
        .header("x-workspace-id", &deployment.workspace)
        .json(&classify_batch(2))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-settlement"], "settled");
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    // Two items each reporting 3 input and 1 output tokens.
    assert_eq!(balance["balance"]["settled"], 2 * CHARGE);
    assert_eq!(balance["balance"]["reserved"], 0);
}

#[tokio::test]
async fn money_classification_stays_outstanding_on_a_silent_item() {
    // One dispatched item reports no usage the price reads: the call's
    // hold is never partially priced — it stays outstanding in full.
    let stub = per_input_backend(|body| {
        if body["state"] == "text 0" {
            (StatusCode::OK, choice_answer())
        } else {
            (
                StatusCode::OK,
                json!({"answers": {"q0": {"type": "choice", "choice": "a",
                "confidence": 0.8, "probabilities": {"a": 0.8, "b": 0.2}}}}),
            )
        }
    });
    let (endpoint, forwards) = backend(stub).await;
    let mut doors = BTreeMap::new();
    doors.insert(
        "acme-kev".to_string(),
        Door {
            endpoint,
            classify: Some(gateway::classify::BackendLimits::product()),
            classify_item_concurrency: 1,
            batching: None,
        },
    );
    let priced: BTreeMap<String, Priced> = [("acme-kev".into(), fixture_priced())]
        .into_iter()
        .collect();
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(3 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.deployment.address))
        .bearer_auth(&token)
        .header("x-workspace-id", &deployment.workspace)
        .json(&classify_batch(2))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-settlement"], "outstanding");
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], HOLD);
    assert_eq!(balance["balance"]["settled"], 0);
}

#[tokio::test]
async fn money_classification_refusals_hold_nothing() {
    // A classify call refused before any fan-out — an unpriced door, or
    // a funded account that cannot cover the hold — dispatches nothing
    // and reserves nothing that stays.
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let mut doors = BTreeMap::new();
    for door in ["acme-kev", "shared-kev"] {
        doors.insert(
            door.to_string(),
            Door {
                endpoint: endpoint.clone(),
                classify: Some(gateway::classify::BackendLimits::product()),
                classify_item_concurrency: 1,
                batching: None,
            },
        );
    }
    let priced: BTreeMap<String, Priced> = [("acme-kev".into(), fixture_priced())]
        .into_iter()
        .collect();
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(CHARGE));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    for (model, capacity) in [("acme-kev", "dedicated"), ("shared-kev", "shared")] {
        let mut batch = classify_batch(1);
        batch["model"] = json!(model);
        batch["capacity"] = json!(capacity);
        let response = reqwest::Client::new()
            .post(format!("{}/v1/classify", deployment.deployment.address))
            .bearer_auth(&token)
            .header("x-workspace-id", &deployment.workspace)
            .json(&batch)
            .send()
            .await
            .unwrap();
        let body: Value = response.json().await.unwrap();
        assert!(
            matches!(
                body["error"]["code"].as_str(),
                Some("insufficient_funds" | "unpriced")
            ),
            "{body}"
        );
    }
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], 0);
}

#[tokio::test]
async fn money_a_failed_identity_check_releases_the_hold() {
    // The backend publishes a digest the binding did not pin: the check
    // refuses before a byte is forwarded, and work that never dispatched
    // is the one release the ledger accepts — the account keeps it all.
    let stub = Backend {
        digest: artifact('z'),
        ..honest(
            artifact('b'),
            json!({"answers": {"q1": 0.9}, "usage": {"input_tokens": 3, "output_tokens": 1}}),
        )
    };
    let (endpoint, forwards) = backend(stub).await;
    let (doors, priced) = money_doors(endpoint);
    let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let response = send_money_call(
        &deployment,
        &call("acme-kev"),
        Some(&token),
        Some(&deployment.workspace),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["x-settlement"], "released");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_mismatch");
    assert_eq!(body["settlement"], "released");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    assert_eq!(balance["balance"]["reserved"], 0);
    assert_eq!(balance["balance"]["available"], HOLD);
}

// ---------------------------------------------------------------------------
// Review and fallback: the opt-in second pass over the primary's
// outputs, every secondary dispatch admitted like the primary's own.
// ---------------------------------------------------------------------------

/// The manifest the review tests share: `acme` holds its classify door
/// plus a reviewer and a fallback door, each pinned to its own artifact.
fn review_manifest() -> Manifest {
    let mut manifest = manifest(None);
    let acme = manifest.tenants.get_mut("acme").unwrap();
    for (door, signature) in [("acme-kev-review", 'c'), ("acme-kev-fb", 'd')] {
        acme.doors.insert(
            door.to_string(),
            Binding {
                lane: Lane::Dedicated,
                artifact: Expected {
                    model: "kev-0.6b".to_string(),
                    adapter: None,
                    artifact_signature: artifact(signature),
                    execution: BTreeMap::new(),
                },
                capacity: None,
                promotion: None,
                scope: vec![],
            },
        );
    }
    manifest
}

/// A deployment with the primary door plus the reviewer and fallback
/// doors the review policy names.
async fn review_deployment(primary: String, reviewer: String, fallback: String) -> Deployment {
    deploy_doors(
        review_manifest(),
        [
            ("acme-kev".into(), classify_door(primary, 1)),
            (
                "acme-kev-review".into(),
                Door {
                    endpoint: reviewer,
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            ),
            (
                "acme-kev-fb".into(),
                Door {
                    endpoint: fallback,
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            ),
        ]
        .into_iter()
        .collect(),
    )
    .await
}

/// A review policy body over the declared cuts.
fn review_policy(reviewer: &str, trigger: &str, on_failure: &str) -> Value {
    json!({
        "v": "openagents.classify-review.v1",
        "reviewer": reviewer,
        "trigger": trigger,
        "on_failure": on_failure,
        "max_items": 8,
        "max_attempts": 8,
        "latency_ms": 5_000,
    })
}

/// A classify call whose policy declares the given review block and the
/// uncertainty cut the `uncertain` trigger reads.
fn review_call(review: Value) -> Value {
    let mut call = classify_call();
    call["policy"]["select"]["single_label"]["uncertain_below"] = json!(0.9);
    call["policy"]["review"] = review;
    call
}

/// The primary's uncertain answer: below the declared 0.9 cut.
fn uncertain_answer() -> Value {
    json!({"model":"kev-0.6b",
           "answers":{"q0":{"type":"choice","choice":"a","confidence":0.55,
                            "probabilities":{"a":0.55,"b":0.45}}},
           "usage":{"input_tokens":3,"output_tokens":1}})
}

/// The reviewer's decisive answer the other way.
fn corrected_answer() -> Value {
    json!({"model":"kev-0.6b",
           "answers":{"q0":{"type":"choice","choice":"b","confidence":0.9,
                            "probabilities":{"a":0.1,"b":0.9}}},
           "usage":{"input_tokens":4,"output_tokens":2}})
}

#[tokio::test]
async fn classify_without_a_review_policy_runs_strict_native() {
    let (endpoint, _) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let (status, body) = send_classification(&deployment, &classify_batch(2)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("review").is_none());
    assert!(body["usage"].get("review").is_none());
    for item in body["results"].as_array().unwrap() {
        assert!(item.get("attempts").is_none());
        assert!(item.get("fallback").is_none());
        assert_eq!(item["review_status"], "not-reviewed");
    }
    assert_eq!(receipt_log(&deployment.dir).len(), 1);
}

#[tokio::test]
async fn classify_review_rejudges_flagged_units_and_records_both_answers() {
    let (primary, primary_forwards) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, fallback_forwards) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let call = review_call(review_policy(
        "acme-kev-review",
        "uncertain",
        "keep-original",
    ));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("review-corrected", &body);
    // Every input re-judged through the reviewer door, none through the
    // fallback.
    assert_eq!(primary_forwards.load(Ordering::SeqCst), 2);
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 2);
    assert_eq!(fallback_forwards.load(Ordering::SeqCst), 0);
    for item in body["results"].as_array().unwrap() {
        let unit = &item["units"][0];
        assert_eq!(unit["selected"], "b", "{unit}");
        assert_eq!(unit["final_source"], "reviewer");
        // The primary's answer survives whole under `original`.
        assert_eq!(unit["original"]["selected"], "a");
        assert_eq!(unit["original"]["uncertain"], true);
        assert_eq!(unit["review"]["outcome"], "answered");
        assert_eq!(unit["review"]["reason"], "uncertain");
        assert_eq!(item["review_status"], "reviewed");
        // The attempt chain names each dispatch's own identity.
        let attempts = item["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["role"], "primary");
        assert_eq!(attempts[1]["role"], "review");
        assert_eq!(attempts[1]["door"], "acme-kev-review");
        assert_ne!(attempts[0]["attempt_id"], attempts[1]["attempt_id"]);
    }
    assert_eq!(body["review"]["v"], "openagents.classify-review.v1");
    assert_eq!(body["review"]["reviewed"], 2);
    assert_eq!(body["review"]["review_answered"], 2);
    assert_eq!(body["review"]["fallback_dispatched"], 0);
    assert_eq!(body["review"]["trigger"], "uncertain");
    assert_eq!(body["review"]["policy_digest"].as_str().unwrap().len(), 71);
    // Usage stays separate: the reviewer's tokens never merge into the
    // primary's counters.
    assert_eq!(body["usage"]["input_tokens"], 6);
    assert_eq!(body["usage"]["review"]["input_tokens"], 8);
    assert_eq!(body["usage"]["review"]["forwards"], 2);
    // One receipt per dispatch: the call plus each review forward.
    assert_eq!(receipt_log(&deployment.dir).len(), 3);
}

#[tokio::test]
async fn classify_review_does_not_fire_on_units_the_trigger_does_not_name() {
    // A decisive answer clears the declared uncertainty cut — the
    // trigger never fires and the reviewer sees nothing.
    let decisive = json!({"model":"kev-0.6b",
           "answers":{"q0":{"type":"choice","choice":"a","confidence":0.95,
                            "probabilities":{"a":0.95,"b":0.05}}},
           "usage":{"input_tokens":3,"output_tokens":1}});
    let (primary, _) = backend(honest(artifact('b'), decisive)).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let call = review_call(review_policy(
        "acme-kev-review",
        "uncertain",
        "keep-original",
    ));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 0);
    assert_eq!(body["review"]["reviewed"], 0);
    for item in body["results"].as_array().unwrap() {
        assert_eq!(item["review_status"], "not-reviewed");
        assert_eq!(item["units"][0]["selected"], "a");
        assert!(item["units"][0].get("review").is_none());
    }
}

#[tokio::test]
async fn classify_review_keeps_the_original_when_the_reviewed_answer_fails_contract() {
    // The reviewer's distribution does not cover the label set — an
    // answer the contract cannot validate is a review that did not
    // answer, and under `keep-original` the primary's selection stands.
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, _) = backend(honest(
        artifact('c'),
        json!({"model":"kev-0.6b",
               "answers":{"q0":{"type":"choice","choice":"a","confidence":0.9,
                                "probabilities":{"a":0.9}}},
               "usage":{"input_tokens":4,"output_tokens":2}}),
    ))
    .await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let call = review_call(review_policy(
        "acme-kev-review",
        "uncertain",
        "keep-original",
    ));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("review-invalid", &body);
    for item in body["results"].as_array().unwrap() {
        let unit = &item["units"][0];
        assert_eq!(unit["selected"], "a", "{unit}");
        assert_eq!(unit["final_source"], "primary");
        assert_eq!(unit["review"]["outcome"], "unavailable");
        assert_eq!(item["review_status"], "review-incomplete");
    }
    assert_eq!(body["review"]["reviewed"], 2);
    assert_eq!(body["review"]["review_answered"], 0);
}

#[tokio::test]
async fn classify_review_keeps_the_score_when_the_reviewer_answers_none() {
    // A score unit re-judged under `always`: the reviewer's reply
    // carries no score answer at all — an absent answer is never
    // invented, and `keep-original` leaves the primary's rubric
    // position standing.
    let (primary, _) = backend(honest(
        artifact('b'),
        json!({"model":"kev-0.6b",
               "answers":{"q0":{"type":"score","score":1.2,"confidence":0.9,
                                "legend":{"0":"weak","1":"fair","2":"strong"},
                                "selected":"1",
                                "probabilities":{"0":0.2,"1":0.6,"2":0.2}}},
               "usage":{"input_tokens":4,"output_tokens":2}}),
    ))
    .await;
    let (reviewer, reviewer_forwards) = backend(honest(
        artifact('c'),
        json!({"model":"kev-0.6b","answers":{}}),
    ))
    .await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending"}},
          "review":{}},
        "inputs":[{"id":"a","text":"a-input"}],
        "mode":"score","levels":["weak","fair","strong"]
    });
    call["policy"]["review"] = review_policy("acme-kev-review", "always", "keep-original");
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 1);
    let unit = &body["results"][0]["units"][0];
    assert_eq!(unit["selected"], 1, "{unit}");
    assert_eq!(unit["raw"]["score"], 1.2);
    assert_eq!(unit["final_source"], "primary");
    assert_eq!(unit["review"]["outcome"], "unavailable");
    assert_eq!(body["review"]["reviewed"], 1);
    assert_eq!(body["review"]["review_answered"], 0);
}

#[tokio::test]
async fn classify_review_strict_governs_when_the_review_does_not_answer() {
    // Under `strict`, an unanswered review replaces the selection: the
    // unit reports the review's failure with the primary's whole answer
    // kept under `original`.
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, _) = backend(honest(
        artifact('c'),
        json!({"model":"kev-0.6b", "answers":{"q0":{"type":"noul","probability":0.7}}}),
    ))
    .await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut call = review_call(review_policy("acme-kev-review", "uncertain", "strict"));
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let unit = &body["results"][0]["units"][0];
    assert_eq!(unit["outcome"], "unavailable", "{unit}");
    assert_eq!(unit["final_source"], "reviewer");
    assert_eq!(unit["original"]["selected"], "a");
    assert_eq!(unit["original"]["outcome"], "answered");
    assert_eq!(body["review"]["on_failure"], "strict");
}

#[tokio::test]
async fn classify_review_stops_at_the_declared_item_bound() {
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut policy = review_policy("acme-kev-review", "uncertain", "keep-original");
    policy["max_items"] = json!(1);
    let call = review_call(policy);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 1);
    assert_eq!(body["review"]["reviewed"], 1);
    // The bound's stop is recorded on the unit it stopped.
    let stopped: Vec<&Value> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["units"][0])
        .filter(|unit| unit["review"]["outcome"].as_str() == Some("unattempted"))
        .collect();
    assert_eq!(stopped.len(), 1, "{body}");
    assert!(
        stopped[0]["review"]["cause"]
            .as_str()
            .unwrap()
            .contains("max_items"),
        "{body}"
    );
    assert_eq!(stopped[0]["final_source"], "primary");
}

#[tokio::test]
async fn classify_review_stops_at_the_declared_latency_bound() {
    // A reviewer slower than the phase's own deadline: the first
    // dispatch times out inside it, the second never dispatches.
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let reviewer_stub = Backend {
        delay_ms: 400,
        ..honest(artifact('c'), corrected_answer())
    };
    let (reviewer, _) = backend(reviewer_stub).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut policy = review_policy("acme-kev-review", "uncertain", "keep-original");
    policy["latency_ms"] = json!(150);
    let call = review_call(policy);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let outcomes: Vec<&str> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["units"][0]["review"]["outcome"].as_str().unwrap())
        .collect();
    assert!(outcomes.contains(&"unavailable"), "{body}");
    assert!(outcomes.contains(&"unattempted"), "{body}");
    // The stopped unit's cause names the latency bound.
    let stopped = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["units"][0])
        .find(|unit| unit["review"]["outcome"].as_str() == Some("unattempted"))
        .unwrap();
    assert!(
        stopped["review"]["cause"]
            .as_str()
            .unwrap()
            .contains("latency_ms"),
        "{body}"
    );
    // Both units keep their primary selections.
    for item in body["results"].as_array().unwrap() {
        assert_eq!(item["units"][0]["selected"], "a");
        assert_eq!(item["units"][0]["final_source"], "primary");
    }
}

#[tokio::test]
async fn classify_fallback_retries_a_declared_transport_failure() {
    // The primary door 503s: the declared `transport` entry retries the
    // item through the fallback door, which answers.
    let (primary, _) = backend(Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        answer_body: json!({"error":"down"}),
        ..honest(artifact('b'), json!({}))
    })
    .await;
    let (reviewer, _) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, fallback_forwards) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut policy = review_policy("acme-kev-review", "no-match", "keep-original");
    policy["fallback"] = json!([{"on": "transport", "model": "acme-kev-fb"}]);
    let mut call = review_call(policy);
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    classification_response_fixture("fallback", &body);
    assert_eq!(fallback_forwards.load(Ordering::SeqCst), 1);
    let item = &body["results"][0];
    assert_eq!(item["outcome"], "answered", "{item}");
    assert_eq!(item["units"][0]["selected"], "a");
    // The primary's failure survives whole under `original`.
    assert_eq!(item["original"]["outcome"], "unavailable");
    assert_eq!(item["fallback"]["on"], "transport");
    assert_eq!(item["fallback"]["outcome"], "answered");
    assert_eq!(item["fallback"]["door"], "acme-kev-fb");
    let attempts = item["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["role"], "primary");
    assert_eq!(attempts[0]["outcome"], "unavailable");
    assert_eq!(attempts[1]["role"], "fallback");
    assert_eq!(body["review"]["fallback_dispatched"], 1);
    assert_eq!(body["review"]["fallback_answered"], 1);
    // Two dispatches, two receipts past the call's own.
    assert_eq!(receipt_log(&deployment.dir).len(), 2);
}

#[tokio::test]
async fn classify_fallback_never_retries_an_undeclared_cause() {
    // The primary 503s but the policy declares no `transport` entry:
    // the failure stands and the skipped fallback is recorded.
    let (primary, primary_forwards) = backend(Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        answer_body: json!({"error":"down"}),
        ..honest(artifact('b'), json!({}))
    })
    .await;
    let (reviewer, _) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, fallback_forwards) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let policy = review_policy("acme-kev-review", "no-match", "keep-original");
    let mut call = review_call(policy);
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(primary_forwards.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_forwards.load(Ordering::SeqCst), 0);
    let item = &body["results"][0];
    assert_eq!(item["outcome"], "unavailable");
    assert_eq!(item["fallback"]["outcome"], "skipped");
    assert_eq!(body["review"]["fallback_dispatched"], 0);
}

#[tokio::test]
async fn classify_fallback_retries_only_the_refusal_codes_it_declared() {
    // A typed refusal is an answer: the `refused` entry retries only
    // the codes it enumerates, and an unlisted cause is kept.
    let refused = |code: &str| Backend {
        answer_status: StatusCode::UNPROCESSABLE_ENTITY,
        answer_body: json!({"error": {"code": code}}),
        ..honest(artifact('b'), json!({}))
    };
    let (primary, _) = backend(refused("content_policy")).await;
    let (reviewer, _) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, fallback_forwards) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut policy = review_policy("acme-kev-review", "no-match", "keep-original");
    policy["fallback"] =
        json!([{"on": "refused", "model": "acme-kev-fb", "codes": ["quota_exceeded"]}]);
    let mut call = review_call(policy);
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(fallback_forwards.load(Ordering::SeqCst), 0);
    let item = &body["results"][0];
    assert_eq!(item["outcome"], "refused", "{item}");
    assert_eq!(item["cause"], "content_policy");
    assert_eq!(item["fallback"]["outcome"], "skipped");

    // The declared code retries and answers through the fallback door.
    let (primary, _) = backend(refused("quota_exceeded")).await;
    let (reviewer, _) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, fallback_forwards) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut policy = review_policy("acme-kev-review", "no-match", "keep-original");
    policy["fallback"] =
        json!([{"on": "refused", "model": "acme-kev-fb", "codes": ["quota_exceeded"]}]);
    let mut call = review_call(policy);
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(fallback_forwards.load(Ordering::SeqCst), 1);
    let item = &body["results"][0];
    assert_eq!(item["outcome"], "answered", "{item}");
    assert_eq!(item["original"]["outcome"], "refused");
    assert_eq!(item["original"]["cause"], "quota_exceeded");
    assert_eq!(item["fallback"]["outcome"], "answered");
}

#[tokio::test]
async fn classify_review_dispatches_only_doors_the_caller_is_bound_to() {
    // globex's call rides `shared-kev`, which its binding names; the
    // policy's reviewer is `acme-kev`, which it does not. The review
    // sub-dispatch is refused at its own authorization — the caller's
    // bindings bound every dispatch, not just the call's own.
    let (primary, _) = backend(honest(artifact('a'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let deployment = deploy_doors(
        review_manifest(),
        [
            ("shared-kev".into(), classify_door(primary, 1)),
            (
                "acme-kev".into(),
                Door {
                    endpoint: reviewer,
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            ),
        ]
        .into_iter()
        .collect(),
    )
    .await;
    let mut call = review_call(review_policy("acme-kev", "uncertain", "keep-original"));
    call["model"] = json!("shared-kev");
    call["capacity"] = json!("shared");
    call["inputs"] = json!([{"id":"one","text":"one"}]);
    let (status, body) =
        send_classification_as(&deployment, &call, Some(&deployment.tokens["globex"])).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 0);
    let unit = &body["results"][0]["units"][0];
    assert_eq!(unit["selected"], "a", "{unit}");
    assert_eq!(unit["final_source"], "primary");
    assert_eq!(unit["review"]["outcome"], "refused", "{unit}");
    let attempts = body["results"][0]["attempts"].as_array().unwrap();
    assert_eq!(attempts[1]["role"], "review");
    assert_eq!(attempts[1]["code"], "door_not_bound");
    assert_eq!(body["review"]["reviewed"], 1);
}

#[tokio::test]
async fn classify_review_refuses_an_unbound_reviewer_door() {
    // A reviewer the caller's bindings do not name: the sub-dispatch is
    // refused at authorization, recorded on the unit, and never reaches
    // a backend.
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let mut call = review_call(review_policy("acme-kev-fb", "uncertain", "keep-original"));
    // Point the policy at a door acme does not bind: `shared-kev` is
    // bound, so use a name nobody holds.
    call["policy"]["review"]["reviewer"] = json!("unbound-door");
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 0);
    for item in body["results"].as_array().unwrap() {
        let unit = &item["units"][0];
        assert_eq!(unit["selected"], "a", "{unit}");
        assert_eq!(unit["final_source"], "primary");
        assert_eq!(unit["review"]["outcome"], "refused", "{unit}");
        let attempts = item["attempts"].as_array().unwrap();
        assert_eq!(attempts[1]["role"], "review");
        assert_eq!(attempts[1]["outcome"], "refused");
        assert_eq!(attempts[1]["code"], "door_not_bound");
    }
    assert_eq!(body["review"]["reviewed"], 2);
    assert_eq!(body["review"]["review_answered"], 0);
    // Each refused sub-dispatch still leaves its own receipt.
    assert_eq!(receipt_log(&deployment.dir).len(), 3);
}

#[tokio::test]
async fn classify_review_policy_rejects_invalid_declarations() {
    let (endpoint, _) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    for review in [
        // Wrong schema tag.
        json!({"v":"openagents.classify-review.v0","reviewer":"acme-kev-review",
               "trigger":"uncertain","on_failure":"keep-original",
               "max_items":1,"max_attempts":1,"latency_ms":1}),
        // A zero bound admits no work.
        json!({"v":"openagents.classify-review.v1","reviewer":"acme-kev-review",
               "trigger":"uncertain","on_failure":"keep-original",
               "max_items":0,"max_attempts":1,"latency_ms":1}),
        // A `refused` fallback without the codes it may carry.
        json!({"v":"openagents.classify-review.v1","reviewer":"acme-kev-review",
               "trigger":"uncertain","on_failure":"keep-original",
               "max_items":1,"max_attempts":1,"latency_ms":1,
               "fallback":[{"on":"refused","model":"acme-kev-fb"}]}),
        // `codes` on a non-refusal entry.
        json!({"v":"openagents.classify-review.v1","reviewer":"acme-kev-review",
               "trigger":"uncertain","on_failure":"keep-original",
               "max_items":1,"max_attempts":1,"latency_ms":1,
               "fallback":[{"on":"transport","model":"acme-kev-fb","codes":["x"]}]}),
    ] {
        let call = review_call(review);
        let (status, body) = send_classification(&deployment, &call).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["error"]["code"], "invalid_request", "{body}");
    }
}

#[tokio::test]
async fn classify_review_holds_money_for_every_attempted_dispatch() {
    // Under monetary admission each secondary dispatch takes its own
    // hold and settles its own usage — the workspace balance reflects
    // every charged call.
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, _) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let mut doors = BTreeMap::new();
    doors.insert("acme-kev".to_string(), classify_door(primary, 1));
    for (door, endpoint) in [("acme-kev-review", reviewer), ("acme-kev-fb", fallback)] {
        doors.insert(
            door.to_string(),
            Door {
                endpoint,
                classify: None,
                classify_item_concurrency: 1,
                batching: None,
            },
        );
    }
    let priced: BTreeMap<String, Priced> = [
        ("acme-kev".into(), fixture_priced()),
        ("acme-kev-review".into(), fixture_priced()),
        ("acme-kev-fb".into(), fixture_priced()),
    ]
    .into_iter()
    .collect();
    let deployment = deploy_money(review_manifest(), doors, priced, |ledger, workspace| {
        provision_account(ledger, workspace, Some(8 * HOLD));
    })
    .await;
    let token = deployment.deployment.tokens["acme"].clone();
    let call = review_call(review_policy(
        "acme-kev-review",
        "uncertain",
        "keep-original",
    ));
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.deployment.address))
        .bearer_auth(&token)
        .header("x-workspace-id", &deployment.workspace)
        .json(&call)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-settlement"], "settled");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["review"]["reviewed"], 2, "{body}");
    // Each review attempt settled its own hold — recorded on the
    // attempt row itself.
    for item in body["results"].as_array().unwrap() {
        let attempts = item["attempts"].as_array().unwrap();
        assert_eq!(attempts[1]["settlement"], "settled", "{item}");
        assert!(attempts[1]["usage_ref"].is_string());
    }
    // The summary's reserved spend counts every secondary hold's
    // worst case.
    assert_eq!(body["review"]["reserved_spend"], 2 * HOLD);
    let (_, balance) = get_balance(&deployment, Some(&token), Some(&deployment.workspace)).await;
    // The primary call plus two review dispatches, each settling the
    // usage its door reported.
    assert_eq!(balance["balance"]["reserved"], 0);
    assert_eq!(balance["balance"]["settled"], 2 * CHARGE + 2 * 120);
}

#[tokio::test]
async fn classify_strict_review_never_retains_an_unconfirmed_selection_at_a_bound() {
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    for bound in ["max_items", "max_attempts"] {
        let mut policy = review_policy("acme-kev-review", "uncertain", "strict");
        policy[bound] = json!(1);
        let before = reviewer_forwards.load(Ordering::SeqCst);
        let (status, body) = send_classification(&deployment, &review_call(policy)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["outcome"], "mixed");
        assert_eq!(reviewer_forwards.load(Ordering::SeqCst) - before, 1);
        let unit = &body["results"][1]["units"][0];
        assert_eq!(unit["outcome"], "unattempted", "{body}");
        assert!(unit["selected"].is_null());
        assert!(unit.get("raw").is_none());
        assert_eq!(unit["original"]["selected"], "a");
        assert!(unit["review"]["cause"].as_str().unwrap().contains(bound));
        assert_eq!(body["aggregates"][0]["outcomes"]["unattempted"], 1);
    }
}

#[tokio::test]
async fn classify_review_requires_known_pricing_to_enforce_a_spending_bound() {
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, reviewer_forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    for failure in ["keep-original", "strict"] {
        let mut policy = review_policy("acme-kev-review", "uncertain", failure);
        policy["max_spend"] = json!(100);
        let (_, body) = send_classification(&deployment, &review_call(policy)).await;
        assert_eq!(reviewer_forwards.load(Ordering::SeqCst), 0);
        assert_eq!(body["review"]["attempts"], 0);
        for item in body["results"].as_array().unwrap() {
            let unit = &item["units"][0];
            assert_eq!(unit["review"]["outcome"], "unattempted");
            assert!(
                unit["review"]["cause"]
                    .as_str()
                    .unwrap()
                    .contains("known configured price")
            );
            if failure == "strict" {
                assert!(unit["selected"].is_null());
                assert_eq!(unit["original"]["selected"], "a");
            } else {
                assert_eq!(unit["selected"], "a");
            }
        }
    }
}

#[tokio::test]
async fn classify_review_attempts_reference_receipts_that_bind_the_actual_response() {
    let answer = corrected_answer();
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let (reviewer, _) = backend(honest(artifact('c'), answer.clone())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let (_, body) = send_classification(
        &deployment,
        &review_call(review_policy("acme-kev-review", "uncertain", "strict")),
    )
    .await;
    let receipts = receipt_log(&deployment.dir);
    for item in body["results"].as_array().unwrap() {
        let attempt = &item["attempts"][1];
        let reference = attempt["receipt"].as_str().unwrap();
        let receipt = receipts
            .iter()
            .find(|receipt| receipt.digest == reference)
            .unwrap();
        use sha2::Digest;
        let expected = format!(
            "sha256:{:x}",
            sha2::Sha256::digest(serde_json::to_vec(&answer).unwrap())
        );
        assert_eq!(receipt.result_digest.as_deref(), Some(expected.as_str()));
        assert_eq!(attempt["served"], json!(receipt.served));
        assert_eq!(attempt["served"]["artifact_signature"], artifact('c'));
    }
}

#[tokio::test]
async fn classify_review_rechecks_a_key_revoked_after_primary_admission() {
    let mutation = Arc::new(Mutex::new(None::<(std::path::PathBuf, String)>));
    let pending = mutation.clone();
    let mut primary = honest(artifact('b'), uncertain_answer());
    primary.respond = Some(Arc::new(move |_| {
        if let Some((path, key)) = pending.lock().unwrap().take() {
            keys::revoke(&path, &key).unwrap();
        }
        (StatusCode::OK, uncertain_answer())
    }));
    let (primary, _) = backend(primary).await;
    let (reviewer, forwards) = backend(honest(artifact('c'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    let token = &deployment.tokens["acme"];
    let key = token
        .strip_prefix("oak_")
        .unwrap()
        .split_once('.')
        .unwrap()
        .0;
    *mutation.lock().unwrap() = Some((deployment.dir.path().to_path_buf(), key.to_string()));
    let (_, body) = send_classification(
        &deployment,
        &review_call(review_policy("acme-kev-review", "uncertain", "strict")),
    )
    .await;
    assert_eq!(forwards.load(Ordering::SeqCst), 0, "{body}");
    for item in body["results"].as_array().unwrap() {
        assert!(item["units"][0]["selected"].is_null(), "{body}");
        assert_eq!(item["units"][0]["original"]["selected"], "a", "{body}");
        assert!(
            item["attempts"].to_string().contains("unauthenticated"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn classify_review_refuses_an_artifact_rebound_after_primary_admission() {
    let mutation = Arc::new(Mutex::new(None::<std::path::PathBuf>));
    let pending = mutation.clone();
    let mut primary = honest(artifact('b'), uncertain_answer());
    primary.respond = Some(Arc::new(move |_| {
        if let Some(path) = pending.lock().unwrap().take() {
            let mut updated = review_manifest();
            updated.sequence = 1;
            updated.supersedes = Some(Registry::open(&path).unwrap().digest().to_string());
            updated
                .tenants
                .get_mut("acme")
                .unwrap()
                .doors
                .get_mut("acme-kev-review")
                .unwrap()
                .artifact
                .artifact_signature = artifact('e');
            Registry::update(&path, updated).unwrap();
        }
        (StatusCode::OK, uncertain_answer())
    }));
    let (primary, _) = backend(primary).await;
    let (reviewer, forwards) = backend(honest(artifact('e'), corrected_answer())).await;
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, reviewer, fallback).await;
    *mutation.lock().unwrap() = Some(deployment.dir.path().to_path_buf());
    let (_, body) = send_classification(
        &deployment,
        &review_call(review_policy("acme-kev-review", "uncertain", "strict")),
    )
    .await;
    assert_eq!(forwards.load(Ordering::SeqCst), 0, "{body}");
    for item in body["results"].as_array().unwrap() {
        assert!(item["units"][0]["selected"].is_null(), "{body}");
        assert_eq!(item["units"][0]["original"]["selected"], "a", "{body}");
        assert!(
            item["attempts"].to_string().contains("identity_mismatch"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn classify_review_bounds_identity_reads_before_inference() {
    let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
    let forwards = Arc::new(AtomicUsize::new(0));
    let observed = forwards.clone();
    let router = axum::Router::new()
        .route(
            "/v1/models",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                Json(json!({"models": [{"id": "kev-0.6b",
                "artifact_identity": {"digest": artifact('c')}}]}))
            }),
        )
        .route(
            "/v1/systemone",
            post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async { Json(corrected_answer()) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    let (fallback, _) = backend(honest(artifact('d'), choice_answer())).await;
    let deployment = review_deployment(primary, format!("http://{address}"), fallback).await;
    let mut policy = review_policy("acme-kev-review", "uncertain", "strict");
    policy["latency_ms"] = json!(100);
    let (_, body) = send_classification(&deployment, &review_call(policy)).await;
    assert_eq!(forwards.load(Ordering::SeqCst), 0, "{body}");
    assert!(
        body["results"][0]["attempts"]
            .to_string()
            .contains("during identity verification"),
        "{body}"
    );
    for item in body["results"].as_array().unwrap() {
        assert!(item["units"][0]["selected"].is_null(), "{body}");
        assert_eq!(item["units"][0]["original"]["selected"], "a", "{body}");
    }
}

#[tokio::test]
async fn classify_public_failure_fixtures_match_runtime() {
    for (name, status, answer) in [
        (
            "refusal",
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error":{"code":"unsupported_primitive","message":"fixture refusal"}}),
        ),
        (
            "unavailable",
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":{"code":"busy","message":"fixture saturation"}}),
        ),
    ] {
        let stub = Backend {
            answer_status: status,
            ..honest(artifact('b'), answer)
        };
        let (endpoint, forwards) = backend(stub).await;
        let deployment = classification_deployment(endpoint).await;
        let mut call = classify_call();
        call["inputs"].as_array_mut().unwrap().truncate(1);
        let (actual_status, body) = send_classification(&deployment, &call).await;
        assert_eq!(actual_status, status, "{body}");
        assert_eq!(forwards.load(Ordering::SeqCst), 1);
        classification_response_fixture(name, &body);
    }
}

#[tokio::test]
async fn backend_redirects_never_change_the_authorized_destination() {
    for redirect_identity in [true, false] {
        for classification in [true, false] {
            let reached = Arc::new(AtomicUsize::new(0));
            let identity_reached = reached.clone();
            let inference_reached = reached.clone();
            let destination = axum::Router::new()
                .route(
                    "/v1/models",
                    get(move || {
                        identity_reached.fetch_add(1, Ordering::SeqCst);
                        async {
                            Json(json!({"models":[{"id":"kev-0.6b",
                        "artifact_identity":{"digest":artifact('b')}}]}))
                        }
                    }),
                )
                .route(
                    "/v1/systemone",
                    post(move || {
                        inference_reached.fetch_add(1, Ordering::SeqCst);
                        async { Json(choice_answer()) }
                    }),
                );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let destination_url = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(axum::serve(listener, destination).into_future());
            let models_url = format!("{destination_url}/v1/models");
            let inference_url = format!("{destination_url}/v1/systemone");
            let source = axum::Router::new()
                .route(
                    "/v1/models",
                    get(move || {
                        let location = models_url.clone();
                        async move {
                            if redirect_identity {
                                (StatusCode::TEMPORARY_REDIRECT, [("location", location)])
                                    .into_response()
                            } else {
                                Json(json!({"models":[{"id":"kev-0.6b",
                                "artifact_identity":{"digest":artifact('b')}}]}))
                                .into_response()
                            }
                        }
                    }),
                )
                .route(
                    "/v1/systemone",
                    post(move || {
                        let location = inference_url.clone();
                        async move { (StatusCode::TEMPORARY_REDIRECT, [("location", location)]) }
                    }),
                );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let source_url = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(axum::serve(listener, source).into_future());
            let deployment = classification_deployment(source_url).await;
            let status = if classification {
                send_classification(&deployment, &classify_call()).await.0
            } else {
                send_call(
                    &deployment,
                    &call("acme-kev"),
                    Some(&deployment.tokens["acme"]),
                )
                .await
                .status()
            };
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(reached.load(Ordering::SeqCst), 0);
            let receipts = receipt_log(&deployment.dir);
            assert_eq!(receipts.len(), 1);
            assert_eq!(
                receipts[0].outcome,
                if redirect_identity {
                    receipts::execution::Outcome::Unattempted
                } else {
                    receipts::execution::Outcome::Unavailable
                }
            );
        }
    }
}

#[tokio::test]
async fn oversized_identity_bodies_refuse_before_inference_or_read_to_end() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for chunked in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 8192);
            }
            assert!(request.starts_with(b"GET /v1/models"));
            if chunked {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n401\r\n")
                    .await
                    .unwrap();
                socket.write_all(&vec![b'x'; 1025]).await.unwrap();
            } else {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1025\r\n\r\n")
                    .await
                    .unwrap();
            }
            // Neither response completes: a read-to-end implementation times out.
            std::future::pending::<()>().await;
        });
        let deployment = classification_deployment_tuned(endpoint, 1, |config| {
            config.max_response_bytes = 128;
            config.forward_timeout_ms = 5000;
        })
        .await;
        let (status, body) = tokio::time::timeout(
            Duration::from_secs(2),
            send_classification(&deployment, &classify_call()),
        )
        .await
        .expect("the byte cap must refuse before the five-second read deadline");
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert!(body.to_string().contains("exceeded 128 bytes"), "{body}");
        let receipts = receipt_log(&deployment.dir);
        assert_eq!(receipts.len(), 1);
        assert_eq!(
            receipts[0].outcome,
            receipts::execution::Outcome::Unattempted
        );
        server.abort();
    }
}

#[tokio::test]
async fn disconnected_inference_retains_a_bounded_settlement() {
    use tokio::io::AsyncWriteExt;
    let fixture: Value = serde_json::from_slice(include_bytes!(
        "../../../docs/decision-models/fixtures/classify-v1/runtime-disconnect.json"
    ))
    .unwrap();
    assert_eq!(fixture["v"], "openagents.classify-runtime-fixture.v1");
    for classification in [true, false] {
        let stub = Backend {
            delay_ms: fixture["backend"]["response_delay_ms"].as_u64().unwrap(),
            ..honest(artifact('b'), choice_answer())
        };
        let (endpoint, forwards) = backend(stub).await;
        let deployment = classification_deployment_tuned(
            endpoint,
            fixture["backend"]["item_concurrency"].as_u64().unwrap(),
            |config| {
                config.forward_timeout_ms =
                    fixture["backend"]["gateway_deadline_ms"].as_u64().unwrap();
            },
        )
        .await;
        let body = serde_json::to_vec(&if classification {
            fixture["request"].clone()
        } else {
            call("acme-kev")
        })
        .unwrap();
        let route = if classification {
            "classify"
        } else {
            "systemone"
        };
        let address = deployment.address.strip_prefix("http://").unwrap();
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let header = format!(
            "POST /v1/{route} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nIdempotency-Key: disconnected-fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            deployment.tokens["acme"],
            body.len()
        );
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while forwards.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the first forward must start");
        drop(socket);
        let receipts = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let receipts = receipt_log(&deployment.dir);
                if !receipts.is_empty() {
                    break receipts;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("caller disconnection must not lose terminal settlement evidence");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].request, "disconnected-fixture");
        assert_eq!(
            receipts[0].outcome,
            receipts::execution::Outcome::Unavailable
        );
        assert_eq!(
            json!(receipts[0].cause),
            fixture["expected"]["receipt_cause"]
        );
        assert_eq!(
            json!(receipts[0].outcome),
            fixture["expected"]["receipt_outcome"]
        );
        assert_eq!(
            json!(forwards.load(Ordering::SeqCst)),
            fixture["expected"]["backend_forwards"]
        );
        let ledger =
            std::fs::read_to_string(deployment.dir.path().join("quota-ledger.jsonl")).unwrap();
        let settlements: Vec<Value> = ledger
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["event"] == "settled")
            .collect();
        assert_eq!(settlements.len(), 1);
        assert_eq!(
            settlements[0]["units"]["questions"],
            fixture["expected"]["settled_questions"]
        );
    }
}

/// Open a request whose caller can disconnect at a chosen backend boundary.
async fn disconnect_socket(
    deployment: &Deployment,
    route: &str,
    body: &Value,
    workspace: Option<&str>,
) -> tokio::net::TcpStream {
    use tokio::io::AsyncWriteExt;
    let bytes = serde_json::to_vec(body).unwrap();
    let address = deployment.address.strip_prefix("http://").unwrap();
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let workspace = workspace.map_or(String::new(), |id| format!("X-Workspace-Id: {id}\r\n"));
    let header = format!(
        "POST /v1/{route} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\n{workspace}Idempotency-Key: disconnect-boundary\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        deployment.tokens["acme"],
        bytes.len()
    );
    socket.write_all(header.as_bytes()).await.unwrap();
    socket.write_all(&bytes).await.unwrap();
    socket
}

async fn wait_for_dispatch(counter: &AtomicUsize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while counter.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the selected backend boundary must be reached");
}

/// Wait for the sealed parent receipt, allowing an append to be in progress.
async fn wait_for_disconnect_receipts(
    deployment: &Deployment,
) -> Vec<receipts::execution::ExecutionReceipt> {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let text = std::fs::read_to_string(deployment.dir.path().join("receipts.jsonl"))
                .unwrap_or_default();
            if text.ends_with('\n') {
                let receipts = text
                    .lines()
                    .map(receipts::execution::ExecutionReceipt::parse)
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(receipts) = receipts
                    && receipts
                        .iter()
                        .any(|receipt| receipt.request == "disconnect-boundary")
                {
                    break receipts;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("disconnection must retain terminal receipt evidence")
}

/// Hold identity verification open without ever requiring inference.
async fn slow_identity_backend() -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let forwards = Arc::new(AtomicUsize::new(0));
    let read = reads.clone();
    let forward = forwards.clone();
    let router = axum::Router::new()
        .route("/v1/models", get(move || {
            read.fetch_add(1, Ordering::SeqCst);
            async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Json(json!({"models":[{"id":"kev-0.6b","artifact_identity":{"digest":artifact('b')}}]}))
            }
        }))
        .route("/v1/systemone", post(move || {
            forward.fetch_add(1, Ordering::SeqCst);
            async { Json(choice_answer()) }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, router).into_future());
    (endpoint, reads, forwards)
}

#[tokio::test]
async fn disconnect_releases_only_provably_undispatched_monetary_holds() {
    for before_inference in [true, false] {
        for classification in [true, false] {
            let (endpoint, observed, forwards) = if before_inference {
                slow_identity_backend().await
            } else {
                let (endpoint, forwards) = backend(Backend {
                    delay_ms: 10_000,
                    ..honest(artifact('b'), choice_answer())
                })
                .await;
                (endpoint, forwards.clone(), forwards)
            };
            let (mut doors, priced) = money_doors(endpoint);
            doors.get_mut("acme-kev").unwrap().classify =
                Some(gateway::classify::BackendLimits::product());
            let deployment = deploy_money(manifest(None), doors, priced, |ledger, workspace| {
                provision_account(ledger, workspace, Some(2 * HOLD));
            })
            .await;
            let (route, body) = if classification {
                ("classify", classify_batch(3))
            } else {
                ("systemone", call("acme-kev"))
            };
            let socket = disconnect_socket(
                &deployment.deployment,
                route,
                &body,
                Some(&deployment.workspace),
            )
            .await;
            wait_for_dispatch(&observed).await;
            drop(socket);
            let receipts = wait_for_disconnect_receipts(&deployment.deployment).await;
            assert_eq!(receipts.len(), 1);
            assert_eq!(
                forwards.load(Ordering::SeqCst),
                usize::from(!before_inference)
            );
            assert_eq!(
                receipts[0].outcome,
                if before_inference {
                    receipts::execution::Outcome::Unattempted
                } else {
                    receipts::execution::Outcome::Unavailable
                }
            );
            let (_, balance) = get_balance(
                &deployment,
                Some(&deployment.deployment.tokens["acme"]),
                Some(&deployment.workspace),
            )
            .await;
            assert_eq!(balance["balance"]["settled"], 0, "{balance}");
            assert_eq!(
                balance["balance"]["reserved"],
                if before_inference { 0 } else { HOLD },
                "{balance}"
            );
            assert_eq!(
                balance["balance"]["available"],
                if before_inference { 2 * HOLD } else { HOLD },
                "{balance}"
            );
        }
    }
}

#[tokio::test]
async fn disconnect_during_review_keeps_its_hold_and_stops_remaining_reviews() {
    for before_inference in [true, false] {
        let (primary, _) = backend(honest(artifact('b'), uncertain_answer())).await;
        let (reviewer, observed, forwards) = if before_inference {
            slow_identity_backend().await
        } else {
            let (reviewer, forwards) = backend(Backend {
                delay_ms: 10_000,
                ..honest(artifact('c'), corrected_answer())
            })
            .await;
            (reviewer, forwards.clone(), forwards)
        };
        let doors = [
            ("acme-kev".into(), classify_door(primary, 1)),
            (
                "acme-kev-review".into(),
                Door {
                    endpoint: reviewer,
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            ),
        ]
        .into_iter()
        .collect();
        let priced = [
            ("acme-kev".into(), fixture_priced()),
            ("acme-kev-review".into(), fixture_priced()),
        ]
        .into_iter()
        .collect();
        let deployment = deploy_money(review_manifest(), doors, priced, |ledger, workspace| {
            provision_account(ledger, workspace, Some(8 * HOLD));
        })
        .await;
        let socket = disconnect_socket(
            &deployment.deployment,
            "classify",
            &review_call(review_policy("acme-kev-review", "uncertain", "strict")),
            Some(&deployment.workspace),
        )
        .await;
        wait_for_dispatch(&observed).await;
        drop(socket);
        let receipts = wait_for_disconnect_receipts(&deployment.deployment).await;
        assert_eq!(
            forwards.load(Ordering::SeqCst),
            usize::from(!before_inference)
        );
        assert_eq!(receipts.len(), 2);
        for receipt in &receipts {
            let expected = if before_inference && receipt.request != "disconnect-boundary" {
                receipts::execution::Outcome::Unattempted
            } else {
                receipts::execution::Outcome::Unavailable
            };
            assert_eq!(receipt.outcome, expected);
        }
        let (_, balance) = get_balance(
            &deployment,
            Some(&deployment.deployment.tokens["acme"]),
            Some(&deployment.workspace),
        )
        .await;
        assert_eq!(
            balance["balance"]["reserved"],
            if before_inference { 0 } else { HOLD },
            "{balance}"
        );
        assert_eq!(balance["balance"]["settled"], 2 * CHARGE, "{balance}");
        assert_eq!(
            balance["balance"]["available"],
            if before_inference {
                8 * HOLD - 2 * CHARGE
            } else {
                7 * HOLD - 2 * CHARGE
            },
            "{balance}"
        );
    }
}

#[tokio::test]
async fn classify_rejects_expanded_context_before_dispatch_or_reservation() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |config| {
        config
            .doors
            .get_mut("acme-kev")
            .unwrap()
            .classify
            .as_mut()
            .unwrap()
            .max_forward_bytes = 128;
    })
    .await;
    assert!(!deployment.dir.path().join("quota-ledger.jsonl").exists());
    let (status, body) = send_classification(&deployment, &classify_call()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "context_limit");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    assert!(!deployment.dir.path().join("quota-ledger.jsonl").exists());
}

#[tokio::test]
async fn classify_checks_later_context_before_forwarding_any_input() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |config| {
        config
            .doors
            .get_mut("acme-kev")
            .unwrap()
            .classify
            .as_mut()
            .unwrap()
            .max_forward_bytes = 4096;
    })
    .await;
    let mut call = classify_call();
    call["inputs"][1]["text"] = json!("界".repeat(1400));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "context_limit");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    assert!(!deployment.dir.path().join("quota-ledger.jsonl").exists());
    let (status, body) = send_classification(&deployment, &classify_call()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
}

// ---------- durable classification jobs ----------

/// Wrap a classify request in the job envelope.
fn job_envelope(request: Value) -> Value {
    json!({
        "v": "openagents.job.v1",
        "kind": "classify",
        "request": request,
    })
}

/// POST a job under a credential and idempotency key.
async fn submit_job(
    deployment: &Deployment,
    envelope: &Value,
    key: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/jobs", deployment.address))
        .header("idempotency-key", key)
        .json(envelope);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

/// GET a job's status under a credential.
async fn get_job(deployment: &Deployment, id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{}/v1/jobs/{id}", deployment.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

/// POST a cancel under a credential.
async fn cancel_job(deployment: &Deployment, id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request =
        reqwest::Client::new().post(format!("{}/v1/jobs/{id}/cancel", deployment.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

/// Poll a job's status until it reports a terminal state.
async fn poll_terminal(deployment: &Deployment, id: &str, token: Option<&str>) -> Value {
    for _ in 0..200 {
        let (_status, body) = get_job(deployment, id, token).await;
        if matches!(
            body["status"].as_str(),
            Some("completed" | "cancelled" | "failed")
        ) {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} never reached a terminal state");
}

/// GET a job's results page.
async fn get_results(
    deployment: &Deployment,
    id: &str,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!(
        "{}/v1/jobs/{id}/results{query}",
        deployment.address
    ));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.json().await.unwrap())
}

#[tokio::test]
async fn a_job_runs_to_completion_and_exports_its_items() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(3)),
        "job-run",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "queued");
    assert_eq!(body["counts"]["expected"], 3);
    let job = body["job"].as_str().unwrap().to_string();
    assert!(job.starts_with("job_"), "{job}");

    let terminal = poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(terminal["status"], "completed", "{terminal}");
    assert_eq!(terminal["counts"]["attempted"], 3);
    assert_eq!(terminal["counts"]["answered"], 3);
    assert_eq!(terminal["counts"]["unknown"], 0);
    assert!(
        terminal["receipt"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")),
        "{terminal}"
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 3);

    // The export names every item's input, index, attempt, and outcome.
    let (status, page) = get_results(&deployment, &job, "", Some(&deployment.tokens["acme"])).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["v"], "openagents.job-results.v1");
    assert_eq!(page["terminal"], "completed");
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    let mut indexes: Vec<u64> = items
        .iter()
        .map(|item| item["index"].as_u64().unwrap())
        .collect();
    indexes.sort_unstable();
    assert_eq!(indexes, vec![0, 1, 2]);
    for item in items {
        assert!(item["dispatched"].as_bool().unwrap());
        assert_eq!(item["item"]["outcome"], "answered");
        assert_eq!(item["item"]["units"][0]["selected"], "a");
        assert!(
            item["attempt_id"]
                .as_str()
                .unwrap()
                .starts_with(&format!("{job}-1-")),
            "{item}"
        );
    }
    assert!(page["next_cursor"].is_null(), "{page}");

    // The run's receipt is sealed in the same log every attempt uses.
    let receipts = receipt_log(&deployment.dir);
    let receipt = receipts
        .iter()
        .find(|receipt| receipt.request == job)
        .expect("a receipt under the job's identity");
    assert_eq!(receipt.attempt, 1);
    assert!(receipt.verify().is_ok());
}

#[tokio::test]
async fn an_identical_resubmission_returns_the_same_job() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let envelope = job_envelope(classify_batch(2));
    let (status, first) = submit_job(
        &deployment,
        &envelope,
        "same-key",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{first}");
    let job = first["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    let (status, second) = submit_job(
        &deployment,
        &envelope,
        "same-key",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["job"], job);
    assert_eq!(second["status"], "completed");
    // One execution, not two.
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_changed_payload_under_the_same_key_conflicts() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let (status, first) = submit_job(
        &deployment,
        &job_envelope(classify_batch(2)),
        "contended",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{first}");
    let (status, conflict) = submit_job(
        &deployment,
        &job_envelope(classify_batch(3)),
        "contended",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["error"]["code"], "idempotency_conflict");
}

#[tokio::test]
async fn a_rejected_submission_persists_nothing() {
    let (endpoint, forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    // Globex binds no dedicated door — the same refusal /v1/classify gives.
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(2)),
        "refused",
        Some(&deployment.tokens["globex"]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(!deployment.dir.path().join("jobs").exists());
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancel_records_then_stops_undispatched_work() {
    let stub = Backend {
        delay_ms: 400,
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |_| {}).await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(4)),
        "to-cancel",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();

    let (status, cancelling) =
        cancel_job(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert!(
        status == StatusCode::ACCEPTED || status == StatusCode::OK,
        "{cancelling}"
    );
    let terminal = poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(terminal["status"], "cancelled", "{terminal}");
    // Undispatched inputs are unattempted, never answered or vanished.
    assert!(
        terminal["counts"]["unattempted"].as_u64().unwrap() > 0,
        "{terminal}"
    );
    assert_eq!(terminal["counts"]["expected"], 4);
    let dispatched = forwards.load(Ordering::SeqCst);
    assert!(dispatched < 4, "{dispatched}");

    // A second cancel on a terminal job is the same status, not an error.
    let (status, again) = cancel_job(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["status"], "cancelled");
}

#[tokio::test]
async fn a_job_is_invisible_to_other_tenants() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(1)),
        "acme-job",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    for (status, body) in [
        get_job(&deployment, &job, Some(&deployment.tokens["globex"])).await,
        cancel_job(&deployment, &job, Some(&deployment.tokens["globex"])).await,
        get_results(&deployment, &job, "", Some(&deployment.tokens["globex"])).await,
    ] {
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "job_not_found");
    }
}

#[tokio::test]
async fn anonymous_jobs_are_bounded_to_anonymous_callers() {
    let (endpoint, _forwards) = backend(honest(artifact('a'), choice_answer())).await;
    let deployment = deploy_doors(
        manifest(None),
        [("shared-kev".to_string(), classify_door(endpoint, 1))]
            .into_iter()
            .collect(),
    )
    .await;
    let mut shared = classify_batch(1);
    shared["model"] = json!("shared-kev");
    shared["capacity"] = json!("shared");
    let (status, body) = submit_job(&deployment, &job_envelope(shared), "anon", None).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    let terminal = poll_terminal(&deployment, &job, None).await;
    assert_eq!(terminal["status"], "completed", "{terminal}");
    // Another tenant's key does not see the anonymous job.
    let (status, body) = get_job(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn results_paginate_by_cursor() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(5)),
        "paged",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    let mut seen = Vec::new();
    let mut cursor = String::new();
    for _ in 0..4 {
        let (status, page) = get_results(
            &deployment,
            &job,
            &format!("?limit=2{cursor}"),
            Some(&deployment.tokens["acme"]),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        seen.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["index"].as_u64().unwrap())
                .collect::<Vec<u64>>(),
        );
        match page["next_cursor"].as_str() {
            Some(next) => cursor = format!("&cursor={next}"),
            None => break,
        }
    }
    seen.sort_unstable();
    assert_eq!(seen, vec![0, 1, 2, 3, 4]);
}

#[tokio::test]
async fn an_expired_cursor_is_gone_not_rewound() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |config| {
        config.job_cursor_ttl_ms = 100;
    })
    .await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(3)),
        "expiring",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    let (status, page) = get_results(
        &deployment,
        &job,
        "?limit=1",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let cursor = page["next_cursor"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let (status, expired) = get_results(
        &deployment,
        &job,
        &format!("?cursor={cursor}"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{expired}");
    assert_eq!(expired["error"]["code"], "cursor_expired");
}

#[tokio::test]
async fn a_terminal_job_deletes_and_a_running_one_refuses() {
    let stub = Backend {
        delay_ms: 300,
        ..honest(artifact('b'), choice_answer())
    };
    let (endpoint, _forwards) = backend(stub).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |_| {}).await;
    // A running job refuses deletion.
    let (status, running) = submit_job(
        &deployment,
        &job_envelope(classify_batch(4)),
        "running-delete",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{running}");
    let running_job = running["job"].as_str().unwrap().to_string();
    let response = reqwest::Client::new()
        .delete(format!("{}/v1/jobs/{running_job}", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "job_running"
    );
    let terminal = poll_terminal(&deployment, &running_job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(terminal["status"], "completed");

    // The terminal job deletes; reads and replays then say so honestly.
    let response = reqwest::Client::new()
        .delete(format!("{}/v1/jobs/{running_job}", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (status, gone) = get_job(&deployment, &running_job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{gone}");
    let (status, replay) = submit_job(
        &deployment,
        &job_envelope(classify_batch(4)),
        "running-delete",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{replay}");
    assert_eq!(replay["error"]["code"], "job_deleted");
}

#[tokio::test]
async fn retention_sweeps_expired_terminal_jobs() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment_tuned(endpoint, 1, |config| {
        config.job_retention_ms = 1;
    })
    .await;
    let (status, first) = submit_job(
        &deployment,
        &job_envelope(classify_batch(1)),
        "swept",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{first}");
    let swept = first["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &swept, Some(&deployment.tokens["acme"])).await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    // The next submission's sweep removes the expired record.
    let (status, _second) = submit_job(
        &deployment,
        &job_envelope(classify_batch(1)),
        "sweeper",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, gone) = get_job(&deployment, &swept, Some(&deployment.tokens["acme"])).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{gone}");
}

#[tokio::test]
async fn the_webhook_delivers_a_signed_terminal_event() {
    // The receiver records every delivery — headers included — so the
    // test verifies the signature itself.
    let received = Arc::new(Mutex::new(Vec::<(String, String, Value)>::new()));
    let captured = received.clone();
    let app = axum::Router::new().route(
        "/hook",
        post(move |headers: HeaderMap, body: Bytes| {
            let captured = captured.clone();
            async move {
                captured.lock().unwrap().push((
                    headers
                        .get("x-openagents-event")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    headers
                        .get("x-openagents-signature")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    serde_json::from_slice::<Value>(&body).unwrap_or_default(),
                ));
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hook = format!("http://{}/hook", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());

    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let mut envelope = job_envelope(classify_batch(2));
    envelope["notify"] = json!({"url": hook, "secret": "testsecret"});
    let (status, body) = submit_job(
        &deployment,
        &envelope,
        "hooked",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    // A caller-supplied secret is never echoed.
    assert!(body["secret"].is_null(), "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    for _ in 0..100 {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let deliveries = received.lock().unwrap();
    assert_eq!(deliveries.len(), 1, "{deliveries:?}");
    let (event_id, signature, event) = &deliveries[0];
    assert_eq!(event["v"], "openagents.job-event.v1");
    assert_eq!(event["type"], "job.completed");
    assert_eq!(event["job"], job);
    assert_eq!(event["counts"]["answered"], 2);
    // The signature is the hmac the receiver can recompute.
    let body_bytes = serde_json::to_vec(event).unwrap();
    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(b"testsecret").unwrap();
    mac.update(event_id.as_bytes());
    mac.update(b".");
    mac.update(&body_bytes);
    let expected = format!(
        "sha256={}",
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(*signature, expected);
    drop(deliveries);

    // Every attempt is recorded — one here, delivered.
    let log = std::fs::read_to_string(
        deployment
            .dir
            .path()
            .join("jobs")
            .join(&job)
            .join("deliveries.jsonl"),
    )
    .unwrap();
    let lines: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["outcome"], "delivered");
    assert_eq!(lines[0]["attempt"], 1);
}

#[tokio::test]
async fn a_failed_delivery_retries_then_completes() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let app = axum::Router::new().route(
        "/hook",
        post(move || {
            let counter = counter.clone();
            async move {
                // The first delivery fails; the retry carries the same
                // event id and completes.
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::OK
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hook = format!("http://{}/hook", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());

    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let mut envelope = job_envelope(classify_batch(1));
    envelope["notify"] = json!({"url": hook, "secret": "s"});
    let (status, body) = submit_job(
        &deployment,
        &envelope,
        "retrying",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;

    for _ in 0..100 {
        if attempts.load(Ordering::SeqCst) >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let log = std::fs::read_to_string(
        deployment
            .dir
            .path()
            .join("jobs")
            .join(&job)
            .join("deliveries.jsonl"),
    )
    .unwrap();
    let outcomes: Vec<String> = log
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .map(|line| line["outcome"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(outcomes, vec!["failed", "delivered"]);
}

#[tokio::test]
async fn notify_secret_rotation_replaces_the_signing_key() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    let mut envelope = job_envelope(classify_batch(1));
    envelope["notify"] = json!({"url": "https://caller.example/hook"});
    let (status, body) = submit_job(
        &deployment,
        &envelope,
        "rotating",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    // A generated secret is returned once, here.
    assert_eq!(body["secret"].as_str().unwrap().len(), 64, "{body}");
    let job = body["job"].as_str().unwrap().to_string();

    let response = reqwest::Client::new()
        .post(format!(
            "{}/v1/jobs/{job}/notify/rotate",
            deployment.address
        ))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let rotated = response.json::<Value>().await.unwrap();
    let new_secret = rotated["secret"].as_str().unwrap().to_string();
    assert_eq!(new_secret.len(), 64);
    assert_ne!(new_secret, body["secret"].as_str().unwrap());
    // The stored manifest carries the rotation, not the response.
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(
            deployment
                .dir
                .path()
                .join("jobs")
                .join(&job)
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["notify"]["secret"], new_secret);
    assert_eq!(manifest["notify"]["rotated"], 1);
}

#[tokio::test]
async fn an_invalid_notify_url_is_refused() {
    let (endpoint, _forwards) = backend(honest(artifact('b'), choice_answer())).await;
    let deployment = classification_deployment(endpoint).await;
    for url in [
        "http://webhook.example.com/hook",
        "ftp://127.0.0.1/hook",
        "https://user:pass@caller.example/hook",
    ] {
        let mut envelope = job_envelope(classify_batch(1));
        envelope["notify"] = json!({"url": url});
        let (status, body) = submit_job(
            &deployment,
            &envelope,
            &format!("bad-{url}"),
            Some(&deployment.tokens["acme"]),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{url}: {body}");
        assert_eq!(body["error"]["code"], "invalid_notify", "{url}");
    }
}

#[tokio::test]
async fn a_backend_outage_fails_the_job_with_honest_counts() {
    let (endpoint, _forwards) = backend(Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        ..honest(artifact('b'), choice_answer())
    })
    .await;
    let deployment = classification_deployment(endpoint).await;
    let (status, body) = submit_job(
        &deployment,
        &job_envelope(classify_batch(3)),
        "outage",
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let job = body["job"].as_str().unwrap().to_string();
    let terminal = poll_terminal(&deployment, &job, Some(&deployment.tokens["acme"])).await;
    assert_eq!(terminal["status"], "completed", "{terminal}");
    // The first dispatch is attempted and unavailable; a door that
    // stops answering halts the rest as unattempted — completed is not
    // complete coverage, and nothing reads as answered.
    assert_eq!(terminal["counts"]["attempted"], 1);
    assert_eq!(terminal["counts"]["unavailable"], 1);
    assert_eq!(terminal["counts"]["unattempted"], 2);
    assert_eq!(terminal["counts"]["answered"], 0);
}
