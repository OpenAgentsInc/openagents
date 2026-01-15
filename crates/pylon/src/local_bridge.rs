//! Local WebSocket bridge for browser-based Pylon discovery.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use codex_client::{
    AppServerClient, AppServerConfig, ClientInfo, GetAccountParams, is_codex_available,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::CodexConfig;

const DEFAULT_APP_ID: &str = "pylon-local";
const DEFAULT_APP_KEY: &str = "local-key";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8081;
const DEFAULT_ACTIVITY_TIMEOUT: u64 = 120;
const DEFAULT_CACHE_SECONDS: u64 = 30;
const SYSTEM_CHANNEL: &str = "pylon.system";

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

#[derive(Clone, Debug)]
struct BridgeState {
    config: LocalBridgeConfig,
    cached: Arc<Mutex<Option<CachedCapabilities>>>,
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

#[derive(Serialize, Debug)]
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
    let tls_acceptor = if let (Some(cert), Some(key)) = (tls_cert.as_ref(), tls_key.as_ref()) {
        Some(load_tls_acceptor(cert, key).context("bridge TLS config failed")?)
    } else {
        warn!("bridge TLS disabled: no certificate/key configured");
        None
    };

    let state = BridgeState {
        config,
        cached: Arc::new(Mutex::new(None)),
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
        let Some(message) = reader.next().await else {
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
        _ => {
            if message.event == "client-pylon.discover" {
                let channel = message
                    .channel
                    .clone()
                    .unwrap_or_else(|| SYSTEM_CHANNEL.to_string());
                if subscriptions.contains(&channel) {
                    send_capabilities(state, writer, &channel).await?;
                }
            }
        }
    }

    Ok(())
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

fn generate_self_signed_paths() -> Option<(PathBuf, PathBuf)> {
    let base = crate::config::PylonConfig::pylon_dir()
        .ok()
        .map(|path| path.join("certs"))?;
    let cert = base.join("pylon.local.crt");
    let key = base.join("pylon.local.key");

    if cert.exists() && key.exists() {
        return Some((cert, key));
    }

    if let Err(err) = std::fs::create_dir_all(&base) {
        warn!(error = %err, "failed to create bridge cert directory");
        return None;
    }

    if let Err(err) = generate_self_signed_cert(&cert, &key) {
        warn!(error = %err, "failed to generate bridge TLS cert");
        return None;
    }

    warn!(
        "generated self-signed cert; trust it to avoid TLS errors: {}",
        cert.display()
    );

    Some((cert, key))
}

fn generate_self_signed_cert(cert_path: &PathBuf, key_path: &PathBuf) -> Result<()> {
    let subject_alt_names = vec![
        "pylon.local".to_string(),
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
        "hyperion.test".to_string(),
    ];
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(subject_alt_names).context("create self-signed cert")?;

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
