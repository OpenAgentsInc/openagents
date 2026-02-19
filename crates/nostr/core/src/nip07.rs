//! NIP-07: window.nostr capability for web browsers
//!
//! Defines the window.nostr API that browser extensions provide to web applications.
//! This module provides Rust types representing the JavaScript API.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/07.md>

use crate::{Event, UnsignedEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during NIP-07 operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Nip07Error {
    #[error("user rejected the request")]
    UserRejected,

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("encryption error: {0}")]
    EncryptionError(String),

    #[error("decryption error: {0}")]
    DecryptionError(String),

    #[error("provider not available")]
    ProviderNotAvailable,

    #[error("internal error: {0}")]
    Internal(String),
}

/// An unsigned event template for signing via window.nostr.signEvent()
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignEventTemplate {
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

impl SignEventTemplate {
    /// Convert to an UnsignedEvent by adding a pubkey
    pub fn to_unsigned_event(self, pubkey: String) -> UnsignedEvent {
        UnsignedEvent {
            pubkey,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags,
            content: self.content,
        }
    }

    /// Create from an UnsignedEvent (strips the pubkey)
    pub fn from_unsigned_event(event: UnsignedEvent) -> Self {
        Self {
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
        }
    }
}

/// Core window.nostr API trait
///
/// This trait represents the required methods that must be implemented
/// by browser extensions providing NIP-07 support.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait WindowNostr {
    /// Get the user's public key as a hex string
    ///
    /// This may prompt the user for permission.
    async fn get_public_key(&self) -> Result<String, Nip07Error>;

    /// Sign an event
    ///
    /// Takes an unsigned event, adds the `id`, `pubkey`, and `sig` fields,
    /// and returns the complete signed event.
    ///
    /// This may prompt the user for permission.
    async fn sign_event(&self, event: SignEventTemplate) -> Result<Event, Nip07Error>;
}

/// Optional NIP-04 encryption support (deprecated)
///
/// These methods provide NIP-04 encrypted direct message support.
/// Note: NIP-04 is deprecated in favor of NIP-44.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait WindowNostrNip04 {
    /// Encrypt a message for a recipient using NIP-04
    ///
    /// Returns ciphertext and iv as specified in NIP-04.
    async fn encrypt(&self, pubkey: &str, plaintext: &str) -> Result<String, Nip07Error>;

    /// Decrypt a NIP-04 encrypted message
    ///
    /// Takes ciphertext and iv as specified in NIP-04.
    async fn decrypt(&self, pubkey: &str, ciphertext: &str) -> Result<String, Nip07Error>;
}

/// Optional NIP-44 encryption support
///
/// These methods provide NIP-44 versioned encryption support.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait WindowNostrNip44 {
    /// Encrypt a message for a recipient using NIP-44
    ///
    /// Returns ciphertext as specified in NIP-44.
    async fn encrypt(&self, pubkey: &str, plaintext: &str) -> Result<String, Nip07Error>;

    /// Decrypt a NIP-44 encrypted message
    ///
    /// Takes ciphertext as specified in NIP-44.
    async fn decrypt(&self, pubkey: &str, ciphertext: &str) -> Result<String, Nip07Error>;
}

/// Combined window.nostr provider with all optional features
///
/// This trait combines all NIP-07 capabilities including the core API
/// and optional encryption features.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait WindowNostrProvider: WindowNostr {
    /// Access to NIP-04 encryption methods (optional)
    fn nip04(&self) -> Option<&dyn WindowNostrNip04> {
        None
    }

    /// Access to NIP-44 encryption methods (optional)
    fn nip44(&self) -> Option<&dyn WindowNostrNip44> {
        None
    }
}

/// Helper to check if window.nostr is available
///
/// In WASM/browser context with appropriate dependencies, this would check
/// for the window.nostr object. Without wasm_bindgen/web_sys dependencies,
/// always returns false.
///
/// Note: To enable actual browser detection, add wasm_bindgen, web_sys, and
/// js_sys as dependencies and use the `full` feature.
pub fn is_available() -> bool {
    // Without wasm_bindgen/web_sys/js_sys dependencies, we can't check
    // for window.nostr. Always return false.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_event_template_from_unsigned() {
        let unsigned = UnsignedEvent {
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["t".to_string(), "test".to_string()]],
            content: "Hello, world!".to_string(),
        };

        let template = SignEventTemplate::from_unsigned_event(unsigned.clone());

        assert_eq!(template.created_at, unsigned.created_at);
        assert_eq!(template.kind, unsigned.kind);
        assert_eq!(template.tags, unsigned.tags);
        assert_eq!(template.content, unsigned.content);
    }

    #[test]
    fn test_sign_event_template_to_unsigned() {
        let template = SignEventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["t".to_string(), "test".to_string()]],
            content: "Hello, world!".to_string(),
        };

        let unsigned = template
            .clone()
            .to_unsigned_event("test_pubkey".to_string());

        assert_eq!(unsigned.created_at, template.created_at);
        assert_eq!(unsigned.kind, template.kind);
        assert_eq!(unsigned.tags, template.tags);
        assert_eq!(unsigned.content, template.content);
        assert_eq!(unsigned.pubkey, "test_pubkey");
    }

    #[test]
    fn test_nip07_error_display() {
        let err = Nip07Error::UserRejected;
        assert_eq!(err.to_string(), "user rejected the request");

        let err = Nip07Error::NotImplemented("getRelays".to_string());
        assert_eq!(err.to_string(), "not implemented: getRelays");

        let err = Nip07Error::ProviderNotAvailable;
        assert_eq!(err.to_string(), "provider not available");
    }

    #[test]
    fn test_is_available() {
        // Without wasm_bindgen/web_sys/js_sys, should always return false
        assert!(!is_available());
    }

    // Mock implementation for testing
    #[allow(dead_code)]
    struct MockNostrProvider {
        public_key: String,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl WindowNostr for MockNostrProvider {
        async fn get_public_key(&self) -> Result<String, Nip07Error> {
            Ok(self.public_key.clone())
        }

        async fn sign_event(&self, _event: SignEventTemplate) -> Result<Event, Nip07Error> {
            Err(Nip07Error::NotImplemented("signEvent".to_string()))
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl WindowNostrProvider for MockNostrProvider {}

    // Note: Async tests are commented out to avoid requiring tokio in dev-dependencies
    // They can be enabled when testing actual implementations

    // #[tokio::test]
    // async fn test_mock_provider_get_public_key() {
    //     let provider = MockNostrProvider {
    //         public_key: "test_pubkey_hex".to_string(),
    //     };
    //
    //     let pubkey = provider.get_public_key().await.unwrap();
    //     assert_eq!(pubkey, "test_pubkey_hex");
    // }

    // #[tokio::test]
    // async fn test_mock_provider_sign_event_not_implemented() {
    //     let provider = MockNostrProvider {
    //         public_key: "test_pubkey_hex".to_string(),
    //     };
    //
    //     let template = SignEventTemplate {
    //         created_at: 1234567890,
    //         kind: 1,
    //         tags: vec![],
    //         content: "test".to_string(),
    //     };
    //
    //     let result = provider.sign_event(template).await;
    //     assert!(result.is_err());
    //     assert!(matches!(
    //         result.unwrap_err(),
    //         Nip07Error::NotImplemented(_)
    //     ));
    // }

    #[test]
    fn test_sign_event_template_serialization() {
        let template = SignEventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["e".to_string(), "event_id".to_string()]],
            content: "Hello".to_string(),
        };

        let json = serde_json::to_string(&template).unwrap();
        let deserialized: SignEventTemplate = serde_json::from_str(&json).unwrap();

        assert_eq!(template, deserialized);
    }
}
