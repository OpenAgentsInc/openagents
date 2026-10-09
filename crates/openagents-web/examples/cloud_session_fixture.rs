//! Fresh loopback-only account and web servers for browser acceptance.
//!
//! The two synthetic credentials are `oak_alice.synthetic-credential` and
//! `oak_bob.synthetic-credential`. This example never reads an account store.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openagents_web::cloud::session::CloudSession;
use serde_json::json;
use std::collections::BTreeSet;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct Native {
    revoked: BTreeSet<String>,
    expiry: u64,
    signins: usize,
    signouts: usize,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_secs()
}

fn token(account: &str) -> String {
    format!(
        "sess_{}",
        if account == "alice" { "a" } else { "b" }.repeat(64)
    )
}

fn acting(headers: &HeaderMap, state: &Native) -> Option<&'static str> {
    let bearer = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    ["alice", "bob"]
        .into_iter()
        .find(|account| bearer == token(account) && !state.revoked.contains(*account))
}

fn refusal(status: StatusCode) -> Response {
    (
        status,
        Json(json!({"error":{"code":"unauthenticated","message":"Synthetic account refusal"}})),
    )
        .into_response()
}

async fn sign_in(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(account) = ["alice", "bob"]
        .into_iter()
        .find(|account| bearer == Some(format!("oak_{account}.synthetic-credential").as_str()))
    else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    let mut state = state.lock().expect("fixture state");
    state.signins += 1;
    state.revoked.remove(account);
    Json(json!({"session":{"id":format!("native-session-{account}"),"kind":"user","account":account,"created_at":now(),"expires_at":state.expiry},"token":token(account)})).into_response()
}

async fn session(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let state = state.lock().expect("fixture state");
    let Some(account) = acting(&headers, &state) else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    Json(json!({"session":{"id":format!("native-session-{account}"),"kind":"user","account":account,"created_at":now()-1,"expires_at":state.expiry,"state":"active"}})).into_response()
}

async fn account(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let state = state.lock().expect("fixture state");
    let Some(account) = acting(&headers, &state) else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    let workspaces = if account == "alice" {
        vec![
            json!({"id":"alice-personal","name":"Alice personal","role":"owner"}),
            json!({"id":"alice-team","name":"Alice team","role":"member"}),
        ]
    } else {
        vec![json!({"id":"bob-personal","name":"Bob personal","role":"owner"})]
    };
    Json(json!({"account":{"id":account,"label":format!("Synthetic {account}"),"principals":["synthetic-native-private-canary"]},"workspaces":workspaces})).into_response()
}

async fn workspace(
    State(state): State<Arc<Mutex<Native>>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().expect("fixture state");
    let Some(account) = acting(&headers, &state) else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    if !(account == "alice" && matches!(id.as_str(), "alice-personal" | "alice-team")
        || account == "bob" && id == "bob-personal")
    {
        return refusal(StatusCode::FORBIDDEN);
    }
    Json(json!({"workspace":{"id":id,"tenant":"synthetic","members_epoch":3},"role":if id == "alice-team" {"member"} else {"owner"}})).into_response()
}

async fn sign_out(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut state = state.lock().expect("fixture state");
    let Some(account) = acting(&headers, &state) else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    state.revoked.insert(account.into());
    state.signouts += 1;
    Json(json!({"session":{"state":"revoked"}})).into_response()
}

async fn counts(State(state): State<Arc<Mutex<Native>>>) -> Json<serde_json::Value> {
    let state = state.lock().expect("fixture state");
    Json(json!({"synthetic":true,"signins":state.signins,"signouts":state.signouts}))
}

fn private_file(path: &FilePath, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|_| "fixture private file creation failed".into())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: cloud_session_fixture NEW_SCRATCH_DIRECTORY CLOUD_WASM_DIRECTORY 127.0.0.1:PORT [PRIVATE_HOSTS_FILE]".into());
    }
    serve(
        &args[0],
        &args[1],
        &args[2],
        args.get(3).map(String::as_str),
    )
    .await
}

pub async fn serve(
    directory: &str,
    build: &str,
    listen: &str,
    hosts: Option<&str>,
) -> Result<(), String> {
    let directory = PathBuf::from(directory);
    let build = PathBuf::from(build);
    let listen: SocketAddr = listen
        .parse()
        .map_err(|_| "invalid fixture listen address")?;
    if !directory.is_absolute()
        || !build.is_absolute()
        || listen.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST)
    {
        return Err("fixture requires absolute paths and a 127.0.0.1 listen address".into());
    }
    std::fs::create_dir(&directory).map_err(|_| "fixture directory must be new")?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "fixture directory permissions failed")?;
    let directory = directory
        .canonicalize()
        .map_err(|_| "fixture path failed")?;
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|_| "fixture web listener failed")?;
    let address = listener
        .local_addr()
        .map_err(|_| "fixture web address failed")?;
    let native_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "fixture account listener failed")?;
    let native_origin = format!(
        "http://{}",
        native_listener
            .local_addr()
            .map_err(|_| "fixture account address failed")?
    );
    let origin = format!("http://{address}");
    let secret = directory.join("csrf.key");
    private_file(&secret, &secp256k1::rand::random::<[u8; 32]>())?;
    let path = directory.join("cloud.json");
    private_file(&path, &serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":native_origin,"csrf_secret":secret})).map_err(|_| "fixture configuration failed")?)?;
    let mut config = openagents_web::Config::development(directory.join("unopened-local-tasks"));
    config.port = address.port();
    config.cloud = Some(Arc::new(CloudSession::load(&path)?));
    config.cloud_build = Some(build);
    config.components_build = std::env::var_os("CLOUD_FIXTURE_COMPONENTS_BUILD").map(PathBuf::from);
    // A scratch store for the visitor's own Claude credential, so Settings
    // offers "Claude credential" (BYO-04) in local tests.
    let byo = directory.join("byo");
    std::fs::create_dir_all(&byo).map_err(|_| "fixture credential store failed")?;
    std::fs::set_permissions(&byo, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "fixture credential store failed")?;
    // A scratch keyring outside the store: saved credentials are encrypted
    // at rest under it (#11041). It lives and dies with this fixture.
    let keys = directory.join("byo-keys.json");
    let (_, document) = oa_seal::Keyring::scratch("fixture")?;
    private_file(&keys, document.as_bytes())?;
    config.cloud_byo = Some(Arc::new(openagents_web::cloud::byo::Computers::open(
        &byo,
        oa_seal::Keyring::load(&keys)?,
    )?));
    config.cloud_hosts = hosts
        .map(|path| openagents_web::cloud::hosts::Hosts::load(FilePath::new(path)))
        .transpose()?
        .map(Arc::new);
    let state = Arc::new(Mutex::new(Native {
        revoked: BTreeSet::new(),
        expiry: now() + 3600,
        signins: 0,
        signouts: 0,
    }));
    let native = Router::new()
        .route("/v1/sessions", post(sign_in))
        .route("/v1/session", get(session).delete(sign_out))
        .route("/v1/account", get(account))
        .route("/v1/workspaces/{id}", get(workspace))
        .route("/fixture/counts", get(counts))
        .with_state(state);
    let native_server = tokio::spawn(async move {
        axum::serve(native_listener, native)
            .await
            .expect("fixture account server");
    });
    println!(
        "{}",
        json!({"synthetic":true,"origin":origin,"account_service":native_origin,"config":path})
    );
    let result = axum::serve(listener, openagents_web::router(config))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    native_server.abort();
    result.map_err(|_| "fixture web server stopped".into())
}
