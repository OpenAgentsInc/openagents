//! Local WebSocket bridge for browser-based Pylon discovery.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use codex_client::{
    AppServerClient, AppServerConfig, AppServerRequestId, ClientInfo, CommandExecParams,
    GetAccountParams, ModelListParams, ReviewStartParams, SkillsListParams,
    ThreadArchiveParams, ThreadListParams, ThreadResumeParams, ThreadStartParams,
    TurnInterruptParams, TurnStartParams, is_codex_available,
};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast, watch};
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use tracing::{debug, info, warn};
use uuid::Uuid;
use x509_parser::extensions::GeneralName;
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::{FromDer, X509Certificate};
use x509_parser::x509::X509Name;

use crate::config::CodexConfig;

const DEFAULT_APP_ID: &str = "pylon-local";
const DEFAULT_APP_KEY: &str = "local-key";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8081;
const DEFAULT_ACTIVITY_TIMEOUT: u64 = 120;
const DEFAULT_CACHE_SECONDS: u64 = 30;
const SYSTEM_CHANNEL: &str = "pylon.system";
const CODEX_CHANNEL: &str = "pylon.codex";
const BRIDGE_CERT_NAME: &str = "pylon.local.crt";
const BRIDGE_KEY_NAME: &str = "pylon.local.key";
const BRIDGE_CA_CERT_NAME: &str = "pylon.local.ca.crt";
const BRIDGE_CA_KEY_NAME: &str = "pylon.local.ca.key";
const BRIDGE_CA_CN: &str = "Pylon Local CA";
const BRIDGE_LEAF_CN: &str = "Pylon Local Bridge";
const BRIDGE_DNS_NAMES: [&str; 3] = ["pylon.local", "localhost", "hyperion.test"];
const BRIDGE_IPS: [IpAddr; 2] = [
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::LOCALHOST),
];

fn bridge_san_strings() -> Vec<String> {
    let mut names = BRIDGE_DNS_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    names.extend(BRIDGE_IPS.iter().map(|ip| ip.to_string()));
    names
}

fn cert_common_name(name: &X509Name<'_>) -> Option<String> {
    name.iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|value| value.to_string())
}

#[derive(Clone, Debug)]
pub struct PylonBridgeInfo {
    pub version: String,
    pub npub: Option<String>,
    pub host_active: bool,
    pub provider_active: bool,
    pub network: String,
}

#[derive(Clone, Debug)]
pub struct LocalBridgeConfig {
    pub host: String,
    pub port: u16,
    pub app_id: String,
    pub app_key: String,
    pub allowed_origins: AllowedOrigins,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub trust_cert: bool,
    pub pylon: PylonBridgeInfo,
    pub codex: CodexConfig,
}

impl LocalBridgeConfig {
    pub fn for_pylon(pylon: PylonBridgeInfo, codex: CodexConfig) -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            app_id: DEFAULT_APP_ID.to_string(),
            app_key: DEFAULT_APP_KEY.to_string(),
            allowed_origins: AllowedOrigins::Any,
            tls_cert: None,
            tls_key: None,
            trust_cert: false,
            pylon,
            codex,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AllowedOrigins {
    Any,
    List(Vec<String>),
}

impl AllowedOrigins {
    fn allows(&self, origin: Option<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::List(list) => origin
                .map(|value| list.iter().any(|item| item == value))
                .unwrap_or(false),
        }
    }
}

#[derive(Debug)]
pub struct LocalBridgeHandle {
    shutdown: watch::Sender<bool>,
    join: JoinHandle<()>,
}

impl LocalBridgeHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(true);
        let _ = self.join.await;
    }
}

#[derive(Clone)]
struct BridgeState {
    config: LocalBridgeConfig,
    cached: Arc<Mutex<Option<CachedCapabilities>>>,
    codex: Arc<Mutex<CodexBridgeState>>,
    codex_tx: broadcast::Sender<OutboundMessage>,
}

#[derive(Default)]
struct CodexBridgeState {
    sessions: HashMap<String, CodexSession>,
}

struct CodexSession {
    client: Arc<Mutex<Option<AppServerClient>>>,
}

#[derive(Clone, Debug)]
struct CachedCapabilities {
    payload: BridgeCapabilities,
    fetched_at: Instant,
}

#[derive(Deserialize, Debug)]
struct ClientMessage {
    event: String,
    data: Value,
    channel: Option<String>,
}

#[derive(Clone, Serialize, Debug)]
struct OutboundMessage {
    event: String,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PusherError {
    code: u16,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeCapabilities {
    timestamp: String,
    pylon: PylonCapabilities,
    codex: CodexCapabilities,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PylonCapabilities {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    npub: Option<String>,
    mode: PylonMode,
    network: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PylonMode {
    host: bool,
    provider: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexCapabilities {
    enabled: bool,
    available: bool,
    requires_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<CodexAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limits: Option<codex_client::RateLimitSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    autonomy: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexAccount {
    auth_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<String>,
}

pub async fn start_local_bridge(config: LocalBridgeConfig) -> Result<LocalBridgeHandle> {
    let (shutdown, shutdown_rx) = watch::channel(false);
    let join = tokio::spawn(async move {
        if let Err(err) = serve_bridge(config, shutdown_rx).await {
            warn!(error = %err, "local bridge stopped");
        }
    });

    Ok(LocalBridgeHandle { shutdown, join })
}

async fn serve_bridge(config: LocalBridgeConfig, mut shutdown_rx: watch::Receiver<bool>) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .context("invalid bridge bind address")?;
    let listener = TcpListener::bind(addr).await.context("bridge bind failed")?;

    let (tls_cert, tls_key) = resolve_tls_paths(config.tls_cert.clone(), config.tls_key.clone());
    if config.trust_cert {
        if let Some(cert_path) = tls_cert.as_ref() {
            maybe_trust_local_cert(cert_path);
        }
    }
    let tls_acceptor = if let (Some(cert), Some(key)) = (tls_cert.as_ref(), tls_key.as_ref()) {
        Some(load_tls_acceptor(cert, key).context("bridge TLS config failed")?)
    } else {
        warn!("bridge TLS disabled: no certificate/key configured");
        None
    };

    let (codex_tx, _) = broadcast::channel(256);
    let state = BridgeState {
        config,
        cached: Arc::new(Mutex::new(None)),
        codex: Arc::new(Mutex::new(CodexBridgeState::default())),
        codex_tx,
    };

    info!(
        "pylon bridge listening on {} (tls: {})",
        addr,
        tls_acceptor.is_some()
    );

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                break;
            }
            result = listener.accept() => {
                let (stream, peer_addr) = result.context("bridge accept failed")?;
                let tls_acceptor = tls_acceptor.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_connection(stream, peer_addr, tls_acceptor, state).await {
                        warn!(%peer_addr, error = %err, "bridge connection failed");
                    }
                });
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    tls_acceptor: Option<TlsAcceptor>,
    state: BridgeState,
) -> Result<()> {
    if let Some(acceptor) = tls_acceptor {
        let tls_stream = acceptor.accept(stream).await.context("TLS handshake failed")?;
        let ws_stream = accept_with_checks(tls_stream, &state.config).await?;
        info!(%peer_addr, "bridge websocket connected (tls)");
        return serve_socket(ws_stream, state).await;
    }

    let ws_stream = accept_with_checks(stream, &state.config).await?;
    info!(%peer_addr, "bridge websocket connected");
    serve_socket(ws_stream, state).await
}

async fn accept_with_checks<S>(
    stream: S,
    config: &LocalBridgeConfig,
) -> Result<WebSocketStream<S>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let app_key = config.app_key.clone();
    let allowed_origins = config.allowed_origins.clone();
    let callback = move |request: &Request, response: Response| {
        let origin = request
            .headers()
            .get("Origin")
            .and_then(|value| value.to_str().ok());
        debug!(path = %request.uri(), origin = ?origin, "bridge ws handshake");
        if !allowed_origins.allows(origin) {
            warn!(origin = ?origin, "rejecting bridge connection due to origin");
            return Err(error_response(403, "origin not allowed"));
        }

        let path = request.uri().path();
        let requested_key = extract_app_key(path);
        if requested_key.as_deref() != Some(app_key.as_str()) {
            warn!(path, "rejecting bridge connection due to app key mismatch");
            return Err(error_response(401, "invalid app key"));
        }

        Ok(response)
    };

    let ws_stream = accept_hdr_async(stream, callback).await?;
    Ok(ws_stream)
}

async fn serve_socket<S>(ws_stream: WebSocketStream<S>, state: BridgeState) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let socket_id = generate_socket_id();
    let (mut writer, mut reader) = ws_stream.split();
    let mut subscriptions: HashSet<String> = HashSet::new();
    let mut codex_rx = state.codex_tx.subscribe();

    let established = OutboundMessage {
        event: "pusher:connection_established".to_string(),
        data: serde_json::json!({
            "socket_id": socket_id.clone(),
            "activity_timeout": DEFAULT_ACTIVITY_TIMEOUT,
        })
        .to_string(),
        channel: None,
    };
    send_outbound(&mut writer, &established).await?;

    loop {
        tokio::select! {
            message = reader.next() => {
                let Some(message) = message else {
                    break;
                };
                match message {
                    Ok(Message::Text(text)) => {
                        if let Err(err) = handle_client_message(
                            &text,
                            &socket_id,
                            &state,
                            &mut subscriptions,
                            &mut writer,
                        )
                        .await
                        {
                            warn!(error = %err, "bridge message handling failed");
                        }
                    }
                    Ok(Message::Ping(payload)) => {
                        writer.send(Message::Pong(payload)).await?;
                    }
                    Ok(Message::Close(frame)) => {
                        debug!(?frame, "bridge client closed websocket");
                        break;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        warn!(error = %err, "bridge websocket error");
                        break;
                    }
                }
            }
            message = codex_rx.recv() => {
                let message = match message {
                    Ok(message) => message,
                    Err(broadcast::error::RecvError::Closed) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                };

                if let Some(channel) = &message.channel {
                    if !subscriptions.contains(channel) {
                        continue;
                    }
                }

                if let Err(err) = send_outbound(&mut writer, &message).await {
                    warn!(error = %err, "bridge failed to forward codex event");
                }
            }
        }
    }

    Ok(())
}

async fn handle_client_message<S>(
    payload: &str,
    _socket_id: &str,
    state: &BridgeState,
    subscriptions: &mut HashSet<String>,
    writer: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let message: ClientMessage = match serde_json::from_str(payload) {
        Ok(message) => message,
        Err(err) => {
            warn!(error = %err, len = payload.len(), "bridge invalid client payload");
            return Ok(());
        }
    };

    match message.event.as_str() {
        "pusher:ping" => {
            let pong = OutboundMessage {
                event: "pusher:pong".to_string(),
                data: "{}".to_string(),
                channel: None,
            };
            send_outbound(writer, &pong).await?;
        }
        "pusher:subscribe" => {
            if let Some(data) = parse_data_object(message.data) {
                if let Some(channel) = extract_string(&data, "channel") {
                    if !channel.starts_with("pylon.") {
                        send_error(writer, "channel not allowed").await?;
                        return Ok(());
                    }
                    subscriptions.insert(channel.clone());
                    let reply = OutboundMessage {
                        event: "pusher_internal:subscription_succeeded".to_string(),
                        data: "{}".to_string(),
                        channel: Some(channel.clone()),
                    };
                    send_outbound(writer, &reply).await?;
                    if channel == SYSTEM_CHANNEL {
                        send_capabilities(state, writer, &channel).await?;
                    }
                }
            }
        }
        "pusher:unsubscribe" => {
            if let Some(data) = parse_data_object(message.data) {
                if let Some(channel) = extract_string(&data, "channel") {
                    subscriptions.remove(&channel);
                }
            }
        }
        "client-pylon.discover" => {
            let channel = message
                .channel
                .clone()
                .unwrap_or_else(|| SYSTEM_CHANNEL.to_string());
            if subscriptions.contains(&channel) {
                send_capabilities(state, writer, &channel).await?;
            }
        }
        "client-pylon.ping" => {
            let channel = message
                .channel
                .clone()
                .unwrap_or_else(|| SYSTEM_CHANNEL.to_string());
            if subscriptions.contains(&channel) {
                let reply = OutboundMessage {
                    event: "pylon.system.pong".to_string(),
                    data: serde_json::json!({
                        "timestamp": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    })
                    .to_string(),
                    channel: Some(channel),
                };
                send_outbound(writer, &reply).await?;
            }
        }
        "client-codex.connect" => {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_codex_connect(&state, message.data).await {
                    warn!(error = %err, "bridge codex connect failed");
                }
            });
        }
        "client-codex.disconnect" => {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_codex_disconnect(&state, message.data).await {
                    warn!(error = %err, "bridge codex disconnect failed");
                }
            });
        }
        "client-codex.request" => {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_codex_request(&state, message.data).await {
                    warn!(error = %err, "bridge codex request failed");
                }
            });
        }
        "client-codex.respond" => {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_codex_response(&state, message.data).await {
                    warn!(error = %err, "bridge codex respond failed");
                }
            });
        }
        _ => {}
    }

    Ok(())
}

fn emit_codex_payload(state: &BridgeState, event: &str, payload: Value) {
    let message = OutboundMessage {
        event: event.to_string(),
        data: payload.to_string(),
        channel: Some(CODEX_CHANNEL.to_string()),
    };
    let _ = state.codex_tx.send(message);
}

fn emit_codex_status(state: &BridgeState, workspace_id: &str, status: &str, detail: Option<String>) {
    emit_codex_payload(
        state,
        "pylon.codex.status",
        serde_json::json!({
            "workspace_id": workspace_id,
            "status": status,
            "detail": detail,
        }),
    );
}

fn emit_codex_response(
    state: &BridgeState,
    workspace_id: &str,
    request_id: &str,
    result: Result<Value, String>,
) {
    match result {
        Ok(payload) => emit_codex_payload(
            state,
            "pylon.codex.response",
            serde_json::json!({
                "workspace_id": workspace_id,
                "request_id": request_id,
                "ok": true,
                "result": payload,
            }),
        ),
        Err(error) => emit_codex_payload(
            state,
            "pylon.codex.response",
            serde_json::json!({
                "workspace_id": workspace_id,
                "request_id": request_id,
                "ok": false,
                "error": error,
            }),
        ),
    }
}

fn emit_codex_error(state: &BridgeState, workspace_id: Option<&str>, message: &str) {
    emit_codex_payload(
        state,
        "pylon.codex.error",
        serde_json::json!({
            "workspace_id": workspace_id,
            "message": message,
        }),
    );
}

async fn handle_codex_connect(state: &BridgeState, payload: Value) -> Result<()> {
    let data = match parse_data_object(payload) {
        Some(data) => data,
        None => {
            emit_codex_error(state, None, "Invalid connect payload");
            return Ok(());
        }
    };

    let workspace_id = extract_string(&data, "workspaceId")
        .or_else(|| extract_string(&data, "workspace_id"))
        .unwrap_or_default();
    let cwd = extract_string(&data, "cwd").unwrap_or_default();

    if workspace_id.is_empty() || cwd.is_empty() {
        emit_codex_error(state, None, "Missing workspaceId or cwd");
        return Ok(());
    }

    if !state.config.codex.enabled {
        emit_codex_status(state, &workspace_id, "disabled", Some("Codex disabled".to_string()));
        return Ok(());
    }

    if !is_codex_available() {
        emit_codex_status(
            state,
            &workspace_id,
            "unavailable",
            Some("Codex CLI not found".to_string()),
        );
        return Ok(());
    }

    {
        let codex = state.codex.lock().await;
        if codex.sessions.contains_key(&workspace_id) {
            emit_codex_status(state, &workspace_id, "connected", None);
            return Ok(());
        }
    }

    match spawn_codex_session(state, workspace_id.clone(), PathBuf::from(cwd)).await {
        Ok(session) => {
            let mut codex = state.codex.lock().await;
            codex.sessions.insert(workspace_id.clone(), session);
            emit_codex_status(state, &workspace_id, "connected", None);
        }
        Err(err) => {
            emit_codex_status(
                state,
                &workspace_id,
                "failed",
                Some(err.to_string()),
            );
        }
    }

    Ok(())
}

async fn handle_codex_disconnect(state: &BridgeState, payload: Value) -> Result<()> {
    let data = match parse_data_object(payload) {
        Some(data) => data,
        None => {
            emit_codex_error(state, None, "Invalid disconnect payload");
            return Ok(());
        }
    };

    let workspace_id = extract_string(&data, "workspaceId")
        .or_else(|| extract_string(&data, "workspace_id"))
        .unwrap_or_default();
    if workspace_id.is_empty() {
        emit_codex_error(state, None, "Missing workspaceId");
        return Ok(());
    }

    let session = {
        let mut codex = state.codex.lock().await;
        codex.sessions.remove(&workspace_id)
    };

    if let Some(session) = session {
        if let Some(client) = session.client.lock().await.take() {
            let _ = client.shutdown().await;
        }
    }

    emit_codex_status(state, &workspace_id, "disconnected", None);
    Ok(())
}

async fn handle_codex_request(state: &BridgeState, payload: Value) -> Result<()> {
    let data = match parse_data_object(payload) {
        Some(data) => data,
        None => {
            emit_codex_error(state, None, "Invalid request payload");
            return Ok(());
        }
    };

    let workspace_id = extract_string(&data, "workspaceId")
        .or_else(|| extract_string(&data, "workspace_id"))
        .unwrap_or_default();
    let request_id = extract_string(&data, "requestId")
        .or_else(|| extract_string(&data, "request_id"))
        .unwrap_or_default();
    let method = extract_string(&data, "method").unwrap_or_default();
    let params = data.get("params").cloned();

    if workspace_id.is_empty() || request_id.is_empty() || method.is_empty() {
        if !workspace_id.is_empty() && !request_id.is_empty() {
            emit_codex_response(
                state,
                &workspace_id,
                &request_id,
                Err("Missing workspaceId, requestId, or method".to_string()),
            );
        } else {
            emit_codex_error(state, None, "Missing workspaceId, requestId, or method");
        }
        return Ok(());
    }

    let client = {
        let codex = state.codex.lock().await;
        codex.sessions.get(&workspace_id).map(|session| session.client.clone())
    };

    let Some(client) = client else {
        emit_codex_response(
            state,
            &workspace_id,
            &request_id,
            Err("Workspace is not connected".to_string()),
        );
        return Ok(());
    };

    let result = dispatch_codex_request(&client, &method, params).await;
    emit_codex_response(state, &workspace_id, &request_id, result);
    Ok(())
}

async fn handle_codex_response(state: &BridgeState, payload: Value) -> Result<()> {
    let data = match parse_data_object(payload) {
        Some(data) => data,
        None => {
            emit_codex_error(state, None, "Invalid response payload");
            return Ok(());
        }
    };

    let workspace_id = extract_string(&data, "workspaceId")
        .or_else(|| extract_string(&data, "workspace_id"))
        .unwrap_or_default();
    let request_id = data
        .get("requestId")
        .or_else(|| data.get("request_id"))
        .cloned();
    let result = data.get("result").cloned().unwrap_or(Value::Null);

    if workspace_id.is_empty() {
        emit_codex_error(state, None, "Missing workspaceId");
        return Ok(());
    }

    let request_id = match request_id {
        Some(value) => match serde_json::from_value::<AppServerRequestId>(value) {
            Ok(id) => id,
            Err(err) => {
                emit_codex_error(state, Some(&workspace_id), &err.to_string());
                return Ok(());
            }
        },
        None => {
            emit_codex_error(state, Some(&workspace_id), "Missing requestId");
            return Ok(());
        }
    };

    let client = {
        let codex = state.codex.lock().await;
        codex.sessions.get(&workspace_id).map(|session| session.client.clone())
    };

    let Some(client) = client else {
        emit_codex_error(state, Some(&workspace_id), "Workspace is not connected");
        return Ok(());
    };

    let response_result = {
        let guard = client.lock().await;
        match guard.as_ref() {
            Some(client) => client.respond(request_id, &result).await,
            None => Err(anyhow::anyhow!("Codex session not available")),
        }
    };

    if let Err(err) = response_result {
        emit_codex_error(state, Some(&workspace_id), &err.to_string());
    }

    Ok(())
}

async fn spawn_codex_session(
    state: &BridgeState,
    workspace_id: String,
    cwd: PathBuf,
) -> Result<CodexSession> {
    let (client, channels) = AppServerClient::spawn(AppServerConfig {
        cwd: Some(cwd.clone()),
        wire_log: None,
    })
    .await?;

    if let Err(err) = client
        .initialize(ClientInfo {
            name: "pylon-bridge".to_string(),
            title: Some("Pylon".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
        .await
    {
        let _ = client.shutdown().await;
        return Err(err);
    }

    let mut notifications = channels.notifications;
    let mut requests = channels.requests;
    let codex_tx = state.codex_tx.clone();
    let workspace_id_events = workspace_id.clone();
    tokio::spawn(async move {
        while let Some(notification) = notifications.recv().await {
            let message = serde_json::json!({
                "method": notification.method,
                "params": notification.params,
            });
            let outbound = OutboundMessage {
                event: "pylon.codex.event".to_string(),
                data: serde_json::json!({
                    "workspace_id": workspace_id_events,
                    "message": message,
                })
                .to_string(),
                channel: Some(CODEX_CHANNEL.to_string()),
            };
            let _ = codex_tx.send(outbound);
        }
    });

    let codex_tx = state.codex_tx.clone();
    let workspace_id_requests = workspace_id.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let id_value = serde_json::to_value(&request.id).unwrap_or(Value::Null);
            let message = serde_json::json!({
                "method": request.method,
                "params": request.params,
                "id": id_value,
            });
            let outbound = OutboundMessage {
                event: "pylon.codex.event".to_string(),
                data: serde_json::json!({
                    "workspace_id": workspace_id_requests,
                    "message": message,
                })
                .to_string(),
                channel: Some(CODEX_CHANNEL.to_string()),
            };
            let _ = codex_tx.send(outbound);
        }
    });

    Ok(CodexSession {
        client: Arc::new(Mutex::new(Some(client))),
    })
}

async fn dispatch_codex_request(
    client: &Arc<Mutex<Option<AppServerClient>>>,
    method: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    let result = {
        let guard = client.lock().await;
        let Some(client) = guard.as_ref() else {
            return Err("Codex session not available".to_string());
        };
        match method {
            "thread/list" => {
                let params = parse_params::<ThreadListParams>(params)?;
                client
                    .thread_list(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "thread/start" => {
                let params = parse_params::<ThreadStartParams>(params)?;
                client
                    .thread_start(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "thread/resume" => {
                let params = parse_params::<ThreadResumeParams>(params)?;
                client
                    .thread_resume(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "thread/archive" => {
                let params = parse_params::<ThreadArchiveParams>(params)?;
                client
                    .thread_archive(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "model/list" => {
                let params = parse_params::<ModelListParams>(params)?;
                client
                    .model_list(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "skills/list" => {
                let params = parse_params::<SkillsListParams>(params)?;
                client
                    .skills_list(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| {
                        let flattened = response
                            .data
                            .into_iter()
                            .flat_map(|entry| entry.skills)
                            .collect::<Vec<_>>();
                        serde_json::to_value(serde_json::json!({ "data": flattened }))
                            .map_err(|err| err.to_string())
                    })
            }
            "turn/start" => {
                let params = parse_params::<TurnStartParams>(params)?;
                client
                    .turn_start(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "turn/interrupt" => {
                let params = parse_params::<TurnInterruptParams>(params)?;
                client
                    .turn_interrupt(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "review/start" => {
                let params = parse_params::<ReviewStartParams>(params)?;
                client
                    .review_start(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "command/exec" => {
                let params = parse_params::<CommandExecParams>(params)?;
                client
                    .command_exec(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            "account/rateLimits/read" => client
                .account_rate_limits_read()
                .await
                .map(|response| serde_json::to_value(response.rate_limits).unwrap_or(Value::Null))
                .map_err(|err| err.to_string()),
            "account/read" => {
                let params = parse_params::<GetAccountParams>(params)?;
                client
                    .account_read(params)
                    .await
                    .map_err(|err| err.to_string())
                    .and_then(|response| serde_json::to_value(response).map_err(|err| err.to_string()))
            }
            _ => Err(format!("Unsupported method: {method}")),
        }?
    };

    Ok(result)
}

fn parse_params<T: DeserializeOwned>(params: Option<Value>) -> Result<T, String> {
    let value = params.unwrap_or_else(|| serde_json::json!({}));
    serde_json::from_value(value).map_err(|err| err.to_string())
}

async fn send_capabilities<S>(
    state: &BridgeState,
    writer: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    channel: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = state.capabilities().await;
    let data = serde_json::to_string(&payload)?;
    let message = OutboundMessage {
        event: "pylon.capabilities".to_string(),
        data,
        channel: Some(channel.to_string()),
    };
    send_outbound(writer, &message).await
}

impl BridgeState {
    async fn capabilities(&self) -> BridgeCapabilities {
        let mut cached = self.cached.lock().await;
        if let Some(existing) = cached.as_ref() {
            if existing.fetched_at.elapsed() < Duration::from_secs(DEFAULT_CACHE_SECONDS) {
                return existing.payload.clone();
            }
        }

        let payload = build_capabilities(&self.config).await;
        *cached = Some(CachedCapabilities {
            payload: payload.clone(),
            fetched_at: Instant::now(),
        });
        payload
    }
}

async fn build_capabilities(config: &LocalBridgeConfig) -> BridgeCapabilities {
    let codex = fetch_codex_capabilities(&config.codex).await;
    BridgeCapabilities {
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        pylon: PylonCapabilities {
            version: config.pylon.version.clone(),
            npub: config.pylon.npub.clone(),
            mode: PylonMode {
                host: config.pylon.host_active,
                provider: config.pylon.provider_active,
            },
            network: config.pylon.network.clone(),
        },
        codex,
    }
}

async fn fetch_codex_capabilities(config: &CodexConfig) -> CodexCapabilities {
    let enabled = config.enabled;
    if !enabled {
        return CodexCapabilities {
            enabled: false,
            available: false,
            requires_auth: false,
            account: None,
            rate_limits: None,
            error: None,
            model: None,
            autonomy: None,
        };
    }

    if !is_codex_available() {
        return CodexCapabilities {
            enabled: true,
            available: false,
            requires_auth: false,
            account: None,
            rate_limits: None,
            error: Some("Codex CLI not found".to_string()),
            model: Some(config.model.clone()),
            autonomy: Some(format!("{:?}", config.autonomy)),
        };
    }

    let (client, channels) = match AppServerClient::spawn(AppServerConfig {
        cwd: config.cwd.clone(),
        wire_log: None,
    })
    .await
    {
        Ok(result) => result,
        Err(err) => {
            return CodexCapabilities {
                enabled: true,
                available: false,
                requires_auth: false,
                account: None,
                rate_limits: None,
                error: Some(err.to_string()),
                model: Some(config.model.clone()),
                autonomy: Some(format!("{:?}", config.autonomy)),
            };
        }
    };

    let mut notifications = channels.notifications;
    let mut requests = channels.requests;

    tokio::spawn(async move {
        while let Some(notification) = notifications.recv().await {
            debug!(method = %notification.method, "bridge ignoring app-server notification");
        }
    });

    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            debug!(method = %request.method, "bridge ignoring app-server request");
            let _ = request;
        }
    });

    let init_result = client
        .initialize(ClientInfo {
            name: "pylon-bridge".to_string(),
            title: Some("Pylon".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
        .await;
    if let Err(err) = init_result {
        let _ = client.shutdown().await;
        return CodexCapabilities {
            enabled: true,
            available: false,
            requires_auth: false,
            account: None,
            rate_limits: None,
            error: Some(err.to_string()),
            model: Some(config.model.clone()),
            autonomy: Some(format!("{:?}", config.autonomy)),
        };
    }

    let account = client
        .account_read(GetAccountParams { refresh_token: false })
        .await;
    let rate_limits = client.account_rate_limits_read().await;
    let _ = client.shutdown().await;

    match account {
        Ok(response) => {
            let account_info = response.account.map(|info| match info {
                codex_client::AccountInfo::ApiKey => CodexAccount {
                    auth_mode: "apiKey".to_string(),
                    email: None,
                    plan: None,
                },
                codex_client::AccountInfo::Chatgpt { email, plan_type } => CodexAccount {
                    auth_mode: "chatgpt".to_string(),
                    email: Some(email),
                    plan: Some(format!("{:?}", plan_type)),
                },
            });

            CodexCapabilities {
                enabled: true,
                available: true,
                requires_auth: response.requires_openai_auth,
                account: account_info,
                rate_limits: rate_limits.ok().map(|limits| limits.rate_limits),
                error: None,
                model: Some(config.model.clone()),
                autonomy: Some(format!("{:?}", config.autonomy)),
            }
        }
        Err(err) => CodexCapabilities {
            enabled: true,
            available: true,
            requires_auth: true,
            account: None,
            rate_limits: None,
            error: Some(err.to_string()),
            model: Some(config.model.clone()),
            autonomy: Some(format!("{:?}", config.autonomy)),
        },
    }
}

async fn send_outbound<S>(
    writer: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    message: &OutboundMessage,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(message)?;
    writer.send(Message::Text(payload)).await?;
    Ok(())
}

async fn send_error<S>(
    writer: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    message: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let error_payload = PusherError {
        code: 4009,
        message: message.to_string(),
    };
    let error = OutboundMessage {
        event: "pusher:error".to_string(),
        data: serde_json::to_string(&error_payload)?,
        channel: None,
    };
    send_outbound(writer, &error).await
}

fn parse_data_object(data: Value) -> Option<Map<String, Value>> {
    match data {
        Value::Object(map) => Some(map),
        Value::String(text) => serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| match value {
                Value::Object(map) => Some(map),
                _ => None,
            }),
        _ => None,
    }
}

fn extract_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn error_response(status: u16, message: &str) -> ErrorResponse {
    Response::builder()
        .status(status)
        .body(Some(message.to_string()))
        .unwrap_or_else(|_| Response::builder().status(status).body(None).expect("response"))
}

fn extract_app_key(path: &str) -> Option<String> {
    let trimmed = path.strip_prefix("/app/")?;
    let key = trimmed.split('/').next()?;
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

fn generate_socket_id() -> String {
    let value = Uuid::new_v4().as_u128();
    let left = (value & 0xffff) as u32;
    let right = ((value >> 16) & 0x00ffffff) as u32;
    format!(
        "{}.{}",
        1000 + (left % 9000),
        1_000_000 + (right % 9_000_000)
    )
}

fn resolve_tls_paths(
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    if cert.is_some() && key.is_some() {
        return (cert, key);
    }

    if let Some((generated_cert, generated_key)) = generate_self_signed_paths() {
        info!(
            "using generated TLS cert/key at {} and {}",
            generated_cert.display(),
            generated_key.display()
        );
        return (Some(generated_cert), Some(generated_key));
    }

    (None, None)
}

fn is_generated_cert(cert_path: &Path) -> bool {
    matches!(
        cert_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some(BRIDGE_CERT_NAME) | Some(BRIDGE_CA_CERT_NAME)
    )
}

fn bridge_ca_path(cert_path: &Path) -> Option<PathBuf> {
    let file_name = cert_path.file_name()?.to_str()?;
    if file_name == BRIDGE_CA_CERT_NAME {
        return Some(cert_path.to_path_buf());
    }
    if file_name == BRIDGE_CERT_NAME {
        let ca_path = cert_path.with_file_name(BRIDGE_CA_CERT_NAME);
        if ca_path.exists() {
            return Some(ca_path);
        }
    }
    None
}

fn ip_from_bytes(bytes: &[u8]) -> Option<IpAddr> {
    match bytes.len() {
        4 => Some(IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))),
        16 => {
            let mut segments = [0u16; 8];
            for (index, segment) in segments.iter_mut().enumerate() {
                let offset = index * 2;
                *segment = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
            }
            Some(IpAddr::V6(Ipv6Addr::new(
                segments[0],
                segments[1],
                segments[2],
                segments[3],
                segments[4],
                segments[5],
                segments[6],
                segments[7],
            )))
        }
        _ => None,
    }
}

fn cert_supports_hosts(cert_path: &Path) -> bool {
    let contents = match std::fs::read(cert_path) {
        Ok(contents) => contents,
        Err(_) => return false,
    };
    let (_, pem) = match parse_x509_pem(&contents) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };
    let (_, cert) = match X509Certificate::from_der(pem.contents.as_slice()) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };

    let mut names = HashSet::new();
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in san.value.general_names.iter() {
            match name {
                GeneralName::DNSName(dns) => {
                    names.insert(dns.to_string());
                }
                GeneralName::IPAddress(bytes) => {
                    if let Some(ip) = ip_from_bytes(bytes) {
                        names.insert(ip.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    let subject_cn = cert_common_name(cert.subject());
    let issuer_cn = cert_common_name(cert.issuer());

    BRIDGE_DNS_NAMES.iter().all(|host| names.contains(*host))
        && BRIDGE_IPS
            .iter()
            .map(|ip| ip.to_string())
            .all(|ip| names.contains(&ip))
        && subject_cn.as_deref() == Some(BRIDGE_LEAF_CN)
        && issuer_cn.as_deref() == Some(BRIDGE_CA_CN)
}

fn maybe_trust_local_cert(cert_path: &Path) {
    let trust_path = bridge_ca_path(cert_path).unwrap_or_else(|| cert_path.to_path_buf());
    if !is_generated_cert(&trust_path) {
        return;
    }

    let marker_path = trust_path.with_extension("trusted");
    if marker_path.exists() {
        if let (Ok(cert_meta), Ok(marker_meta)) =
            (std::fs::metadata(&trust_path), std::fs::metadata(&marker_path))
        {
            if let (Ok(cert_modified), Ok(marker_modified)) =
                (cert_meta.modified(), marker_meta.modified())
            {
                if marker_modified >= cert_modified {
                    return;
                }
            }
        } else {
            return;
        }
    }

    #[cfg(target_os = "macos")]
    {
        match trust_cert_macos(&trust_path) {
            Ok(()) => {
                if let Err(err) = std::fs::write(&marker_path, "trusted") {
                    warn!(error = %err, "failed to write bridge cert trust marker");
                }
                info!("trusted local bridge TLS cert");
            }
            Err(err) => {
                warn!(error = %err, "failed to trust local bridge TLS cert");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("automatic trust not supported on this platform; trust the bridge cert manually");
    }
}

#[cfg(target_os = "macos")]
fn trust_cert_macos(cert_path: &Path) -> Result<()> {
    let home = dirs::home_dir().context("locate home directory for keychain")?;
    let keychain = home.join("Library/Keychains/login.keychain-db");

    let status = Command::new("security")
        .arg("add-trusted-cert")
        .arg("-d")
        .arg("-r")
        .arg("trustRoot")
        .arg("-k")
        .arg(&keychain)
        .arg(cert_path)
        .status()
        .context("run security add-trusted-cert")?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "security add-trusted-cert failed with status {}",
            status
        ));
    }

    Ok(())
}

fn generate_self_signed_paths() -> Option<(PathBuf, PathBuf)> {
    let base = crate::config::PylonConfig::pylon_dir()
        .ok()
        .map(|path| path.join("certs"))?;
    let cert = base.join(BRIDGE_CERT_NAME);
    let key = base.join(BRIDGE_KEY_NAME);
    let ca_cert = base.join(BRIDGE_CA_CERT_NAME);
    let ca_key = base.join(BRIDGE_CA_KEY_NAME);
    let needs_regen = !cert.exists()
        || !key.exists()
        || !ca_cert.exists()
        || !ca_key.exists()
        || !cert_supports_hosts(&cert);

    if !needs_regen {
        return Some((cert, key));
    }

    if let Err(err) = std::fs::create_dir_all(&base) {
        warn!(error = %err, "failed to create bridge cert directory");
        return None;
    }

    if let Err(err) = generate_self_signed_cert(&cert, &key, &ca_cert, &ca_key) {
        warn!(error = %err, "failed to generate bridge TLS cert");
        return None;
    }

    warn!(
        "generated local bridge TLS cert; trust it to avoid TLS errors: {}",
        ca_cert.display()
    );

    Some((cert, key))
}

fn generate_self_signed_cert(
    cert_path: &PathBuf,
    key_path: &PathBuf,
    ca_cert_path: &PathBuf,
    ca_key_path: &PathBuf,
) -> Result<()> {
    let mut ca_params = rcgen::CertificateParams::new(vec![BRIDGE_DNS_NAMES[0].to_string()])
        .context("create bridge CA params")?;
    let mut ca_dn = rcgen::DistinguishedName::new();
    ca_dn.push(rcgen::DnType::CommonName, BRIDGE_CA_CN);
    ca_params.distinguished_name = ca_dn;
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params
        .key_usages
        .push(rcgen::KeyUsagePurpose::KeyCertSign);
    ca_params
        .key_usages
        .push(rcgen::KeyUsagePurpose::DigitalSignature);
    let ca_key = rcgen::KeyPair::generate().context("create bridge CA key")?;
    let ca = ca_params
        .self_signed(&ca_key)
        .context("create bridge CA certificate")?;
    std::fs::write(ca_cert_path, ca.pem())
        .with_context(|| format!("write ca cert {}", ca_cert_path.display()))?;
    std::fs::write(ca_key_path, ca_key.serialize_pem())
        .with_context(|| format!("write ca key {}", ca_key_path.display()))?;

    let mut params = rcgen::CertificateParams::new(bridge_san_strings())
        .context("create bridge cert params")?;
    let mut leaf_dn = rcgen::DistinguishedName::new();
    leaf_dn.push(rcgen::DnType::CommonName, BRIDGE_LEAF_CN);
    params.distinguished_name = leaf_dn;
    params.is_ca = rcgen::IsCa::NoCa;
    params
        .key_usages
        .push(rcgen::KeyUsagePurpose::DigitalSignature);
    params
        .key_usages
        .push(rcgen::KeyUsagePurpose::KeyEncipherment);
    params
        .extended_key_usages
        .push(rcgen::ExtendedKeyUsagePurpose::ServerAuth);
    let key_pair = rcgen::KeyPair::generate().context("create bridge key")?;
    let cert = params
        .signed_by(&key_pair, &ca, &ca_key)
        .context("create bridge certificate")?;
    std::fs::write(cert_path, cert.pem())
        .with_context(|| format!("write cert {}", cert_path.display()))?;
    std::fs::write(key_path, key_pair.serialize_pem())
        .with_context(|| format!("write key {}", key_path.display()))?;

    Ok(())
}

fn load_tls_acceptor(cert_path: &PathBuf, key_path: &PathBuf) -> Result<TlsAcceptor> {
    let cert_file =
        std::fs::File::open(cert_path).with_context(|| format!("open cert {}", cert_path.display()))?;
    let key_file =
        std::fs::File::open(key_path).with_context(|| format!("open key {}", key_path.display()))?;

    let mut cert_reader = std::io::BufReader::new(cert_file);
    let mut key_reader = std::io::BufReader::new(key_file);

    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("read certs")?;
    let key = rustls_pemfile::private_key(&mut key_reader)
        .context("read private key")?
        .context("no private key found")?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("invalid cert or key")?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}
