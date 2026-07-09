//! oa-cloud-run-bridge — narrow bearer-token-gated HTTPS reverse proxy
//! (cloud/#issue: Agent Computers, openagents#8503).
//!
//! Cloudflare Workers run from Cloudflare's globally-distributed edge IPs,
//! which cannot reach `oa-codex-control-1` through its IAP/single-static-IP
//! firewall (`oa-codex-control-port`), and that firewall must not be widened
//! to accept Cloudflare's broad IP ranges. This service is the ONLY new
//! public entry point: it terminates HTTPS on Cloud Run, independently
//! re-validates the same bearer token the Worker already sends (defense in
//! depth — it does not just blindly forward), and forwards the request to
//! the control node's *internal* IP over a Serverless VPC Access connector
//! attached to the `default` network, where the existing
//! `default-allow-internal` firewall rule (10.128.0.0/9) already permits the
//! hop with no firewall change.
//!
//! Deliberately dependency-light (std `TcpListener` + a hand-rolled HTTP/1.1
//! parser, matching the style of `oa-codex-control`), so this stays a single
//! small binary that is fast to build and easy to audit.
//!
//! Fails closed: if the shared control token or the upstream control-plane
//! URL is not configured, every forwarded route refuses with a typed 503
//! rather than silently passing traffic through or accepting any bearer.

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 512 * 1024;
const DEFAULT_PORT: &str = "8080";
const DEFAULT_ALLOWED_PATH_PREFIXES: &str = "/v1/placement";
const DEFAULT_UPSTREAM_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone)]
struct Config {
    bind: String,
    /// The shared bearer token. Read from Secret Manager (mounted as an env
    /// var by Cloud Run), never hardcoded, never logged.
    control_token: Option<String>,
    /// Internal base URL of `oa-codex-control-1`, e.g.
    /// `http://10.128.0.8:8787`. Reachable only via the Serverless VPC
    /// Access connector attached to the `default` network.
    control_url: Option<String>,
    allowed_path_prefixes: Vec<String>,
    upstream_timeout: Duration,
}

impl Config {
    fn from_env() -> Self {
        let port = env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
        let bind = env::var("OA_BRIDGE_BIND").unwrap_or_else(|_| format!("0.0.0.0:{port}"));
        let control_token = env::var("OA_BRIDGE_CONTROL_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());
        let control_url = env::var("OA_BRIDGE_CONTROL_URL")
            .ok()
            .map(|value| value.trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let allowed_path_prefixes = env::var("OA_BRIDGE_ALLOWED_PATH_PREFIXES")
            .unwrap_or_else(|_| DEFAULT_ALLOWED_PATH_PREFIXES.to_string())
            .split(',')
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        let upstream_timeout = env::var("OA_BRIDGE_UPSTREAM_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_UPSTREAM_TIMEOUT_SECS));
        Config {
            bind,
            control_token,
            control_url,
            allowed_path_prefixes,
            upstream_timeout,
        }
    }
}

#[derive(Debug)]
struct ParsedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    Incomplete,
    Malformed,
    HeadersTooLarge,
    BodyTooLarge,
}

/// Parses a full HTTP/1.1 request out of `buf`, given the caller has already
/// confirmed the full body (per `Content-Length`) is present. Pure and
/// side-effect-free so it is directly unit-testable without a live socket.
fn parse_http_request(buf: &[u8]) -> Result<ParsedRequest, ParseError> {
    let header_end = find_subslice(buf, b"\r\n\r\n").ok_or(ParseError::Incomplete)?;
    if header_end > MAX_HEADER_BYTES {
        return Err(ParseError::HeadersTooLarge);
    }
    let header_text = std::str::from_utf8(&buf[..header_end]).map_err(|_| ParseError::Malformed)?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or(ParseError::Malformed)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or(ParseError::Malformed)?.to_string();
    let raw_target = parts.next().ok_or(ParseError::Malformed)?.to_string();
    let path = raw_target
        .split('?')
        .next()
        .unwrap_or(&raw_target)
        .to_string();

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or(ParseError::Malformed)?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    let body_start = header_end + 4;
    let content_length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(ParseError::BodyTooLarge);
    }
    let available_body = buf.len().saturating_sub(body_start);
    if available_body < content_length {
        return Err(ParseError::Incomplete);
    }
    let body = buf[body_start..body_start + content_length].to_vec();

    Ok(ParsedRequest {
        method,
        path,
        headers,
        body,
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_str())
}

/// Extracts the raw token out of an `Authorization: Bearer <token>` header
/// value. Returns `None` for anything else (missing header, wrong scheme,
/// empty token).
fn extract_bearer_token(header_value: Option<&str>) -> Option<&str> {
    let value = header_value?;
    let token = value.strip_prefix("Bearer ")?;
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Constant-time string comparison so a timing side-channel cannot leak the
/// expected bearer token one byte at a time.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Whether `path` is on the allow-list of paths this bridge will forward.
/// Everything else refuses with 404 rather than silently proxying an
/// unreviewed route to the control plane.
fn is_path_allowed(path: &str, allowed_prefixes: &[String]) -> bool {
    allowed_prefixes
        .iter()
        .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/")))
}

fn build_upstream_url(base_url: &str, path: &str) -> String {
    format!("{base_url}{path}")
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: String,
    body: Vec<u8>,
}

impl HttpResponse {
    fn json(status: u16, reason: &'static str, body: &str) -> Self {
        HttpResponse {
            status,
            reason,
            content_type: "application/json".to_string(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn write_to(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let header = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            self.reason,
            self.content_type,
            self.body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()
    }
}

fn read_full_request(stream: &mut TcpStream) -> Result<Vec<u8>, ParseError> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match parse_http_request(&buf) {
            Ok(_) => return Ok(buf),
            Err(ParseError::Incomplete) => {
                if buf.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
                    return Err(ParseError::HeadersTooLarge);
                }
                let read = stream.read(&mut chunk).map_err(|_| ParseError::Malformed)?;
                if read == 0 {
                    return Err(ParseError::Malformed);
                }
                buf.extend_from_slice(&chunk[..read]);
            }
            Err(other) => return Err(other),
        }
    }
}

fn handle_connection(mut stream: TcpStream, config: &Config) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let raw = match read_full_request(&mut stream) {
        Ok(raw) => raw,
        Err(_) => {
            let _ = HttpResponse::json(400, "Bad Request", r#"{"error":"malformed_request"}"#)
                .write_to(&mut stream);
            return;
        }
    };
    let request = match parse_http_request(&raw) {
        Ok(request) => request,
        Err(_) => {
            let _ = HttpResponse::json(400, "Bad Request", r#"{"error":"malformed_request"}"#)
                .write_to(&mut stream);
            return;
        }
    };

    if request.path == "/healthz" {
        let _ = HttpResponse::json(200, "OK", r#"{"status":"ok"}"#).write_to(&mut stream);
        return;
    }

    let response = route(&request, config);
    let _ = response.write_to(&mut stream);
}

fn route(request: &ParsedRequest, config: &Config) -> HttpResponse {
    let (control_token, control_url) = match (&config.control_token, &config.control_url) {
        (Some(token), Some(url)) => (token, url),
        _ => {
            eprintln!("oa-cloud-run-bridge: refusing request, bridge_not_armed (missing control token or url)");
            return HttpResponse::json(
                503,
                "Service Unavailable",
                r#"{"error":"bridge_not_armed"}"#,
            );
        }
    };

    let presented = extract_bearer_token(header_value(&request.headers, "authorization"));
    let authorized = match presented {
        Some(token) => constant_time_eq(token, control_token),
        None => false,
    };
    if !authorized {
        eprintln!("oa-cloud-run-bridge: rejected unauthorized request path={}", request.path);
        return HttpResponse::json(401, "Unauthorized", r#"{"error":"unauthorized"}"#);
    }

    if !is_path_allowed(&request.path, &config.allowed_path_prefixes) {
        eprintln!("oa-cloud-run-bridge: rejected disallowed path={}", request.path);
        return HttpResponse::json(404, "Not Found", r#"{"error":"not_found"}"#);
    }

    forward(request, control_url, control_token, config.upstream_timeout)
}

fn forward(
    request: &ParsedRequest,
    control_url: &str,
    control_token: &str,
    timeout: Duration,
) -> HttpResponse {
    let upstream_url = build_upstream_url(control_url, &request.path);
    let client = match reqwest::blocking::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("oa-cloud-run-bridge: failed to build upstream client: {error}");
            return HttpResponse::json(
                502,
                "Bad Gateway",
                r#"{"error":"upstream_client_build_failed"}"#,
            );
        }
    };

    let method = match request.method.to_ascii_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "DELETE" => reqwest::Method::DELETE,
        other => {
            eprintln!("oa-cloud-run-bridge: rejected unsupported method={other}");
            return HttpResponse::json(405, "Method Not Allowed", r#"{"error":"method_not_allowed"}"#);
        }
    };

    let mut builder = client
        .request(method, &upstream_url)
        .header("Authorization", format!("Bearer {control_token}"))
        .header("Content-Type", "application/json");
    if !request.body.is_empty() {
        builder = builder.body(request.body.clone());
    }

    match builder.send() {
        Ok(upstream_response) => {
            let status = upstream_response.status().as_u16();
            let content_type = upstream_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/json")
                .to_string();
            let body = upstream_response.bytes().map(|b| b.to_vec()).unwrap_or_default();
            HttpResponse {
                status,
                reason: status_reason(status),
                content_type,
                body,
            }
        }
        Err(error) => {
            eprintln!("oa-cloud-run-bridge: upstream request failed: {error}");
            HttpResponse::json(502, "Bad Gateway", r#"{"error":"upstream_unreachable"}"#)
        }
    }
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Response",
    }
}

fn main() {
    let config = Config::from_env();
    if config.control_token.is_none() {
        eprintln!("oa-cloud-run-bridge: WARNING starting with no OA_BRIDGE_CONTROL_TOKEN configured; all forwarded routes will refuse with bridge_not_armed");
    }
    if config.control_url.is_none() {
        eprintln!("oa-cloud-run-bridge: WARNING starting with no OA_BRIDGE_CONTROL_URL configured; all forwarded routes will refuse with bridge_not_armed");
    }
    let listener = TcpListener::bind(&config.bind).unwrap_or_else(|error| {
        eprintln!("oa-cloud-run-bridge: failed to bind {}: {error}", config.bind);
        std::process::exit(1);
    });
    eprintln!("oa-cloud-run-bridge listening on {}", config.bind);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = config.clone();
                std::thread::spawn(move || handle_connection(stream, &config));
            }
            Err(error) => {
                eprintln!("oa-cloud-run-bridge: accept error: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_post_request() {
        let raw = b"POST /v1/placement HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer abc\r\nContent-Length: 13\r\n\r\n{\"goal\":\"x\"}\n";
        let parsed = parse_http_request(raw).expect("should parse");
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.path, "/v1/placement");
        assert_eq!(parsed.body, b"{\"goal\":\"x\"}\n");
        assert_eq!(
            header_value(&parsed.headers, "authorization"),
            Some("Bearer abc")
        );
    }

    #[test]
    fn strips_query_string_from_path() {
        let raw = b"GET /healthz?probe=1 HTTP/1.1\r\nHost: x\r\n\r\n";
        let parsed = parse_http_request(raw).expect("should parse");
        assert_eq!(parsed.path, "/healthz");
    }

    #[test]
    fn reports_incomplete_when_body_not_fully_buffered() {
        let raw = b"POST /v1/placement HTTP/1.1\r\nContent-Length: 20\r\n\r\n{\"partial\":true";
        assert_eq!(parse_http_request(raw).unwrap_err(), ParseError::Incomplete);
    }

    #[test]
    fn reports_incomplete_when_headers_not_terminated() {
        let raw = b"POST /v1/placement HTTP/1.1\r\nContent-Length: 20";
        assert_eq!(parse_http_request(raw).unwrap_err(), ParseError::Incomplete);
    }

    #[test]
    fn rejects_oversized_body() {
        let header = b"POST /v1/placement HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n";
        assert_eq!(parse_http_request(header).unwrap_err(), ParseError::BodyTooLarge);
    }

    #[test]
    fn extracts_bearer_token() {
        assert_eq!(extract_bearer_token(Some("Bearer secret-token")), Some("secret-token"));
        assert_eq!(extract_bearer_token(Some("Basic abc")), None);
        assert_eq!(extract_bearer_token(Some("Bearer ")), None);
        assert_eq!(extract_bearer_token(None), None);
    }

    #[test]
    fn constant_time_eq_matches_equal_and_rejects_different() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("short", "muchlongervalue"));
        assert!(!constant_time_eq("", "nonempty"));
    }

    #[test]
    fn path_allow_list_matches_exact_and_subpaths_only() {
        let allowed = vec!["/v1/placement".to_string()];
        assert!(is_path_allowed("/v1/placement", &allowed));
        assert!(is_path_allowed("/v1/placement/sub", &allowed));
        assert!(!is_path_allowed("/v1/placement-other", &allowed));
        assert!(!is_path_allowed("/v1/other", &allowed));
        assert!(!is_path_allowed("/", &allowed));
    }

    #[test]
    fn builds_upstream_url_by_joining_base_and_path() {
        assert_eq!(
            build_upstream_url("http://10.128.0.8:8787", "/v1/placement"),
            "http://10.128.0.8:8787/v1/placement"
        );
    }

    #[test]
    fn route_refuses_when_not_armed() {
        let config = Config {
            bind: "127.0.0.1:0".to_string(),
            control_token: None,
            control_url: None,
            allowed_path_prefixes: vec!["/v1/placement".to_string()],
            upstream_timeout: Duration::from_secs(1),
        };
        let request = ParsedRequest {
            method: "POST".to_string(),
            path: "/v1/placement".to_string(),
            headers: vec![("authorization".to_string(), "Bearer anything".to_string())],
            body: vec![],
        };
        let response = route(&request, &config);
        assert_eq!(response.status, 503);
    }

    #[test]
    fn route_rejects_wrong_bearer_token() {
        let config = Config {
            bind: "127.0.0.1:0".to_string(),
            control_token: Some("expected-token".to_string()),
            control_url: Some("http://127.0.0.1:1".to_string()),
            allowed_path_prefixes: vec!["/v1/placement".to_string()],
            upstream_timeout: Duration::from_secs(1),
        };
        let request = ParsedRequest {
            method: "POST".to_string(),
            path: "/v1/placement".to_string(),
            headers: vec![("authorization".to_string(), "Bearer wrong-token".to_string())],
            body: vec![],
        };
        let response = route(&request, &config);
        assert_eq!(response.status, 401);
    }

    #[test]
    fn route_rejects_disallowed_path_even_with_valid_token() {
        let config = Config {
            bind: "127.0.0.1:0".to_string(),
            control_token: Some("expected-token".to_string()),
            control_url: Some("http://127.0.0.1:1".to_string()),
            allowed_path_prefixes: vec!["/v1/placement".to_string()],
            upstream_timeout: Duration::from_secs(1),
        };
        let request = ParsedRequest {
            method: "POST".to_string(),
            path: "/v1/some-other-route".to_string(),
            headers: vec![("authorization".to_string(), "Bearer expected-token".to_string())],
            body: vec![],
        };
        let response = route(&request, &config);
        assert_eq!(response.status, 404);
    }
}
