use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender as SyncSender};
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::FromRequestParts;
use axum::extract::Request;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use reqwest::Url;
use serde::Serialize;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message as UpstreamMessage};

const ENV_LISTEN_ADDR: &str = "NEXUS_RELAY_LISTEN_ADDR";
const ENV_CONTROL_BASE_URL: &str = "NEXUS_RELAY_CONTROL_BASE_URL";
const ENV_UPSTREAM_LISTEN_ADDR: &str = "NEXUS_RELAY_UPSTREAM_LISTEN_ADDR";
const ENV_DATA_DIR: &str = "NEXUS_RELAY_DATA_DIR";
const ENV_PUBLIC_WS_URL: &str = "NEXUS_RELAY_PUBLIC_WS_URL";
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42110";
const DEFAULT_CONTROL_BASE_URL: &str = "http://127.0.0.1:42020";
const DEFAULT_UPSTREAM_LISTEN_ADDR: &str = "127.0.0.1:42111";
const DEFAULT_DATA_DIR: &str = ".nexus-relay-data";
const MAX_PROXY_REQUEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DurableRelayConfig {
    pub listen_addr: SocketAddr,
    pub control_base_url: String,
    pub upstream_listen_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub public_ws_url: String,
}

impl DurableRelayConfig {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = parse_socket_addr(ENV_LISTEN_ADDR, DEFAULT_LISTEN_ADDR)?;
        let upstream_listen_addr =
            parse_socket_addr(ENV_UPSTREAM_LISTEN_ADDR, DEFAULT_UPSTREAM_LISTEN_ADDR)?;
        if listen_addr == upstream_listen_addr {
            return Err(format!(
                "{ENV_LISTEN_ADDR} and {ENV_UPSTREAM_LISTEN_ADDR} must not be equal"
            ));
        }

        let control_base_url = std::env::var(ENV_CONTROL_BASE_URL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_CONTROL_BASE_URL.to_string());
        let data_dir = std::env::var(ENV_DATA_DIR)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
        let public_ws_url = std::env::var(ENV_PUBLIC_WS_URL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("ws://127.0.0.1:{}/", listen_addr.port()));

        Ok(Self {
            listen_addr,
            control_base_url,
            upstream_listen_addr,
            data_dir,
            public_ws_url,
        })
    }

    fn ensure_data_dir(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all(&self.data_dir)?;
        Ok(())
    }

    fn build_upstream_settings(&self) -> nostr_rs_relay_upstream::config::Settings {
        let mut settings = nostr_rs_relay_upstream::config::Settings::default();
        settings.info.name = Some("OpenAgents Nexus".to_string());
        settings.info.relay_url = Some(self.public_ws_url.clone());
        settings.network.address = self.upstream_listen_addr.ip().to_string();
        settings.network.port = self.upstream_listen_addr.port();
        settings.database.data_directory = self.data_dir.display().to_string();
        settings.authorization.nip42_auth = false;
        settings
    }

    fn upstream_http_url(&self) -> String {
        format!("http://{}/", self.upstream_listen_addr)
    }

    fn upstream_ws_url(&self) -> String {
        format!("ws://{}/", self.upstream_listen_addr)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub relay_backend: String,
    pub upstream_ws_url: String,
    pub control_base_url: String,
    pub data_directory: String,
}

#[derive(Clone)]
struct AppState {
    config: DurableRelayConfig,
    control_http_client: reqwest::Client,
}

pub async fn run_server(config: DurableRelayConfig) -> Result<(), anyhow::Error> {
    config.ensure_data_dir()?;
    let _upstream = UpstreamRelayHandle::spawn(config.clone()).await?;

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("nexus-relay durable shell listening on {}", local_addr);
    axum::serve(listener, build_router(config)).await?;
    Ok(())
}

fn build_router(config: DurableRelayConfig) -> Router {
    let state = AppState {
        control_http_client: reqwest::Client::new(),
        config,
    };
    Router::new()
        .route("/", any(relay_root))
        .route("/ws", any(relay_websocket))
        .route("/api/{*path}", any(proxy_control_request))
        .route("/v1/{*path}", any(proxy_control_request))
        .route("/healthz", get(healthz))
        .with_state(state)
}

async fn relay_root(State(state): State<AppState>, request: Request) -> Response {
    if is_websocket_upgrade_request(request.headers()) {
        return upgrade_websocket(state, request).await;
    }
    Html(render_homepage(&state.config)).into_response()
}

async fn relay_websocket(State(state): State<AppState>, request: Request) -> Response {
    upgrade_websocket(state, request).await
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    Json(HealthResponse {
        ok: true,
        service: "nexus-relay".to_string(),
        relay_backend: "durable-upstream".to_string(),
        upstream_ws_url: state.config.upstream_ws_url(),
        control_base_url: state.config.control_base_url.clone(),
        data_directory: state.config.data_dir.display().to_string(),
    })
}

async fn proxy_control_request(State(state): State<AppState>, request: Request) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read proxied request body: {error}"),
            )
                .into_response();
        }
    };
    let target = match proxy_target_url(state.config.control_base_url.as_str(), &uri) {
        Ok(url) => url,
        Err(error) => return (StatusCode::BAD_GATEWAY, error).into_response(),
    };

    let mut upstream_request = state.control_http_client.request(method, target);
    upstream_request = copy_proxy_request_headers(upstream_request, &headers);
    upstream_request = upstream_request.body(body_bytes.to_vec());

    let upstream = match upstream_request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to reach nexus-control upstream: {error}"),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let body_stream = upstream.bytes_stream().map_err(std::io::Error::other);
    let mut response = match Response::builder()
        .status(status)
        .body(Body::from_stream(body_stream))
    {
        Ok(response) => response,
        Err(error) => {
            let mut response = Response::new(Body::from(format!(
                "failed to build proxied response: {error}"
            )));
            *response.status_mut() = StatusCode::BAD_GATEWAY;
            return response;
        }
    };
    copy_proxy_response_headers(response.headers_mut(), &upstream_headers);
    response
}

async fn upgrade_websocket(state: AppState, request: Request) -> Response {
    let (mut parts, _body) = request.into_parts();
    match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(ws) => {
            let upstream_ws_url = state.config.upstream_ws_url();
            ws.on_upgrade(move |socket| proxy_websocket(socket, upstream_ws_url))
                .into_response()
        }
        Err(rejection) => rejection.into_response(),
    }
}

async fn proxy_websocket(socket: WebSocket, upstream_ws_url: String) {
    let upstream = connect_async(upstream_ws_url.as_str()).await;
    let (upstream, _) = match upstream {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("failed to connect to durable upstream relay: {error}");
            return;
        }
    };

    let (mut client_sink, mut client_stream) = socket.split();
    let (mut upstream_sink, mut upstream_stream) = upstream.split();

    let client_to_upstream = async {
        while let Some(message) = client_stream.next().await {
            let message = match message {
                Ok(message) => message,
                Err(error) => return Err(anyhow::anyhow!("client websocket error: {error}")),
            };
            let message = client_message_to_upstream(message);
            let should_close = matches!(message, UpstreamMessage::Close(_));
            upstream_sink.send(message).await?;
            if should_close {
                break;
            }
        }
        anyhow::Ok(())
    };

    let upstream_to_client = async {
        while let Some(message) = upstream_stream.next().await {
            let message = message?;
            let should_close = matches!(message, UpstreamMessage::Close(_));
            if let Some(message) = upstream_message_to_client(message) {
                client_sink.send(message).await?;
            }
            if should_close {
                break;
            }
        }
        anyhow::Ok(())
    };

    tokio::select! {
        result = client_to_upstream => {
            if let Err(error) = result {
                tracing::warn!("client->upstream websocket proxy ended with error: {error}");
            }
        }
        result = upstream_to_client => {
            if let Err(error) = result {
                tracing::warn!("upstream->client websocket proxy ended with error: {error}");
            }
        }
    }
}

fn client_message_to_upstream(message: Message) -> UpstreamMessage {
    match message {
        Message::Text(text) => UpstreamMessage::Text(text.to_string()),
        Message::Binary(bytes) => UpstreamMessage::Binary(bytes.to_vec()),
        Message::Ping(bytes) => UpstreamMessage::Ping(bytes.to_vec()),
        Message::Pong(bytes) => UpstreamMessage::Pong(bytes.to_vec()),
        Message::Close(frame) => UpstreamMessage::Close(frame.map(|frame| {
            tungstenite::protocol::CloseFrame {
                code: CloseCode::from(u16::from(frame.code)),
                reason: frame.reason.to_string().into(),
            }
        })),
    }
}

fn upstream_message_to_client(message: UpstreamMessage) -> Option<Message> {
    match message {
        UpstreamMessage::Text(text) => Some(Message::Text(text.into())),
        UpstreamMessage::Binary(bytes) => Some(Message::Binary(bytes.into())),
        UpstreamMessage::Ping(bytes) => Some(Message::Ping(bytes.into())),
        UpstreamMessage::Pong(bytes) => Some(Message::Pong(bytes.into())),
        UpstreamMessage::Close(frame) => Some(Message::Close(frame.map(|frame| CloseFrame {
            code: u16::from(frame.code).into(),
            reason: frame.reason.to_string().into(),
        }))),
        UpstreamMessage::Frame(_) => None,
    }
}

fn render_homepage(config: &DurableRelayConfig) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>OpenAgents Nexus Relay</title></head><body><h1>OpenAgents Nexus Relay</h1><p>Durable relay proxy is running.</p><ul><li>Relay websocket: {}</li><li>Control upstream: {}</li><li>Data dir: {}</li></ul></body></html>",
        config.public_ws_url,
        config.control_base_url,
        config.data_dir.display(),
    )
}

fn proxy_target_url(base_url: &str, uri: &Uri) -> Result<Url, String> {
    let base = Url::parse(base_url).map_err(|error| format!("invalid control base url: {error}"))?;
    let path_and_query = uri
        .path_and_query()
        .map_or_else(|| uri.path().to_string(), std::string::ToString::to_string);
    base.join(path_and_query.as_str())
        .map_err(|error| format!("invalid proxied path: {error}"))
}

fn copy_proxy_request_headers(
    mut request: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for (name, value) in headers {
        if should_skip_proxy_request_header(name) {
            continue;
        }
        request = request.header(name, value);
    }
    request
}

fn copy_proxy_response_headers(target: &mut HeaderMap, source: &HeaderMap) {
    for (name, value) in source {
        if should_skip_proxy_response_header(name) {
            continue;
        }
        target.insert(name, value.clone());
    }
}

fn should_skip_proxy_request_header(name: &header::HeaderName) -> bool {
    matches!(
        name,
        &header::HOST
            | &header::CONNECTION
            | &header::CONTENT_LENGTH
            | &header::TRANSFER_ENCODING
    )
}

fn should_skip_proxy_response_header(name: &header::HeaderName) -> bool {
    matches!(
        name,
        &header::CONNECTION
            | &header::CONTENT_LENGTH
            | &header::TRANSFER_ENCODING
    )
}

fn is_websocket_upgrade_request(headers: &HeaderMap) -> bool {
    headers.contains_key(header::UPGRADE)
}

fn parse_socket_addr(name: &str, default: &str) -> Result<SocketAddr, String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid {name}: {error}"))
}

#[cfg_attr(not(test), allow(dead_code))]
struct UpstreamRelayHandle {
    shutdown_tx: SyncSender<()>,
    join: JoinHandle<anyhow::Result<()>>,
}

impl UpstreamRelayHandle {
    async fn spawn(config: DurableRelayConfig) -> Result<Self, anyhow::Error> {
        let settings = config.build_upstream_settings();
        let readiness_url = config.upstream_http_url();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let join = tokio::task::spawn_blocking(move || {
            nostr_rs_relay_upstream::server::start_server(&settings, shutdown_rx)
                .map_err(anyhow::Error::from)
        });
        wait_for_upstream_ready(readiness_url.as_str()).await?;
        Ok(Self { shutdown_tx, join })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    async fn shutdown(self) -> Result<(), anyhow::Error> {
        let _ = self.shutdown_tx.send(());
        self.join.await??;
        Ok(())
    }
}

async fn wait_for_upstream_ready(url: &str) -> Result<(), anyhow::Error> {
    let client = reqwest::Client::new();
    for _ in 0..100 {
        let response = client.get(url).send().await;
        match response {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    Err(anyhow::anyhow!(
        "timed out waiting for durable upstream relay at {url}"
    ))
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use axum::http::StatusCode;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value;
    use std::net::SocketAddr;
    use std::path::Path;
    use tempfile::tempdir;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    use super::{DurableRelayConfig, UpstreamRelayHandle};

    const SAMPLE_EVENT: &str = r#"["EVENT", {"content": "hello world","created_at": 1691239763,"id":"f3ce6798d70e358213ebbeba4886bbdfacf1ecfd4f65ee5323ef5f404de32b86","kind": 1,"pubkey": "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","sig": "30ca29e8581eeee75bf838171dec818af5e6de2b74f5337de940f5cc91186534c0b20d6cf7ad1043a2c51dbd60b979447720a471d346322103c83f6cb66e4e98","tags": []}]"#;

    fn unused_socket_addr() -> Result<SocketAddr> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);
        Ok(addr)
    }

    fn durable_config(data_dir: &Path) -> Result<DurableRelayConfig> {
        let listen_addr = unused_socket_addr()?;
        let upstream_listen_addr = unused_socket_addr()?;
        Ok(DurableRelayConfig {
            listen_addr,
            control_base_url: "http://127.0.0.1:42020".to_string(),
            upstream_listen_addr,
            data_dir: data_dir.to_path_buf(),
            public_ws_url: format!("ws://{listen_addr}/"),
        })
    }

    #[tokio::test]
    async fn durable_upstream_persists_events_across_restart() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let relay_url = config.upstream_ws_url();

        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (mut publisher, _) = connect_async(relay_url.as_str()).await?;
        publisher
            .send(WsMessage::Text(SAMPLE_EVENT.to_string().into()))
            .await?;
        let publish_ok = timeout(Duration::from_secs(2), publisher.next()).await;
        let publish_ok = publish_ok
            .ok()
            .and_then(|frame| frame)
            .ok_or_else(|| anyhow::anyhow!("expected publish confirmation"))??;
        let publish_ok = match publish_ok {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected publish frame: {other:?}")),
        };
        assert!(publish_ok.contains("\"OK\""));

        relay.shutdown().await?;

        let restarted = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (mut subscriber, _) = connect_async(relay_url.as_str()).await?;
        subscriber
            .send(WsMessage::Text(r#"["REQ", "durable-check", {}]"#.to_string().into()))
            .await?;

        let first = timeout(Duration::from_secs(2), subscriber.next()).await;
        let first = first
            .ok()
            .and_then(|frame| frame)
            .ok_or_else(|| anyhow::anyhow!("expected replayed event"))??;
        let first = match first {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected replay frame: {other:?}")),
        };
        assert!(first.contains("\"EVENT\""));
        assert!(first.contains("\"hello world\""));

        let second = timeout(Duration::from_secs(2), subscriber.next()).await;
        let second = second
            .ok()
            .and_then(|frame| frame)
            .ok_or_else(|| anyhow::anyhow!("expected EOSE after replay"))??;
        let second = match second {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected EOSE frame: {other:?}")),
        };
        assert!(second.contains("\"EOSE\""));

        restarted.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_healthz_reports_upstream_backend() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let expected_upstream = config.upstream_ws_url();
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;

        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, super::build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let response = reqwest::get(format!("http://{addr}/healthz")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let health: Value = response.json().await?;
        assert_eq!(health["relay_backend"], "durable-upstream");
        assert_eq!(health["upstream_ws_url"], expected_upstream);

        server.abort();
        relay.shutdown().await?;
        Ok(())
    }
}
