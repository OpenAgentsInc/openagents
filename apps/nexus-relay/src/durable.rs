use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc::{self, Sender as SyncSender};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes, to_bytes};
use axum::extract::FromRequestParts;
use axum::extract::Request;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use reqwest::Url;
use serde::Serialize;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message as UpstreamMessage};

const ENV_LISTEN_ADDR: &str = "NEXUS_RELAY_LISTEN_ADDR";
const ENV_UPSTREAM_LISTEN_ADDR: &str = "NEXUS_RELAY_UPSTREAM_LISTEN_ADDR";
const ENV_DATA_DIR: &str = "NEXUS_RELAY_DATA_DIR";
const ENV_PUBLIC_WS_URL: &str = "NEXUS_RELAY_PUBLIC_WS_URL";
const ENV_UPSTREAM_CONFIG_FILE: &str = "NEXUS_RELAY_UPSTREAM_CONFIG_FILE";
const ENV_ENABLE_NIP42_AUTH: &str = "NEXUS_RELAY_ENABLE_NIP42_AUTH";
const ENV_MAX_WEBSOCKETS: &str = "NEXUS_RELAY_MAX_WEBSOCKETS";
const ENV_AUTHORITY_MAX_IN_FLIGHT: &str = "NEXUS_RELAY_AUTHORITY_MAX_IN_FLIGHT";
const ENV_AUTHORITY_TOKIO_WORKER_THREADS: &str = "NEXUS_RELAY_AUTHORITY_TOKIO_WORKER_THREADS";
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42110";
const DEFAULT_UPSTREAM_LISTEN_ADDR: &str = "127.0.0.1:42111";
const DEFAULT_DATA_DIR: &str = ".nexus-relay-data";
const DEFAULT_MAX_WEBSOCKETS: usize = 512;
const DEFAULT_AUTHORITY_MAX_IN_FLIGHT: usize = 256;
const DEFAULT_AUTHORITY_TOKIO_WORKER_THREADS: usize = 4;
const MAX_PROXY_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const HOT_AUTHORITY_CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const HOT_AUTHORITY_CACHE_MAX_STALE: Duration = Duration::from_secs(300);
const HOT_AUTHORITY_CACHE_PATHS: &[&str] = &["/stats", "/api/stats", "/api/training/rollout"];
const NEXUS_PRODUCT_NAME: &str = "OpenAgents Nexus";
const NEXUS_PRODUCT_DESCRIPTION: &str = "The OpenAgents relay and authority host for Autopilot.";
const MANAGED_GROUPS_MODE_DEFERRED: &str = "deferred";
const HOMEPAGE_TEMPLATE: &str = include_str!("homepage_template.html");

#[derive(Debug, Clone)]
pub struct DurableRelayConfig {
    pub listen_addr: SocketAddr,
    pub upstream_listen_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub public_ws_url: String,
    pub upstream_config_file: Option<PathBuf>,
    pub enable_nip42_auth: bool,
    pub max_websockets: usize,
    pub authority_max_in_flight: usize,
    pub authority_tokio_worker_threads: usize,
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
        let upstream_config_file = std::env::var(ENV_UPSTREAM_CONFIG_FILE)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let enable_nip42_auth = std::env::var(ENV_ENABLE_NIP42_AUTH)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map_or(Ok(true), |value| {
                parse_bool_env(ENV_ENABLE_NIP42_AUTH, &value)
            })?;
        let max_websockets = parse_usize_env(ENV_MAX_WEBSOCKETS, DEFAULT_MAX_WEBSOCKETS)?;
        let authority_max_in_flight =
            parse_usize_env(ENV_AUTHORITY_MAX_IN_FLIGHT, DEFAULT_AUTHORITY_MAX_IN_FLIGHT)?.max(1);
        let authority_tokio_worker_threads = parse_usize_env(
            ENV_AUTHORITY_TOKIO_WORKER_THREADS,
            DEFAULT_AUTHORITY_TOKIO_WORKER_THREADS,
        )?
        .max(1);

        Ok(Self {
            listen_addr,
            upstream_listen_addr,
            data_dir,
            public_ws_url,
            upstream_config_file,
            enable_nip42_auth,
            max_websockets,
            authority_max_in_flight,
            authority_tokio_worker_threads,
        })
    }

    fn ensure_data_dir(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all(&self.data_dir)?;
        Ok(())
    }

    fn build_upstream_settings(
        &self,
    ) -> Result<nostr_rs_relay_upstream::config::Settings, anyhow::Error> {
        let mut settings = if let Some(path) = &self.upstream_config_file {
            nostr_rs_relay_upstream::config::Settings::new(&Some(path.display().to_string()))
                .map_err(|error| anyhow::anyhow!("failed to load upstream relay config: {error}"))?
        } else {
            nostr_rs_relay_upstream::config::Settings::default()
        };
        settings.info.name = Some(NEXUS_PRODUCT_NAME.to_string());
        settings.info.description = Some(NEXUS_PRODUCT_DESCRIPTION.to_string());
        settings.info.relay_url = Some(self.public_ws_url.clone());
        settings.info.relay_page = self.public_http_url();
        settings.network.address = self.upstream_listen_addr.ip().to_string();
        settings.network.port = self.upstream_listen_addr.port();
        settings.database.data_directory = self.data_dir.display().to_string();
        settings.authorization.nip42_auth = self.enable_nip42_auth;
        Ok(settings)
    }

    fn upstream_http_url(&self) -> String {
        format!("http://{}/", self.upstream_listen_addr)
    }

    fn upstream_ws_url(&self) -> String {
        format!("ws://{}/", self.upstream_listen_addr)
    }

    fn public_http_url(&self) -> Option<String> {
        let mut url = Url::parse(self.public_ws_url.as_str()).ok()?;
        match url.scheme() {
            "ws" => url.set_scheme("http").ok()?,
            "wss" => url.set_scheme("https").ok()?,
            _ => return None,
        }
        Some(url.to_string())
    }

    fn upstream_http_url_for_path(&self, path_and_query: &str) -> Result<Url, String> {
        let base = Url::parse(self.upstream_http_url().as_str())
            .map_err(|error| format!("invalid upstream base url: {error}"))?;
        base.join(path_and_query)
            .map_err(|error| format!("invalid upstream path: {error}"))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub relay_backend: String,
    pub upstream_ws_url: String,
    pub authority_mode: String,
    pub managed_groups_mode: String,
    pub data_directory: String,
}

#[derive(Clone)]
struct AppState {
    config: DurableRelayConfig,
    http_client: reqwest::Client,
    websocket_slots: Arc<Semaphore>,
    authority_slots: Arc<Semaphore>,
    authority_http_base_url: Url,
    authority_hot_cache: Arc<RwLock<HashMap<String, CachedAuthorityResponse>>>,
}

#[derive(Clone)]
struct CachedAuthorityResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    cached_at: Instant,
}

pub async fn run_server(config: DurableRelayConfig) -> Result<(), anyhow::Error> {
    config.ensure_data_dir()?;
    let authority_config = build_authority_config(&config)?;

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    let authority_http_base_url =
        spawn_authority_server(authority_config, config.authority_tokio_worker_threads)?;
    let upstream = UpstreamRelayHandle::spawn(config.clone()).await?;
    tracing::info!("nexus-relay durable shell accepting on {}", local_addr);
    let serve_result = axum::serve(listener, build_router(config, authority_http_base_url)).await;
    let shutdown_result = upstream.shutdown().await;
    serve_result.map_err(anyhow::Error::from)?;
    shutdown_result?;
    Ok(())
}

fn spawn_authority_server(
    authority_config: nexus_control::ServiceConfig,
    authority_tokio_worker_threads: usize,
) -> Result<Url, anyhow::Error> {
    let authority_http_base_url = authority_url_for_addr(authority_config.listen_addr)?;
    let authority_listen_addr = authority_config.listen_addr;
    std::thread::Builder::new()
        .name("nexus-control-api".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(authority_tokio_worker_threads)
                .thread_name("nexus-control-api")
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(
                        error = %error,
                        "failed to build embedded Nexus control API runtime"
                    );
                    return;
                }
            };
            tracing::info!(
                listen_addr = %authority_listen_addr,
                tokio_worker_threads = authority_tokio_worker_threads,
                "starting embedded Nexus control API server"
            );
            if let Err(error) = runtime.block_on(nexus_control::run_server(authority_config)) {
                tracing::error!(
                    error = %error,
                    "embedded Nexus control API server exited"
                );
            }
        })?;
    Ok(authority_http_base_url)
}

fn authority_url_for_addr(addr: std::net::SocketAddr) -> Result<Url, anyhow::Error> {
    Url::parse(format!("http://{addr}/").as_str())
        .map_err(|error| anyhow::anyhow!("invalid embedded authority URL for {addr}: {error}"))
}

fn build_router(config: DurableRelayConfig, authority_http_base_url: Url) -> Router {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "failed to build Nexus relay HTTP client; using default client"
            );
            reqwest::Client::new()
        });
    let state = AppState {
        http_client,
        websocket_slots: Arc::new(Semaphore::new(config.max_websockets)),
        authority_slots: Arc::new(Semaphore::new(config.authority_max_in_flight)),
        authority_http_base_url,
        authority_hot_cache: Arc::new(RwLock::new(HashMap::new())),
        config,
    };
    spawn_authority_hot_cache_refresh_loop(state.clone());
    Router::new()
        .route("/", any(relay_root))
        .route("/ws", any(relay_websocket))
        .route("/metrics", get(proxy_upstream_metrics))
        .route("/healthz", get(healthz))
        .fallback(proxy_authority_request)
        .with_state(state)
}

async fn proxy_authority_request(State(state): State<AppState>, request: Request) -> Response {
    proxy_authority_http_request(&state, request).await
}

async fn relay_root(State(state): State<AppState>, request: Request) -> Response {
    if is_websocket_upgrade_request(request.headers()) {
        return upgrade_websocket(state, request).await;
    }
    if is_nip11_request(request.headers()) {
        return proxy_upstream_request(&state, request, true).await;
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
        authority_mode: "in-process".to_string(),
        managed_groups_mode: MANAGED_GROUPS_MODE_DEFERRED.to_string(),
        data_directory: state.config.data_dir.display().to_string(),
    })
}

async fn proxy_upstream_metrics(State(state): State<AppState>) -> Response {
    let request = match Request::builder().uri("/metrics").body(Body::empty()) {
        Ok(request) => request,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to build upstream metrics request: {error}"),
            )
                .into_response();
        }
    };
    proxy_upstream_request(&state, request, false).await
}

async fn proxy_upstream_request(
    state: &AppState,
    request: Request,
    preserve_accept: bool,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read proxied upstream request body: {error}"),
            )
                .into_response();
        }
    };
    let path_and_query = uri
        .path_and_query()
        .map_or_else(|| uri.path().to_string(), std::string::ToString::to_string);
    let target = match state
        .config
        .upstream_http_url_for_path(path_and_query.as_str())
    {
        Ok(url) => url,
        Err(error) => return (StatusCode::BAD_GATEWAY, error).into_response(),
    };

    let mut upstream_request = state.http_client.request(method, target);
    for (name, value) in &headers {
        if should_skip_proxy_request_header(name) {
            continue;
        }
        if !preserve_accept && *name == header::ACCEPT {
            continue;
        }
        upstream_request = upstream_request.header(name, value);
    }
    upstream_request = upstream_request.body(body_bytes.to_vec());

    let upstream = match upstream_request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to reach durable upstream relay: {error}"),
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
                "failed to build upstream relay response: {error}"
            )));
            *response.status_mut() = StatusCode::BAD_GATEWAY;
            return response;
        }
    };
    copy_proxy_response_headers(response.headers_mut(), &upstream_headers);
    response
}

async fn proxy_authority_http_request(state: &AppState, request: Request) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let cache_key = authority_hot_cache_key(&method, uri.path());
    if let Some(cache_key) = cache_key.as_deref()
        && let Some(response) = try_cached_authority_response(state, cache_key)
    {
        return response;
    }
    let _permit = match state.authority_slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "embedded Nexus control API capacity exhausted",
            )
                .into_response();
        }
    };
    let headers = request.headers().clone();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read proxied Nexus control request body: {error}"),
            )
                .into_response();
        }
    };
    let path_and_query = uri
        .path_and_query()
        .map_or_else(|| uri.path().to_string(), std::string::ToString::to_string);
    let target = match state.authority_http_base_url.join(path_and_query.as_str()) {
        Ok(url) => url,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("invalid embedded Nexus control API path: {error}"),
            )
                .into_response();
        }
    };

    let mut authority_request = state.http_client.request(method, target);
    for (name, value) in &headers {
        if should_skip_proxy_request_header(name) {
            continue;
        }
        authority_request = authority_request.header(name, value);
    }
    authority_request = authority_request.body(body_bytes.to_vec());

    let authority = match authority_request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to reach embedded Nexus control API: {error}"),
            )
                .into_response();
        }
    };

    let status = authority.status();
    let authority_headers = authority.headers().clone();
    let body_bytes = match authority.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to read embedded Nexus control API response: {error}"),
            )
                .into_response();
        }
    };
    if let Some(cache_key) = cache_key {
        store_authority_hot_cache(
            state,
            cache_key,
            status,
            authority_headers.clone(),
            body_bytes.clone(),
        );
    }
    let mut response = match Response::builder()
        .status(status)
        .body(Body::from(body_bytes))
    {
        Ok(response) => response,
        Err(error) => {
            let mut response = Response::new(Body::from(format!(
                "failed to build embedded Nexus control API response: {error}"
            )));
            *response.status_mut() = StatusCode::BAD_GATEWAY;
            return response;
        }
    };
    copy_proxy_response_headers(response.headers_mut(), &authority_headers);
    response
}

fn authority_hot_cache_key(method: &Method, path: &str) -> Option<String> {
    if method != Method::GET {
        return None;
    }
    HOT_AUTHORITY_CACHE_PATHS
        .iter()
        .any(|candidate| *candidate == path)
        .then(|| path.to_string())
}

fn try_cached_authority_response(state: &AppState, cache_key: &str) -> Option<Response> {
    let cached = state
        .authority_hot_cache
        .read()
        .ok()?
        .get(cache_key)
        .cloned()?;
    if cached.cached_at.elapsed() > HOT_AUTHORITY_CACHE_MAX_STALE {
        return None;
    }
    let mut response = Response::builder()
        .status(cached.status)
        .body(Body::from(cached.body))
        .ok()?;
    copy_proxy_response_headers(response.headers_mut(), &cached.headers);
    Some(response)
}

fn store_authority_hot_cache(
    state: &AppState,
    cache_key: String,
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
) {
    if !status.is_success() {
        return;
    }
    if let Ok(mut cache) = state.authority_hot_cache.write() {
        cache.insert(
            cache_key,
            CachedAuthorityResponse {
                status,
                headers,
                body,
                cached_at: Instant::now(),
            },
        );
    }
}

fn spawn_authority_hot_cache_refresh_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HOT_AUTHORITY_CACHE_REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            for path in HOT_AUTHORITY_CACHE_PATHS {
                refresh_authority_hot_cache_path(&state, path).await;
            }
        }
    });
}

async fn refresh_authority_hot_cache_path(state: &AppState, path: &str) {
    let target = match state.authority_http_base_url.join(path) {
        Ok(target) => target,
        Err(error) => {
            tracing::warn!(
                path,
                error = %error,
                "skipping hot Nexus authority cache refresh for invalid path"
            );
            return;
        }
    };
    let authority = match state
        .http_client
        .get(target)
        .header(header::USER_AGENT, "nexus-relay-hot-cache")
        .send()
        .await
    {
        Ok(authority) => authority,
        Err(error) => {
            tracing::debug!(
                path,
                error = %error,
                "hot Nexus authority cache refresh failed"
            );
            return;
        }
    };
    let status = authority.status();
    let headers = authority.headers().clone();
    let body = match authority.bytes().await {
        Ok(body) => body,
        Err(error) => {
            tracing::debug!(
                path,
                error = %error,
                "hot Nexus authority cache body read failed"
            );
            return;
        }
    };
    store_authority_hot_cache(state, path.to_string(), status, headers, body);
}

async fn upgrade_websocket(state: AppState, request: Request) -> Response {
    let (mut parts, _body) = request.into_parts();
    match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(ws) => {
            let permit = match state.websocket_slots.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "nexus relay websocket capacity exhausted",
                    )
                        .into_response();
                }
            };
            let upstream_ws_url = state.config.upstream_ws_url();
            ws.on_upgrade(move |socket| async move {
                let _permit = permit;
                proxy_websocket(socket, upstream_ws_url).await;
            })
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
                tracing::debug!("client->upstream websocket proxy ended with error: {error}");
            }
        }
        result = upstream_to_client => {
            if let Err(error) = result {
                tracing::debug!("upstream->client websocket proxy ended with error: {error}");
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
        Message::Close(frame) => {
            UpstreamMessage::Close(frame.map(|frame| tungstenite::protocol::CloseFrame {
                code: CloseCode::from(frame.code),
                reason: frame.reason.to_string().into(),
            }))
        }
    }
}

fn upstream_message_to_client(message: UpstreamMessage) -> Option<Message> {
    match message {
        UpstreamMessage::Text(text) => Some(Message::Text(text.into())),
        UpstreamMessage::Binary(bytes) => Some(Message::Binary(bytes.into())),
        UpstreamMessage::Ping(bytes) => Some(Message::Ping(bytes.into())),
        UpstreamMessage::Pong(bytes) => Some(Message::Pong(bytes.into())),
        UpstreamMessage::Close(frame) => Some(Message::Close(frame.map(|frame| CloseFrame {
            code: frame.code.into(),
            reason: frame.reason.to_string().into(),
        }))),
        UpstreamMessage::Frame(_) => None,
    }
}

fn render_homepage(config: &DurableRelayConfig) -> String {
    let public_http_url = config
        .public_http_url()
        .unwrap_or_else(|| "http://127.0.0.1/".to_string());
    HOMEPAGE_TEMPLATE
        .replace("__NEXUS_NAME__", NEXUS_PRODUCT_NAME)
        .replace("__NEXUS_DESCRIPTION__", NEXUS_PRODUCT_DESCRIPTION)
        .replace("__RELAY_WS__", config.public_ws_url.as_str())
        .replace("__RELAY_HTTP__", public_http_url.as_str())
        .replace("__DATA_DIR__", &config.data_dir.display().to_string())
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
        &header::HOST | &header::CONNECTION | &header::CONTENT_LENGTH | &header::TRANSFER_ENCODING
    )
}

fn should_skip_proxy_response_header(name: &header::HeaderName) -> bool {
    matches!(
        name,
        &header::CONNECTION | &header::CONTENT_LENGTH | &header::TRANSFER_ENCODING
    )
}

fn is_websocket_upgrade_request(headers: &HeaderMap) -> bool {
    headers.contains_key(header::UPGRADE)
}

fn is_nip11_request(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("application/nostr+json"))
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

fn parse_bool_env(name: &str, value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("invalid {name}: expected boolean value")),
    }
}

fn parse_usize_env(name: &str, default: usize) -> Result<usize, String> {
    let Some(raw) = std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(default);
    };
    let parsed = raw
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}: {error}"))?;
    if parsed == 0 {
        return Err(format!("invalid {name}: expected value greater than zero"));
    }
    Ok(parsed)
}

fn build_authority_config(
    config: &DurableRelayConfig,
) -> Result<nexus_control::ServiceConfig, anyhow::Error> {
    let mut authority_config = nexus_control::ServiceConfig::from_env()
        .map_err(|error| anyhow::anyhow!("failed to load in-process authority config: {error}"))?;
    authority_config
        .hosted_nexus_relay_url
        .clone_from(&config.public_ws_url);
    Ok(authority_config)
}

#[cfg_attr(not(test), allow(dead_code))]
struct UpstreamRelayHandle {
    shutdown_tx: SyncSender<()>,
    join: JoinHandle<anyhow::Result<()>>,
}

impl UpstreamRelayHandle {
    async fn spawn(config: DurableRelayConfig) -> Result<Self, anyhow::Error> {
        let settings = config.build_upstream_settings()?;
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
    use axum::http::{StatusCode, header};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value;
    use std::net::SocketAddr;
    use std::path::Path;
    use tempfile::tempdir;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    use super::{DurableRelayConfig, UpstreamRelayHandle, build_authority_config, build_router};

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
            upstream_listen_addr,
            data_dir: data_dir.to_path_buf(),
            public_ws_url: format!("ws://{listen_addr}/"),
            upstream_config_file: None,
            enable_nip42_auth: false,
            max_websockets: super::DEFAULT_MAX_WEBSOCKETS,
            authority_max_in_flight: super::DEFAULT_AUTHORITY_MAX_IN_FLIGHT,
            authority_tokio_worker_threads: super::DEFAULT_AUTHORITY_TOKIO_WORKER_THREADS,
        })
    }

    async fn start_shell_server(
        config: DurableRelayConfig,
    ) -> Result<(
        SocketAddr,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    )> {
        let mut authority_config = build_authority_config(&config)?;
        authority_config.listen_addr = unused_socket_addr()?;
        let authority_http_base_url = super::authority_url_for_addr(authority_config.listen_addr)?;
        let authority_health_url = authority_http_base_url.join("healthz")?;
        let authority = tokio::spawn(nexus_control::run_server(authority_config));
        super::wait_for_upstream_ready(authority_health_url.as_str()).await?;

        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config, authority_http_base_url))
                .await
                .map_err(anyhow::Error::from)
        });
        Ok((addr, server, authority))
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
            .send(WsMessage::Text(
                r#"["REQ", "durable-check", {}]"#.to_string().into(),
            ))
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
        let (addr, server, authority) = start_shell_server(config).await?;

        let response = reqwest::get(format!("http://{addr}/healthz")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let health: Value = response.json().await?;
        assert_eq!(health["relay_backend"], "durable-upstream");
        assert_eq!(health["upstream_ws_url"], expected_upstream);
        assert_eq!(health["authority_mode"], "in-process");
        assert_eq!(
            health["managed_groups_mode"],
            super::MANAGED_GROUPS_MODE_DEFERRED
        );

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_shell_serves_health_while_authority_router_is_starting() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let authority_addr = unused_socket_addr()?;
        let authority_http_base_url = super::authority_url_for_addr(authority_addr)?;
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let shell_config = config.clone();
        let server_authority_http_base_url = authority_http_base_url.clone();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                build_router(shell_config, server_authority_http_base_url),
            )
            .await
            .map_err(anyhow::Error::from)
        });

        let client = reqwest::Client::new();
        let health = client.get(format!("http://{addr}/healthz")).send().await?;
        assert_eq!(health.status(), StatusCode::OK);

        let stats_while_starting = client
            .get(format!("http://{addr}/api/stats"))
            .send()
            .await?;
        assert_eq!(stats_while_starting.status(), StatusCode::BAD_GATEWAY);

        let mut authority_config = build_authority_config(&config)?;
        authority_config.listen_addr = authority_addr;
        let authority_health_url = authority_http_base_url.join("healthz")?;
        let authority = tokio::spawn(nexus_control::run_server(authority_config));
        super::wait_for_upstream_ready(authority_health_url.as_str()).await?;
        let stats_ready = client
            .get(format!("http://{addr}/api/stats"))
            .send()
            .await?;
        assert_eq!(stats_ready.status(), StatusCode::OK);

        server.abort();
        authority.abort();
        Ok(())
    }

    #[tokio::test]
    async fn run_server_does_not_start_upstream_when_public_port_is_occupied() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let _occupied_public_listener = tokio::net::TcpListener::bind(config.listen_addr).await?;

        let result = timeout(Duration::from_secs(2), super::run_server(config.clone()))
            .await
            .map_err(|_| anyhow::anyhow!("run_server should fail promptly on bind conflict"))?;
        let error = result.expect_err("public bind conflict should fail startup");
        assert!(
            error.to_string().contains("Address already in use")
                || error.to_string().contains("os error 48")
                || error.to_string().contains("os error 98"),
            "unexpected bind error: {error}"
        );

        let _upstream_listener = tokio::net::TcpListener::bind(config.upstream_listen_addr).await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_root_proxies_nip11_requests() -> Result<()> {
        let tempdir = tempdir()?;
        let mut config = durable_config(tempdir.path())?;
        config.enable_nip42_auth = true;
        let expected_relay_url = config.public_ws_url.clone();
        let expected_relay_page = config
            .public_http_url()
            .ok_or_else(|| anyhow::anyhow!("expected public http url"))?;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{addr}/"))
            .header(header::ACCEPT, "application/nostr+json")
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/nostr+json")
        );
        let body: Value = response.json().await?;
        assert_eq!(body["name"], super::NEXUS_PRODUCT_NAME);
        assert_eq!(body["description"], super::NEXUS_PRODUCT_DESCRIPTION);
        assert_eq!(body["id"], expected_relay_url);
        assert_eq!(
            body["software"],
            "https://git.sr.ht/~gheartsfield/nostr-rs-relay"
        );
        assert!(
            body["version"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(body["supported_nips"].as_array().map(Vec::len), Some(12));
        assert_eq!(body["limitation"]["payment_required"], false);
        assert_eq!(body["limitation"]["restricted_writes"], false);
        assert!(body.get("payment_url").is_none());
        assert!(body.get("fees").is_none());
        assert!(expected_relay_page.starts_with("http://"));
        assert!(
            body["supported_nips"]
                .as_array()
                .is_some_and(|nips| nips.iter().any(|value| value == 42))
        );

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_root_renders_nexus_branding() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let expected_relay_url = config.public_ws_url.clone();
        let expected_http_url = config
            .public_http_url()
            .ok_or_else(|| anyhow::anyhow!("expected public http url"))?;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let response = reqwest::get(format!("http://{addr}/")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await?;
        assert!(body.contains(super::NEXUS_PRODUCT_NAME));
        assert!(body.contains(super::NEXUS_PRODUCT_DESCRIPTION));
        assert!(body.contains(expected_relay_url.as_str()));
        assert!(body.contains(expected_http_url.as_str()));
        assert!(body.contains("Connected Pylons"));
        assert!(body.contains("Public relay heartbeat and ready capacity"));
        assert!(body.contains("Training Topology"));
        assert!(body.contains("Work Class Mix"));
        assert!(body.contains("Run Index"));
        assert!(body.contains("Window Matrix"));
        assert!(body.contains("Validator Pressure"));
        assert!(body.contains("Checkpoint Lineage"));
        assert!(body.contains("/api/homepage"));
        assert!(body.contains("pylon-constellation"));
        assert!(body.contains("What connected pylons are doing right now"));
        assert!(body.contains("kernel authority APIs"));

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_metrics_proxy_exposes_upstream_metrics() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let response = reqwest::get(format!("http://{addr}/metrics")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await?;
        assert!(body.contains("nostr_connections"));
        assert!(body.contains("nostr_cmd_event_total"));

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_websocket_proxies_nip42_auth_challenge() -> Result<()> {
        let tempdir = tempdir()?;
        let mut config = durable_config(tempdir.path())?;
        config.enable_nip42_auth = true;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let (mut socket, _) = connect_async(format!("ws://{addr}/")).await?;
        let first = timeout(Duration::from_secs(2), socket.next()).await;
        let first = first
            .ok()
            .and_then(|frame| frame)
            .ok_or_else(|| anyhow::anyhow!("expected auth challenge"))??;
        let first = match first {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected auth frame: {other:?}")),
        };
        assert!(first.contains("\"AUTH\""));

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_shell_serves_in_process_authority_routes() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let client = reqwest::Client::new();
        let session = client
            .post(format!("http://{addr}/api/session/desktop"))
            .json(&serde_json::json!({
                "desktop_client_id": "autopilot-desktop:test",
                "device_name": "integration"
            }))
            .send()
            .await?;
        assert_eq!(session.status(), StatusCode::OK);
        let session: Value = session.json().await?;
        let access_token = session["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing access token"))?;

        let stats = client
            .get(format!("http://{addr}/api/stats"))
            .send()
            .await?;
        assert_eq!(stats.status(), StatusCode::OK);

        let homepage = client
            .get(format!("http://{addr}/api/homepage"))
            .send()
            .await?;
        assert_eq!(homepage.status(), StatusCode::OK);
        let homepage: Value = homepage.json().await?;
        assert!(homepage.get("stats").is_some());
        assert!(homepage.get("training_summary").is_some());
        assert!(homepage.get("training_nodes").is_some());

        let snapshot = client
            .get(format!("http://{addr}/v1/kernel/snapshots/0"))
            .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await?;
        assert_eq!(snapshot.status(), StatusCode::OK);

        server.abort();
        authority.abort();
        relay.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn durable_shell_serves_cached_hot_authority_routes() -> Result<()> {
        let tempdir = tempdir()?;
        let config = durable_config(tempdir.path())?;
        let relay = UpstreamRelayHandle::spawn(config.clone()).await?;
        let (addr, server, authority) = start_shell_server(config).await?;

        let client = reqwest::Client::new();
        let url = format!("http://{addr}/api/stats");
        let warm = client.get(url.as_str()).send().await?;
        assert_eq!(warm.status(), StatusCode::OK);
        let warm_body: Value = warm.json().await?;
        assert_eq!(warm_body["service"], "nexus-control");

        authority.abort();
        let cached = client.get(url.as_str()).send().await?;
        assert_eq!(cached.status(), StatusCode::OK);
        let cached_body: Value = cached.json().await?;
        assert_eq!(cached_body["service"], "nexus-control");

        server.abort();
        relay.shutdown().await?;
        Ok(())
    }
}
