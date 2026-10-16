use super::*;
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{os::unix::fs::PermissionsExt, sync::Arc};

async fn auth(headers: HeaderMap) -> Response {
    let who = match headers.get("authorization").and_then(|h| h.to_str().ok()) {
        Some("Bearer sess_1186700000000000000000000000000000000000000000000000000000000001") => {
            "thread-contract-alice"
        }
        Some("Bearer sess_1186700000000000000000000000000000000000000000000000000000000002") => {
            "thread-contract-bob"
        }
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    axum::Json(json!({"account":{"id":who,"admin":false},"workspaces":[],"session":{"id":"thread-session","account":who,"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    (
        base,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}
fn validate(schema: Value, value: &Value) {
    let doc = crate::app_openapi::document();
    let mut root = schema;
    root["components"] = doc["components"].clone();
    root["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    let validator = jsonschema::validator_for(&root).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}: {value}");
}
async fn checked(response: reqwest::Response, path: &str, method: &str, status: u16) -> Value {
    assert_eq!(response.status().as_u16(), status, "{method} {path}");
    let body: Value = response.json().await.unwrap();
    let doc = crate::app_openapi::document();
    validate(
        doc["paths"][path][method]["responses"][status.to_string()]["content"]["application/json"]
            ["schema"]
            .clone(),
        &body,
    );
    body
}

/// These real handlers use an isolated account service and local files only.
/// Taking a reply never launches an engine in this test.
#[tokio::test]
async fn qualified_threads_check_authority_retry_paging_and_sync_tombstones() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", get(auth))
            .route("/v1/session", get(auth)),
    )
    .await;
    let private = dir.path().canonicalize().unwrap();
    let key = private.join("csrf");
    std::fs::write(&key, [7u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_path = private.join("cloud.json");
    std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    let app = crate::App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(
        crate::phone_api::routes()
            .merge(crate::coder_sync::routes())
            .merge(crate::pages::chat::share::routes())
            .with_state(app),
    )
    .await;
    let http = reqwest::Client::new();
    let alice = "sess_1186700000000000000000000000000000000000000000000000000000000001";
    let bob = "sess_1186700000000000000000000000000000000000000000000000000000000002";
    let upload = json!({"computer":"Contract Mac","title":"Synthetic chat","messages":[{"role":"user","text":"Hello"},{"role":"assistant","text":"Hi"}]});
    validate(r("SyncUpload"), &upload);
    let path = "/v1/threads/synced/contract-session";
    let saved = checked(
        http.put(format!("{base}{path}"))
            .bearer_auth(alice)
            .json(&upload)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}",
        "put",
        200,
    )
    .await;
    let id = saved["chat"].as_str().unwrap();
    let repeated = checked(
        http.put(format!("{base}{path}"))
            .bearer_auth(alice)
            .json(&upload)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}",
        "put",
        200,
    )
    .await;
    assert_eq!(repeated["changed"], false);
    for credential in [None, Some(bob)] {
        let mut req = http.get(format!("{base}/v1/threads/{id}"));
        if let Some(token) = credential {
            req = req.bearer_auth(token);
        }
        checked(
            req.send().await.unwrap(),
            "/v1/threads/{id}",
            "get",
            if credential.is_none() { 401 } else { 404 },
        )
        .await;
    }
    checked(
        http.get(format!("{base}/v1/threads?limit=1"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads",
        "get",
        200,
    )
    .await;
    checked(
        http.get(format!("{base}/v1/threads/{id}"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/{id}",
        "get",
        200,
    )
    .await;
    let page = checked(
        http.get(format!("{base}/v1/threads/{id}/messages?limit=1"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/{id}/messages",
        "get",
        200,
    )
    .await;
    assert_eq!(page["next"], "msg_0");
    checked(
        http.get(format!("{base}/v1/threads/{id}/messages?after=unknown"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/{id}/messages",
        "get",
        400,
    )
    .await;
    for (credential, status) in [(alice, 200), (bob, 404)] {
        checked(
            http.get(format!("{base}/v1/threads/{id}/share"))
                .bearer_auth(credential)
                .send()
                .await
                .unwrap(),
            "/v1/threads/{id}/share",
            "get",
            status,
        )
        .await;
    }
    for shared in [true, false] {
        let state = checked(
            http.post(format!("{base}/v1/threads/{id}/share"))
                .bearer_auth(alice)
                .json(&json!({"shared":shared}))
                .send()
                .await
                .unwrap(),
            "/v1/threads/{id}/share",
            "post",
            200,
        )
        .await;
        assert_eq!(state["shared"], shared);
    }
    let reply =
        json!({"request_id":"d75ef8a1-16ca-49b6-83be-000000000001","text":"A synthetic follow-up"});
    validate(r("Reply"), &reply);
    checked(
        http.post(format!("{base}/v1/threads/{id}/messages"))
            .bearer_auth(alice)
            .json(&reply)
            .send()
            .await
            .unwrap(),
        "/v1/threads/{id}/messages",
        "post",
        409,
    )
    .await;
    checked(
        http.post(format!("{base}/v1/computers/check-in"))
            .bearer_auth(alice)
            .json(&json!({"computer":"Contract Mac"}))
            .send()
            .await
            .unwrap(),
        "/v1/computers/check-in",
        "post",
        200,
    )
    .await;
    for _ in 0..2 {
        checked(
            http.post(format!("{base}/v1/threads/{id}/messages"))
                .bearer_auth(alice)
                .json(&reply)
                .send()
                .await
                .unwrap(),
            "/v1/threads/{id}/messages",
            "post",
            202,
        )
        .await;
    }
    let first = checked(
        http.post(format!("{base}{path}/replies"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}/replies",
        "post",
        200,
    )
    .await;
    assert_eq!(first["replies"].as_array().unwrap().len(), 1);
    let second = checked(
        http.post(format!("{base}{path}/replies"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}/replies",
        "post",
        200,
    )
    .await;
    assert!(second["replies"].as_array().unwrap().is_empty());
    checked(
        http.post(format!("{base}{path}/status"))
            .bearer_auth(alice)
            .json(&json!({"working":false}))
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}/status",
        "post",
        200,
    )
    .await;
    assert_eq!(
        http.delete(format!("{base}/v1/threads/{id}"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    checked(
        http.put(format!("{base}{path}"))
            .bearer_auth(alice)
            .json(&upload)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced/{session}",
        "put",
        410,
    )
    .await;
    let sessions = checked(
        http.get(format!("{base}/v1/threads/synced"))
            .bearer_auth(alice)
            .send()
            .await
            .unwrap(),
        "/v1/threads/synced",
        "get",
        200,
    )
    .await;
    assert_eq!(sessions["sessions"][0]["deleted"], true);
    task.abort();
    auth_task.abort();
}
