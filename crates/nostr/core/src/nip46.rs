//! NIP-46: Nostr Remote Signing
//!
//! This NIP describes a method for 2-way communication between a remote signer
//! and a Nostr client. The remote signer (bunker) holds the user's private keys
//! and signs events on behalf of the client.
//!
//! ## Key Concepts
//! - **client**: User-facing application that sends requests to remote-signer
//! - **remote-signer**: Daemon/server (bunker) that signs events
//! - **client-keypair**: Ephemeral keys used by client to communicate
//! - **remote-signer-keypair**: Keys used by signer to communicate (may be same as user keys)
//! - **user-keypair**: Actual keys representing the user (used to sign events)
//!
//! ## Protocol
//! 1. Client generates ephemeral client-keypair
//! 2. Connection established (via bunker:// or nostrconnect:// URL)
//! 3. Client sends encrypted requests (kind 24133) to remote-signer
//! 4. Remote-signer sends encrypted responses (kind 24133) to client
//! 5. Client calls get_public_key to learn user-pubkey

use crate::nip01::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for Nostr Connect request/response events
pub const KIND_NOSTR_CONNECT: u16 = 24133;

/// Errors that can occur during NIP-46 operations.
#[derive(Debug, Error)]
pub enum Nip46Error {
    #[error("invalid connection URL: {0}")]
    InvalidConnectionUrl(String),

    #[error("missing required parameter: {0}")]
    MissingParameter(String),

    #[error("invalid method: {0}")]
    InvalidMethod(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("decryption error: {0}")]
    Decryption(String),

    #[error("request error: {0}")]
    RequestError(String),
}

/// Nostr Connect method/command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrConnectMethod {
    /// Establish connection
    Connect,
    /// Sign an event
    SignEvent,
    /// Ping for keep-alive
    Ping,
    /// Get the user's public key
    GetPublicKey,
    /// Encrypt with NIP-04
    Nip04Encrypt,
    /// Decrypt with NIP-04
    Nip04Decrypt,
    /// Encrypt with NIP-44
    Nip44Encrypt,
    /// Decrypt with NIP-44
    Nip44Decrypt,
}

impl NostrConnectMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            NostrConnectMethod::Connect => "connect",
            NostrConnectMethod::SignEvent => "sign_event",
            NostrConnectMethod::Ping => "ping",
            NostrConnectMethod::GetPublicKey => "get_public_key",
            NostrConnectMethod::Nip04Encrypt => "nip04_encrypt",
            NostrConnectMethod::Nip04Decrypt => "nip04_decrypt",
            NostrConnectMethod::Nip44Encrypt => "nip44_encrypt",
            NostrConnectMethod::Nip44Decrypt => "nip44_decrypt",
        }
    }
}

impl std::str::FromStr for NostrConnectMethod {
    type Err = Nip46Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "connect" => Ok(NostrConnectMethod::Connect),
            "sign_event" => Ok(NostrConnectMethod::SignEvent),
            "ping" => Ok(NostrConnectMethod::Ping),
            "get_public_key" => Ok(NostrConnectMethod::GetPublicKey),
            "nip04_encrypt" => Ok(NostrConnectMethod::Nip04Encrypt),
            "nip04_decrypt" => Ok(NostrConnectMethod::Nip04Decrypt),
            "nip44_encrypt" => Ok(NostrConnectMethod::Nip44Encrypt),
            "nip44_decrypt" => Ok(NostrConnectMethod::Nip44Decrypt),
            _ => Err(Nip46Error::InvalidMethod(s.to_string())),
        }
    }
}

/// Nostr Connect request (encrypted in event content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrConnectRequest {
    /// Random request ID
    pub id: String,
    /// Method to call
    pub method: String,
    /// Positional parameters
    pub params: Vec<String>,
}

impl NostrConnectRequest {
    /// Create a new request.
    pub fn new(method: NostrConnectMethod, params: Vec<String>) -> Self {
        Self {
            id: generate_request_id(),
            method: method.as_str().to_string(),
            params,
        }
    }

    /// Create a connect request.
    pub fn connect(
        remote_signer_pubkey: impl Into<String>,
        secret: Option<String>,
        permissions: Option<String>,
    ) -> Self {
        let mut params = vec![remote_signer_pubkey.into()];
        if let Some(secret) = secret {
            params.push(secret);
            if let Some(perms) = permissions {
                params.push(perms);
            }
        }
        Self::new(NostrConnectMethod::Connect, params)
    }

    /// Create a sign_event request.
    pub fn sign_event(event_json: impl Into<String>) -> Self {
        Self::new(NostrConnectMethod::SignEvent, vec![event_json.into()])
    }

    /// Create a ping request.
    pub fn ping() -> Self {
        Self::new(NostrConnectMethod::Ping, vec![])
    }

    /// Create a get_public_key request.
    pub fn get_public_key() -> Self {
        Self::new(NostrConnectMethod::GetPublicKey, vec![])
    }

    /// Create a nip04_encrypt request.
    pub fn nip04_encrypt(
        third_party_pubkey: impl Into<String>,
        plaintext: impl Into<String>,
    ) -> Self {
        Self::new(
            NostrConnectMethod::Nip04Encrypt,
            vec![third_party_pubkey.into(), plaintext.into()],
        )
    }

    /// Create a nip04_decrypt request.
    pub fn nip04_decrypt(
        third_party_pubkey: impl Into<String>,
        ciphertext: impl Into<String>,
    ) -> Self {
        Self::new(
            NostrConnectMethod::Nip04Decrypt,
            vec![third_party_pubkey.into(), ciphertext.into()],
        )
    }

    /// Create a nip44_encrypt request.
    pub fn nip44_encrypt(
        third_party_pubkey: impl Into<String>,
        plaintext: impl Into<String>,
    ) -> Self {
        Self::new(
            NostrConnectMethod::Nip44Encrypt,
            vec![third_party_pubkey.into(), plaintext.into()],
        )
    }

    /// Create a nip44_decrypt request.
    pub fn nip44_decrypt(
        third_party_pubkey: impl Into<String>,
        ciphertext: impl Into<String>,
    ) -> Self {
        Self::new(
            NostrConnectMethod::Nip44Decrypt,
            vec![third_party_pubkey.into(), ciphertext.into()],
        )
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, Nip46Error> {
        serde_json::to_string(self).map_err(|e| Nip46Error::Serialization(e.to_string()))
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, Nip46Error> {
        serde_json::from_str(json).map_err(|e| Nip46Error::Deserialization(e.to_string()))
    }
}

/// Nostr Connect response (encrypted in event content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrConnectResponse {
    /// Request ID this is responding to
    pub id: String,
    /// Result string (may be JSON stringified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Error string if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl NostrConnectResponse {
    /// Create a successful response.
    pub fn success(id: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            result: Some(result.into()),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(error.into()),
        }
    }

    /// Create an auth challenge response.
    pub fn auth_challenge(id: impl Into<String>, auth_url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            result: Some("auth_url".to_string()),
            error: Some(auth_url.into()),
        }
    }

    /// Check if this is an auth challenge.
    pub fn is_auth_challenge(&self) -> bool {
        self.result.as_deref() == Some("auth_url") && self.error.is_some()
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, Nip46Error> {
        serde_json::to_string(self).map_err(|e| Nip46Error::Serialization(e.to_string()))
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, Nip46Error> {
        serde_json::from_str(json).map_err(|e| Nip46Error::Deserialization(e.to_string()))
    }
}

/// Bunker URL for direct connection initiated by remote-signer.
///
/// Format: `bunker://<remote-signer-pubkey>?relay=<wss://relay>&secret=<optional-secret>`
#[derive(Debug, Clone)]
pub struct BunkerUrl {
    /// Remote signer's public key
    pub remote_signer_pubkey: String,
    /// Relay URLs to connect on
    pub relays: Vec<String>,
    /// Optional secret for single-use connection
    pub secret: Option<String>,
}

impl BunkerUrl {
    /// Parse a bunker:// URL.
    pub fn parse(url: &str) -> Result<Self, Nip46Error> {
        if !url.starts_with("bunker://") {
            return Err(Nip46Error::InvalidConnectionUrl(
                "URL must start with bunker://".to_string(),
            ));
        }

        let url = &url[9..]; // Remove "bunker://"

        // Split on '?' to separate pubkey from query params
        let parts: Vec<&str> = url.splitn(2, '?').collect();
        let remote_signer_pubkey = parts[0].to_string();

        if remote_signer_pubkey.is_empty() {
            return Err(Nip46Error::InvalidConnectionUrl(
                "Missing remote signer pubkey".to_string(),
            ));
        }

        let mut relays = Vec::new();
        let mut secret = None;

        if parts.len() > 1 {
            for param in parts[1].split('&') {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() == 2 {
                    match kv[0] {
                        "relay" => relays.push(
                            urlencoding::decode(kv[1])
                                .map_err(|e| Nip46Error::InvalidConnectionUrl(e.to_string()))?
                                .to_string(),
                        ),
                        "secret" => secret = Some(kv[1].to_string()),
                        _ => {}
                    }
                }
            }
        }

        if relays.is_empty() {
            return Err(Nip46Error::InvalidConnectionUrl(
                "At least one relay is required".to_string(),
            ));
        }

        Ok(Self {
            remote_signer_pubkey,
            relays,
            secret,
        })
    }
}

impl std::fmt::Display for BunkerUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut url = format!("bunker://{}", self.remote_signer_pubkey);

        let mut params = Vec::new();
        for relay in &self.relays {
            params.push(format!("relay={}", urlencoding::encode(relay)));
        }
        if let Some(secret) = &self.secret {
            params.push(format!("secret={}", secret));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        write!(f, "{}", url)
    }
}

/// Nostr Connect URL for direct connection initiated by client.
///
/// Format: `nostrconnect://<client-pubkey>?relay=<wss://relay>&secret=<secret>&...`
#[derive(Debug, Clone)]
pub struct NostrConnectUrl {
    /// Client's public key
    pub client_pubkey: String,
    /// Relay URLs where client is listening
    pub relays: Vec<String>,
    /// Secret that remote-signer should return
    pub secret: String,
    /// Optional requested permissions
    pub perms: Option<String>,
    /// Optional client name
    pub name: Option<String>,
    /// Optional client URL
    pub url: Option<String>,
    /// Optional client image
    pub image: Option<String>,
}

impl NostrConnectUrl {
    /// Create a new NostrConnect URL.
    pub fn new(
        client_pubkey: impl Into<String>,
        relays: Vec<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self {
            client_pubkey: client_pubkey.into(),
            relays,
            secret: secret.into(),
            perms: None,
            name: None,
            url: None,
            image: None,
        }
    }

    /// Set requested permissions.
    pub fn with_perms(mut self, perms: impl Into<String>) -> Self {
        self.perms = Some(perms.into());
        self
    }

    /// Set client name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set client URL.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set client image.
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.image = Some(image.into());
        self
    }

    /// Parse a nostrconnect:// URL.
    pub fn parse(url: &str) -> Result<Self, Nip46Error> {
        if !url.starts_with("nostrconnect://") {
            return Err(Nip46Error::InvalidConnectionUrl(
                "URL must start with nostrconnect://".to_string(),
            ));
        }

        let url = &url[15..]; // Remove "nostrconnect://"

        // Split on '?' to separate pubkey from query params
        let parts: Vec<&str> = url.splitn(2, '?').collect();
        let client_pubkey = parts[0].to_string();

        if client_pubkey.is_empty() {
            return Err(Nip46Error::InvalidConnectionUrl(
                "Missing client pubkey".to_string(),
            ));
        }

        let mut relays = Vec::new();
        let mut secret = None;
        let mut perms = None;
        let mut name = None;
        let mut url_param = None;
        let mut image = None;

        if parts.len() > 1 {
            for param in parts[1].split('&') {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() == 2 {
                    let key = kv[0];
                    let value = urlencoding::decode(kv[1])
                        .map_err(|e| Nip46Error::InvalidConnectionUrl(e.to_string()))?
                        .to_string();

                    match key {
                        "relay" => relays.push(value),
                        "secret" => secret = Some(value),
                        "perms" => perms = Some(value),
                        "name" => name = Some(value),
                        "url" => url_param = Some(value),
                        "image" => image = Some(value),
                        _ => {}
                    }
                }
            }
        }

        let secret = secret.ok_or_else(|| {
            Nip46Error::InvalidConnectionUrl("Missing required secret parameter".to_string())
        })?;

        if relays.is_empty() {
            return Err(Nip46Error::InvalidConnectionUrl(
                "At least one relay is required".to_string(),
            ));
        }

        Ok(Self {
            client_pubkey,
            relays,
            secret,
            perms,
            name,
            url: url_param,
            image,
        })
    }
}

impl std::fmt::Display for NostrConnectUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut url = format!("nostrconnect://{}", self.client_pubkey);

        let mut params = Vec::new();

        for relay in &self.relays {
            params.push(format!("relay={}", urlencoding::encode(relay)));
        }

        params.push(format!("secret={}", urlencoding::encode(&self.secret)));

        if let Some(perms) = &self.perms {
            params.push(format!("perms={}", urlencoding::encode(perms)));
        }
        if let Some(name) = &self.name {
            params.push(format!("name={}", urlencoding::encode(name)));
        }
        if let Some(url_str) = &self.url {
            params.push(format!("url={}", urlencoding::encode(url_str)));
        }
        if let Some(image) = &self.image {
            params.push(format!("image={}", urlencoding::encode(image)));
        }

        url.push('?');
        url.push_str(&params.join("&"));
        write!(f, "{}", url)
    }
}

/// Generate a random request ID.
pub fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let random_bytes: [u8; 16] = rng.random();
    hex::encode(random_bytes)
}

/// Check if an event is a Nostr Connect request/response.
pub fn is_nostr_connect_event(event: &Event) -> bool {
    event.kind == KIND_NOSTR_CONNECT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_nostr_connect_method_conversion() {
        assert_eq!(NostrConnectMethod::Connect.as_str(), "connect");
        assert_eq!(NostrConnectMethod::SignEvent.as_str(), "sign_event");
        assert_eq!(NostrConnectMethod::Ping.as_str(), "ping");
        assert_eq!(NostrConnectMethod::GetPublicKey.as_str(), "get_public_key");

        assert!(matches!(
            NostrConnectMethod::from_str("connect"),
            Ok(NostrConnectMethod::Connect)
        ));
        assert!(matches!(
            NostrConnectMethod::from_str("sign_event"),
            Ok(NostrConnectMethod::SignEvent)
        ));
        assert!(NostrConnectMethod::from_str("invalid").is_err());
    }

    #[test]
    fn test_nostr_connect_request() {
        let req = NostrConnectRequest::ping();
        assert_eq!(req.method, "ping");
        assert!(req.params.is_empty());
        assert!(!req.id.is_empty());

        let event_json = r#"{"kind":1,"content":"test"}"#;
        let req = NostrConnectRequest::sign_event(event_json);
        assert_eq!(req.method, "sign_event");
        assert_eq!(req.params.len(), 1);
        assert_eq!(req.params[0], event_json);
    }

    #[test]
    fn test_nostr_connect_request_json() {
        let req = NostrConnectRequest::get_public_key();
        let json = req.to_json().unwrap();
        let recovered = NostrConnectRequest::from_json(&json).unwrap();

        assert_eq!(req.id, recovered.id);
        assert_eq!(req.method, recovered.method);
        assert_eq!(req.params, recovered.params);
    }

    #[test]
    fn test_nostr_connect_response() {
        let resp = NostrConnectResponse::success("req123", "result_data");
        assert_eq!(resp.id, "req123");
        assert_eq!(resp.result, Some("result_data".to_string()));
        assert!(resp.error.is_none());

        let resp = NostrConnectResponse::error("req456", "something went wrong");
        assert_eq!(resp.id, "req456");
        assert!(resp.result.is_none());
        assert_eq!(resp.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_auth_challenge() {
        let resp = NostrConnectResponse::auth_challenge("req789", "https://auth.example.com");
        assert!(resp.is_auth_challenge());
        assert_eq!(resp.result, Some("auth_url".to_string()));
        assert_eq!(resp.error, Some("https://auth.example.com".to_string()));
    }

    #[test]
    fn test_bunker_url_parse() {
        let url = "bunker://pubkey123?relay=wss%3A%2F%2Frelay.example.com&secret=secret123";
        let bunker = BunkerUrl::parse(url).unwrap();

        assert_eq!(bunker.remote_signer_pubkey, "pubkey123");
        assert_eq!(bunker.relays, vec!["wss://relay.example.com"]);
        assert_eq!(bunker.secret, Some("secret123".to_string()));
    }

    #[test]
    fn test_bunker_url_to_string() {
        let bunker = BunkerUrl {
            remote_signer_pubkey: "pubkey123".to_string(),
            relays: vec!["wss://relay.example.com".to_string()],
            secret: Some("secret123".to_string()),
        };

        let url = bunker.to_string();
        assert!(url.starts_with("bunker://pubkey123?"));
        assert!(url.contains("relay=wss%3A%2F%2Frelay.example.com"));
        assert!(url.contains("secret=secret123"));
    }

    #[test]
    fn test_bunker_url_multiple_relays() {
        let url = "bunker://pubkey?relay=wss%3A%2F%2Frelay1.com&relay=wss%3A%2F%2Frelay2.com";
        let bunker = BunkerUrl::parse(url).unwrap();

        assert_eq!(bunker.relays.len(), 2);
        assert_eq!(bunker.relays[0], "wss://relay1.com");
        assert_eq!(bunker.relays[1], "wss://relay2.com");
    }

    #[test]
    fn test_nostrconnect_url_parse() {
        let url = "nostrconnect://client_pubkey?relay=wss%3A%2F%2Frelay.com&secret=abc123&name=My%20Client";
        let nc = NostrConnectUrl::parse(url).unwrap();

        assert_eq!(nc.client_pubkey, "client_pubkey");
        assert_eq!(nc.relays, vec!["wss://relay.com"]);
        assert_eq!(nc.secret, "abc123");
        assert_eq!(nc.name, Some("My Client".to_string()));
    }

    #[test]
    fn test_nostrconnect_url_to_string() {
        let nc = NostrConnectUrl::new(
            "client_pk",
            vec!["wss://relay.com".to_string()],
            "secret123",
        )
        .with_name("Test Client")
        .with_perms("sign_event:1,nip44_encrypt");

        let url = nc.to_string();
        assert!(url.starts_with("nostrconnect://client_pk?"));
        assert!(url.contains("relay=wss%3A%2F%2Frelay.com"));
        assert!(url.contains("secret=secret123"));
        assert!(url.contains("name=Test%20Client"));
        assert!(url.contains("perms=sign_event%3A1%2Cnip44_encrypt"));
    }

    #[test]
    fn test_nostrconnect_url_missing_secret() {
        let url = "nostrconnect://client_pk?relay=wss%3A%2F%2Frelay.com";
        assert!(NostrConnectUrl::parse(url).is_err());
    }

    #[test]
    fn test_nostrconnect_url_missing_relay() {
        let url = "nostrconnect://client_pk?secret=abc123";
        assert!(NostrConnectUrl::parse(url).is_err());
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        assert_eq!(id1.len(), 32); // 16 bytes = 32 hex chars
        assert_eq!(id2.len(), 32);
        assert_ne!(id1, id2); // Should be different
    }
}
