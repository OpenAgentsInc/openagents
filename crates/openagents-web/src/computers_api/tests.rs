use super::*;
use axum::{Json, Router, extract::Request, response::IntoResponse, routing::any};
use std::os::unix::fs::PermissionsExt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_0123456789abcdef0123";
const CANARY: &str = "private-provider-token-and-address";

fn token(who: &str) -> String {
    format!(
        "sess_{}",
        match who {
            "alice" => "a",
            "bob" => "b",
            _ => "c",
        }
        .repeat(64)
    )
}

#[derive(Default)]
struct Native {
    revoked: AtomicBool,
    calls: Mutex<Vec<(String, String, String)>>,
}

async fn account(State(state): State<Arc<Native>>, headers: HeaderMap) -> Response {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if state.revoked.load(Ordering::SeqCst) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":CANARY}))).into_response();
    }
    let who = if bearer == format!("Bearer {}", token("alice"))
        || bearer == format!("Bearer oa_sk_{}", "a".repeat(64))
        || bearer == "Bearer oak_alice.account-scoped-synthetic-key"
    {
        "alice"
    } else if bearer == format!("Bearer {}", token("bob")) {
        "bob"
    } else if bearer == format!("Bearer {}", token("charlie")) {
        "charlie"
    } else {
        return (StatusCode::FORBIDDEN, Json(json!({"error":CANARY}))).into_response();
    };
    Json(json!({"account":{"id":who,"admin":false},"workspaces":[]})).into_response()
}

async fn session(State(state): State<Arc<Native>>, headers: HeaderMap) -> Response {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if state.revoked.load(Ordering::SeqCst) || bearer != format!("Bearer {}", token("alice")) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":CANARY}))).into_response();
    }
    Json(json!({"session":{"id":"alice-session","account":"alice","kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
}

async fn provider(State(state): State<Arc<Native>>, request: Request) -> Response {
    let owner = request
        .headers()
        .get("x-openagents-account")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        request.headers()[header::AUTHORIZATION],
        "Bearer synthetic-service-token"
    );
    let path = request.uri().path().to_owned();
    let method = request.method().to_string();
    // Cloud Run rejects bodyless POSTs without an explicit zero length,
    // before the request reaches the application.
    if method == "POST"
        && path != "/v1/computers"
        && !path.ends_with("/commands")
        && request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            != Some("0")
    {
        return (StatusCode::LENGTH_REQUIRED, Json(json!({"message":CANARY}))).into_response();
    }
    state
        .calls
        .lock()
        .unwrap()
        .push((owner.clone(), method.clone(), path.clone()));
    if path == "/v1/capabilities" {
        return Json(
            json!({"driver":CANARY,"profiles":["core","suspend","snapshot","fork","command"],"sizes":["small","medium"]}),
        )
        .into_response();
    }
    let mut row = json!({"id":ID,"chat":null,"size":"small","desired":"running","state":"awake","turns":0,"last_error":CANARY,"driver":CANARY,"resource":CANARY});
    if path == "/v1/computers" && method == "GET" {
        let rows = if owner == crate::chat_store::account_owner("alice") {
            vec![row]
        } else {
            vec![]
        };
        return Json(json!({"computers":rows,"next_cursor":null})).into_response();
    }
    if owner != crate::chat_store::account_owner("alice") {
        return (StatusCode::NOT_FOUND, Json(json!({"message":CANARY}))).into_response();
    }
    if path.contains("/commands") {
        let command = json!({"id":COMMAND,"computer":ID,"state":"succeeded","digest":"a".repeat(64),"exit_code":0,"provider_reference":CANARY,"owner":CANARY,"cancel_requested":false,"created_unix":1,"started_unix":2,"finished_unix":3,"output_expires_unix":604801,"output_limit_bytes":262144,"stdout_truncated":false,"stderr_truncated":false,"stdout_bytes":5,"stderr_bytes":5});
        if path.ends_with("/output") {
            if request
                .uri()
                .query()
                .is_some_and(|q| q.contains("max_bytes=13"))
            {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error":CANARY})),
                )
                    .into_response();
            }
            if request
                .uri()
                .query()
                .is_some_and(|q| q.contains("stdout=262144"))
            {
                return (StatusCode::GONE, Json(json!({"error":CANARY}))).into_response();
            }
            return Json(json!({"command":command,"encoding":"base64","stdout":"ZWFybHk=","stderr":"ZXJyb3I=","next_cursor":{"stdout":5,"stderr":5},"gaps":{"stdout":false,"stderr":false},"terminal":true,"has_more":false,"provider":CANARY})).into_response();
        }
        return (
            if method == "POST" {
                StatusCode::ACCEPTED
            } else {
                StatusCode::OK
            },
            Json(command),
        )
            .into_response();
    }
    if path.ends_with("/stop") {
        row["desired"] = json!("stopped");
    }
    if method == "DELETE" {
        row["desired"] = json!("deleted");
    }
    let status = if method == "GET" {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    (status, Json(row)).into_response()
}

async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, task)
}

#[tokio::test]
async fn public_lifecycle_uses_current_account_authority_and_keeps_service_credentials_private() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = Arc::new(Native::default());
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", axum::routing::get(account))
            .route("/v1/session", axum::routing::get(session))
            .with_state(state.clone()),
    )
    .await;
    let (provider_base, provider_task) = serve(
        Router::new()
            .fallback(any(provider))
            .with_state(state.clone()),
    )
    .await;
    let private = dir.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = private.join("csrf");
    std::fs::write(&key, [9_u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config = private.join("cloud.json");
    std::fs::write(&config,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path(&private)).unwrap(),
    ));
    config.agent_accounts = vec!["alice".into(), "bob".into()];
    crate::computers::configure(
        &provider_base,
        crate::computers::Token::Value("synthetic-service-token".into()),
    );
    let store = config.chat_store.clone();
    crate::coder_sync::check_in(&store, &crate::chat_store::account_owner("alice"), "My Mac")
        .await
        .unwrap()
        .unwrap();
    let app = App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(crate::phone_api::routes().with_state(app)).await;
    let http = reqwest::Client::new();
    let create = http
        .post(format!("{base}/v1/computers"))
        .bearer_auth(token("alice"))
        .header(
            "x-openagents-account",
            crate::chat_store::account_owner("bob"),
        )
        .header("Idempotency-Key", "logical-request-one")
        .json(&json!({"size":"small"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 202);
    let created = create.text().await.unwrap();
    assert!(!created.contains(CANARY) && !created.contains("synthetic-service-token"));
    assert_eq!(serde_json::from_str::<Value>(&created).unwrap()["id"], ID);
    for path in [
        format!("/v1/computers/{ID}"),
        "/v1/computers/capabilities".into(),
    ] {
        let r = http
            .get(format!("{base}{path}"))
            .bearer_auth("oak_alice.account-scoped-synthetic-key")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let text = r.text().await.unwrap();
        assert!(!text.contains(CANARY));
        if path.ends_with("capabilities") {
            assert_eq!(
                serde_json::from_str::<Value>(&text).unwrap()["profiles"],
                json!(["core", "suspend"])
            );
        }
    }
    let list: Value = http
        .get(format!("{base}/v1/computers"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        list["computers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "My Mac")
    );
    assert!(
        list["computers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == ID)
    );
    crate::coder_sync::check_in(
        &store,
        &crate::chat_store::account_owner("alice"),
        "capabilities",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        http.delete(format!("{base}/v1/computers/capabilities"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    for suffix in ["/stop", "/wake"] {
        assert_eq!(
            http.post(format!("{base}/v1/computers/{ID}{suffix}"))
                .bearer_auth(token("alice"))
                .send()
                .await
                .unwrap()
                .status(),
            202
        );
    }
    crate::coder_sync::check_in(&store, &crate::chat_store::account_owner("alice"), ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        http.delete(format!("{base}/v1/computers/{ID}?kind=cloud"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    assert!(
        store
            .computers(&crate::chat_store::account_owner("alice"))
            .await
            .unwrap()
            .seen
            .contains_key(ID)
    );
    assert_eq!(
        http.delete(format!("{base}/v1/computers/{ID}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        http.get(format!("{base}/v1/computers/{ID}"))
            .bearer_auth(token("bob"))
            .header(
                "X-OpenAgents-Account",
                crate::chat_store::account_owner("alice")
            )
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        http.delete(format!("{base}/v1/computers/My%20Mac"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );

    let commands = format!("{base}/v1/computers/{ID}/commands");
    let r = http
        .post(&commands)
        .bearer_auth(token("alice"))
        .header("Idempotency-Key", "command-1")
        .json(&json!({"command":"printf early"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(r.json::<Value>().await.unwrap()["id"], COMMAND);
    for suffix in [format!("/{COMMAND}"), format!("/{COMMAND}/output")] {
        let r = http
            .get(format!("{commands}{suffix}"))
            .bearer_auth(token("alice"))
            .header("x-openagents-account", "foreign")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let body = r.text().await.unwrap();
        assert!(!body.contains(CANARY));
        assert!(!body.contains("provider_reference"));
        assert_eq!(
            http.get(format!("{commands}{suffix}"))
                .bearer_auth(token("bob"))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let r = http
        .post(format!("{commands}/{COMMAND}/cancel"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    assert!(!r.text().await.unwrap().contains(CANARY));
    let r = http
        .get(format!("{commands}/{COMMAND}/output?stdout=262144"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 410);
    let v = r.text().await.unwrap();
    assert!(v.contains("expired"));
    assert!(!v.contains(CANARY));
    let r = http
        .get(format!("{commands}/{COMMAND}/output?max_bytes=13"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 503);
    assert!(!r.text().await.unwrap().contains(CANARY));
    let before = state.calls.lock().unwrap().len();
    let too_large = http
        .post(format!("{base}/v1/computers"))
        .bearer_auth(token("alice"))
        .body("x".repeat(20 * 1024))
        .send()
        .await
        .unwrap();
    assert_eq!(too_large.status(), 413);
    assert_eq!(
        too_large.json::<Value>().await.unwrap()["error"]["code"],
        "too_large"
    );
    assert_eq!(
        http.post(format!("{base}/v1/computers"))
            .bearer_auth(token("charlie"))
            .json(&json!({"idempotency_key":"gated"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        http.post(format!("{base}/v1/computers"))
            .bearer_auth(token("alice"))
            .json(&json!({"size":"small"}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.post(format!("{base}/v1/computers"))
            .bearer_auth(token("alice"))
            .header("Idempotency-Key", "a")
            .json(&json!({"idempotency_key":"b"}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.post(format!("{base}/v1/computers"))
            .bearer_auth(token("alice"))
            .json(&json!({"idempotency_key":"key","image":"untrusted-image"}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.get(format!("{base}/v1/computers/{ID}"))
            .bearer_auth("oak_unbound.decision-only-synthetic-key")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        http.get(format!("{base}/v1/computers/{ID}"))
            .header("Cookie", format!("oa_cloud_session={}", token("alice")))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for request in [
        http.get(format!("{commands}/{COMMAND}/output"))
            .bearer_auth(token("alice"))
            .header("Origin", "https://hostile.example"),
        http.get(format!(
            "{commands}/{COMMAND}/output?token=untrusted-attach-token"
        ))
        .bearer_auth(token("alice")),
        http.get(format!("{commands}/{COMMAND}/output?max_bytes=65537"))
            .bearer_auth(token("alice")),
        http.post(&commands)
            .bearer_auth(token("alice"))
            .json(&json!({"command":"true","credentials":["foreign"],"idempotency_key":"x"})),
    ] {
        let r = request.send().await.unwrap();
        assert!(matches!(r.status().as_u16(), 400 | 403));
        assert!(!r.text().await.unwrap().contains(CANARY));
    }
    state.revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        http.get(format!("{base}/v1/computers/{ID}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        http.get(format!("{commands}/{COMMAND}/output"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        state.calls.lock().unwrap().len(),
        before,
        "refusals must precede provider calls"
    );
    assert!(state.calls.lock().unwrap().iter().all(|(owner, _, _)| owner
        == &crate::chat_store::account_owner("alice")
        || owner == &crate::chat_store::account_owner("bob")));
    task.abort();
    auth_task.abort();
    provider_task.abort();
}

fn config_path(private: &std::path::Path) -> std::path::PathBuf {
    private.join("cloud.json")
}

#[test]
fn service_origins_and_public_paths_are_bounded() {
    for bad in [
        "http://example.com",
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "https://example.com#secret",
    ] {
        assert!(Client::new(bad, crate::computers::Token::Value("synthetic".into())).is_err());
    }
    for good in ["https://example.com", "http://127.0.0.1:1234"] {
        assert!(Client::new(good, crate::computers::Token::Value("synthetic".into())).is_ok());
    }
    assert!(cloud_id(ID));
    for bad in ["cmp_../alice", "cmp_short", "My Mac"] {
        assert!(!cloud_id(bad));
    }
    assert!(
        validate_page(&ListQuery {
            limit: Some(201),
            ..Default::default()
        })
        .is_err()
    );
}
