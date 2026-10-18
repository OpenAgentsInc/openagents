use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error, Session,
    land::{LandEntry, LandRecord, LandState, LandSubmit},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Queue {
    entries: Mutex<HashMap<String, LandEntry>>,
    requests: Mutex<HashMap<String, Value>>,
    lose_response: AtomicBool,
    calls: AtomicUsize,
}

fn authenticated(headers: &HeaderMap) {
    assert!(
        headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("Bearer oa_sk_")
    );
    assert_eq!(headers["x-openagents-test"], "1");
}

async fn submit(
    State(queue): State<Arc<Queue>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    authenticated(&headers);
    queue.calls.fetch_add(1, Ordering::SeqCst);
    let key = headers["idempotency-key"].to_str().unwrap().to_owned();
    let mut requests = queue.requests.lock().unwrap();
    if requests.get(&key).is_some_and(|saved| saved != &body) {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"idempotency_conflict"}})),
        )
            .into_response();
    }
    requests.insert(key.clone(), body.clone());
    let mut entries = queue.entries.lock().unwrap();
    let entry = entries.entry(key.clone()).or_insert_with(|| LandEntry {
        id: key,
        branch: body["branch"].as_str().unwrap().into(),
        target: "main".into(),
        issue: body["issue"].as_u64(),
        close: true,
        author: "Test contributor".into(),
        machine: "fixture".into(),
        summary: body["summary"].as_str().unwrap_or_default().into(),
        head: "abc123".into(),
        submitted_at: 10,
        updated_at: 10,
        ..LandEntry::default()
    });
    if queue.lose_response.swap(false, Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    (StatusCode::CREATED, Json(entry.clone())).into_response()
}

async fn status(State(queue): State<Arc<Queue>>, headers: HeaderMap) -> Json<Value> {
    authenticated(&headers);
    queue.calls.fetch_add(1, Ordering::SeqCst);
    let entries = queue.entries.lock().unwrap();
    let with_state = |state| {
        entries
            .values()
            .filter(|entry| entry.state == state)
            .cloned()
            .collect::<Vec<_>>()
    };
    Json(json!({
        "queue":"fixture-queue",
        "integrator":{"running":true,"heartbeat":{
            "machine":"fixture-integrator","at":11,
            "instance":{"project":"fixture","zone":"test-a","name":"integrator"},
            "slots":[{"slot":"code-1","lane":"code","entry":"request-a","since":10}],
            "capacity":2
        }},
        "waiting":with_state(LandState::Queued),
        "landing":with_state(LandState::Landing),
        "failed":with_state(LandState::Bounced)
    }))
}

async fn show(
    State(queue): State<Arc<Queue>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    authenticated(&headers);
    queue.calls.fetch_add(1, Ordering::SeqCst);
    match queue.entries.lock().unwrap().get(&id) {
        Some(entry) => Json(entry.clone()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"code":"not_found"}})),
        )
            .into_response(),
    }
}

async fn withdraw(
    State(queue): State<Arc<Queue>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> StatusCode {
    authenticated(&headers);
    queue.calls.fetch_add(1, Ordering::SeqCst);
    queue.entries.lock().unwrap().get_mut(&id).unwrap().state = LandState::Withdrawn;
    StatusCode::NO_CONTENT
}

struct Fixture {
    client: Client,
    session: Session,
    queue: Arc<Queue>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture() -> Fixture {
    let queue = Arc::new(Queue::default());
    let app = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(
                    json!({"account":{"id":"test-account","label":"Test account"},"workspaces":[]}),
                )
            }),
        )
        .route("/v1/land", get(status).post(submit))
        .route("/v1/land/{id}", get(show).delete(withdraw))
        .with_state(queue.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::with_test_run(
        Deployment::new(&format!("{origin}/v1"), &origin).unwrap(),
        std::time::Duration::from_secs(20),
        true,
    )
    .unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    Fixture {
        client,
        session,
        queue,
        task,
    }
}

fn change() -> LandSubmit {
    LandSubmit {
        repo: "OpenAgentsInc/openagents".into(),
        branch: "land/test-change".into(),
        issue: Some(12038),
        summary: Some("Add the landing client".into()),
    }
}

#[tokio::test]
async fn submit_reuses_the_request_key_after_a_lost_response_and_exposes_attempts() {
    let f = fixture().await;
    let land = f.client.land(&f.session).unwrap();
    let cancel = CancellationToken::new();
    f.queue.lose_response.store(true, Ordering::SeqCst);
    assert!(matches!(
        land.submit("request-a", change(), &cancel).await,
        Err(Error::Unavailable { .. })
    ));
    let entry = land.submit("request-a", change(), &cancel).await.unwrap();
    assert_eq!(entry.issue, Some(12038));
    assert_eq!(entry.state, LandState::Queued);
    assert_eq!(f.queue.entries.lock().unwrap().len(), 1);
    assert_eq!(f.queue.requests.lock().unwrap().len(), 1);

    let mut other = change();
    other.branch = "land/different-change".into();
    assert!(matches!(
        land.submit("request-a", other, &cancel).await,
        Err(Error::Refused {
            status: 409,
            code: Some("idempotency_conflict")
        })
    ));
    let status = land.status(&cancel).await.unwrap();
    assert_eq!(status.waiting, vec![entry]);
    assert!(status.integrator.running);
    assert_eq!(status.integrator.heartbeat.unwrap().capacity, Some(2));

    {
        let mut entries = f.queue.entries.lock().unwrap();
        let entry = entries.get_mut("request-a").unwrap();
        entry.state = LandState::Bounced;
        entry.reason = Some("Tests failed".into());
        entry.records.push(LandRecord {
            entry: entry.id.clone(),
            number: 1,
            outcome: "bounced".into(),
            checks: vec!["cargo test -p openagents-sdk".into()],
            problems: vec!["Tests failed".into()],
            ..LandRecord::default()
        });
    }
    let shown = land.show("request-a", &cancel).await.unwrap();
    assert_eq!(shown.records[0].number, 1);
    let status = land.status(&cancel).await.unwrap();
    assert!(status.waiting.is_empty());
    assert_eq!(status.failed, vec![shown]);
    land.withdraw("request-a", &cancel).await.unwrap();
    assert_eq!(
        land.show("request-a", &cancel).await.unwrap().state,
        LandState::Withdrawn
    );
    assert!(matches!(
        land.show("unknown", &cancel).await,
        Err(Error::Refused {
            status: 404,
            code: Some("not_found")
        })
    ));
}

#[tokio::test]
async fn cancellation_and_invalid_ids_do_not_send_queue_requests() {
    let f = fixture().await;
    let land = f.client.land(&f.session).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        land.submit("bad\nkey", change(), &cancel).await,
        Err(Error::InvalidReference)
    ));
    assert!(matches!(
        land.show("../other", &cancel).await,
        Err(Error::InvalidReference)
    ));
    assert!(matches!(
        land.withdraw("", &cancel).await,
        Err(Error::InvalidReference)
    ));
    cancel.cancel();
    assert!(matches!(
        land.submit("request-a", change(), &cancel).await,
        Err(Error::Cancelled)
    ));
    assert!(matches!(land.status(&cancel).await, Err(Error::Cancelled)));
    assert!(matches!(
        land.show("request-a", &cancel).await,
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        land.withdraw("request-a", &cancel).await,
        Err(Error::Cancelled)
    ));
    assert_eq!(f.queue.calls.load(Ordering::SeqCst), 0);

    let other = Client::new(Deployment::production()).unwrap();
    assert!(matches!(
        other.land(&f.session),
        Err(Error::WrongDeployment)
    ));
}
