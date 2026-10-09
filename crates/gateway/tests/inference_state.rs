//! The stateful inference routes (#11071): stored responses over HTTP
//! (read, continue, delete, owner scope, zero retention), compaction, and
//! the WebSocket transport through the flows of the Open Responses
//! acceptance suite's seven WebSocket tests.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, EventStream, ModelRow,
    Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};
use tenancy::{Registry, keys};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use gateway::config::{Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

const MODEL: &str = "test/model";

/// Answers `items:<n>`: how many input items it was sent.
struct Echo {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
}

fn event(value: Value) -> inference::Event {
    serde_json::from_value(value).expect("event")
}

fn response(status: &str, output: Value, usage: Value) -> Value {
    json!({"id": "upstream", "object": "response", "created_at": 1, "status": status,
           "model": MODEL, "output": output, "usage": usage})
}

impl Upstream for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.models
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = self.model(model).expect("model");
            let meter = AttemptMeter::start(self, row);
            let text = format!("items:{}", request.input_items().len());
            let message = json!({"type": "message", "id": "msg_1", "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text, "annotations": []}]});
            let events: Vec<Result<inference::Event, AttemptError>> = vec![
                Ok(event(
                    json!({"type": "response.created", "sequence_number": 0,
                    "response": response("in_progress", json!([]), Value::Null)}),
                )),
                Ok(event(
                    json!({"type": "response.output_item.added", "sequence_number": 1,
                    "output_index": 0, "item": {"type": "message", "id": "msg_1",
                    "status": "in_progress", "role": "assistant", "content": []}}),
                )),
                Ok(event(
                    json!({"type": "response.content_part.added", "sequence_number": 2,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []}}),
                )),
                Ok(event(
                    json!({"type": "response.output_text.delta", "sequence_number": 3,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0, "delta": text}),
                )),
                Ok(event(
                    json!({"type": "response.output_text.done", "sequence_number": 4,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0, "text": text}),
                )),
                Ok(event(
                    json!({"type": "response.content_part.done", "sequence_number": 5,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": text, "annotations": []}}),
                )),
                Ok(event(
                    json!({"type": "response.output_item.done", "sequence_number": 6,
                    "output_index": 0, "item": message}),
                )),
                Ok(event(
                    json!({"type": "response.completed", "sequence_number": 7,
                    "response": response("completed", json!([message]),
                        json!({"input_tokens": 10, "output_tokens": 5,
                               "input_tokens_details": {"cached_tokens": 0},
                               "output_tokens_details": {"reasoning_tokens": 0},
                               "total_tokens": 15}))}),
                )),
            ];
            let events: EventStream = Box::pin(futures_util::stream::iter(events));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

fn echo() -> Arc<dyn Upstream> {
    Arc::new(Echo {
        account: Account {
            id: "echo-account".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![ModelRow {
            id: MODEL.into(),
            upstream_model: MODEL.into(),
            capabilities: Capabilities {
                tools: true,
                reasoning: false,
                reasoning_always_on: false,
                json_schema: true,
                images: true,
                context: 100_000,
                max_output: 8_000,
            },
            price: Price::micro(1_000_000, 100_000, 2_000_000),
            price_source: "test",
        }],
    })
}

struct Deployment {
    acme: String,
    globex: String,
    quiet: String,
    address: String,
    dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

async fn deploy() -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let mut manifest = common::manifest(&common::artifact('a'), None);
    for name in ["globex", "quiet"] {
        let mut tenant = manifest.tenants["acme"].clone();
        tenant.credential = format!("key-ref:{name}");
        manifest.tenants.insert(name.into(), tenant);
    }
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let issue = |tenant: &str| {
        keys::issue(dir.path(), registry.manifest(), tenant)
            .unwrap()
            .token
    };
    let (acme, globex, quiet) = (issue("acme"), issue("globex"), issue("quiet"));
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "INFERENCE_STATE_TEST_ADMIN",
        "service_tenants": ["acme", "globex", "quiet"],
        "zero_retention_tenants": ["quiet"],
        "store": {"retention_days": 30, "key_env": "INFERENCE_STATE_TEST_UNSET_KEY"}
    }))
    .unwrap();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: Some(inference),
        accounts: None,
        billing: None,
        funding: None,
        earnings: None,
        commercial: None,
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
        doors: Default::default(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let state = ServeState::open_with_upstreams(config, Some(vec![echo()])).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        acme,
        globex,
        quiet,
        address,
        dir,
        _state: state,
    }
}

async fn call(
    d: &Deployment,
    method: reqwest::Method,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("http://{}{path}", d.address))
        .bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let answer = request.send().await.unwrap();
    let status = answer.status();
    (status, answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn stored_responses_are_read_continued_and_deleted_by_their_owner_only() {
    let d = deploy().await;
    let post = reqwest::Method::POST;
    let (status, first) = call(
        &d,
        post.clone(),
        "/v1/responses",
        &d.acme,
        Some(json!({"model": MODEL, "input": "remember cobalt", "store": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = first["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("resp_"));
    assert_eq!(first["store"], true);

    // Kept sealed on disk: no text, no tenant name in any path.
    let stored = d.dir.path().join("inference").join("responses");
    let mut files = 0;
    for owner in std::fs::read_dir(&stored).unwrap().flatten() {
        assert!(!owner.file_name().to_string_lossy().contains("acme"));
        for file in std::fs::read_dir(owner.path()).unwrap().flatten() {
            files += 1;
            let text = std::fs::read_to_string(file.path()).unwrap();
            assert!(
                !text.contains("cobalt") && !text.contains("items:"),
                "{text}"
            );
        }
    }
    assert_eq!(files, 1);

    let path = format!("/v1/responses/{id}");
    let (status, read) = call(&d, reqwest::Method::GET, &path, &d.acme, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["id"], id);
    assert_eq!(read["output"][0]["content"][0]["text"], "items:1");

    // Continued over HTTP: earlier input, its output, the new input.
    let (status, next) = call(
        &d,
        post.clone(),
        "/v1/responses",
        &d.acme,
        Some(json!({"model": MODEL, "input": "and?", "previous_response_id": id})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{next}");
    assert_eq!(next["output"][0]["content"][0]["text"], "items:3");
    assert_eq!(next["previous_response_id"], id);

    // Another tenant reads nothing and continues nothing.
    let (status, _) = call(&d, reqwest::Method::GET, &path, &d.globex, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&d, reqwest::Method::DELETE, &path, &d.globex, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, refused) = call(
        &d,
        post.clone(),
        "/v1/responses",
        &d.globex,
        Some(json!({"model": MODEL, "input": "x", "previous_response_id": id})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(refused["error"]["code"], "previous_response_not_found");

    // The owner deletes it, and it is gone at once.
    let (status, deleted) = call(&d, reqwest::Method::DELETE, &path, &d.acme, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    let (status, _) = call(&d, reqwest::Method::GET, &path, &d.acme, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A zero-retention tenant cannot store.
    let (status, refused) = call(
        &d,
        post,
        "/v1/responses",
        &d.quiet,
        Some(json!({"model": MODEL, "input": "x", "store": true})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(refused["error"]["code"], "store_not_allowed");
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(d: &Deployment, token: &str) -> Socket {
    let mut request = format!("ws://{}/v1/responses", d.address)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    tokio_tungstenite::connect_async(request).await.unwrap().0
}

/// Sends one `response.create` and reads to its terminal event or error
/// envelope: (every message, the last).
async fn turn(socket: &mut Socket, body: Value) -> (Vec<Value>, Value) {
    socket
        .send(Message::Text(body.to_string().into()))
        .await
        .unwrap();
    let mut seen = Vec::new();
    while let Some(message) = socket.next().await {
        let Message::Text(text) = message.unwrap() else {
            continue;
        };
        let value: Value = serde_json::from_str(&text).unwrap();
        let kind = value["type"].as_str().unwrap_or_default().to_owned();
        seen.push(value.clone());
        if kind == "error"
            || kind == "response.completed"
            || kind == "response.failed"
            || kind == "response.incomplete"
        {
            return (seen, value);
        }
    }
    panic!("the socket closed mid-turn: {seen:?}");
}

fn create(input: Value, previous: Option<&str>) -> Value {
    let mut body = json!({"type": "response.create", "model": MODEL, "store": false,
                          "input": input});
    if let Some(previous) = previous {
        body["previous_response_id"] = previous.into();
    }
    body
}

#[tokio::test]
async fn the_websocket_transport_runs_the_acceptance_suites_flows() {
    let d = deploy().await;

    // WebSocket Response and Sequential Responses: events in order,
    // numbered from zero, two turns on one connection.
    let mut socket = connect(&d, &d.acme).await;
    let (events, done) = turn(
        &mut socket,
        create(json!("Reply with exactly: first"), None),
    )
    .await;
    assert_eq!(done["type"], "response.completed");
    assert_eq!(events[0]["type"], "response.created");
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"], index as u64);
    }
    let first = done["response"]["id"].as_str().unwrap().to_owned();
    let (_, second) = turn(
        &mut socket,
        create(json!("Reply with exactly: second"), None),
    )
    .await;
    assert_eq!(second["type"], "response.completed");
    let second_id = second["response"]["id"].as_str().unwrap().to_owned();
    assert_ne!(first, second_id);

    // HTTP-only fields are refused with the error envelope, and the
    // connection stays usable.
    let (_, refused) = turn(
        &mut socket,
        json!({"type": "response.create", "model": MODEL, "input": "x", "stream": true}),
    )
    .await;
    assert_eq!(refused["status"], 400);
    assert_eq!(refused["error"]["param"], "stream");

    // WebSocket Continuation: store:false, previous_response_id, only the
    // new input.
    let (_, continued) = turn(
        &mut socket,
        create(json!("What is the code word?"), Some(&second_id)),
    )
    .await;
    assert_eq!(continued["type"], "response.completed", "{continued}");
    assert_eq!(
        continued["response"]["output"][0]["content"][0]["text"],
        "items:3"
    );
    assert_eq!(continued["response"]["previous_response_id"], second_id);
    let continued_id = continued["response"]["id"].as_str().unwrap().to_owned();

    // WebSocket Missing Previous Response.
    let (_, missing) = turn(
        &mut socket,
        create(json!("x"), Some("resp_openresponses_missing_1")),
    )
    .await;
    assert_eq!(missing["type"], "error");
    assert_eq!(missing["status"], 400);
    assert_eq!(missing["error"]["code"], "previous_response_not_found");
    assert_eq!(missing["error"]["param"], "previous_response_id");

    // WebSocket Store False Reconnect Recovery: a new connection does not
    // know the response; a clean response without the id works.
    let mut fresh = connect(&d, &d.acme).await;
    let (_, lost) = turn(&mut fresh, create(json!("x"), Some(&continued_id))).await;
    assert_eq!(lost["error"]["code"], "previous_response_not_found");
    let (_, recovered) = turn(&mut fresh, create(json!("start over"), None)).await;
    assert_eq!(recovered["type"], "response.completed");
    // Nothing was stored for any of these store:false turns.
    let (status, _) = call(
        &d,
        reqwest::Method::GET,
        &format!("/v1/responses/{continued_id}"),
        &d.acme,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // WebSocket Failed Continuation Evicts Cache.
    let (_, base) = turn(&mut socket, create(json!("remember ember"), None)).await;
    let base_id = base["response"]["id"].as_str().unwrap().to_owned();
    let (_, failed) = turn(
        &mut socket,
        create(
            json!([{"type": "function_call_output", "call_id": "call_openresponses_missing",
                    "output": "No matching tool call exists in the previous response."}]),
            Some(&base_id),
        ),
    )
    .await;
    assert_eq!(failed["type"], "error");
    let (_, stale) = turn(
        &mut socket,
        create(json!("Reply with exactly: stale"), Some(&base_id)),
    )
    .await;
    assert_eq!(stale["error"]["code"], "previous_response_not_found");

    // WebSocket Compact New Chain: /responses/compact's output as the
    // base input of a new response without previous_response_id.
    let (status, compacted) = call(
        &d,
        reqwest::Method::POST,
        "/v1/responses/compact",
        &d.acme,
        Some(json!({"model": MODEL, "prompt_cache_key": "k", "input": [
            {"type": "message", "role": "user",
             "content": [{"type": "input_text", "text": "Remember the compaction code word: slate."}]},
            {"type": "message", "role": "assistant", "content": "OK."}
        ]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{compacted}");
    assert_eq!(compacted["object"], "response.compaction");
    let output = compacted["output"].as_array().unwrap();
    assert_eq!(output.last().unwrap()["type"], "compaction");
    let mut input = output.clone();
    input.push(json!({"type": "message", "role": "user", "content": "The code word?"}));
    let (_, chained) = turn(&mut socket, create(Value::Array(input), None)).await;
    assert_eq!(chained["type"], "response.completed", "{chained}");

    // Compaction Missing Required Model.
    let (status, _) = call(
        &d,
        reqwest::Method::POST,
        "/v1/responses/compact",
        &d.acme,
        Some(json!({"input": [{"type": "message", "role": "user",
                               "content": "Compact this conversation."}]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Another tenant's connection cannot open the compaction item.
    let mut other = connect(&d, &d.globex).await;
    let (_, refused) = turn(&mut other, create(Value::Array(output.clone()), None)).await;
    assert_eq!(refused["error"]["code"], "invalid_compaction");
}

#[tokio::test]
async fn a_websocket_needs_a_service_key() {
    let d = deploy().await;
    let mut request = format!("ws://{}/v1/responses", d.address)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", "Bearer oak_nope.nope".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
}
