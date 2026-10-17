//! Explicit live canary frontend. Real staging account reads authorize the
//! normal router; its Computers backend is a separate disposable deployment.
use super::*;
use axum::{Json, Router, extract::Request, response::IntoResponse, routing::any};
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const STAGING: &str = "https://openagents-web-1-staging-ezxz4mgdsq-uc.a.run.app";
#[derive(Deserialize)]
struct Account {
    id: String,
    token: String,
    session: Value,
}
#[derive(Deserialize)]
struct Configuration {
    accounts: Vec<Account>,
    directory: PathBuf,
    backend: String,
    backend_token: String,
    port: u16,
}
struct Native {
    http: reqwest::Client,
    accounts: Vec<Account>,
}
async fn native(State(n): State<Arc<Native>>, request: Request) -> Response {
    if request.method() != axum::http::Method::GET
        || request
            .headers()
            .get_all(header::AUTHORIZATION)
            .iter()
            .count()
            != 1
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or_default();
    let Some(account) = n.accounts.iter().find(|a| a.token == token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let path = request.uri().path();
    if path != "/v1/session" && path != "/v1/account" && !path.starts_with("/v1/workspaces/") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let target = if path == "/v1/session" {
        "/v1/account"
    } else {
        path
    };
    let Ok(mut reply) = n
        .http
        .get(format!("{STAGING}{target}"))
        .bearer_auth(token)
        .send()
        .await
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let status = reply.status();
    let mut bytes = Vec::new();
    loop {
        match reply.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 1048576 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    }
    if path == "/v1/session" && status.is_success() {
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        if value["account"]["id"] != account.id
            || value["account"]["email"] == "chris@openagents.com"
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let mut session = account.session.clone();
        session["state"] = json!("active");
        return Json(json!({"session":session})).into_response();
    }
    (status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires private canary configuration and dedicated staging accounts."]
async fn serve_the_live_credential_canary_frontend() {
    let path = PathBuf::from(
        std::env::var_os("OA_CREDENTIAL_QUALIFY_CONFIG")
            .expect("Set the private canary configuration file."),
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let q: Configuration = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(q.accounts.len(), 2);
    assert_ne!(q.accounts[0].id, q.accounts[1].id);
    for a in &q.accounts {
        assert_eq!(a.session["account"], a.id);
        assert_eq!(a.session["kind"], "user");
        assert!(a.token.starts_with("sess_"));
    }
    std::fs::create_dir_all(&q.directory).unwrap();
    std::fs::set_permissions(&q.directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", q.port))
        .await
        .unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let account_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let account_origin = format!("http://{}", account_listener.local_addr().unwrap());
    let accounts: Vec<_> = q.accounts.iter().map(|a| a.id.clone()).collect();
    let router = Router::new()
        .fallback(any(native))
        .with_state(Arc::new(Native {
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            accounts: q.accounts,
        }));
    let account_server = tokio::spawn(async move {
        axum::serve(account_listener, router).await.unwrap();
    });
    let csrf = q.directory.join("csrf");
    std::fs::write(&csrf, [13; 32]).unwrap();
    std::fs::set_permissions(&csrf, std::fs::Permissions::from_mode(0o600)).unwrap();
    let cloud = q.directory.join("cloud.json");
    std::fs::write(&cloud,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":account_origin,"csrf_secret":csrf})).unwrap()).unwrap();
    std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o600)).unwrap();
    crate::computers::configure(&q.backend, crate::computers::Token::Value(q.backend_token));
    let mut config = crate::Config::development(q.directory.join("tasks"));
    config.public_hosts = vec![origin.trim_start_matches("http://").to_owned()];
    config.agent_accounts = accounts.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&cloud).unwrap(),
    ));
    config.inference = Some(Arc::new(
        crate::upstream::Upstream::new(&account_origin).unwrap(),
    ));
    let public = crate::router(config);
    let server = tokio::spawn(async move {
        axum::serve(listener, public).await.unwrap();
    });
    std::fs::write(
        q.directory.join("frontend-ready.json"),
        serde_json::to_vec(&json!({"origin":origin,"accounts":accounts,"inference_calls":0}))
            .unwrap(),
    )
    .unwrap();
    let started = Instant::now();
    while !q.directory.join("frontend-done").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(3600),
            "The canary frontend reached its time limit."
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    server.abort();
    account_server.abort();
}
