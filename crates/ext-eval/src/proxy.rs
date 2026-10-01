//! The door proxy: the only way a run's child reaches its pinned door.
//!
//! A run never hands the child the door's key. It starts a loopback
//! listener for the run, gives the child that listener's URL and a fresh
//! random token as its door, and forwards each request whose bearer is
//! the token to the real door with the real key. The token is worthless
//! once the run ends, and the key never enters the child's environment,
//! its trajectory, or anything under `out/`. On macOS the sandbox denies
//! every other address, so the proxy is also the child's only network.
//!
//! A decision door may carry Jev's other doors ([`Upstream::decisions`]):
//! then a decision (`POST /v1/systemone`) goes through
//! [`jev::doors::Failover`], which asks each door in its order and leaves
//! one only when it couldn't answer for a reason of its own (a 402 for an
//! account out of credits, a 5xx, a timeout). Every other route goes to
//! [`Upstream::url`] alone. Without it, one door that can't pay takes Jev,
//! and with Jev the programs Coder picks, away from both arms
//! ([#10122](https://github.com/OpenAgentsInc/openagents/issues/10122)).

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
    /// Jev's doors, in the order a decision asks them, when this is a
    /// decision door with more than one. `None` sends every request to
    /// `url`.
    pub decisions: Option<jev::doors::Failover>,
}

/// The longest a decision may take across all of Jev's doors.
pub const DECISION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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
    let mut bytes = [0_u8; 32];
    #[cfg(unix)]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    }
    #[cfg(not(unix))]
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
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

/// A decision through Jev's doors in order, answered as the door that
/// answered it (or, when none did, as the first door refused).
async fn decide(
    shared: &Shared,
    failover: &jev::doors::Failover,
    headers: &HeaderMap,
    path: &str,
    body: Vec<u8>,
) -> Response {
    use jev::exchange::{Call, Exchange, Failure};
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    };
    let call = Call {
        method: "POST".into(),
        path: path.to_string(),
        body: Some(body),
        idempotency_key: header("idempotency-key"),
        attempt: header("x-attempt")
            .and_then(|attempt| attempt.parse().ok())
            .unwrap_or(1),
        timeout: DECISION_TIMEOUT,
    };
    let reply = match failover.exchange(call).await {
        Ok(reply) => reply,
        Err(Failure::Timeout) => {
            return plain(StatusCode::GATEWAY_TIMEOUT, "no Jev door answered in time");
        }
        Err(Failure::Unreachable(_)) => {
            return plain(StatusCode::BAD_GATEWAY, "no Jev door could be reached");
        }
    };
    if reply.status == 401 || reply.status == 403 {
        shared.refused_credential.store(true, Ordering::SeqCst);
    }
    let mut response = Response::new(Body::from(reply.body));
    *response.status_mut() = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::BAD_GATEWAY);
    for (name, value) in reply.headers {
        let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) else {
            continue;
        };
        if !HOP_BY_HOP.contains(&name) {
            response.headers_mut().insert(name, value);
        }
    }
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
    if let Some(failover) = &shared.upstream.decisions
        && parts.method == axum::http::Method::POST
        && parts.uri.path() == jev::doors::SYSTEM_ONE_PATH
    {
        return decide(&shared, failover, &parts.headers, path, bytes.to_vec()).await;
    }
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
