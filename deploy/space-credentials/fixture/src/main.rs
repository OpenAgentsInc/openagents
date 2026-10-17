//! Controlled model and private Git endpoints for synthetic qualification only.
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use computer_credentials::{
    SecretProvider,
    secret_manager::{GoogleSecretManager, Version},
};
use serde_json::{Value, json};
use std::{convert::Infallible, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;

struct Fixture {
    provider: GoogleSecretManager,
    version: Version,
    root: PathBuf,
}
impl Fixture {
    async fn authorized(&self, headers: &HeaderMap, git: bool) -> bool {
        let values = headers.get_all(if git { "authorization" } else { "x-api-key" });
        if values.iter().count() != 1 {
            return false;
        }
        let Some(value) = values.iter().next() else {
            return false;
        };
        let Ok(secret) = self.provider.access(&self.version).await else {
            return false;
        };
        let expected = if git {
            let mut bytes = Zeroizing::new(b"git:".to_vec());
            bytes.extend_from_slice(secret.bytes());
            Zeroizing::new(format!("Basic {}", STANDARD.encode(&bytes)))
        } else {
            let Ok(value) = std::str::from_utf8(secret.bytes()) else {
                return false;
            };
            Zeroizing::new(value.to_owned())
        };
        value.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() == 1
    }
    async fn bearer(&self, headers: &HeaderMap) -> bool {
        let values = headers.get_all(header::AUTHORIZATION);
        if values.iter().count() != 1 {
            return false;
        }
        let Some(value) = values
            .iter()
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
        else {
            return false;
        };
        let Ok(secret) = self.provider.access(&self.version).await else {
            return false;
        };
        value.as_bytes().ct_eq(secret.bytes()).unwrap_u8() == 1
    }
}
async fn github_user(State(f): State<Arc<Fixture>>, headers: HeaderMap) -> Response {
    if !f.bearer(&headers).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    println!("github_client_request_ok");
    Json(json!({"login":"credential-fixture","id":11813,"type":"User"})).into_response()
}
async fn messages(State(f): State<Arc<Fixture>>, headers: HeaderMap, body: Bytes) -> Response {
    if !f.authorized(&headers, false).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(body) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let model = body["model"]
        .as_str()
        .filter(|s| s.len() <= 128)
        .unwrap_or("fixture-model");
    let message = json!({"id":"msg_credential_qualification","type":"message","role":"assistant","model":model,
        "content":[{"type":"text","text":"SDK_PROXY_OK"}],"stop_reason":"end_turn","stop_sequence":null,
        "usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}});
    println!("model_request_ok");
    if body["stream"] != true {
        return axum::Json(message).into_response();
    }
    let mut start = message;
    start["content"] = json!([]);
    start["stop_reason"] = Value::Null;
    let events = [
        (
            "message_start",
            json!({"type":"message_start","message":start}),
        ),
        (
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        ),
        (
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"SDK_PROXY_OK"}}),
        ),
        (
            "content_block_stop",
            json!({"type":"content_block_stop","index":0}),
        ),
        (
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}),
        ),
        ("message_stop", json!({"type":"message_stop"})),
    ];
    let (send, receive) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        for (name, value) in events {
            if send
                .send(Ok::<_, Infallible>(
                    Event::default().event(name).data(value.to_string()),
                ))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    Sse::new(tokio_stream::wrappers::ReceiverStream::new(receive)).into_response()
}
async fn redirect(State(f): State<Arc<Fixture>>, headers: HeaderMap) -> Response {
    if !f.authorized(&headers, false).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    (
        StatusCode::TEMPORARY_REDIRECT,
        [(header::LOCATION, "https://example.com/")],
    )
        .into_response()
}
async fn git(
    State(f): State<Arc<Fixture>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !f.authorized(&headers, true).await {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"qualification\"")],
        )
            .into_response();
    }
    if !matches!(method, Method::GET | Method::POST)
        || !matches!(
            uri.path(),
            "/repo.git/info/refs" | "/repo.git/git-upload-pack"
        )
        || uri.query().is_some_and(|s| s != "service=git-upload-pack")
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let task = async {
        let mut child = tokio::process::Command::new("/usr/bin/git")
            .arg("http-backend")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &f.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_PROJECT_ROOT", &f.root)
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("REQUEST_METHOD", method.as_str())
            .env("PATH_INFO", uri.path())
            .env("QUERY_STRING", uri.query().unwrap_or(""))
            .env("CONTENT_TYPE", content_type)
            .env("CONTENT_LENGTH", body.len().to_string())
            .env("SERVER_PROTOCOL", "HTTP/1.1")
            .env("REMOTE_USER", "git")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| ())?;
        child
            .stdin
            .take()
            .ok_or(())?
            .write_all(&body)
            .await
            .map_err(|_| ())?;
        child.wait_with_output().await.map_err(|_| ())
    };
    let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(15), task).await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !output.status.success() || output.stdout.len() > 2 * 1024 * 1024 {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let Some(split) = output.stdout.windows(4).position(|s| s == b"\r\n\r\n") else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let Ok(head) = std::str::from_utf8(&output.stdout[..split]) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let mut response = Response::builder().status(200);
    for line in head.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        if name.eq_ignore_ascii_case("status") {
            let Some(status) = value
                .trim()
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u16>().ok())
            else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            response = response.status(status);
        } else if matches!(
            name.to_ascii_lowercase().as_str(),
            "content-type" | "cache-control" | "expires" | "pragma"
        ) {
            response = response.header(name, value.trim());
        }
    }
    println!("git_request_ok");
    response
        .body(Body::from(output.stdout[split + 4..].to_vec()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}
async fn run() -> Result<(), ()> {
    let version = Version::parse(&std::env::var("FIXTURE_SECRET_VERSION").map_err(|_| ())?)
        .map_err(|_| ())?;
    let root = PathBuf::from("/srv/fixture");
    let fixture = Arc::new(Fixture {
        provider: GoogleSecretManager::from_metadata().map_err(|_| ())?,
        version,
        root,
    });
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/redirect", post(redirect))
        .route("/repo.git/info/refs", get(git))
        .route("/api/v3/user", get(github_user))
        .route("/repo.git/git-upload-pack", post(git))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(fixture);
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|_| ())?;
    axum::serve(listener, router).await.map_err(|_| ())
}
#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("The qualification fixture failed.");
        std::process::exit(1);
    }
}
