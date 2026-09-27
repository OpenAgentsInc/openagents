//! Wake transports.
//!
//! A transport receives a [`WakeRequest`]: a stable request ID, the opaque
//! lease endpoint, and a deadline. The type carries no event, lease, or
//! relay content, so no transport can put one in a push. The APNs and FCM
//! adapters post the NIP-PL relay-delivery request to a credential-holding
//! push gateway, which sends the platform's registered wake constant. The
//! relay never holds an APNs or FCM credential.

use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::domain::{RelaySigner, Tag};

/// NIP-98 HTTP authorization kind.
const HTTP_AUTH_KIND: u16 = 27_235;
const MAX_RESPONSE_BYTES: usize = 16_384;
const MAX_ENDPOINT_BYTES: usize = 4_096;

/// A boxed transport future.
pub type WakeFuture<'a> = Pin<Box<dyn Future<Output = WakeOutcome> + Send + 'a>>;

/// A platform with a registered wake constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Apple Push Notification service.
    Apns,
    /// Firebase Cloud Messaging.
    Fcm,
}

impl Platform {
    /// The lease `transport` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
        }
    }

    /// The fixed application payload the gateway sends for this platform.
    #[must_use]
    pub const fn wake_constant(self) -> &'static str {
        match self {
            Self::Apns => nostr::push_lease::APNS_BODY,
            Self::Fcm => nostr::push_lease::FCM_DATA,
        }
    }

    /// Parse a lease `transport` value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "apns" => Some(Self::Apns),
            "fcm" => Some(Self::Fcm),
            _ => None,
        }
    }

    const fn delivery_path(self) -> &'static str {
        match self {
            Self::Apns => "/v1/deliveries/apns",
            Self::Fcm => "/v1/deliveries/fcm",
        }
    }
}

/// Everything a transport may know about one wake.
#[derive(Clone, PartialEq, Eq)]
pub struct WakeRequest {
    /// The durable job UUID. Stable across retries.
    pub request_id: String,
    /// The opaque lease endpoint, such as a gateway delivery capability.
    pub endpoint: String,
    /// Unix seconds after which the wake is useless.
    pub expires_at: u64,
}

impl fmt::Debug for WakeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WakeRequest")
            .field("request_id", &self.request_id)
            .field("endpoint", &"<withheld>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// A transport's classification of one attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeOutcome {
    /// The provider accepted the wake.
    Accepted,
    /// The endpoint is permanently invalid.
    InvalidEndpoint,
    /// A transient failure. Retry, no sooner than `after_seconds` if given.
    Retry {
        /// Provider-requested delay.
        after_seconds: Option<u64>,
        /// Operator-facing cause code.
        reason: &'static str,
    },
    /// A permanent refusal of this request.
    Rejected(&'static str),
}

/// Sends wake signals for one platform.
pub trait WakeTransport: Send + Sync {
    /// The platform whose constant this transport sends.
    fn platform(&self) -> Platform;
    /// Attempt one wake. Must not panic and must bound its own time.
    fn deliver<'a>(&'a self, request: &'a WakeRequest) -> WakeFuture<'a>;
}

/// APNs through a push gateway at `…/v1/deliveries/apns`.
pub struct ApnsGateway(GatewayClient);

/// FCM through a push gateway at `…/v1/deliveries/fcm`.
pub struct FcmGateway(GatewayClient);

impl ApnsGateway {
    /// A gateway adapter whose NIP-98 events are signed by `signer`.
    ///
    /// # Errors
    ///
    /// Returns a reason when `base_url` is not an `http://` base URL.
    pub fn new(base_url: &str, signer: RelaySigner) -> Result<Self, String> {
        GatewayClient::new(base_url, signer, Platform::Apns).map(Self)
    }
}

impl FcmGateway {
    /// A gateway adapter whose NIP-98 events are signed by `signer`.
    ///
    /// # Errors
    ///
    /// Returns a reason when `base_url` is not an `http://` base URL.
    pub fn new(base_url: &str, signer: RelaySigner) -> Result<Self, String> {
        GatewayClient::new(base_url, signer, Platform::Fcm).map(Self)
    }
}

impl WakeTransport for ApnsGateway {
    fn platform(&self) -> Platform {
        Platform::Apns
    }
    fn deliver<'a>(&'a self, request: &'a WakeRequest) -> WakeFuture<'a> {
        Box::pin(self.0.deliver(request))
    }
}

impl WakeTransport for FcmGateway {
    fn platform(&self) -> Platform {
        Platform::Fcm
    }
    fn deliver<'a>(&'a self, request: &'a WakeRequest) -> WakeFuture<'a> {
        Box::pin(self.0.deliver(request))
    }
}

/// An in-memory transport that records requests and returns scripted
/// outcomes, `Accepted` once the script is empty. For tests and local runs.
pub struct TestTransport {
    platform: Platform,
    sent: Mutex<Vec<WakeRequest>>,
    script: Mutex<VecDeque<WakeOutcome>>,
}

impl TestTransport {
    /// A recording transport for `platform`.
    #[must_use]
    pub fn new(platform: Platform) -> Arc<Self> {
        Arc::new(Self {
            platform,
            sent: Mutex::new(Vec::new()),
            script: Mutex::new(VecDeque::new()),
        })
    }

    /// Queue outcomes for the next attempts.
    pub fn script(&self, outcomes: impl IntoIterator<Item = WakeOutcome>) {
        self.script
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend(outcomes);
    }

    /// Every request received so far, in order.
    #[must_use]
    pub fn sent(&self) -> Vec<WakeRequest> {
        self.sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl WakeTransport for TestTransport {
    fn platform(&self) -> Platform {
        self.platform
    }
    fn deliver<'a>(&'a self, request: &'a WakeRequest) -> WakeFuture<'a> {
        self.sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        let outcome = self
            .script
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or(WakeOutcome::Accepted);
        Box::pin(async move { outcome })
    }
}

/// A bounded HTTP/1.1 client for the NIP-PL relay-delivery route.
struct GatewayClient {
    host: String,
    port: u16,
    authority: String,
    path: String,
    url: String,
    signer: RelaySigner,
    timeout: Duration,
    attempts: AtomicU64,
}

impl GatewayClient {
    fn new(base_url: &str, signer: RelaySigner, platform: Platform) -> Result<Self, String> {
        let rest = base_url
            .strip_prefix("http://")
            .ok_or_else(|| "the push gateway URL must start with http://".to_owned())?;
        if base_url.len() > 2_048
            || base_url.contains(['?', '#', '@'])
            || base_url
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(
                "the push gateway URL must not contain a query, fragment, credentials, or whitespace"
                    .to_owned(),
            );
        }
        let (authority, prefix) = match rest.split_once('/') {
            Some((authority, prefix)) => (authority, format!("/{}", prefix.trim_end_matches('/'))),
            None => (rest, String::new()),
        };
        let prefix = if prefix == "/" { String::new() } else { prefix };
        if authority.is_empty() {
            return Err("the push gateway URL has no host".to_owned());
        }
        let parse_port = |port: &str| {
            port.parse::<u16>()
                .map_err(|_| "the push gateway port is not a number".to_owned())
        };
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, after) = bracketed
                .split_once(']')
                .ok_or_else(|| "the push gateway host is not valid".to_owned())?;
            let port = match after.strip_prefix(':') {
                Some(port) => parse_port(port)?,
                None if after.is_empty() => 80,
                None => return Err("the push gateway host is not valid".to_owned()),
            };
            (host.to_owned(), port)
        } else {
            match authority.split_once(':') {
                Some((host, port)) => (host.to_owned(), parse_port(port)?),
                None => (authority.to_owned(), 80),
            }
        };
        if host.is_empty() {
            return Err("the push gateway URL has no host".to_owned());
        }
        let path = format!("{prefix}{}", platform.delivery_path());
        Ok(Self {
            url: format!("http://{authority}{path}"),
            host,
            port,
            authority: authority.to_owned(),
            path,
            signer,
            timeout: Duration::from_secs(5),
            attempts: AtomicU64::new(0),
        })
    }

    async fn deliver(&self, request: &WakeRequest) -> WakeOutcome {
        if request.endpoint.is_empty() || request.endpoint.len() > MAX_ENDPOINT_BYTES {
            return WakeOutcome::Rejected("endpoint_bounds");
        }
        let body = delivery_body(request);
        let authorization = self.authorization(body.as_bytes(), super::unix_now());
        let head = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAuthorization: {authorization}\r\nConnection: close\r\n\r\n",
            self.path,
            self.authority,
            body.len()
        );
        match timeout(self.timeout, self.exchange(head, body)).await {
            Ok(Ok((status, body))) => classify(status, &body),
            Ok(Err(_)) | Err(_) => WakeOutcome::Retry {
                after_seconds: None,
                reason: "gateway_unreachable",
            },
        }
    }

    /// A NIP-98 authorization for one attempt. Signing is deterministic,
    /// and a retry of the same job carries the same body, so an `attempt`
    /// tag makes each authorization distinct. Without it, a retry signed in
    /// the same second as the previous attempt repeats that event ID, which
    /// the gateway burns, and the retry fails as `invalid_grant`.
    fn authorization(&self, body: &[u8], now: u64) -> String {
        let attempt = format!(
            "{:x}-{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            self.attempts.fetch_add(1, Ordering::Relaxed)
        );
        let event = self.signer.sign(
            now,
            HTTP_AUTH_KIND,
            vec![
                Tag::new(vec!["u".into(), self.url.clone()]),
                Tag::new(vec!["method".into(), "POST".into()]),
                Tag::new(vec!["payload".into(), hex(&Sha256::digest(body))]),
                Tag::new(vec!["attempt".into(), attempt]),
            ],
            String::new(),
        );
        let json = serde_json::to_vec(&event).expect("serializing an event cannot fail");
        format!("Nostr {}", nostr::nip44::primitives::base64_encode(&json))
    }

    async fn exchange(&self, head: String, body: String) -> std::io::Result<(u16, Vec<u8>)> {
        let mut stream = TcpStream::connect((self.host.as_str(), self.port)).await?;
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(body.as_bytes()).await?;
        stream.flush().await?;
        let mut response = Vec::new();
        let mut buffer = [0_u8; 4_096];
        loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..read]);
            if response.len() > MAX_RESPONSE_BYTES {
                return Err(std::io::Error::other("response too large"));
            }
        }
        parse_response(&response).ok_or_else(|| std::io::Error::other("malformed response"))
    }
}

/// The complete relay-delivery body. It has exactly these four members.
#[must_use]
pub fn delivery_body(request: &WakeRequest) -> String {
    serde_json::json!({
        "v": 1,
        "endpoint_grant": request.endpoint,
        "request_id": request.request_id,
        "expires_at": request.expires_at,
    })
    .to_string()
}

fn parse_response(bytes: &[u8]) -> Option<(u16, Vec<u8>)> {
    let split = bytes.windows(4).position(|window| window == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&bytes[..split]).ok()?;
    let mut lines = head.split("\r\n");
    let status = lines.next()?.split(' ').nth(1)?.parse::<u16>().ok()?;
    let chunked = lines.any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    let body = &bytes[split + 4..];
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    Some((status, body))
}

fn dechunk(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = body.windows(2).position(|window| window == b"\r\n")?;
        let size_text = std::str::from_utf8(&body[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if body.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

/// Map a gateway response to an outcome, per the NIP-PL relay-delivery list.
fn classify(status: u16, body: &[u8]) -> WakeOutcome {
    let value: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let status_field = value.get("status").and_then(serde_json::Value::as_str);
    let error_field = value.get("error").and_then(serde_json::Value::as_str);
    match (status, status_field, error_field) {
        (200, Some("accepted"), _) => WakeOutcome::Accepted,
        (410, Some("invalid_endpoint"), _) => WakeOutcome::InvalidEndpoint,
        (503, Some("retry"), _) => WakeOutcome::Retry {
            after_seconds: value
                .get("retry_after_seconds")
                .and_then(serde_json::Value::as_u64),
            reason: "provider_retry",
        },
        (503, _, Some("configuration_fault")) => WakeOutcome::Retry {
            after_seconds: None,
            reason: "gateway_configuration_fault",
        },
        (429, _, _) => WakeOutcome::Retry {
            after_seconds: None,
            reason: "rate_limited",
        },
        (400, _, _) => WakeOutcome::Rejected("invalid_request"),
        (401, _, _) => WakeOutcome::Rejected("invalid_auth"),
        (404, _, _) => WakeOutcome::Rejected("invalid_grant"),
        (500..=599, _, _) => WakeOutcome::Retry {
            after_seconds: None,
            reason: "gateway_unavailable",
        },
        _ => WakeOutcome::Rejected("unexpected_response"),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap()
    }

    async fn one_exchange(response: &'static str) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            let mut buffer = [0_u8; 4_096];
            loop {
                let read = stream.read(&mut buffer).await.unwrap();
                received.extend_from_slice(&buffer[..read]);
                if let Some(split) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&received[..split]).to_string();
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if received.len() >= split + 4 + length {
                        break;
                    }
                }
            }
            stream.write_all(response.as_bytes()).await.unwrap();
            received
        });
        (format!("http://{address}"), server)
    }

    #[tokio::test]
    async fn the_gateway_request_carries_only_the_closed_wake_fields() {
        let (base, server) = one_exchange(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 21\r\n\r\n{\"status\":\"accepted\"}",
        )
        .await;
        let transport = ApnsGateway::new(&base, signer()).unwrap();
        let request = WakeRequest {
            request_id: "0f0e0d0c-0b0a-4908-8706-050403020100".into(),
            endpoint: "opaque-grant".into(),
            expires_at: super::super::unix_now() + 60,
        };
        assert_eq!(transport.deliver(&request).await, WakeOutcome::Accepted);
        let received = server.await.unwrap();
        let split = received.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(received[..split].to_vec()).unwrap();
        let body = &received[split + 4..];
        assert!(head.starts_with("POST /v1/deliveries/apns HTTP/1.1\r\n"));
        let value: serde_json::Value = serde_json::from_slice(body).unwrap();
        let object = value.as_object().unwrap();
        let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, ["endpoint_grant", "expires_at", "request_id", "v"]);
        assert_eq!(object["v"], 1);
        assert_eq!(object["endpoint_grant"], "opaque-grant");
        let authorization = head
            .lines()
            .find_map(|line| line.strip_prefix("Authorization: "))
            .unwrap();
        let auth = crate::domain::parse_http_authorization(
            authorization,
            "POST",
            &format!("{base}/v1/deliveries/apns"),
            body,
            super::super::unix_now(),
        )
        .unwrap();
        assert_eq!(auth.pubkey, signer().pubkey());
    }

    #[tokio::test]
    async fn fcm_uses_its_own_route_and_constant() {
        let (base, server) = one_exchange(
            "HTTP/1.1 410 Gone\r\nContent-Length: 62\r\n\r\n{\"status\":\"invalid_endpoint\",\"generation\":1,\"invalid_at\":null}",
        )
        .await;
        let transport = FcmGateway::new(&format!("{base}/push/"), signer()).unwrap();
        assert_eq!(
            transport.platform().wake_constant(),
            nostr::push_lease::FCM_DATA
        );
        let request = WakeRequest {
            request_id: "0f0e0d0c-0b0a-4908-8706-050403020100".into(),
            endpoint: "fcm-grant".into(),
            expires_at: 10,
        };
        assert_eq!(
            transport.deliver(&request).await,
            WakeOutcome::InvalidEndpoint
        );
        let received = String::from_utf8(server.await.unwrap()).unwrap();
        assert!(received.starts_with("POST /push/v1/deliveries/fcm HTTP/1.1\r\n"));
    }

    #[test]
    fn retries_in_one_second_get_distinct_authorizations() {
        let client = GatewayClient::new("http://gateway.test", signer(), Platform::Fcm).unwrap();
        let body = br#"{"v":1}"#;
        let first = client.authorization(body, 1_000);
        let second = client.authorization(body, 1_000);
        assert_ne!(first, second);
        for header in [&first, &second] {
            assert!(
                crate::domain::parse_http_authorization(
                    header,
                    "POST",
                    "http://gateway.test/v1/deliveries/fcm",
                    body,
                    1_000,
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn gateway_responses_map_to_bounded_outcomes() {
        assert_eq!(
            classify(503, br#"{"status":"retry","retry_after_seconds":30}"#),
            WakeOutcome::Retry {
                after_seconds: Some(30),
                reason: "provider_retry"
            }
        );
        assert!(matches!(
            classify(503, br#"{"error":"temporarily_unavailable"}"#),
            WakeOutcome::Retry { .. }
        ));
        assert!(matches!(classify(429, b"{}"), WakeOutcome::Retry { .. }));
        assert_eq!(classify(404, b"{}"), WakeOutcome::Rejected("invalid_grant"));
        assert_eq!(classify(400, b""), WakeOutcome::Rejected("invalid_request"));
        assert_eq!(
            classify(302, b""),
            WakeOutcome::Rejected("unexpected_response")
        );
        assert_eq!(
            parse_response(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n0\r\n\r\n"
            ),
            Some((200, b"abcd".to_vec()))
        );
        assert!(ApnsGateway::new("https://gateway.test", signer()).is_err());
        assert!(ApnsGateway::new("http://gateway.test/?q", signer()).is_err());
        assert!(ApnsGateway::new("http://", signer()).is_err());
    }

    #[test]
    fn a_wake_request_does_not_print_its_endpoint() {
        let request = WakeRequest {
            request_id: "id".into(),
            endpoint: "device-token-material".into(),
            expires_at: 1,
        };
        assert!(!format!("{request:?}").contains("device-token-material"));
    }
}
