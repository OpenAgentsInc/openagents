//! Local synthetic account approval and sync data for the simulator acceptance test.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use coder_sync::server::{
    self, StateData, auth::Accounts, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
struct Account {
    id: String,
    origin: String,
    tokens: Arc<Mutex<HashSet<String>>>,
    grants: Arc<Mutex<HashSet<String>>>,
}
fn secret() -> String {
    format!(
        "sess_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
async fn code(State(account): State<Account>) -> Json<Value> {
    let grant = format!("dvc_{}", uuid::Uuid::new_v4().simple());
    account.grants.lock().unwrap().insert(grant.clone());
    Json(
        json!({"device_code":grant,"user_code":"TEST-CODE","verification_uri":format!("{}/device",account.origin),"verification_uri_complete":format!("{}/device?code=TEST-CODE",account.origin),"interval":1,"expires_in":600}),
    )
}
async fn redeem(State(account): State<Account>, Json(body): Json<Value>) -> Response {
    let code = body["device_code"].as_str().unwrap_or("");
    if !account.grants.lock().unwrap().remove(code) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"expired_token"})),
        )
            .into_response();
    }
    let token = secret();
    account.tokens.lock().unwrap().insert(token.clone());
    Json(json!({"access_token":token,"token_type":"Bearer","expires_in":3600,"account":{"id":account.id,"label":"Coder test account"}})).into_response()
}
async fn view(State(account): State<Account>, headers: HeaderMap) -> Response {
    if !token(&headers).is_some_and(|token| account.tokens.lock().unwrap().contains(token)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"account":{"id":account.id,"label":"Coder test account"},"workspaces":[]}))
        .into_response()
}
async fn sign_out(State(account): State<Account>, headers: HeaderMap) -> Json<Value> {
    if let Some(token) = token(&headers) {
        account.tokens.lock().unwrap().remove(token);
    }
    Json(json!({"signed_out":true}))
}
#[tokio::main]
async fn main() {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("Pass a scratch JSON configuration path"),
    );
    let api_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", api_listener.local_addr().unwrap());
    let account = Account {
        id: format!("acct_ios_{}", uuid::Uuid::new_v4().simple()),
        origin: origin.clone(),
        tokens: Arc::default(),
        grants: Arc::default(),
    };
    let seed = secret();
    account.tokens.lock().unwrap().insert(seed.clone());
    let api = Router::new()
        .route("/v1/device/code", post(code))
        .route("/v1/device/token", post(redeem))
        .route("/v1/account", get(view))
        .route("/v1/device/sign-out", post(sign_out))
        .route(
            "/device",
            get(|| async { Html("<p>Dedicated local fixture approval.</p>") }),
        )
        .with_state(account.clone());
    let api_task = tokio::spawn(async move {
        axum::serve(api_listener, api).await.unwrap();
    });
    let db = Database::new(
        &std::env::var("CODER_SYNC_TEST_DATABASE_URL")
            .expect("Run scripts/coder-sync/test.sh --ios-auth-fixture"),
    )
    .unwrap();
    db.migrate().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = StateData {
            proof_origin: None,
        database: db,
        bucket: Bucket::memory(),
        rooms: Rooms::default(),
        devices: Default::default(),
        auth: AuthMode::OpenAgents,
        accounts: Some(
            Accounts::new(openagents::Deployment::new(&format!("{origin}/v1"), &origin).unwrap())
                .unwrap(),
        ),
        revision: "ios-account-fixture".into(),
        push: None,
    };
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let app = server::router(state).route(
        "/__fixture/stop",
        post(move || {
            let stop = stop.clone();
            async move {
                let _ = stop.send(true);
                "stopped"
            }
        }),
    );
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let chat = uuid::Uuid::new_v4().to_string();
    let space = uuid::Uuid::new_v4().to_string();
    let device = "fixture-host";
    let op = |kind: &str, id: &str, set: Value| json!({"kind":kind,"id":id,"op":"upsert","hlc":format!("{now:013}-000000-fixture"),"set":set});
    let ops = vec![
        op(
            "devices",
            device,
            json!({"id":device,"name":"Fixture computer","platform":"macos","lastSeenAt":now,"createdAt":now,"version":"fixture","capabilities":[]}),
        ),
        op(
            "spaces",
            &space,
            json!({"id":space,"deviceId":device,"path":"/fixture","name":"Fixture","gitDetected":false,"createdAt":now}),
        ),
        op(
            "chats",
            &chat,
            json!({"id":chat,"deviceId":device,"spaceId":space,"title":"Fixture sync session","archived":false,"cwd":"/fixture","createdAt":now,"lastMessageAt":now,"lastSeenAt":now,"roomGen":2}),
        ),
        op(
            "sessions",
            &chat,
            json!({"chatId":chat,"deviceId":device,"status":"idle","updatedAt":now,"runningSubagents":0}),
        ),
    ];
    let response = reqwest::Client::new()
        .post(format!(
            "{base}/registry/{}/push?device=fixture",
            account.id
        ))
        .bearer_auth(seed)
        .json(&json!({"batch":"seed","ops":ops}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    std::fs::write(&output,serde_json::to_vec_pretty(&json!({"api":format!("{origin}/v1"),"website":origin,"sync":base,"account":account.id,"chat":chat})).unwrap()).unwrap();
    println!("READY: dedicated simulator account and sync fixture");
    tokio::select! {_=tokio::signal::ctrl_c()=>{},_=stopped.changed()=>{}}
    task.abort();
    api_task.abort();
}
