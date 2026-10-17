use super::*;
use axum::{Json, Router, extract::Request, response::IntoResponse, routing::any};
use std::os::unix::fs::PermissionsExt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

mod usage;

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
    file: Mutex<Vec<u8>>,
    calls: Mutex<Vec<(String, String, String)>>,
    usage_queries: Mutex<Vec<String>>,
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
    if path == "/v1/usage" {
        return usage::provider(&state, &owner, &request);
    }
    if path == "/v1/capabilities" {
        return Json(
            json!({"driver":CANARY,"profiles":["core","suspend","snapshot","fork","command"],"sizes":["small","medium"],"workspace_file_bytes":computer_files::MAX_BYTES,
                "default_placement":"our-cloud","placements":[{"placement":"our-cloud","deployment":CANARY,"offers":[{"driver":CANARY,"profiles":["core","suspend",CANARY],"sizes":["small"],"shapes":{"small":{"vcpus":2,"memory_mib":8192,"provider":CANARY}},"memory_suspend":false,"command_sessions":true,"workspace_file_bytes":computer_files::MAX_BYTES,"default_environment_available":true}]}]}),
        )
        .into_response();
    }
    let mut row = json!({"id":ID,"chat":null,"size":"small","desired":"running","state":"awake","turns":0,"created_unix":1,"updated_unix":1,"last_error":CANARY,"driver":CANARY,"resource":CANARY});
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
    if path.ends_with("/files") {
        if request
            .uri()
            .query()
            .is_some_and(|q| q.contains("provider-error"))
        {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":CANARY,"message":CANARY})),
            )
                .into_response();
        }
        if method == "PUT" {
            let explicit_zero = request
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|h| h.to_str().ok())
                == Some("0");
            let data = axum::body::to_bytes(request.into_body(), computer_files::MAX_BYTES)
                .await
                .unwrap()
                .to_vec();
            if data.is_empty() && !explicit_zero {
                return (StatusCode::LENGTH_REQUIRED, Json(json!({"message":CANARY})))
                    .into_response();
            }
            let info = json!({"size":data.len(),"sha256":computer_files::digest(&data)});
            *state.file.lock().unwrap() = data;
            return Json(info).into_response();
        }
        let bytes = state.file.lock().unwrap().clone();
        let hash = if request
            .uri()
            .query()
            .is_some_and(|q| q.contains("bad-hash"))
        {
            "0".repeat(64)
        } else {
            computer_files::digest(&bytes)
        };
        return (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
                (
                    axum::http::HeaderName::from_static("x-content-sha256"),
                    hash,
                ),
            ],
            bytes,
        )
            .into_response();
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
    if method == "POST" && path == "/v1/computers" {
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(request.into_body(), 16 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        if body["idempotency_key"] == "logical-request-one" {
            assert_eq!(body["placement"], "fast-sandbox");
            assert_eq!(body["needs"]["files"], true);
            assert_eq!(body["needs"]["min_memory_mib"], 2048);
        }
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

// Each provider fixture owns the process-wide production configuration.
fn own_provider_process<T>(fixture: T) -> bool {
    let name = std::any::type_name_of_val(&fixture);
    let (_, name) = name.split_once("::").expect("qualified fixture name");
    const CHILD: &str = "OPENAGENTS_COMPUTERS_FIXTURE_CHILD";
    if std::env::var(CHILD).as_deref() == Ok(name) {
        return true;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, name)
        .output()
        .expect("start the isolated provider fixture");
    assert!(
        output.status.success(),
        "{name}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[tokio::test]
async fn public_lifecycle_uses_current_account_authority_and_keeps_service_credentials_private() {
    if !own_provider_process(
        public_lifecycle_uses_current_account_authority_and_keeps_service_credentials_private,
    ) {
        return;
    }
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
    let (base, task) = serve(crate::phone_api::routes().with_state(app).layer(
        axum::middleware::from_fn(super::contract_tests::check_response),
    ))
    .await;
    let http = reqwest::Client::new();
    let create = http
        .post(format!("{base}/v1/computers"))
        .bearer_auth(token("alice"))
        .header(
            "x-openagents-account",
            crate::chat_store::account_owner("bob"),
        )
        .header("Idempotency-Key", "logical-request-one")
        .json(&json!({"size":"small","placement":"fast-sandbox","needs":{"files":true,"min_memory_mib":2048}}))
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
            let capabilities: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(capabilities["default_placement"], "our-cloud");
            assert_eq!(
                capabilities["placements"][0]["offers"][0]["shapes"]["small"],
                json!({"vcpus":2,"memory_mib":8192})
            );
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
    for name in ["capabilities", "usage"] {
        crate::coder_sync::check_in(&store, &crate::chat_store::account_owner("alice"), name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            http.delete(format!("{base}/v1/computers/{name}"))
                .bearer_auth(token("alice"))
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
    }
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
    let file_url = format!("{base}/v1/computers/{ID}/files?path=input.bin");
    let file_bytes = b"\x00\xffSDK binary\x00".to_vec();
    let file_hash = computer_files::digest(&file_bytes);
    let r = http
        .put(&file_url)
        .bearer_auth(token("alice"))
        .header("content-type", "application/octet-stream")
        .header("x-content-sha256", &file_hash)
        .header(
            "x-openagents-account",
            crate::chat_store::account_owner("bob"),
        )
        .body(file_bytes.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.json::<Value>().await.unwrap()["sha256"], file_hash);
    let r = http
        .get(&file_url)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(r.headers()["x-content-sha256"], file_hash);
    assert_eq!(r.bytes().await.unwrap().as_ref(), file_bytes);
    assert_eq!(
        http.get(&file_url)
            .bearer_auth(token("bob"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        http.put(&file_url)
            .bearer_auth(token("bob"))
            .header("content-type", "application/octet-stream")
            .header("x-content-sha256", &file_hash)
            .body(file_bytes.clone())
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    for (path, status) in [("bad-hash", 503), ("provider-error", 503)] {
        let r = http
            .get(format!("{base}/v1/computers/{ID}/files?path={path}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), status);
        assert!(!r.text().await.unwrap().contains(CANARY));
    }
    let r = http
        .put(&file_url)
        .bearer_auth(token("alice"))
        .header("content-type", "application/octet-stream")
        .header("x-content-sha256", computer_files::digest(b""))
        .header(header::CONTENT_LENGTH, "0")
        .body(Vec::<u8>::new())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.json::<Value>().await.unwrap()["size"], 0);
    let r = http
        .get(&file_url)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["x-content-sha256"], computer_files::digest(b""));
    assert!(r.bytes().await.unwrap().is_empty());
    let caps: Value = http
        .get(format!("{base}/v1/computers/capabilities"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(caps["files"]["max_bytes"], computer_files::MAX_BYTES);
    let file_calls_before = state
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, method, path)| method == "PUT" && path.ends_with("/files"))
        .count();
    let oversized = vec![b'x'; computer_files::MAX_BYTES + 1];
    let r = http
        .put(&file_url)
        .bearer_auth(token("alice"))
        .header("content-type", "application/octet-stream")
        .header("x-content-sha256", computer_files::digest(&oversized))
        .body(oversized)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 413);
    assert_eq!(
        r.json::<Value>().await.unwrap()["error"]["code"],
        "file_too_large"
    );
    assert_eq!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, method, path)| method == "PUT" && path.ends_with("/files"))
            .count(),
        file_calls_before
    );
    assert!(state.file.lock().unwrap().is_empty());
    // Headers alone get an immediate413, while the unread body holds its slot.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = base.strip_prefix("http://").unwrap();
    let mut stalled = Vec::new();
    for _ in 0..2 {
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let headers = format!(
            "PUT /v1/computers/{ID}/files?path=input.bin HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nX-Content-SHA256: {}\r\n\r\n",
            token("alice"),
            computer_files::MAX_BYTES + 1,
            "a".repeat(64)
        );
        stream.write_all(headers.as_bytes()).await.unwrap();
        let mut response = [0; 2048];
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.read(&mut response),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 413"));
        stalled.push(stream);
    }
    assert_eq!(
        http.get(&file_url)
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        429
    );
    drop(stalled);
    for attempt in 0..50 {
        let r = http
            .get(&file_url)
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap();
        if r.status() == 200 {
            assert!(r.bytes().await.unwrap().is_empty());
            break;
        }
        assert!(
            attempt < 49,
            "Disconnected rejected uploads kept their slots"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    usage::check(&http, &base, &state).await;
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
    for (query, status) in [
        ("path=../outside", 400),
        ("path=%252e%252e/outside", 400),
        ("path=%FF", 400),
        ("path=input.bin&token=secret", 400),
        ("path=input.bin&overwrite=secret", 400),
    ] {
        let r = http
            .get(format!("{base}/v1/computers/{ID}/files?{query}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), status);
        let text = r.text().await.unwrap();
        assert!(!text.contains("secret") && !text.contains(CANARY));
    }
    assert_eq!(
        http.get(&file_url)
            .bearer_auth(token("alice"))
            .header("Origin", "https://hostile.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    state.revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        http.get(format!("{base}/v1/computer-usage"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        http.get(&file_url)
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
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

struct Handoff {
    /// Account lookups that still answer as unavailable.
    down: AtomicUsize,
    revoked: AtomicBool,
    lookups: AtomicUsize,
    calls: Mutex<Vec<(String, String)>>,
}

async fn handoff_account(State(state): State<Arc<Handoff>>, headers: HeaderMap) -> Response {
    state.lookups.fetch_add(1, Ordering::SeqCst);
    let mut down = state.down.load(Ordering::SeqCst);
    while down > 0 {
        match state
            .down
            .compare_exchange(down, down - 1, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Err(current) => down = current,
        }
    }
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if state.revoked.load(Ordering::SeqCst) || bearer != format!("Bearer {}", token("alice")) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"signed_out"})),
        )
            .into_response();
    }
    Json(json!({"account":{"id":"alice","admin":false},"workspaces":[]})).into_response()
}

async fn handoff_provider(State(state): State<Arc<Handoff>>, request: Request) -> Response {
    let owner = request
        .headers()
        .get("x-openagents-account")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    state.calls.lock().unwrap().push((method.clone(), path));
    assert_eq!(owner, crate::chat_store::account_owner("alice"));
    let row = json!({"id":ID,"chat":null,"size":"small","desired":"running","state":"awake","turns":0,"created_unix":1,"updated_unix":1});
    if method == "GET" {
        (StatusCode::OK, Json(row)).into_response()
    } else {
        (StatusCode::ACCEPTED, Json(row)).into_response()
    }
}

#[tokio::test]
async fn account_handoff_retries_a_read_and_does_not_replay_a_delete() {
    if !own_provider_process(account_handoff_retries_a_read_and_does_not_replay_a_delete) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = Arc::new(Handoff {
        down: AtomicUsize::new(1),
        revoked: AtomicBool::new(false),
        lookups: AtomicUsize::new(0),
        calls: Mutex::new(Vec::new()),
    });
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", axum::routing::get(handoff_account))
            .with_state(state.clone()),
    )
    .await;
    let (provider_base, provider_task) = serve(
        Router::new()
            .fallback(any(handoff_provider))
            .with_state(state.clone()),
    )
    .await;
    let private = dir.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = private.join("csrf");
    std::fs::write(&key, [9_u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(
        config_path(&private),
        serde_json::to_vec(&json!({
            "schema": "openagents.cloud.web-config.v1",
            "public_origin": "http://127.0.0.1:4300",
            "account_service": auth_base,
            "csrf_secret": key
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(
        config_path(&private),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path(&private)).unwrap(),
    ));
    config.agent_accounts = vec!["alice".into()];
    crate::computers::configure(
        &provider_base,
        crate::computers::Token::Value("synthetic-service-token".into()),
    );
    let app = App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(crate::phone_api::routes().with_state(app).layer(
        axum::middleware::from_fn(super::contract_tests::check_response),
    ))
    .await;
    let http = reqwest::Client::new();
    let read = format!("{base}/v1/computers/{ID}");
    let remove = format!("{base}/v1/computers/{ID}?kind=cloud");

    let recovered = http
        .get(&read)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(recovered.status(), 200);
    assert_eq!(state.lookups.load(Ordering::SeqCst), 2);
    assert_eq!(
        state.calls.lock().unwrap().as_slice(),
        [("GET".into(), format!("/v1/computers/{ID}"))]
    );

    state.down.store(2, Ordering::SeqCst);
    let before = state.calls.lock().unwrap().len();
    let down = http
        .get(&read)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(down.status(), 503);
    assert_eq!(down.headers()[header::RETRY_AFTER], "1");
    let body = down.text().await.unwrap();
    assert!(body.contains("Try again later"));
    assert_eq!(state.lookups.load(Ordering::SeqCst), 4);
    assert_eq!(state.calls.lock().unwrap().len(), before);

    state.down.store(0, Ordering::SeqCst);
    let again = http
        .get(&read)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 200);
    assert_eq!(state.lookups.load(Ordering::SeqCst), 5);
    assert_eq!(state.calls.lock().unwrap().len(), before + 1);

    state.revoked.store(true, Ordering::SeqCst);
    let revoked = http
        .get(&read)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), 401);
    assert_eq!(state.lookups.load(Ordering::SeqCst), 6);
    assert_eq!(state.calls.lock().unwrap().len(), before + 1);
    state.revoked.store(false, Ordering::SeqCst);

    let forged = http
        .get(&read)
        .bearer_auth(token("charlie"))
        .send()
        .await
        .unwrap();
    assert_eq!(forged.status(), 401);
    assert_eq!(state.lookups.load(Ordering::SeqCst), 7);
    let shaped = state.lookups.load(Ordering::SeqCst);
    let garbage = http.get(&read).bearer_auth("forged").send().await.unwrap();
    assert_eq!(garbage.status(), 401);
    assert_eq!(state.lookups.load(Ordering::SeqCst), shaped);

    state.down.store(2, Ordering::SeqCst);
    let missed = http
        .delete(&remove)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(missed.status(), 503);
    assert_eq!(missed.headers()[header::RETRY_AFTER], "1");
    assert!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(method, _)| method != "DELETE")
    );

    state.down.store(0, Ordering::SeqCst);
    let deleted = http
        .delete(&remove)
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), 202);
    assert_eq!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(method, _)| method == "DELETE")
            .count(),
        1
    );

    task.abort();
    auth_task.abort();
    provider_task.abort();
}
