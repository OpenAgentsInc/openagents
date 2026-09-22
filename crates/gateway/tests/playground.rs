//! The playground's end-to-end contract: session-signed pages, a bounded
//! classification run against the real admission path, a deterministic
//! simulated lane that bills nothing, and a bounded chat demo whose only
//! tool is the classify facade.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{self, Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

use common::*;

/// A deployed gateway with accounts and a classify-enabled door.
struct Deployment {
    address: String,
    _dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

async fn deploy() -> Deployment {
    // The backend answers each question with a fixed noul distribution.
    let (endpoint, _forwards) = backend(
        &artifact('b'),
        StatusCode::OK,
        json!({
            "model": "kev-0.6b",
            "answers": {"q0": {"type": "choice", "choice": "spam", "confidence": 0.9,
                "probabilities": {"spam": 0.9, "ham": 0.1}}},
            "usage": {"input_tokens": 8, "output_tokens": 1},
        }),
        0,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest(&artifact('b'), None);
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
                    endpoint: endpoint.clone(),
                    classify: Some(gateway::classify::BackendLimits::product()),
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
        accounts: Some(config::Accounts {
            signup_tenant: Some("acme".to_string()),
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            anonymous: None,
        }),
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        address,
        _dir: dir,
        _state: state,
    }
}

/// Sign an account up and return its session token and workspace.
async fn join(deployment: &Deployment) -> (String, String) {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/accounts", deployment.address))
        .json(&json!({"label": "ada"}))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    (
        body["session_token"].as_str().unwrap().to_string(),
        body["workspace"]["id"].as_str().unwrap().to_string(),
    )
}

fn playground_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn playground_pages_require_a_session_and_render_the_form() {
    let deployment = deploy().await;
    let client = playground_client();
    let response = client
        .get(format!("{}/playground", deployment.address))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("sign in"), "{body}");

    let (session, workspace) = join(&deployment).await;
    let response = client
        .get(format!("{}/playground", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("name=\"model\""), "{body}");
    assert!(body.contains("simulate"), "{body}");
    drop(workspace);
}

#[tokio::test]
async fn a_simulated_run_marks_itself_and_bills_nothing() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    let response = client
        .post(format!("{}/playground/run", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("mode", "single-label"),
            ("labels", "spam,ham"),
            ("items", "free money now\nmeeting at ten"),
            ("simulate", "on"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SIMULATED"), "{body}");
    assert!(body.contains("item-0"), "{body}");
    assert!(body.contains("item-1"), "{body}");
    assert!(body.contains("openagents.classify.v1"), "{body}");
}

#[tokio::test]
async fn a_live_run_reaches_the_backend_and_reports_its_receipt() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    let response = client
        .post(format!("{}/playground/run", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("mode", "single-label"),
            ("labels", "spam,ham"),
            ("items", "free money now"),
            ("threshold", "0.5"),
            ("uncertain_below", "0.95"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("spam"), "{body}");
    assert!(body.contains("0.90"), "{body}");
    assert!(body.contains("receipt"), "{body}");
    assert!(!body.contains("SIMULATED"), "{body}");
}

#[tokio::test]
async fn the_chat_demo_bounds_turns_and_shows_real_tool_answers() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    // A simulated turn renders the tool's selected intent.
    let response = client
        .post(format!("{}/playground/chat", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("message", "is this message spam?"),
            ("history", ""),
            ("simulate", "on"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("classified your message"), "{body}");
    assert!(body.contains("SIMULATED"), "{body}");
    assert!(body.contains("turn 1/10"), "{body}");

    // A history at the cap is refused rather than silently extended.
    let full: String = (0..10)
        .map(|turn| format!("user|m{turn}\nassistant|r{turn}"))
        .collect::<Vec<_>>()
        .join("\n");
    let response = client
        .post(format!("{}/playground/chat", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("message", "one more"),
            ("history", full.as_str()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.text().await.unwrap();
    assert!(body.contains("10 turns"), "{body}");
}

#[tokio::test]
async fn out_of_bounds_runs_refuse_before_any_backend_call() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    let oversized = "x".repeat(5000);
    let response = client
        .post(format!("{}/playground/run", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("mode", "single-label"),
            ("labels", "spam,ham"),
            ("items", oversized.as_str()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.text().await.unwrap();
    assert!(body.contains("too large"), "{body}");
}

#[tokio::test]
async fn a_native_run_answers_typed_questions_and_reports_its_receipt() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    let questions = json!({
        "q0": {"type": "choice", "instructions": "spam or ham?",
               "criteria": {"spam": "junk", "ham": "not junk"}},
    });
    let response = client
        .post(format!("{}/playground/native", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("state", "free money now"),
            ("questions", questions.to_string().as_str()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("q0"), "{body}");
    assert!(body.contains("spam"), "{body}");
    assert!(body.contains("/v1/systemone"), "{body}");
    assert!(body.contains("receipt"), "{body}");
    assert!(
        !body.contains(&session),
        "the session token must never render"
    );

    // The simulated lane marks itself and never reaches a door.
    let response = client
        .post(format!("{}/playground/native", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("state", "free money now"),
            ("questions", questions.to_string().as_str()),
            ("simulate", "on"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SIMULATED"), "{body}");
}

#[tokio::test]
async fn an_envelope_override_runs_dimensions_verbatim() {
    let deployment = deploy().await;
    let client = playground_client();
    let (session, workspace) = join(&deployment).await;
    // One request, two dimensions — a shape the form fields cannot
    // name — sent verbatim through the real classify path.
    let envelope = json!({
        "v": "openagents.classify.v1",
        "model": "acme-kev",
        "capacity": "dedicated",
        "dimensions": [{
            "id": "intent", "mode": "single-label",
            "labels": [{"id": "spam"}, {"id": "ham"}],
        }],
        "policy": {"v": "openagents.classify-policy.v1", "name": "playground",
            "select": {"single_label": {"ties": "first-declared", "no_match": {"kind": "null"}}}},
        "inputs": [{"id": "m1", "text": "free money now"}],
    });
    let response = client
        .post(format!("{}/playground/run", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("mode", "single-label"),
            ("envelope", envelope.to_string().as_str()),
        ])
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("m1"), "{body}");
    assert!(body.contains("/v1/classify"), "{body}");
    assert!(
        !body.contains(&session),
        "the session token must never render"
    );

    // A document that is not a classify envelope refuses before a door.
    let response = client
        .post(format!("{}/playground/run", deployment.address))
        .header("cookie", format!("oa_session={session}"))
        .form(&[
            ("workspace", workspace.as_str()),
            ("model", "acme-kev"),
            ("mode", "single-label"),
            ("envelope", "{\"v\":\"other\"}"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
