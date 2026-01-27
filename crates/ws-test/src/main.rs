use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::Sha256;
use tokio::net::TcpListener;
use tokio::time::{Duration, interval};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{Response as HttpResponse, StatusCode};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "ws-test",
    about = "Local Pusher-compatible WebSocket server for Hyperion"
)]
struct Args {
    /// Interface to bind to (e.g. 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// Port to bind to
    #[arg(long, default_value_t = 8081)]
    port: u16,
    /// Seconds between server tick messages
    #[arg(long, default_value_t = 5)]
    tick_seconds: u64,
    /// Activity timeout advertised to clients
    #[arg(long, default_value_t = 120)]
    activity_timeout: u64,
    /// Optional TLS certificate (PEM)
    #[arg(long)]
    tls_cert: Option<PathBuf>,
    /// Optional TLS private key (PEM)
    #[arg(long)]
    tls_key: Option<PathBuf>,
    /// Optional comma-separated list of allowed Origin values
    #[arg(long)]
    allowed_origins: Option<String>,
    /// App id used for logging and connection info
    #[arg(long, default_value = "local-app")]
    app_id: String,
    /// Pusher app key (must match /app/{key} in the WebSocket URL)
    #[arg(long, default_value = "local-key")]
    app_key: String,
    /// Optional Pusher app secret (required for private/presence channels)
    #[arg(long)]
    app_secret: Option<String>,
}

#[derive(Clone, Debug)]
struct AppConfig {
    app_id: String,
    app_key: String,
    app_secret: Option<String>,
    activity_timeout: u64,
}

#[derive(Clone, Debug)]
enum AllowedOrigins {
    Any,
    List(Vec<String>),
}

impl AllowedOrigins {
    fn from_arg(value: Option<String>) -> Self {
        let Some(raw) = value else {
            return Self::Any;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "*" {
            return Self::Any;
        }
        let items = trimmed
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        if items.is_empty() {
            Self::Any
        } else {
            Self::List(items)
        }
    }

    fn allows(&self, origin: Option<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::List(list) => origin
                .map(|value| list.iter().any(|item| item == value))
                .unwrap_or(false),
        }
    }
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
struct PusherError {
    code: u16,
    message: String,
}

#[derive(Serialize)]
struct TickPayload {
    server_time: String,
    socket_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    if args.tls_cert.is_some() ^ args.tls_key.is_some() {
        anyhow::bail!("--tls-cert and --tls-key must be provided together");
    }
    if args.tick_seconds == 0 {
        anyhow::bail!("--tick-seconds must be at least 1");
    }
    if args.activity_timeout == 0 {
        anyhow::bail!("--activity-timeout must be at least 1");
    }

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("invalid bind address")?;
    let listener = TcpListener::bind(addr).await.context("failed to bind")?;

    let (tls_cert, tls_key) = resolve_tls_paths(args.tls_cert, args.tls_key);
    let tls_acceptor = if let (Some(cert), Some(key)) = (tls_cert.as_ref(), tls_key.as_ref()) {
        Some(load_tls_acceptor(cert, key).context("failed to load TLS config")?)
    } else {
        warn!("tls disabled: no certificate/key configured");
        None
    };

    let allowed_origins = AllowedOrigins::from_arg(args.allowed_origins);
    let app_config = AppConfig {
        app_id: args.app_id,
        app_key: args.app_key,
        app_secret: args.app_secret,
        activity_timeout: args.activity_timeout,
    };

    info!(
        "ws-test listening on {} (tls: {}, app_id: {}, app_key: {}, tick: {}s, activity_timeout: {}s, origins: {:?})",
        addr,
        tls_acceptor.is_some(),
        app_config.app_id,
        app_config.app_key,
        args.tick_seconds,
        app_config.activity_timeout,
        allowed_origins
    );

    loop {
        let (stream, peer_addr) = listener.accept().await.context("accept failed")?;
        let tls_acceptor = tls_acceptor.clone();
        let allowed_origins = allowed_origins.clone();
        let app_config = app_config.clone();
        let tick_seconds = args.tick_seconds;

        tokio::spawn(async move {
            if let Err(err) = handle_connection(
                stream,
                peer_addr,
                tls_acceptor,
                allowed_origins,
                app_config,
                tick_seconds,
            )
            .await
            {
                error!(%peer_addr, error = %err, "connection failed");
            }
        });
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    tls_acceptor: Option<TlsAcceptor>,
    allowed_origins: AllowedOrigins,
    app_config: AppConfig,
    tick_seconds: u64,
) -> Result<()> {
    if let Some(acceptor) = tls_acceptor {
        let tls_stream = acceptor
            .accept(stream)
            .await
            .context("TLS handshake failed")?;
        let ws_stream =
            accept_with_checks(tls_stream, allowed_origins, &app_config.app_key).await?;
        info!(%peer_addr, "websocket connected (tls)");
        return serve_socket(ws_stream, app_config, tick_seconds).await;
    }

    let ws_stream = accept_with_checks(stream, allowed_origins, &app_config.app_key).await?;
    info!(%peer_addr, "websocket connected");
    serve_socket(ws_stream, app_config, tick_seconds).await
}

async fn accept_with_checks<S>(
    stream: S,
    allowed_origins: AllowedOrigins,
    app_key: &str,
) -> Result<WebSocketStream<S>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let app_key = app_key.to_string();
    let callback = move |request: &Request, response: Response| {
        let origin = request
            .headers()
            .get("Origin")
            .and_then(|value| value.to_str().ok());
        debug!(path = %request.uri(), origin = ?origin, "ws handshake");
        if !allowed_origins.allows(origin) {
            warn!(origin = ?origin, "rejecting websocket due to origin");
            return Err(error_response(403, "origin not allowed"));
        }

        let path = request.uri().path();
        let requested_key = extract_app_key(path);
        if requested_key.as_deref() != Some(app_key.as_str()) {
            warn!(path, "rejecting websocket due to app key mismatch");
            return Err(error_response(401, "invalid app key"));
        }

        Ok(response)
    };

    let ws_stream = accept_hdr_async(stream, callback).await?;
    Ok(ws_stream)
}

fn error_response(status: u16, message: &str) -> ErrorResponse {
    HttpResponse::builder()
        .status(status)
        .body(Some(message.to_string()))
        .unwrap_or_else(|_| {
            let mut response = HttpResponse::new(None::<String>);
            let fallback_status =
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            *response.status_mut() = fallback_status;
            response
        })
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

async fn serve_socket<S>(
    ws_stream: WebSocketStream<S>,
    app_config: AppConfig,
    tick_seconds: u64,
) -> Result<()>
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
            "activity_timeout": app_config.activity_timeout,
        })
        .to_string(),
        channel: None,
    };
    send_outbound(&mut writer, &established).await?;
    info!(%socket_id, "pusher connection established");

    let mut ticker = interval(Duration::from_secs(tick_seconds));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if subscriptions.is_empty() {
                    continue;
                }

                let tick_payload = TickPayload {
                    server_time: now_timestamp(),
                    socket_id: socket_id.clone(),
                };
                let data = serde_json::to_string(&tick_payload)?;

                for channel in subscriptions.iter() {
                    let message = OutboundMessage {
                        event: "local.tick".to_string(),
                        data: data.clone(),
                        channel: Some(channel.clone()),
                    };
                    if let Err(err) = send_outbound(&mut writer, &message).await {
                        warn!(error = %err, channel = %channel, "failed to send tick");
                        return Ok(());
                    }
                    debug!(channel = %channel, "sent tick");
                }
            }
            message = reader.next() => {
                let Some(message) = message else {
                    break;
                };
                match message {
                    Ok(Message::Text(text)) => {
                        if let Err(err) = handle_client_message(
                            &text,
                            &socket_id,
                            &app_config,
                            &mut subscriptions,
                            &mut writer,
                        ).await {
                            warn!(error = %err, "failed to handle message");
                        }
                    }
                    Ok(Message::Ping(payload)) => {
                        writer.send(Message::Pong(payload)).await?;
                    }
                    Ok(Message::Close(frame)) => {
                        info!(?frame, "client closed websocket");
                        break;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        warn!(error = %err, "websocket error");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_client_message<S>(
    payload: &str,
    socket_id: &str,
    app_config: &AppConfig,
    subscriptions: &mut HashSet<String>,
    writer: &mut futures::stream::SplitSink<WebSocketStream<S>, Message>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let message: ClientMessage = match serde_json::from_str(payload) {
        Ok(message) => message,
        Err(err) => {
            warn!(error = %err, len = payload.len(), "invalid client payload");
            return Ok(());
        }
    };

    match message.event.as_str() {
        "pusher:ping" => {
            debug!(%socket_id, "received ping");
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
                    debug!(%socket_id, channel = %channel, "received subscribe");
                    if requires_auth(&channel) {
                        let auth = extract_string(&data, "auth");
                        let channel_data = extract_channel_data(&data);
                        let Some(secret) = app_config.app_secret.as_deref() else {
                            warn!(%socket_id, channel = %channel, "missing app secret for auth");
                            send_auth_error(writer, "missing app secret").await?;
                            return Ok(());
                        };
                        if !validate_auth(
                            socket_id,
                            &channel,
                            auth.as_deref(),
                            channel_data.as_deref(),
                            &app_config.app_key,
                            secret,
                        ) {
                            warn!(%socket_id, channel = %channel, "invalid auth signature");
                            send_auth_error(writer, "invalid auth signature").await?;
                            return Ok(());
                        }
                    }

                    subscriptions.insert(channel.clone());
                    let data = if channel.starts_with("presence-") {
                        build_presence_payload(extract_channel_data(&data).as_deref())
                    } else {
                        "{}".to_string()
                    };
                    let reply = OutboundMessage {
                        event: "pusher_internal:subscription_succeeded".to_string(),
                        data,
                        channel: Some(channel.clone()),
                    };
                    send_outbound(writer, &reply).await?;
                    info!(%socket_id, channel = %channel, "subscription succeeded");
                }
            }
        }
        "pusher:unsubscribe" => {
            if let Some(data) = parse_data_object(message.data) {
                if let Some(channel) = extract_string(&data, "channel") {
                    subscriptions.remove(&channel);
                    info!(%socket_id, channel = %channel, "unsubscribed");
                }
            }
        }
        _ => {
            if message.event.starts_with("client-") {
                if let Some(channel) = message.channel.clone() {
                    if subscriptions.contains(&channel) {
                        let payload = serde_json::json!({
                            "socket_id": socket_id,
                            "event": message.event,
                            "data": message.data,
                        });
                        let outbound = OutboundMessage {
                            event: "local.message".to_string(),
                            data: payload.to_string(),
                            channel: Some(channel.clone()),
                        };
                        send_outbound(writer, &outbound).await?;
                        info!(%socket_id, channel = %channel, "client event forwarded");
                    } else {
                        warn!(%socket_id, channel = %channel, "client event ignored (not subscribed)");
                    }
                } else {
                    warn!(%socket_id, "client event missing channel");
                }
            } else {
                info!(event = %message.event, "ignoring client event");
            }
        }
    }

    Ok(())
}

async fn send_outbound<S>(
    writer: &mut futures::stream::SplitSink<WebSocketStream<S>, Message>,
    message: &OutboundMessage,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(message)?;
    writer.send(Message::Text(payload)).await?;
    Ok(())
}

async fn send_auth_error<S>(
    writer: &mut futures::stream::SplitSink<WebSocketStream<S>, Message>,
    message: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    warn!(%message, "sending auth error");
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
        Value::String(text) => {
            serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|value| match value {
                    Value::Object(map) => Some(map),
                    _ => None,
                })
        }
        _ => None,
    }
}

fn extract_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_channel_data(map: &Map<String, Value>) -> Option<String> {
    match map.get("channel_data") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Object(_)) | Some(Value::Array(_)) => {
            serde_json::to_string(map.get("channel_data")?).ok()
        }
        _ => None,
    }
}

fn requires_auth(channel: &str) -> bool {
    channel.starts_with("private-") || channel.starts_with("presence-")
}

fn validate_auth(
    socket_id: &str,
    channel: &str,
    auth: Option<&str>,
    channel_data: Option<&str>,
    app_key: &str,
    app_secret: &str,
) -> bool {
    let Some(auth) = auth else {
        return false;
    };
    let mut iter = auth.splitn(2, ':');
    let key = iter.next().unwrap_or("");
    let signature = iter.next().unwrap_or("");
    if key != app_key {
        return false;
    }

    let mut sign_data = format!("{}:{}", socket_id, channel);
    if let Some(channel_data) = channel_data {
        if !channel_data.is_empty() {
            sign_data = format!("{}:{}", sign_data, channel_data);
        }
    }

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(app_secret.as_bytes()) else {
        error!("Failed to initialize HMAC; invalid app secret");
        return false;
    };
    mac.update(sign_data.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    expected.eq_ignore_ascii_case(signature)
}

fn build_presence_payload(channel_data: Option<&str>) -> String {
    let mut user_id = "local".to_string();
    let mut user_info = Value::Object(Map::new());

    if let Some(channel_data) = channel_data {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(channel_data) {
            if let Some(id_value) = map.get("user_id") {
                if let Some(id_str) = id_value.as_str() {
                    user_id = id_str.to_string();
                } else {
                    user_id = id_value.to_string();
                }
            }
            if let Some(info_value) = map.get("user_info") {
                user_info = info_value.clone();
            }
        }
    }

    serde_json::json!({
        "presence": {
            "count": 1,
            "ids": [user_id.clone()],
            "hash": {
                user_id: user_info,
            },
        },
    })
    .to_string()
}

fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn generate_socket_id() -> String {
    let mut rng = rand::rng();
    let left: u32 = rng.random_range(100..=9_999);
    let right: u32 = rng.random_range(100..=9_999_999);
    format!("{}.{}", left, right)
}

fn resolve_tls_paths(
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    if cert.is_some() && key.is_some() {
        return (cert, key);
    }

    if let Some((default_cert, default_key)) = default_tls_paths() {
        info!(
            "using default TLS cert/key at {} and {}",
            default_cert.display(),
            default_key.display()
        );
        return (Some(default_cert), Some(default_key));
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

fn default_tls_paths() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Herd")
        .join("config")
        .join("valet")
        .join("Certificates");
    let cert = base.join("hyperion.test.crt");
    let key = base.join("hyperion.test.key");
    if cert.exists() && key.exists() {
        return Some((cert, key));
    }

    None
}

fn generate_self_signed_paths() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home)
        .join(".openagents")
        .join("ws-test")
        .join("certs");
    let cert = base.join("ws-test.local.crt");
    let key = base.join("ws-test.local.key");

    if cert.exists() && key.exists() {
        return Some((cert, key));
    }

    if let Err(err) = std::fs::create_dir_all(&base) {
        warn!(error = %err, "failed to create cert directory");
        return None;
    }

    if let Err(err) = generate_self_signed_cert(&cert, &key) {
        warn!(error = %err, "failed to generate self-signed cert");
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
        "hyperion.test".to_string(),
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
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
    let cert_file = std::fs::File::open(cert_path)
        .with_context(|| format!("open cert {}", cert_path.display()))?;
    let key_file = std::fs::File::open(key_path)
        .with_context(|| format!("open key {}", key_path.display()))?;

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
