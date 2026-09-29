//! The door proxy: the only way a run's child reaches its pinned door.
//!
//! A run never hands the child the door's key. It starts a loopback
//! listener for the run, gives the child that listener's URL and a fresh
//! random token as its door, and forwards each request whose bearer is
//! the token to the real door with the real key. The token is worthless
//! once the run ends, and the key never enters the child's environment,
//! its trajectory, or anything under `out/`. On macOS the sandbox denies
//! every other address, so the proxy is also the child's only network.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::Response;
use futures_util::TryStreamExt;

/// The largest request body the proxy forwards.
pub const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// A credential that never prints.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps `value`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value, for the one place that sends it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

/// A door the proxy forwards to.
#[derive(Clone, Debug)]
pub struct Upstream {
    /// The door's base URL, such as `https://ai-gateway.vercel.sh`.
    pub url: String,
    /// The bearer the door takes.
    pub key: Secret,
}

struct Shared {
    upstream: Upstream,
    token: String,
    client: reqwest::Client,
    requests: AtomicU64,
    refused_credential: AtomicBool,
}

/// A running proxy. Dropping it stops the listener.
pub struct Proxy {
    addr: SocketAddr,
    token: Secret,
    shared: Arc<Shared>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proxy").field("addr", &self.addr).finish()
    }
}

/// A fresh random token, 32 bytes as hex.
///
/// # Errors
///
/// Returns the I/O error when the system's random source can't be read.
pub fn random_token() -> std::io::Result<String> {
    use std::io::Read;
    let mut bytes = [0_u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl Proxy {
    /// Starts a proxy to `upstream` on a free loopback port.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the listener or its runtime can't start.
    pub fn start(upstream: Upstream) -> std::io::Result<Self> {
        let token = random_token()?;
        let client = reqwest::Client::builder()
            .build()
            .map_err(std::io::Error::other)?;
        let shared = Arc::new(Shared {
            upstream,
            token: token.clone(),
            client,
            requests: AtomicU64::new(0),
            refused_credential: AtomicBool::new(false),
        });
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let state = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("ext-eval-door-proxy".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
                        return;
                    };
                    let app = axum::Router::new().fallback(forward).with_state(state);
                    let _ = axum::serve(listener, app)
                        .with_graceful_shutdown(async move {
                            let _ = stopped.await;
                        })
                        .await;
                });
            })?;
        Ok(Self {
            addr,
            token: Secret::new(token),
            shared,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// The URL the child names as its door.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The token the child sends as its bearer.
    #[must_use]
    pub fn token(&self) -> &Secret {
        &self.token
    }

    /// How many requests the child made through the proxy.
    #[must_use]
    pub fn requests(&self) -> u64 {
        self.shared.requests.load(Ordering::SeqCst)
    }

    /// Whether the door refused the key (401 or 403) at least once.
    #[must_use]
    pub fn refused_credential(&self) -> bool {
        self.shared.refused_credential.load(Ordering::SeqCst)
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const HOP_BY_HOP: [HeaderName; 5] = [
    header::CONNECTION,
    header::TRANSFER_ENCODING,
    header::CONTENT_LENGTH,
    header::HOST,
    header::UPGRADE,
];

fn plain(status: StatusCode, message: &'static str) -> Response {
    let mut response = Response::new(Body::from(message));
    *response.status_mut() = status;
    response
}

async fn forward(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let expected = format!("Bearer {}", shared.token);
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());
    if bearer != Some(expected.as_str()) && api_key != Some(shared.token.as_str()) {
        return plain(StatusCode::UNAUTHORIZED, "the run's door token is required");
    }
    shared.requests.fetch_add(1, Ordering::SeqCst);
    let (parts, body) = request.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_REQUEST_BYTES).await else {
        return plain(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the request body is too large",
        );
    };
    let path = parts
        .uri
        .path_and_query()
        .map_or("/", axum::http::uri::PathAndQuery::as_str);
    let url = format!("{}{path}", shared.upstream.url.trim_end_matches('/'));
    let mut headers = HeaderMap::new();
    for (name, value) in &parts.headers {
        if HOP_BY_HOP.contains(name) || name == header::AUTHORIZATION || name == "x-api-key" {
            continue;
        }
        headers.insert(name.clone(), value.clone());
    }
    let Ok(method) = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()) else {
        return plain(StatusCode::METHOD_NOT_ALLOWED, "unsupported method");
    };
    let sent = shared
        .client
        .request(method, url)
        .headers(headers)
        .bearer_auth(shared.upstream.key.expose())
        .body(bytes)
        .send()
        .await;
    let upstream = match sent {
        Ok(upstream) => upstream,
        Err(_) => return plain(StatusCode::BAD_GATEWAY, "the door did not answer"),
    };
    let status = upstream.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        shared.refused_credential.store(true, Ordering::SeqCst);
    }
    // Headers are copied but not the upstream's own framing: the proxy
    // re-frames the streamed body.
    let copied: Vec<(HeaderName, HeaderValue)> = upstream
        .headers()
        .iter()
        .filter(|(name, _)| !HOP_BY_HOP.contains(*name))
        .filter_map(|(name, value)| {
            Some((
                HeaderName::from_bytes(name.as_str().as_bytes()).ok()?,
                HeaderValue::from_bytes(value.as_bytes()).ok()?,
            ))
        })
        .collect();
    let mut response = Response::new(Body::from_stream(
        upstream.bytes_stream().map_err(std::io::Error::other),
    ));
    *response.status_mut() =
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    for (name, value) in copied {
        response.headers_mut().insert(name, value);
    }
    response
}
