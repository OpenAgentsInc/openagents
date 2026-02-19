//! NIP-98: HTTP Auth
//!
//! Implements HTTP authentication using ephemeral Nostr events (kind 27235).
//!
//! Features:
//! - Authorization event creation with URL and HTTP method
//! - Payload hash validation for POST/PUT/PATCH requests
//! - Base64 encoding for Authorization header
//! - Server-side validation (timestamp, URL, method, payload)
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/98.md>

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "full")]
use base64::Engine;
#[cfg(feature = "full")]
use sha2::{Digest, Sha256};

/// Event kind for HTTP auth (reference to RFC 7235)
pub const KIND_HTTP_AUTH: u16 = 27235;

/// Authorization scheme for HTTP header
pub const AUTH_SCHEME: &str = "Nostr";

/// Default timestamp window for validation (60 seconds)
pub const DEFAULT_TIMESTAMP_WINDOW: u64 = 60;

/// Errors that can occur during NIP-98 operations
#[derive(Debug, Error)]
pub enum Nip98Error {
    #[error("invalid event kind: expected 27235, got {0}")]
    InvalidKind(u16),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("timestamp out of window: event is {0} seconds old")]
    TimestampOutOfWindow(u64),

    #[error("url mismatch: expected {expected}, got {actual}")]
    UrlMismatch { expected: String, actual: String },

    #[error("method mismatch: expected {expected}, got {actual}")]
    MethodMismatch { expected: String, actual: String },

    #[error("payload hash mismatch: expected {expected}, got {actual}")]
    PayloadHashMismatch { expected: String, actual: String },

    #[error("base64 encode error: {0}")]
    Base64Encode(String),

    #[error("base64 decode error: {0}")]
    Base64Decode(String),

    #[error("json serialization error: {0}")]
    JsonSerialize(String),

    #[error("json deserialization error: {0}")]
    JsonDeserialize(String),

    #[error("invalid authorization header: {0}")]
    InvalidAuthHeader(String),
}

/// HTTP method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Trace,
    Connect,
}

impl HttpMethod {
    /// Parse HTTP method from string
    pub fn parse(s: &str) -> Result<Self, Nip98Error> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "PATCH" => Ok(HttpMethod::Patch),
            "DELETE" => Ok(HttpMethod::Delete),
            "HEAD" => Ok(HttpMethod::Head),
            "OPTIONS" => Ok(HttpMethod::Options),
            "TRACE" => Ok(HttpMethod::Trace),
            "CONNECT" => Ok(HttpMethod::Connect),
            _ => Err(Nip98Error::InvalidAuthHeader(format!(
                "unknown HTTP method: {}",
                s
            ))),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Trace => "TRACE",
            HttpMethod::Connect => "CONNECT",
        }
    }
}

/// HTTP authentication event data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAuth {
    /// Absolute URL (including query parameters)
    pub url: String,

    /// HTTP method
    pub method: HttpMethod,

    /// Optional SHA256 hash of request body (hex-encoded)
    pub payload_hash: Option<String>,
}

impl HttpAuth {
    /// Create new HTTP auth
    pub fn new(url: String, method: HttpMethod) -> Self {
        Self {
            url,
            method,
            payload_hash: None,
        }
    }

    /// Set payload hash
    pub fn with_payload_hash(mut self, hash: String) -> Self {
        self.payload_hash = Some(hash);
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // u tag (URL)
        tags.push(vec!["u".to_string(), self.url.clone()]);

        // method tag
        tags.push(vec!["method".to_string(), self.method.as_str().to_string()]);

        // payload tag (optional)
        if let Some(ref hash) = self.payload_hash {
            tags.push(vec!["payload".to_string(), hash.clone()]);
        }

        tags
    }

    /// Parse from event tags
    pub fn from_tags(tags: &[Vec<String>]) -> Result<Self, Nip98Error> {
        let mut url = None;
        let mut method = None;
        let mut payload_hash = None;

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "u" => {
                    if tag.len() < 2 {
                        return Err(Nip98Error::MissingTag("u tag requires URL".to_string()));
                    }
                    url = Some(tag[1].clone());
                }
                "method" => {
                    if tag.len() < 2 {
                        return Err(Nip98Error::MissingTag(
                            "method tag requires HTTP method".to_string(),
                        ));
                    }
                    method = Some(HttpMethod::parse(&tag[1])?);
                }
                "payload" => {
                    if tag.len() >= 2 {
                        payload_hash = Some(tag[1].clone());
                    }
                }
                _ => {}
            }
        }

        let url = url.ok_or_else(|| Nip98Error::MissingTag("u tag required".to_string()))?;
        let method =
            method.ok_or_else(|| Nip98Error::MissingTag("method tag required".to_string()))?;

        Ok(Self {
            url,
            method,
            payload_hash,
        })
    }
}

/// Calculate SHA256 hash of payload
#[cfg(feature = "full")]
pub fn hash_payload(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Encode event as base64 for Authorization header
#[cfg(feature = "full")]
pub fn encode_authorization_header(event_json: &str) -> Result<String, Nip98Error> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(event_json.as_bytes());
    Ok(format!("{} {}", AUTH_SCHEME, encoded))
}

/// Decode base64 event from Authorization header
#[cfg(feature = "full")]
pub fn decode_authorization_header(header: &str) -> Result<String, Nip98Error> {
    // Parse "Nostr <base64>"
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(Nip98Error::InvalidAuthHeader(
            "expected format: Nostr <base64>".to_string(),
        ));
    }

    if parts[0] != AUTH_SCHEME {
        return Err(Nip98Error::InvalidAuthHeader(format!(
            "expected scheme '{}', got '{}'",
            AUTH_SCHEME, parts[0]
        )));
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|e| Nip98Error::Base64Decode(e.to_string()))?;

    String::from_utf8(decoded)
        .map_err(|e| Nip98Error::Base64Decode(format!("invalid UTF-8: {}", e)))
}

/// Validation parameters for HTTP auth
#[derive(Debug, Clone)]
pub struct ValidationParams {
    /// Expected absolute URL
    pub url: String,

    /// Expected HTTP method
    pub method: HttpMethod,

    /// Optional expected payload hash
    pub payload_hash: Option<String>,

    /// Current timestamp (for window validation)
    pub now: u64,

    /// Timestamp window in seconds (default: 60)
    pub timestamp_window: u64,
}

impl ValidationParams {
    /// Create new validation params
    pub fn new(url: String, method: HttpMethod, now: u64) -> Self {
        Self {
            url,
            method,
            payload_hash: None,
            now,
            timestamp_window: DEFAULT_TIMESTAMP_WINDOW,
        }
    }

    /// Set payload hash
    pub fn with_payload_hash(mut self, hash: String) -> Self {
        self.payload_hash = Some(hash);
        self
    }

    /// Set timestamp window
    pub fn with_timestamp_window(mut self, window: u64) -> Self {
        self.timestamp_window = window;
        self
    }
}

/// Validate HTTP auth event
///
/// Performs the following checks:
/// 1. Event kind must be 27235
/// 2. Timestamp must be within window
/// 3. URL must match exactly
/// 4. Method must match
/// 5. Payload hash must match (if provided)
pub fn validate_http_auth_event(
    kind: u16,
    created_at: u64,
    tags: &[Vec<String>],
    params: &ValidationParams,
) -> Result<(), Nip98Error> {
    // Check kind
    if kind != KIND_HTTP_AUTH {
        return Err(Nip98Error::InvalidKind(kind));
    }

    // Check timestamp window
    let age = if params.now >= created_at {
        params.now - created_at
    } else {
        return Err(Nip98Error::InvalidTimestamp(
            "event timestamp is in the future".to_string(),
        ));
    };

    if age > params.timestamp_window {
        return Err(Nip98Error::TimestampOutOfWindow(age));
    }

    // Parse tags and validate
    let auth = HttpAuth::from_tags(tags)?;

    // Validate URL (must match exactly)
    if auth.url != params.url {
        return Err(Nip98Error::UrlMismatch {
            expected: params.url.clone(),
            actual: auth.url,
        });
    }

    // Validate method
    if auth.method != params.method {
        return Err(Nip98Error::MethodMismatch {
            expected: params.method.as_str().to_string(),
            actual: auth.method.as_str().to_string(),
        });
    }

    // Validate payload hash (if provided)
    if let Some(ref expected_hash) = params.payload_hash {
        match auth.payload_hash {
            Some(ref actual_hash) => {
                if actual_hash != expected_hash {
                    return Err(Nip98Error::PayloadHashMismatch {
                        expected: expected_hash.clone(),
                        actual: actual_hash.clone(),
                    });
                }
            }
            None => {
                return Err(Nip98Error::MissingTag(
                    "payload tag required for this request".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_parse() {
        assert_eq!(HttpMethod::parse("GET").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::parse("get").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::parse("POST").unwrap(), HttpMethod::Post);
        assert_eq!(HttpMethod::parse("PUT").unwrap(), HttpMethod::Put);
        assert_eq!(HttpMethod::parse("PATCH").unwrap(), HttpMethod::Patch);
        assert_eq!(HttpMethod::parse("DELETE").unwrap(), HttpMethod::Delete);
        assert!(HttpMethod::parse("INVALID").is_err());
    }

    #[test]
    fn test_http_method_as_str() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
        assert_eq!(HttpMethod::Put.as_str(), "PUT");
    }

    #[test]
    fn test_http_auth_to_tags() {
        let auth = HttpAuth::new(
            "https://api.example.com/endpoint".to_string(),
            HttpMethod::Get,
        );

        let tags = auth.to_tags();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["u", "https://api.example.com/endpoint"]);
        assert_eq!(tags[1], vec!["method", "GET"]);
    }

    #[test]
    fn test_http_auth_with_payload() {
        let auth = HttpAuth::new(
            "https://api.example.com/endpoint".to_string(),
            HttpMethod::Post,
        )
        .with_payload_hash("abc123".to_string());

        let tags = auth.to_tags();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[2], vec!["payload", "abc123"]);
    }

    #[test]
    fn test_http_auth_from_tags() {
        let tags = vec![
            vec![
                "u".to_string(),
                "https://api.example.com/endpoint".to_string(),
            ],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let auth = HttpAuth::from_tags(&tags).unwrap();
        assert_eq!(auth.url, "https://api.example.com/endpoint");
        assert_eq!(auth.method, HttpMethod::Get);
        assert_eq!(auth.payload_hash, None);
    }

    #[test]
    fn test_http_auth_from_tags_with_payload() {
        let tags = vec![
            vec![
                "u".to_string(),
                "https://api.example.com/endpoint".to_string(),
            ],
            vec!["method".to_string(), "POST".to_string()],
            vec!["payload".to_string(), "abc123".to_string()],
        ];

        let auth = HttpAuth::from_tags(&tags).unwrap();
        assert_eq!(auth.payload_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_http_auth_missing_url() {
        let tags = vec![vec!["method".to_string(), "GET".to_string()]];
        let result = HttpAuth::from_tags(&tags);
        assert!(result.is_err());
    }

    #[test]
    fn test_http_auth_missing_method() {
        let tags = vec![vec!["u".to_string(), "https://example.com".to_string()]];
        let result = HttpAuth::from_tags(&tags);
        assert!(result.is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_hash_payload() {
        let payload = b"test payload";
        let hash = hash_payload(payload);
        // SHA256 hash should be 64 hex characters
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_encode_decode_authorization_header() {
        let event_json = r#"{"id":"test","kind":27235}"#;
        let header = encode_authorization_header(event_json).unwrap();

        assert!(header.starts_with("Nostr "));

        let decoded = decode_authorization_header(&header).unwrap();
        assert_eq!(decoded, event_json);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_decode_invalid_scheme() {
        let header = "Bearer abc123";
        let result = decode_authorization_header(header);
        assert!(result.is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_decode_invalid_format() {
        let header = "Nostr";
        let result = decode_authorization_header(header);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_http_auth_event_success() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        let result = validate_http_auth_event(KIND_HTTP_AUTH, 950, &tags, &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_http_auth_event_invalid_kind() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        let result = validate_http_auth_event(1, 950, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip98Error::InvalidKind(_)));
    }

    #[test]
    fn test_validate_http_auth_event_timestamp_out_of_window() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        // Event is 100 seconds old (beyond default 60 second window)
        let result = validate_http_auth_event(KIND_HTTP_AUTH, 900, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip98Error::TimestampOutOfWindow(_)
        ));
    }

    #[test]
    fn test_validate_http_auth_event_url_mismatch() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/wrong".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        let result = validate_http_auth_event(KIND_HTTP_AUTH, 950, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip98Error::UrlMismatch { .. }
        ));
    }

    #[test]
    fn test_validate_http_auth_event_method_mismatch() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "POST".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        let result = validate_http_auth_event(KIND_HTTP_AUTH, 950, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip98Error::MethodMismatch { .. }
        ));
    }

    #[test]
    fn test_validate_http_auth_event_with_payload() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "POST".to_string()],
            vec!["payload".to_string(), "abc123".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Post,
            1000,
        )
        .with_payload_hash("abc123".to_string());

        let result = validate_http_auth_event(KIND_HTTP_AUTH, 950, &tags, &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_http_auth_event_payload_mismatch() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "POST".to_string()],
            vec!["payload".to_string(), "wrong".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Post,
            1000,
        )
        .with_payload_hash("abc123".to_string());

        let result = validate_http_auth_event(KIND_HTTP_AUTH, 950, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip98Error::PayloadHashMismatch { .. }
        ));
    }

    #[test]
    fn test_validate_http_auth_event_custom_window() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        )
        .with_timestamp_window(120); // 120 seconds

        // Event is 100 seconds old (within 120 second window)
        let result = validate_http_auth_event(KIND_HTTP_AUTH, 900, &tags, &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_http_auth_event_future_timestamp() {
        let tags = vec![
            vec!["u".to_string(), "https://api.example.com/test".to_string()],
            vec!["method".to_string(), "GET".to_string()],
        ];

        let params = ValidationParams::new(
            "https://api.example.com/test".to_string(),
            HttpMethod::Get,
            1000,
        );

        // Event timestamp is in the future
        let result = validate_http_auth_event(KIND_HTTP_AUTH, 1100, &tags, &params);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip98Error::InvalidTimestamp(_)
        ));
    }
}
