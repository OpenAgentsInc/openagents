//! NIP-86: Relay Management API
//!
//! Defines a JSON-RPC-like HTTP API for relay management tasks. Relays provide this
//! API on the same URI as the WebSocket endpoint, using HTTP with a special content type.
//! Requests must be authorized using NIP-98 authentication headers.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/86.md>

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Content type for relay management API requests
pub const CONTENT_TYPE: &str = "application/nostr+json+rpc";

/// Errors that can occur during NIP-86 operations
#[derive(Debug, Error)]
pub enum Nip86Error {
    #[error("invalid method: {0}")]
    InvalidMethod(String),

    #[error("invalid params: {0}")]
    InvalidParams(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("request error: {0}")]
    RequestError(String),
}

/// Relay management API methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    #[serde(rename = "supportedmethods")]
    SupportedMethods,
    #[serde(rename = "banpubkey")]
    BanPubkey,
    #[serde(rename = "listbannedpubkeys")]
    ListBannedPubkeys,
    #[serde(rename = "allowpubkey")]
    AllowPubkey,
    #[serde(rename = "listallowedpubkeys")]
    ListAllowedPubkeys,
    #[serde(rename = "listeventsneedingmoderation")]
    ListEventsNeedingModeration,
    #[serde(rename = "allowevent")]
    AllowEvent,
    #[serde(rename = "banevent")]
    BanEvent,
    #[serde(rename = "listbannedevents")]
    ListBannedEvents,
    #[serde(rename = "changerelayname")]
    ChangeRelayName,
    #[serde(rename = "changerelaydescription")]
    ChangeRelayDescription,
    #[serde(rename = "changerelayicon")]
    ChangeRelayIcon,
    #[serde(rename = "allowkind")]
    AllowKind,
    #[serde(rename = "disallowkind")]
    DisallowKind,
    #[serde(rename = "listallowedkinds")]
    ListAllowedKinds,
    #[serde(rename = "blockip")]
    BlockIp,
    #[serde(rename = "unblockip")]
    UnblockIp,
    #[serde(rename = "listblockedips")]
    ListBlockedIps,
}

impl Method {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            Method::SupportedMethods => "supportedmethods",
            Method::BanPubkey => "banpubkey",
            Method::ListBannedPubkeys => "listbannedpubkeys",
            Method::AllowPubkey => "allowpubkey",
            Method::ListAllowedPubkeys => "listallowedpubkeys",
            Method::ListEventsNeedingModeration => "listeventsneedingmoderation",
            Method::AllowEvent => "allowevent",
            Method::BanEvent => "banevent",
            Method::ListBannedEvents => "listbannedevents",
            Method::ChangeRelayName => "changerelayname",
            Method::ChangeRelayDescription => "changerelaydescription",
            Method::ChangeRelayIcon => "changerelayicon",
            Method::AllowKind => "allowkind",
            Method::DisallowKind => "disallowkind",
            Method::ListAllowedKinds => "listallowedkinds",
            Method::BlockIp => "blockip",
            Method::UnblockIp => "unblockip",
            Method::ListBlockedIps => "listblockedips",
        }
    }
}

/// Relay management API request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: Vec<Value>,
}

impl Request {
    /// Create a new request
    pub fn new(method: Method, params: Vec<Value>) -> Self {
        Self {
            method: method.as_str().to_string(),
            params,
        }
    }

    /// Create a request with no parameters
    pub fn new_no_params(method: Method) -> Self {
        Self {
            method: method.as_str().to_string(),
            params: vec![],
        }
    }
}

/// Relay management API response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// Result (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error message (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    /// Create a success response
    pub fn success(result: Value) -> Self {
        Self {
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(message: String) -> Self {
        Self {
            result: None,
            error: Some(message),
        }
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Check if this is a success response
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }
}

/// Pubkey entry (for banned/allowed pubkey lists)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PubkeyEntry {
    /// Public key (hex)
    pub pubkey: String,
    /// Optional reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl PubkeyEntry {
    /// Create a new pubkey entry
    pub fn new(pubkey: String, reason: Option<String>) -> Self {
        Self { pubkey, reason }
    }
}

/// Event entry (for banned events or events needing moderation)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEntry {
    /// Event ID (hex)
    pub id: String,
    /// Optional reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl EventEntry {
    /// Create a new event entry
    pub fn new(id: String, reason: Option<String>) -> Self {
        Self { id, reason }
    }
}

/// IP entry (for blocked IPs)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpEntry {
    /// IP address
    pub ip: String,
    /// Optional reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl IpEntry {
    /// Create a new IP entry
    pub fn new(ip: String, reason: Option<String>) -> Self {
        Self { ip, reason }
    }
}

/// Create a supportedmethods request
pub fn create_supported_methods_request() -> Request {
    Request::new_no_params(Method::SupportedMethods)
}

/// Create a banpubkey request
pub fn create_ban_pubkey_request(pubkey: String, reason: Option<String>) -> Request {
    let mut params = vec![Value::String(pubkey)];
    if let Some(r) = reason {
        params.push(Value::String(r));
    }
    Request::new(Method::BanPubkey, params)
}

/// Create a listbannedpubkeys request
pub fn create_list_banned_pubkeys_request() -> Request {
    Request::new_no_params(Method::ListBannedPubkeys)
}

/// Create an allowpubkey request
pub fn create_allow_pubkey_request(pubkey: String, reason: Option<String>) -> Request {
    let mut params = vec![Value::String(pubkey)];
    if let Some(r) = reason {
        params.push(Value::String(r));
    }
    Request::new(Method::AllowPubkey, params)
}

/// Create a listallowedpubkeys request
pub fn create_list_allowed_pubkeys_request() -> Request {
    Request::new_no_params(Method::ListAllowedPubkeys)
}

/// Create a listeventsneedingmoderation request
pub fn create_list_events_needing_moderation_request() -> Request {
    Request::new_no_params(Method::ListEventsNeedingModeration)
}

/// Create an allowevent request
pub fn create_allow_event_request(event_id: String, reason: Option<String>) -> Request {
    let mut params = vec![Value::String(event_id)];
    if let Some(r) = reason {
        params.push(Value::String(r));
    }
    Request::new(Method::AllowEvent, params)
}

/// Create a banevent request
pub fn create_ban_event_request(event_id: String, reason: Option<String>) -> Request {
    let mut params = vec![Value::String(event_id)];
    if let Some(r) = reason {
        params.push(Value::String(r));
    }
    Request::new(Method::BanEvent, params)
}

/// Create a listbannedevents request
pub fn create_list_banned_events_request() -> Request {
    Request::new_no_params(Method::ListBannedEvents)
}

/// Create a changerelayname request
pub fn create_change_relay_name_request(name: String) -> Request {
    Request::new(Method::ChangeRelayName, vec![Value::String(name)])
}

/// Create a changerelaydescription request
pub fn create_change_relay_description_request(description: String) -> Request {
    Request::new(
        Method::ChangeRelayDescription,
        vec![Value::String(description)],
    )
}

/// Create a changerelayicon request
pub fn create_change_relay_icon_request(icon_url: String) -> Request {
    Request::new(Method::ChangeRelayIcon, vec![Value::String(icon_url)])
}

/// Create an allowkind request
pub fn create_allow_kind_request(kind: u16) -> Request {
    Request::new(Method::AllowKind, vec![Value::Number(kind.into())])
}

/// Create a disallowkind request
pub fn create_disallow_kind_request(kind: u16) -> Request {
    Request::new(Method::DisallowKind, vec![Value::Number(kind.into())])
}

/// Create a listallowedkinds request
pub fn create_list_allowed_kinds_request() -> Request {
    Request::new_no_params(Method::ListAllowedKinds)
}

/// Create a blockip request
pub fn create_block_ip_request(ip: String, reason: Option<String>) -> Request {
    let mut params = vec![Value::String(ip)];
    if let Some(r) = reason {
        params.push(Value::String(r));
    }
    Request::new(Method::BlockIp, params)
}

/// Create an unblockip request
pub fn create_unblock_ip_request(ip: String) -> Request {
    Request::new(Method::UnblockIp, vec![Value::String(ip)])
}

/// Create a listblockedips request
pub fn create_list_blocked_ips_request() -> Request {
    Request::new_no_params(Method::ListBlockedIps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_as_str() {
        assert_eq!(Method::SupportedMethods.as_str(), "supportedmethods");
        assert_eq!(Method::BanPubkey.as_str(), "banpubkey");
        assert_eq!(Method::AllowKind.as_str(), "allowkind");
    }

    #[test]
    fn test_request_new() {
        let req = Request::new(
            Method::BanPubkey,
            vec![Value::String("pubkey123".to_string())],
        );
        assert_eq!(req.method, "banpubkey");
        assert_eq!(req.params.len(), 1);
    }

    #[test]
    fn test_request_new_no_params() {
        let req = Request::new_no_params(Method::SupportedMethods);
        assert_eq!(req.method, "supportedmethods");
        assert_eq!(req.params.len(), 0);
    }

    #[test]
    fn test_request_serialization() {
        let req = Request::new(Method::BanPubkey, vec![Value::String("abc123".to_string())]);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"method\":\"banpubkey\""));
        assert!(json.contains("\"params\":[\"abc123\"]"));
    }

    #[test]
    fn test_response_success() {
        let resp = Response::success(Value::Bool(true));
        assert!(resp.is_success());
        assert!(!resp.is_error());
        assert_eq!(resp.result, Some(Value::Bool(true)));
        assert_eq!(resp.error, None);
    }

    #[test]
    fn test_response_error() {
        let resp = Response::error("something went wrong".to_string());
        assert!(resp.is_error());
        assert!(!resp.is_success());
        assert_eq!(resp.result, None);
        assert_eq!(resp.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_pubkey_entry() {
        let entry = PubkeyEntry::new("abc123".to_string(), Some("spam".to_string()));
        assert_eq!(entry.pubkey, "abc123");
        assert_eq!(entry.reason, Some("spam".to_string()));
    }

    #[test]
    fn test_event_entry() {
        let entry = EventEntry::new("event123".to_string(), None);
        assert_eq!(entry.id, "event123");
        assert_eq!(entry.reason, None);
    }

    #[test]
    fn test_ip_entry() {
        let entry = IpEntry::new("192.168.1.1".to_string(), Some("abuse".to_string()));
        assert_eq!(entry.ip, "192.168.1.1");
        assert_eq!(entry.reason, Some("abuse".to_string()));
    }

    #[test]
    fn test_create_supported_methods_request() {
        let req = create_supported_methods_request();
        assert_eq!(req.method, "supportedmethods");
        assert_eq!(req.params.len(), 0);
    }

    #[test]
    fn test_create_ban_pubkey_request() {
        let req = create_ban_pubkey_request("pubkey123".to_string(), Some("spam".to_string()));
        assert_eq!(req.method, "banpubkey");
        assert_eq!(req.params.len(), 2);
        assert_eq!(req.params[0], Value::String("pubkey123".to_string()));
        assert_eq!(req.params[1], Value::String("spam".to_string()));
    }

    #[test]
    fn test_create_ban_pubkey_request_no_reason() {
        let req = create_ban_pubkey_request("pubkey123".to_string(), None);
        assert_eq!(req.params.len(), 1);
    }

    #[test]
    fn test_create_list_banned_pubkeys_request() {
        let req = create_list_banned_pubkeys_request();
        assert_eq!(req.method, "listbannedpubkeys");
        assert_eq!(req.params.len(), 0);
    }

    #[test]
    fn test_create_allow_pubkey_request() {
        let req = create_allow_pubkey_request("pubkey456".to_string(), None);
        assert_eq!(req.method, "allowpubkey");
        assert_eq!(req.params.len(), 1);
    }

    #[test]
    fn test_create_change_relay_name_request() {
        let req = create_change_relay_name_request("My Relay".to_string());
        assert_eq!(req.method, "changerelayname");
        assert_eq!(req.params.len(), 1);
        assert_eq!(req.params[0], Value::String("My Relay".to_string()));
    }

    #[test]
    fn test_create_allow_kind_request() {
        let req = create_allow_kind_request(1);
        assert_eq!(req.method, "allowkind");
        assert_eq!(req.params.len(), 1);
        assert_eq!(req.params[0], Value::Number(1.into()));
    }

    #[test]
    fn test_create_block_ip_request() {
        let req = create_block_ip_request("10.0.0.1".to_string(), Some("ddos".to_string()));
        assert_eq!(req.method, "blockip");
        assert_eq!(req.params.len(), 2);
    }

    #[test]
    fn test_create_unblock_ip_request() {
        let req = create_unblock_ip_request("10.0.0.1".to_string());
        assert_eq!(req.method, "unblockip");
        assert_eq!(req.params.len(), 1);
    }

    #[test]
    fn test_full_request_response_cycle() {
        // Create request
        let req = create_ban_pubkey_request("spammer123".to_string(), Some("spam".to_string()));
        let req_json = serde_json::to_string(&req).unwrap();

        // Deserialize request
        let parsed_req: Request = serde_json::from_str(&req_json).unwrap();
        assert_eq!(parsed_req.method, "banpubkey");

        // Create success response
        let resp = Response::success(Value::Bool(true));
        let resp_json = serde_json::to_string(&resp).unwrap();

        // Deserialize response
        let parsed_resp: Response = serde_json::from_str(&resp_json).unwrap();
        assert!(parsed_resp.is_success());
        assert_eq!(parsed_resp.result, Some(Value::Bool(true)));
    }

    #[test]
    fn test_error_response_serialization() {
        let resp = Response::error("unauthorized".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\":\"unauthorized\""));
        assert!(!json.contains("result"));
    }

    #[test]
    fn test_list_results() {
        // Test parsing list of pubkeys
        let pubkeys = vec![
            PubkeyEntry::new("key1".to_string(), Some("spam".to_string())),
            PubkeyEntry::new("key2".to_string(), None),
        ];
        let value = serde_json::to_value(&pubkeys).unwrap();
        let resp = Response::success(value);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: Response = serde_json::from_str(&json).unwrap();

        if let Some(Value::Array(arr)) = parsed.result {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected array result");
        }
    }
}
