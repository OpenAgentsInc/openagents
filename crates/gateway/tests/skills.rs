//! The skill directory's end-to-end contract: bounded `SKILL.md`
//! submissions through the recorded review pipeline, the published
//! catalog's browse/search/version/raw-Markdown reads, deduplication
//! and supersession, withdrawal and moderation, and the author's
//! private submission view — over real HTTP against an in-process
//! deployment with accounts and a stub review backend.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{self, Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

use common::*;

/// The review answer that admits: safety and coherence high, quality
/// at the top of the rubric.
fn review_answer(safe: f64, coherent: f64, score: f64) -> Value {
    json!({
        "model": "kev-0.6b",
        "answers": {
            "safe": {"type": "noul", "noul": safe},
            "coherent": {"type": "noul", "noul": coherent},
            "quality": {"type": "score", "score": score},
        },
        "usage": {"input_tokens": 120, "output_tokens": 6},
    })
}

/// The accounts block: sign-up onto `acme`, no anonymous lane.
fn account_config() -> config::Accounts {
    config::Accounts {
        signup_tenant: Some("acme".to_string()),
        session_ttl_secs: 28_800,
        recovery_ttl_secs: 3_600,
        anonymous: None,
    }
}

/// The skills block pointing its review stage at the stub backend.
fn skills_config(endpoint: &str) -> config::Skills {
    config::Skills {
        max_body_bytes: 65_536,
        submissions_per_day: 20,
        pending_per_author: 10,
        admit_score: 0.6,
        review: config::Review {
            endpoint: endpoint.to_string(),
            model: "kev-0.6b".to_string(),
            timeout_ms: 10_000,
        },
    }
}

/// A deployed gateway with accounts and the skill directory — the
/// door backend is a second stub serving decision calls.
struct Deployment {
    _dir: tempfile::TempDir,
    registry: std::path::PathBuf,
    address: String,
    _state: Arc<ServeState>,
}

async fn deploy(answer_status: StatusCode, review_body: Value) -> Deployment {
    let (review_endpoint, _forwards) = backend(&artifact('r'), answer_status, review_body, 0).await;
    let (endpoint, _forwards) = backend(&artifact('b'), StatusCode::OK, answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest(&artifact('b'), None);
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    for tenant in manifest.tenants.keys() {
        keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
    }
    let doors = ["shared-kev", "acme-kev"]
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
        .collect();
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        accounts: Some(account_config()),
        billing: None,
        skills: Some(skills_config(&review_endpoint)),
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
        registry: dir.path().to_path_buf(),
        _dir: dir,
        address,
        _state: state,
    }
}

async fn exchange(request: reqwest::RequestBuilder) -> (StatusCode, Value) {
    let response = request.send().await.unwrap();
    let status = response.status();
    let body = response.json().await.unwrap_or_default();
    (status, body)
}

async fn get(deployment: &Deployment, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{}{path}", deployment.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

async fn post(
    deployment: &Deployment,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}{path}", deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

/// A sign-up's session token — the credential the submission routes
/// authenticate.
async fn join(deployment: &Deployment, label: &str) -> String {
    let (status, body) = post(deployment, "/v1/accounts", None, &json!({"label": label})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["session_token"].as_str().unwrap().to_string()
}

/// A valid `SKILL.md` body — frontmatter plus instructions.
fn document(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Route a request to the right lane.\n---\n\n\
         # {name}\n\nRead the request's state and name the lane it belongs to.\n"
    )
}

/// A submission body for `name` at `version`.
fn submission(name: &str, version: &str) -> Value {
    json!({
        "name": name,
        "version": version,
        "license": "MIT",
        "category": "routing",
        "tags": ["triage", "routing"],
        "consent": true,
        "markdown": document(name),
    })
}

fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or_default()
}

/// The state a submission's version stands at in its response view.
fn state_of(body: &Value) -> String {
    body["submission"]["status"]["state"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn submission_publishes_through_the_recorded_review() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    let (status, body) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state_of(&body), "published");

    // The published catalog resolves the entry, the version, the raw
    // Markdown, and the recorded review — all without a credential.
    let (status, entry) = get(&deployment, "/v1/skills/triage", None).await;
    assert_eq!(status, StatusCode::OK, "{entry}");
    assert_eq!(entry["latest"]["version"], "1.0.0");
    assert_eq!(entry["latest"]["evidence"]["measured"], false);
    assert!(
        entry["install"]["markdown_url"]
            .as_str()
            .unwrap()
            .contains("/v1/skills/triage/versions/1.0.0/SKILL.md")
    );

    let (status, version) = get(&deployment, "/v1/skills/triage/versions/1.0.0", None).await;
    assert_eq!(status, StatusCode::OK, "{version}");

    let markdown = reqwest::Client::new()
        .get(format!(
            "{}/v1/skills/triage/versions/1.0.0/SKILL.md",
            deployment.address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(markdown.status(), StatusCode::OK);
    assert_eq!(markdown.text().await.unwrap(), document("triage"));

    // The review record shows every stage with its reviewer, policy,
    // outcome, score, and cost.
    let (status, review) = get(&deployment, "/v1/skills/triage/versions/1.0.0/review", None).await;
    assert_eq!(status, StatusCode::OK, "{review}");
    let stages = review["review"].as_array().unwrap();
    assert_eq!(stages.len(), 3, "{stages:?}");
    assert_eq!(stages[0]["stage"], "static");
    assert_eq!(stages[0]["outcome"], "pass");
    assert_eq!(stages[1]["stage"], "decision");
    assert_eq!(stages[1]["reviewer"], "kev-0.6b");
    assert_eq!(stages[1]["cost"], 126);
    assert_eq!(stages[2]["stage"], "reasoning");
    assert_eq!(stages[2]["outcome"], "pass");

    // Browse finds it by name, category, tag, and author.
    let (status, list) = get(&deployment, "/v1/skills?q=triage", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["entries"].as_array().unwrap().len(), 1);
    let (_, list) = get(&deployment, "/v1/skills?category=routing", None).await;
    assert_eq!(list["entries"].as_array().unwrap().len(), 1);
    let (_, list) = get(&deployment, "/v1/skills?category=other", None).await;
    assert_eq!(list["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn static_failure_rejects_without_a_model_call() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    // A leaked credential-shaped literal fails the static screen — the
    // decision backend is never reached.
    let mut body = submission("leaky", "1.0.0");
    body["markdown"] = json!(format!(
        "{}\nkey: oak_abcdef1234567890.deadbeef\n",
        document("leaky")
    ));
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(state_of(&answer), "rejected");
    assert!(
        answer["submission"]["status"]["resolution"]
            .as_str()
            .unwrap()
            .contains("oak_")
    );
    // Rejected content never reaches the catalog.
    let (status, _) = get(&deployment, "/v1/skills/leaky", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The author's own submissions view still shows it, with its reason.
    let (status, mine) = get(&deployment, "/v1/submissions", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine["submissions"].as_array().unwrap().len(), 1);
    assert_eq!(mine["submissions"][0]["status"]["state"], "rejected");
}

#[tokio::test]
async fn consent_and_field_bounds_are_enforced() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;

    let mut body = submission("noconsent", "1.0.0");
    body["consent"] = json!(false);
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&answer), "consent_required");

    let mut body = submission("Bad-Name", "1.0.0");
    body["name"] = json!("Bad-Name");
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&answer), "invalid_submission");

    let mut body = submission("toomanytags", "1.0.0");
    body["tags"] = json!((0..9).map(|i| format!("tag{i}")).collect::<Vec<_>>());
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&answer), "invalid_submission");
}

#[tokio::test]
async fn duplicates_are_stable_and_versions_conflict() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    let (status, first) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = first["submission"]["id"].as_str().unwrap().to_string();

    // The identical resubmission is the same record — no second review.
    let (status, again) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["submission"]["id"], id);
    assert_eq!(state_of(&again), "published");

    // The same version at different content conflicts — the author
    // supersedes by bumping the version, never by rewriting it.
    let mut changed = submission("triage", "1.0.0");
    changed["markdown"] = json!(format!("{}\nMore text.\n", document("triage")));
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &changed).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&answer), "version_conflict");
}

#[tokio::test]
async fn failed_review_never_admits_and_appeals_record() {
    let deployment = deploy(StatusCode::OK, review_answer(0.2, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    let (status, body) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("unsafe", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state_of(&body), "rejected");
    let (status, _) = get(&deployment, "/v1/skills/unsafe", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The appeal records against the submission.
    let id = body["submission"]["id"].as_str().unwrap().to_string();
    let (status, appeal) = post(
        &deployment,
        &format!("/v1/submissions/{id}/appeal"),
        Some(&token),
        &json!({"reason": "the safety read is wrong — the document only names lanes"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{appeal}");

    // The operator's appeal-granted admit publishes through the same
    // gate — exercised on the book the binary wraps.
    let directory = tenancy::skills::Directory::open(&deployment.registry).unwrap();
    directory
        .mutate(|book, _, now| {
            book.moderate_admit("unsafe", "1.0.0", "operator", "appeal granted", now)
        })
        .unwrap();
    let (status, entry) = get(&deployment, "/v1/skills/unsafe", None).await;
    assert_eq!(status, StatusCode::OK, "{entry}");
    let (status, review) = get(&deployment, "/v1/skills/unsafe/versions/1.0.0/review", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        review["review"]
            .as_array()
            .unwrap()
            .iter()
            .any(|stage| stage["stage"] == "moderation" && stage["outcome"] == "pass")
    );
}

#[tokio::test]
async fn review_error_stays_under_review_and_retries() {
    let deployment = deploy(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error": {"code": "down", "message": "backend down"}}),
    )
    .await;
    let token = join(&deployment, "ada").await;
    let (status, body) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("pending", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state_of(&body), "under_review");
    // The error stage is recorded; the version is not published.
    let (status, mine) = get(&deployment, "/v1/submissions", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let stages = mine["submissions"][0]["status"]["review"]
        .as_array()
        .unwrap();
    assert!(
        stages
            .iter()
            .any(|stage| stage["stage"] == "decision" && stage["outcome"] == "error"),
        "{stages:?}"
    );
    let (status, _) = get(&deployment, "/v1/skills/pending", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The identical resubmission retries the same record rather than
    // opening a second submission.
    let (status, retry) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("pending", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry["submission"]["id"], body["submission"]["id"]);
}

#[tokio::test]
async fn withdrawal_stops_discovery() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    let (status, _) = post(
        &deployment,
        "/v1/skills",
        Some(&token),
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, answer) = post(
        &deployment,
        "/v1/skills/triage/versions/1.0.0/withdraw",
        Some(&token),
        &json!({"reason": "superseded internally"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");

    let (status, _) = get(&deployment, "/v1/skills/triage", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let markdown = reqwest::Client::new()
        .get(format!(
            "{}/v1/skills/triage/versions/1.0.0/SKILL.md",
            deployment.address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(markdown.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn names_belong_to_their_authors() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let ada = join(&deployment, "ada").await;
    let bob = join(&deployment, "bob").await;
    let (status, _) = post(
        &deployment,
        "/v1/skills",
        Some(&ada),
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Another account cannot submit under the same name.
    let (status, answer) = post(
        &deployment,
        "/v1/skills",
        Some(&bob),
        &submission("triage", "2.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code(&answer), "forbidden");

    // Nor withdraw another's version.
    let (status, _) = post(
        &deployment,
        "/v1/skills/triage/versions/1.0.0/withdraw",
        Some(&bob),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // And sees none of the other's submissions.
    let (status, mine) = get(&deployment, "/v1/submissions", Some(&bob)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine["submissions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn supersession_keeps_pinned_versions_resolving() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    for version in ["1.0.0", "1.1.0"] {
        let (status, body) = post(
            &deployment,
            "/v1/skills",
            Some(&token),
            &submission("triage", version),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(state_of(&body), "published");
    }
    let (status, entry) = get(&deployment, "/v1/skills/triage", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry["latest"]["version"], "1.1.0");
    let (status, old) = get(&deployment, "/v1/skills/triage/versions/1.0.0", None).await;
    assert_eq!(status, StatusCode::OK, "{old}");
    assert_eq!(old["version"]["superseded_by"], "1.1.0");
}

#[tokio::test]
async fn evidence_stays_separate_from_review_scores() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let token = join(&deployment, "ada").await;
    let mut body = submission("measured", "1.0.0");
    body["evidence"] = json!({
        "suite": "openagents.recipes.v1",
        "report": "reports/2026-10-04-measured.json",
        "digest": "sha256:deadbeef",
    });
    let (status, answer) = post(&deployment, "/v1/skills", Some(&token), &body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let (status, version) = get(&deployment, "/v1/skills/measured/versions/1.0.0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(version["version"]["evidence"]["measured"], true);
    assert_eq!(
        version["version"]["evidence"]["suite"],
        "openagents.recipes.v1"
    );
    // The review score is present but distinct.
    assert_eq!(version["version"]["review"]["score"], 0.8);
}

#[tokio::test]
async fn unauthenticated_and_anonymous_callers_cannot_submit() {
    let deployment = deploy(StatusCode::OK, review_answer(0.95, 0.9, 3.2)).await;
    let (status, answer) = post(
        &deployment,
        "/v1/skills",
        None,
        &submission("triage", "1.0.0"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&answer), "unauthenticated");
}
