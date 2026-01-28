//! NIP-55: Android Signer Application
//!
//! Defines types and utilities for Android signer applications that enable
//! external apps to request event signing via Android intents and content resolvers.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/55.md>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// URI scheme for Android signer intents
pub const NOSTRSIGNER_SCHEME: &str = "nostrsigner";

/// Errors that can occur during NIP-55 operations
#[derive(Debug, Error)]
pub enum Nip55Error {
    #[error("invalid request type: {0}")]
    InvalidRequestType(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid URI: {0}")]
    InvalidUri(String),

    #[error("request rejected by user")]
    Rejected,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Type of signer request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerRequestType {
    GetPublicKey,
    SignEvent,
    Nip04Encrypt,
    Nip04Decrypt,
    Nip44Encrypt,
    Nip44Decrypt,
    DecryptZapEvent,
}

impl SignerRequestType {
    pub fn as_str(&self) -> &str {
        match self {
            SignerRequestType::GetPublicKey => "get_public_key",
            SignerRequestType::SignEvent => "sign_event",
            SignerRequestType::Nip04Encrypt => "nip04_encrypt",
            SignerRequestType::Nip04Decrypt => "nip04_decrypt",
            SignerRequestType::Nip44Encrypt => "nip44_encrypt",
            SignerRequestType::Nip44Decrypt => "nip44_decrypt",
            SignerRequestType::DecryptZapEvent => "decrypt_zap_event",
        }
    }
}

impl std::str::FromStr for SignerRequestType {
    type Err = Nip55Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "get_public_key" => Ok(SignerRequestType::GetPublicKey),
            "sign_event" => Ok(SignerRequestType::SignEvent),
            "nip04_encrypt" => Ok(SignerRequestType::Nip04Encrypt),
            "nip04_decrypt" => Ok(SignerRequestType::Nip04Decrypt),
            "nip44_encrypt" => Ok(SignerRequestType::Nip44Encrypt),
            "nip44_decrypt" => Ok(SignerRequestType::Nip44Decrypt),
            "decrypt_zap_event" => Ok(SignerRequestType::DecryptZapEvent),
            _ => Err(Nip55Error::InvalidRequestType(s.to_string())),
        }
    }
}

/// Permission requested from the signer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    #[serde(rename = "type")]
    pub permission_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u16>,
}

impl Permission {
    pub fn new(permission_type: String) -> Self {
        Self {
            permission_type,
            kind: None,
        }
    }

    pub fn with_kind(mut self, kind: u16) -> Self {
        self.kind = Some(kind);
        self
    }
}

/// Return type for web application callbacks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReturnType {
    Signature,
    Event,
}

impl ReturnType {
    pub fn as_str(&self) -> &str {
        match self {
            ReturnType::Signature => "signature",
            ReturnType::Event => "event",
        }
    }
}

/// Compression type for web application responses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    None,
    Gzip,
}

impl CompressionType {
    pub fn as_str(&self) -> &str {
        match self {
            CompressionType::None => "none",
            CompressionType::Gzip => "gzip",
        }
    }
}

/// A signer request for Android applications
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignerRequest {
    /// Type of request
    pub request_type: SignerRequestType,
    /// Content/data for the request (event JSON, plaintext, encrypted text, etc.)
    pub content: Option<String>,
    /// Package name of the signer application
    pub package: Option<String>,
    /// Request ID for tracking multiple requests
    pub id: Option<String>,
    /// Current logged-in user's pubkey
    pub current_user: Option<String>,
    /// Target pubkey for encryption/decryption
    pub pubkey: Option<String>,
    /// Permissions to request (for get_public_key)
    pub permissions: Option<Vec<Permission>>,
}

impl SignerRequest {
    pub fn new(request_type: SignerRequestType) -> Self {
        Self {
            request_type,
            content: None,
            package: None,
            id: None,
            current_user: None,
            pubkey: None,
            permissions: None,
        }
    }

    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn with_package(mut self, package: String) -> Self {
        self.package = Some(package);
        self
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_current_user(mut self, current_user: String) -> Self {
        self.current_user = Some(current_user);
        self
    }

    pub fn with_pubkey(mut self, pubkey: String) -> Self {
        self.pubkey = Some(pubkey);
        self
    }

    pub fn with_permissions(mut self, permissions: Vec<Permission>) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Build intent URI for Android (nostrsigner:content)
    pub fn to_intent_uri(&self) -> String {
        let content = self.content.as_deref().unwrap_or("");
        format!("{}:{}", NOSTRSIGNER_SCHEME, content)
    }

    /// Build extras map for Android intent
    pub fn to_intent_extras(&self) -> HashMap<String, String> {
        let mut extras = HashMap::new();
        extras.insert("type".to_string(), self.request_type.as_str().to_string());

        if let Some(id) = &self.id {
            extras.insert("id".to_string(), id.clone());
        }
        if let Some(current_user) = &self.current_user {
            extras.insert("current_user".to_string(), current_user.clone());
        }
        if let Some(pubkey) = &self.pubkey {
            extras.insert("pubkey".to_string(), pubkey.clone());
        }
        if let Some(permissions) = &self.permissions
            && let Ok(json) = serde_json::to_string(permissions)
        {
            extras.insert("permissions".to_string(), json);
        }

        extras
    }
}

/// A signer request for web applications
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSignerRequest {
    /// Type of request
    pub request_type: SignerRequestType,
    /// Content/data for the request
    pub content: Option<String>,
    /// Compression type
    pub compression_type: Option<CompressionType>,
    /// Return type
    pub return_type: Option<ReturnType>,
    /// Callback URL
    pub callback_url: Option<String>,
    /// Target pubkey for encryption/decryption
    pub pubkey: Option<String>,
}

impl WebSignerRequest {
    pub fn new(request_type: SignerRequestType) -> Self {
        Self {
            request_type,
            content: None,
            compression_type: None,
            return_type: None,
            callback_url: None,
            pubkey: None,
        }
    }

    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn with_compression_type(mut self, compression_type: CompressionType) -> Self {
        self.compression_type = Some(compression_type);
        self
    }

    pub fn with_return_type(mut self, return_type: ReturnType) -> Self {
        self.return_type = Some(return_type);
        self
    }

    pub fn with_callback_url(mut self, callback_url: String) -> Self {
        self.callback_url = Some(callback_url);
        self
    }

    pub fn with_pubkey(mut self, pubkey: String) -> Self {
        self.pubkey = Some(pubkey);
        self
    }

    /// Build URI for web application
    pub fn to_uri(&self) -> String {
        let content = self.content.as_deref().unwrap_or("");
        let mut uri = format!("{}:{}", NOSTRSIGNER_SCHEME, content);

        let mut params = vec![];

        if let Some(compression_type) = &self.compression_type {
            params.push(format!("compressionType={}", compression_type.as_str()));
        }
        if let Some(return_type) = &self.return_type {
            params.push(format!("returnType={}", return_type.as_str()));
        }
        params.push(format!("type={}", self.request_type.as_str()));
        if let Some(callback_url) = &self.callback_url {
            params.push(format!("callbackUrl={}", callback_url));
        }
        if let Some(pubkey) = &self.pubkey {
            params.push(format!("pubkey={}", pubkey));
        }

        if !params.is_empty() {
            uri.push('?');
            uri.push_str(&params.join("&"));
        }

        uri
    }
}

/// Response from signer application
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerResponse {
    /// Result (signature, encrypted text, decrypted text, etc.)
    pub result: Option<String>,
    /// Package name of the signer
    pub package: Option<String>,
    /// Request ID (if provided in request)
    pub id: Option<String>,
    /// Signed event JSON (for sign_event requests)
    pub event: Option<String>,
    /// Whether the request was rejected
    pub rejected: bool,
}

impl SignerResponse {
    pub fn new() -> Self {
        Self {
            result: None,
            package: None,
            id: None,
            event: None,
            rejected: false,
        }
    }

    pub fn with_result(mut self, result: String) -> Self {
        self.result = Some(result);
        self
    }

    pub fn with_package(mut self, package: String) -> Self {
        self.package = Some(package);
        self
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_event(mut self, event: String) -> Self {
        self.event = Some(event);
        self
    }

    pub fn rejected() -> Self {
        Self {
            result: None,
            package: None,
            id: None,
            event: None,
            rejected: true,
        }
    }
}

impl Default for SignerResponse {
    fn default() -> Self {
        Self::new()
    }
}

/// Content resolver URI for different operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentResolverUri {
    SignEvent,
    Nip04Encrypt,
    Nip04Decrypt,
    Nip44Encrypt,
    Nip44Decrypt,
    DecryptZapEvent,
}

impl ContentResolverUri {
    /// Get the content resolver URI for a signer package
    pub fn to_uri(&self, package: &str) -> String {
        let operation = match self {
            ContentResolverUri::SignEvent => "SIGN_EVENT",
            ContentResolverUri::Nip04Encrypt => "NIP04_ENCRYPT",
            ContentResolverUri::Nip04Decrypt => "NIP04_DECRYPT",
            ContentResolverUri::Nip44Encrypt => "NIP44_ENCRYPT",
            ContentResolverUri::Nip44Decrypt => "NIP44_DECRYPT",
            ContentResolverUri::DecryptZapEvent => "DECRYPT_ZAP_EVENT",
        };
        format!("content://{}.{}", package, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_signer_request_type_as_str() {
        assert_eq!(SignerRequestType::GetPublicKey.as_str(), "get_public_key");
        assert_eq!(SignerRequestType::SignEvent.as_str(), "sign_event");
        assert_eq!(SignerRequestType::Nip04Encrypt.as_str(), "nip04_encrypt");
        assert_eq!(SignerRequestType::Nip04Decrypt.as_str(), "nip04_decrypt");
        assert_eq!(SignerRequestType::Nip44Encrypt.as_str(), "nip44_encrypt");
        assert_eq!(SignerRequestType::Nip44Decrypt.as_str(), "nip44_decrypt");
        assert_eq!(
            SignerRequestType::DecryptZapEvent.as_str(),
            "decrypt_zap_event"
        );
    }

    #[test]
    fn test_signer_request_type_from_str() {
        assert_eq!(
            SignerRequestType::from_str("get_public_key")
                .ok()
                .unwrap_or(SignerRequestType::GetPublicKey),
            SignerRequestType::GetPublicKey
        );
        assert_eq!(
            SignerRequestType::from_str("sign_event")
                .ok()
                .unwrap_or(SignerRequestType::SignEvent),
            SignerRequestType::SignEvent
        );
        assert!(SignerRequestType::from_str("invalid").is_err());
    }

    #[test]
    fn test_permission_basic() {
        let perm = Permission::new("sign_event".to_string());
        assert_eq!(perm.permission_type, "sign_event");
        assert!(perm.kind.is_none());
    }

    #[test]
    fn test_permission_with_kind() {
        let perm = Permission::new("sign_event".to_string()).with_kind(1);
        assert_eq!(perm.permission_type, "sign_event");
        assert_eq!(perm.kind, Some(1));
    }

    #[test]
    fn test_signer_request_to_intent_uri() {
        let request = SignerRequest::new(SignerRequestType::GetPublicKey);
        assert_eq!(request.to_intent_uri(), "nostrsigner:");

        let request = SignerRequest::new(SignerRequestType::SignEvent)
            .with_content("{\"kind\":1}".to_string());
        assert_eq!(request.to_intent_uri(), "nostrsigner:{\"kind\":1}");
    }

    #[test]
    fn test_signer_request_to_intent_extras() {
        let request = SignerRequest::new(SignerRequestType::SignEvent)
            .with_id("123".to_string())
            .with_current_user("pubkey1".to_string());

        let extras = request.to_intent_extras();
        assert_eq!(extras.get("type"), Some(&"sign_event".to_string()));
        assert_eq!(extras.get("id"), Some(&"123".to_string()));
        assert_eq!(extras.get("current_user"), Some(&"pubkey1".to_string()));
    }

    #[test]
    fn test_signer_request_with_permissions() {
        let perms = vec![
            Permission::new("sign_event".to_string()).with_kind(1),
            Permission::new("nip04_decrypt".to_string()),
        ];

        let request = SignerRequest::new(SignerRequestType::GetPublicKey).with_permissions(perms);

        let extras = request.to_intent_extras();
        assert!(extras.contains_key("permissions"));
    }

    #[test]
    fn test_web_signer_request_to_uri_basic() {
        let request = WebSignerRequest::new(SignerRequestType::GetPublicKey)
            .with_compression_type(CompressionType::None)
            .with_return_type(ReturnType::Signature)
            .with_callback_url("https://example.com/?event=".to_string());

        let uri = request.to_uri();
        assert!(uri.starts_with("nostrsigner:"));
        assert!(uri.contains("compressionType=none"));
        assert!(uri.contains("returnType=signature"));
        assert!(uri.contains("type=get_public_key"));
        assert!(uri.contains("callbackUrl=https://example.com/?event="));
    }

    #[test]
    fn test_web_signer_request_sign_event() {
        let event_json = "{\"kind\":1,\"content\":\"test\"}";
        let request = WebSignerRequest::new(SignerRequestType::SignEvent)
            .with_content(event_json.to_string())
            .with_compression_type(CompressionType::None)
            .with_return_type(ReturnType::Signature)
            .with_callback_url("https://example.com/?event=".to_string());

        let uri = request.to_uri();
        assert!(uri.contains(event_json));
        assert!(uri.contains("type=sign_event"));
    }

    #[test]
    fn test_web_signer_request_with_pubkey() {
        let request = WebSignerRequest::new(SignerRequestType::Nip04Encrypt)
            .with_content("plaintext".to_string())
            .with_pubkey("target_pubkey".to_string())
            .with_compression_type(CompressionType::None)
            .with_return_type(ReturnType::Signature);

        let uri = request.to_uri();
        assert!(uri.contains("pubkey=target_pubkey"));
        assert!(uri.contains("type=nip04_encrypt"));
    }

    #[test]
    fn test_signer_response_basic() {
        let response = SignerResponse::new()
            .with_result("signature_here".to_string())
            .with_package("com.example.signer".to_string());

        assert_eq!(response.result, Some("signature_here".to_string()));
        assert_eq!(response.package, Some("com.example.signer".to_string()));
        assert!(!response.rejected);
    }

    #[test]
    fn test_signer_response_rejected() {
        let response = SignerResponse::rejected();
        assert!(response.rejected);
        assert!(response.result.is_none());
    }

    #[test]
    fn test_signer_response_with_event() {
        let event_json = "{\"kind\":1,\"sig\":\"...\"}";
        let response = SignerResponse::new()
            .with_result("signature".to_string())
            .with_event(event_json.to_string())
            .with_id("req123".to_string());

        assert_eq!(response.result, Some("signature".to_string()));
        assert_eq!(response.event, Some(event_json.to_string()));
        assert_eq!(response.id, Some("req123".to_string()));
    }

    #[test]
    fn test_content_resolver_uri() {
        let uri = ContentResolverUri::SignEvent.to_uri("com.example.signer");
        assert_eq!(uri, "content://com.example.signer.SIGN_EVENT");

        let uri = ContentResolverUri::Nip04Encrypt.to_uri("com.example.signer");
        assert_eq!(uri, "content://com.example.signer.NIP04_ENCRYPT");

        let uri = ContentResolverUri::Nip44Decrypt.to_uri("com.example.signer");
        assert_eq!(uri, "content://com.example.signer.NIP44_DECRYPT");
    }

    #[test]
    fn test_return_type_as_str() {
        assert_eq!(ReturnType::Signature.as_str(), "signature");
        assert_eq!(ReturnType::Event.as_str(), "event");
    }

    #[test]
    fn test_compression_type_as_str() {
        assert_eq!(CompressionType::None.as_str(), "none");
        assert_eq!(CompressionType::Gzip.as_str(), "gzip");
    }

    #[test]
    fn test_permission_serialization() {
        let perm = Permission::new("sign_event".to_string()).with_kind(22242);
        let json = serde_json::to_string(&perm).unwrap();
        assert!(json.contains("\"type\":\"sign_event\""));
        assert!(json.contains("\"kind\":22242"));
    }

    #[test]
    fn test_signer_response_serialization() {
        let response = SignerResponse::new()
            .with_result("sig123".to_string())
            .with_package("com.signer".to_string());

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: SignerResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.result, Some("sig123".to_string()));
        assert_eq!(deserialized.package, Some("com.signer".to_string()));
    }
}
