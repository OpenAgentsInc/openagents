#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `POST /v1/decisions` end to end against stub Clef doors: the request in
//! OpenAI's Decisions shape goes out as one NIP-DEC body to the door its
//! model names, and the answers come back in request order with
//! `openagents.door`; images refuse each question; an Idempotency-Key
//! replays; the OpenAPI document describes the route.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use serde_json::{Map, Value, json};
use tenancy::Registry;

use gateway::config::{Config, Door, SCHEMA};
use gateway::decision_dispatch::Decisions;
use gateway::serve::{self, ServeState};

type Seen = Arc<Mutex<Vec<Value>>>;

/// A Psionic-shaped `/v1/systemone`: answers every question (the first
/// option most likely), echoes the requested model, and keeps each body.
async fn clef_stub(status: StatusCode) -> (String, Seen) {
    clef_stub_slow(status, 0).await
}

/// [`clef_stub`] taking `delay_ms` per answer.
async fn clef_stub_slow(status: StatusCode, delay_ms: u64) -> (String, Seen) {
    async fn answer(
        State((seen, status, delay_ms)): State<(Seen, StatusCode, u64)>,
        Json(body): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        seen.lock().unwrap().push(body.clone());
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        if status != StatusCode::OK {
            return (
                status,
                Json(json!({"error": {"code": "busy", "message": "busy"}})),
            );
        }
        let mut answers = Map::new();
        for (id, q) in body["questions"].as_object().unwrap() {
            let answer = match q["type"].as_str().unwrap() {
                "noul" => json!({"type": "noul", "noul": 0.8}),
                "choice" => {
                    let keys: Vec<&String> = q["criteria"].as_object().unwrap().keys().collect();
                    let rest = 0.2 / (keys.len() - 1) as f64;
                    let probabilities: Map<String, Value> = keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| ((*k).clone(), json!(if i == 0 { 0.8 } else { rest })))
                        .collect();
                    json!({"type": "choice", "choice": keys[0], "probabilities": probabilities})
                }
                _ => {
                    let n = q["criteria"].as_array().unwrap().len();
                    let probabilities: Map<String, Value> = (0..n)
                        .map(|i| (i.to_string(), json!(if i == n - 1 { 1.0 } else { 0.0 })))
                        .collect();
                    json!({"type": "score", "probabilities": probabilities})
                }
            };
            answers.insert(id.clone(), answer);
        }
        (
            StatusCode::OK,
            Json(json!({"model": body["model"], "answers": answers,
                        "usage": {"input_tokens": 99, "output_tokens": 0, "cached_input_tokens": 0}})),
        )
    }
    let seen: Seen = Arc::default();
    let app = axum::Router::new()
        .route("/v1/systemone", post(answer))
        .with_state((seen.clone(), status, delay_ms));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    (url, seen)
}

struct Deployment {
    address: String,
    _dir: tempfile::TempDir,
}

async fn deploy(decisions: Decisions) -> Deployment {
    let (endpoint, _forwards) =
        common::backend(&common::artifact('a'), StatusCode::OK, common::answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    Registry::install(dir.path(), manifest).unwrap();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: None,
        decisions: Some(decisions),
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
        doors: ["shared-kev", "acme-kev"]
            .into_iter()
            .map(|door| {
                (
                    door.to_string(),
                    Door {
                        endpoint: endpoint.clone(),
                        classify: None,
                        classify_item_concurrency: 1,
                        batching: None,
                    },
                )
            })
            .collect(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state)).into_future());
    Deployment { address, _dir: dir }
}

fn hosted(flash: &str, big: Option<&str>) -> Decisions {
    let mut clef_models = BTreeMap::new();
    clef_models.insert("clef-flash".to_owned(), flash.to_owned());
    if let Some(big) = big {
        clef_models.insert("clef".to_owned(), big.to_owned());
    }
    Decisions {
        clef_models,
        vertex: None,
        no_jev: true,
        jev_secret: None,
        shadow: 0.0,
        deadline_ms: 5_000,
        pylon_ms: 500,
        // No relay is reachable; the hosted orders never ask the Pylons.
        relay: Some("ws://127.0.0.1:9".into()),
        ..Decisions::default()
    }
}

fn request(model: &str) -> Value {
    json!({
        "model": model,
        "input": "I was charged twice for my order.",
        "questions": [
            {"type": "score", "name": "severity", "instructions": "How severe?",
             "levels": [{"label": "Cosmetic"}, {"label": "Workaround"}, {"label": "Blocked"}]},
            {"type": "choice", "name": "department", "instructions": "Which department?",
             "choices": [{"value": "billing", "description": "Payments."},
                         {"value": "technical"}, {"value": "other"}]},
            {"type": "predicate", "instructions": "Is it urgent?"}
        ],
        "metadata": {"ticket": "T-1"}
    })
}

async fn call(address: &str, body: &Value, headers: &[(&str, &str)]) -> (u16, Value) {
    let (status, body, _) = call_full(address, body, headers).await;
    (status, body)
}

/// [`call`] with the `X-Decision-Backend` header.
async fn call_full(
    address: &str,
    body: &Value,
    headers: &[(&str, &str)],
) -> (u16, Value, Option<String>) {
    let mut call = reqwest::Client::new()
        .post(format!("{address}/v1/decisions"))
        .json(body);
    for (name, value) in headers {
        call = call.header(*name, *value);
    }
    let response = call.send().await.unwrap();
    let status = response.status().as_u16();
    let backend = response
        .headers()
        .get("x-decision-backend")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    (status, response.json().await.unwrap(), backend)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_model_reaches_its_clef_door_and_answers_in_order() {
    let (flash, flash_seen) = clef_stub(StatusCode::OK).await;
    let (big, big_seen) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&flash, Some(&big))).await;

    let (status, body) = call(&gateway.address, &request("openagents/decide"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert!(body["id"].as_str().unwrap().starts_with("dec_"));
    assert_eq!(body["object"], "decision");
    assert_eq!(body["model"], "clef-flash");
    assert_eq!(body["openagents"]["door"], "clef");
    assert_eq!(body["usage"]["input_tokens"], 99);
    assert_eq!(body["metadata"], json!({"ticket": "T-1"}));
    let names: Vec<&str> = body["answers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["severity", "department", "q2"]);
    assert_eq!(body["answers"][0]["score"], json!(2.0));
    assert_eq!(body["answers"][1]["choice"], "billing");
    assert_eq!(body["answers"][2]["probability"], json!(0.8));
    // One NIP-DEC body went to the Flash door, questions keyed by name.
    let sent = flash_seen.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["model"], "clef-flash");
    assert_eq!(sent[0]["state"], "I was charged twice for my order.");
    assert_eq!(sent[0]["questions"]["q2"]["type"], "noul");
    assert_eq!(
        sent[0]["questions"]["severity"]["criteria"],
        json!(["Cosmetic", "Workaround", "Blocked"])
    );

    let (status, body) = call(&gateway.address, &request("clef"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["model"], "clef");
    assert_eq!(big_seen.lock().unwrap().len(), 1);
    assert_eq!(big_seen.lock().unwrap()[0]["model"], "clef");
    assert_eq!(flash_seen.lock().unwrap().len(), 1);

    let (status, body) = call(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(flash_seen.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_named_clef_fails_rather_than_falling_through() {
    let (flash, _) = clef_stub(StatusCode::SERVICE_UNAVAILABLE).await;
    let gateway = deploy(hosted(&flash, None)).await;
    let (status, body) = call(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["error"]["code"], "unavailable");
    assert_eq!(body["error"]["type"], "unavailable");
    // No 27B door is configured: `clef` has nowhere to go.
    let (status, _) = call(&gateway.address, &request("clef"), &[]).await;
    assert_eq!(status, 503);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn images_refuse_each_question_without_asking_a_door() {
    let (flash, seen) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&flash, None)).await;
    let mut body = request("openagents/decide");
    body["input"] = json!([{"role": "user", "content": [
        {"type": "input_image", "image_url": "data:image/png;base64,iVBORw0KGgo="},
        {"type": "input_text", "text": "Is the box damaged?"}]}]);
    let (status, answer) = call(&gateway.address, &body, &[]).await;
    assert_eq!(status, 200, "{answer}");
    assert!(
        answer["answers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["type"] == "refusal" && a["code"] == "unsupported_input")
    );
    assert!(answer["openagents"]["door"].is_null());
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_idempotency_key_replays_and_refuses_another_body() {
    let (flash, seen) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&flash, None)).await;
    let key = [("idempotency-key", "decisions-test-replay-1")];
    let (_, first) = call(&gateway.address, &request("openagents/decide"), &key).await;
    let (status, again) = call(&gateway.address, &request("openagents/decide"), &key).await;
    assert_eq!(status, 200);
    assert_eq!(first["id"], again["id"]);
    assert_eq!(seen.lock().unwrap().len(), 1);
    let mut other = request("openagents/decide");
    other["input"] = json!("Something else.");
    let (status, body) = call(&gateway.address, &other, &key).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"]["code"], "idempotency_conflict");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bad_request_is_one_error_object() {
    let (flash, _) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&flash, None)).await;
    let mut body = request("openagents/decide");
    body["questions"][1]["choices"] = json!([{"value": "only"}]);
    let (status, body) = call(&gateway.address, &body, &[]).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(body["error"]["type"], "invalid_request");
    assert_eq!(body["error"]["param"], "questions[1].choices");
    assert!(body["error"]["request_id"].is_string());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_url_is_a_pool_of_one_named_by_its_host() {
    let (flash, _) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&flash, None)).await;
    let (status, body, backend) = call_full(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 200, "{body}");
    let host = flash.trim_start_matches("http://");
    assert_eq!(body["openagents"]["backend"], host);
    assert_eq!(backend.as_deref(), Some(host));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_busy_backend_hands_the_decision_to_the_next_and_sits_out() {
    let (busy, busy_seen) = clef_stub(StatusCode::SERVICE_UNAVAILABLE).await;
    let (good, good_seen) = clef_stub(StatusCode::OK).await;
    let gateway = deploy(hosted(&format!("h100={busy},a100={good}"), None)).await;
    let (status, body, backend) = call_full(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["openagents"]["door"], "clef");
    assert_eq!(body["openagents"]["backend"], "a100");
    assert_eq!(backend.as_deref(), Some("a100"));
    assert_eq!(busy_seen.lock().unwrap().len(), 1);
    // The busy one sits out: the next decision goes straight to the other.
    let (status, body) = call(&gateway.address, &request("openagents/decide"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["openagents"]["backend"], "a100");
    assert_eq!(busy_seen.lock().unwrap().len(), 1);
    assert_eq!(good_seen.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_backend_hands_the_decision_to_the_next() {
    let (good, _) = clef_stub(StatusCode::OK).await;
    // Nothing listens on port 9.
    let gateway = deploy(hosted(
        &format!("h100=http://127.0.0.1:9,a100={good}"),
        Some(&format!("a100={good}")),
    ))
    .await;
    let (status, body) = call(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["openagents"]["backend"], "a100");
    let (status, body) = call(&gateway.address, &request("clef"), &[]).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["model"], "clef");
    assert_eq!(body["openagents"]["backend"], "a100");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decisions_at_once_go_to_the_backend_with_fewer_in_flight() {
    let (h100, h100_seen) = clef_stub_slow(StatusCode::OK, 400).await;
    let (a100, a100_seen) = clef_stub_slow(StatusCode::OK, 400).await;
    let gateway = deploy(hosted(&format!("h100={h100},a100={a100}"), None)).await;
    // One at a time: every decision goes to the first listed.
    for _ in 0..2 {
        let (_, body) = call(&gateway.address, &request("clef-flash"), &[]).await;
        assert_eq!(body["openagents"]["backend"], "h100");
    }
    // Four at once: split two and two.
    let calls: Vec<_> = (0..4)
        .map(|_| {
            let address = gateway.address.clone();
            tokio::spawn(async move { call(&address, &request("clef-flash"), &[]).await })
        })
        .collect();
    let mut by = BTreeMap::new();
    for done in calls {
        let (status, body) = done.await.unwrap();
        assert_eq!(status, 200, "{body}");
        *by.entry(body["openagents"]["backend"].as_str().unwrap().to_owned())
            .or_insert(0) += 1;
    }
    assert_eq!(by.get("h100"), Some(&2), "{by:?}");
    assert_eq!(by.get("a100"), Some(&2), "{by:?}");
    assert_eq!(h100_seen.lock().unwrap().len(), 4);
    assert_eq!(a100_seen.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pool_with_every_backend_down_fails_a_named_clef() {
    let gateway = deploy(hosted(
        "h100=http://127.0.0.1:9,a100=http://127.0.0.1:9",
        None,
    ))
    .await;
    let (status, body) = call(&gateway.address, &request("clef-flash"), &[]).await;
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["error"]["code"], "unavailable");
}
