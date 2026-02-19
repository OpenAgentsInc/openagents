//! NIP-05: Mapping Nostr Keys to DNS-based Internet Identifiers
//!
//! Defines how to map Nostr public keys to human-readable internet identifiers
//! (like email addresses) using DNS and HTTPS.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/05.md>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during NIP-05 operations
#[derive(Debug, Error)]
pub enum Nip05Error {
    #[error("invalid identifier format: {0}")]
    InvalidIdentifier(String),

    #[error("invalid local part: {0}")]
    InvalidLocalPart(String),

    #[error("invalid domain: {0}")]
    InvalidDomain(String),

    #[error("pubkey not found for identifier: {0}")]
    PubkeyNotFound(String),

    #[error("pubkey mismatch: expected {expected}, got {actual}")]
    PubkeyMismatch { expected: String, actual: String },

    #[error("invalid pubkey format: {0}")]
    InvalidPubkey(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("json error: {0}")]
    Json(String),
}

/// NIP-05 identifier (name@domain format)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nip05Identifier {
    /// Local part (name)
    pub local: String,

    /// Domain part
    pub domain: String,
}

impl Nip05Identifier {
    /// Parse a NIP-05 identifier from a string
    pub fn parse(identifier: &str) -> Result<Self, Nip05Error> {
        let parts: Vec<&str> = identifier.split('@').collect();

        if parts.len() != 2 {
            return Err(Nip05Error::InvalidIdentifier(format!(
                "identifier must be in format 'name@domain', got: {}",
                identifier
            )));
        }

        let local = parts[0].to_lowercase();
        let domain = parts[1].to_lowercase();

        // Validate local part: only a-z0-9-_.
        if !local.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        }) {
            return Err(Nip05Error::InvalidLocalPart(format!(
                "local part must only contain a-z0-9-_., got: {}",
                local
            )));
        }

        if local.is_empty() {
            return Err(Nip05Error::InvalidLocalPart(
                "local part cannot be empty".to_string(),
            ));
        }

        if domain.is_empty() {
            return Err(Nip05Error::InvalidDomain(
                "domain cannot be empty".to_string(),
            ));
        }

        Ok(Self { local, domain })
    }

    /// Construct the .well-known URL for this identifier
    pub fn well_known_url(&self) -> String {
        format!(
            "https://{}/.well-known/nostr.json?name={}",
            self.domain, self.local
        )
    }

    /// Check if this is a root identifier (_@domain)
    pub fn is_root(&self) -> bool {
        self.local == "_"
    }
}

impl std::fmt::Display for Nip05Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.local == "_" {
            // Special case: _@domain displays as just domain
            write!(f, "{}", self.domain)
        } else {
            write!(f, "{}@{}", self.local, self.domain)
        }
    }
}

/// Response from .well-known/nostr.json endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nip05Response {
    /// Map of names to hex public keys
    pub names: HashMap<String, String>,

    /// Optional map of public keys to relay URLs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relays: Option<HashMap<String, Vec<String>>>,
}

impl Nip05Response {
    /// Parse a NIP-05 response from JSON
    pub fn from_json(json: &str) -> Result<Self, Nip05Error> {
        serde_json::from_str(json).map_err(|e| Nip05Error::Json(e.to_string()))
    }

    /// Get the public key for a given name
    pub fn get_pubkey(&self, name: &str) -> Option<&String> {
        self.names.get(name)
    }

    /// Get the relay URLs for a given public key
    pub fn get_relays(&self, pubkey: &str) -> Option<&Vec<String>> {
        self.relays.as_ref()?.get(pubkey)
    }

    /// Verify that an identifier maps to the expected public key
    pub fn verify(
        &self,
        identifier: &Nip05Identifier,
        expected_pubkey: &str,
    ) -> Result<(), Nip05Error> {
        let actual_pubkey = self
            .get_pubkey(&identifier.local)
            .ok_or_else(|| Nip05Error::PubkeyNotFound(identifier.to_string()))?;

        // Validate pubkey format (64-char hex)
        if actual_pubkey.len() != 64 || !actual_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Nip05Error::InvalidPubkey(actual_pubkey.clone()));
        }

        if actual_pubkey != expected_pubkey {
            return Err(Nip05Error::PubkeyMismatch {
                expected: expected_pubkey.to_string(),
                actual: actual_pubkey.clone(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identifier() {
        let id = Nip05Identifier::parse("alice@example.com").unwrap();
        assert_eq!(id.local, "alice");
        assert_eq!(id.domain, "example.com");
    }

    #[test]
    fn test_parse_identifier_case_insensitive() {
        let id = Nip05Identifier::parse("Alice@Example.COM").unwrap();
        assert_eq!(id.local, "alice");
        assert_eq!(id.domain, "example.com");
    }

    #[test]
    fn test_parse_identifier_with_special_chars() {
        let id = Nip05Identifier::parse("alice-bob_123.test@example.com").unwrap();
        assert_eq!(id.local, "alice-bob_123.test");
        assert_eq!(id.domain, "example.com");
    }

    #[test]
    fn test_parse_identifier_root() {
        let id = Nip05Identifier::parse("_@example.com").unwrap();
        assert_eq!(id.local, "_");
        assert_eq!(id.domain, "example.com");
        assert!(id.is_root());
    }

    #[test]
    fn test_parse_identifier_invalid_format() {
        assert!(Nip05Identifier::parse("alice").is_err());
        assert!(Nip05Identifier::parse("alice@").is_err());
        assert!(Nip05Identifier::parse("@example.com").is_err());
        assert!(Nip05Identifier::parse("alice@example@com").is_err());
    }

    #[test]
    fn test_parse_identifier_invalid_chars() {
        assert!(Nip05Identifier::parse("alice!@example.com").is_err());
        assert!(Nip05Identifier::parse("alice@example@com").is_err());
        assert!(Nip05Identifier::parse("alice space@example.com").is_err());
    }

    #[test]
    fn test_well_known_url() {
        let id = Nip05Identifier::parse("alice@example.com").unwrap();
        assert_eq!(
            id.well_known_url(),
            "https://example.com/.well-known/nostr.json?name=alice"
        );
    }

    #[test]
    fn test_identifier_to_string() {
        let id = Nip05Identifier::parse("alice@example.com").unwrap();
        assert_eq!(id.to_string(), "alice@example.com");
    }

    #[test]
    fn test_identifier_to_string_root() {
        let id = Nip05Identifier::parse("_@example.com").unwrap();
        assert_eq!(id.to_string(), "example.com");
    }

    #[test]
    fn test_nip05_response_parse() {
        let json = r#"{
            "names": {
                "alice": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9",
                "bob": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        assert_eq!(response.names.len(), 2);
        assert_eq!(
            response.get_pubkey("alice"),
            Some(&"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9".to_string())
        );
    }

    #[test]
    fn test_nip05_response_with_relays() {
        let json = r#"{
            "names": {
                "alice": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
            },
            "relays": {
                "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9": [
                    "wss://relay.example.com",
                    "wss://relay2.example.com"
                ]
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        let relays = response
            .get_relays("b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9")
            .unwrap();
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "wss://relay.example.com");
    }

    #[test]
    fn test_nip05_response_verify_success() {
        let json = r#"{
            "names": {
                "alice": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        let id = Nip05Identifier::parse("alice@example.com").unwrap();

        assert!(
            response
                .verify(
                    &id,
                    "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
                )
                .is_ok()
        );
    }

    #[test]
    fn test_nip05_response_verify_mismatch() {
        let json = r#"{
            "names": {
                "alice": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        let id = Nip05Identifier::parse("alice@example.com").unwrap();

        let result = response.verify(
            &id,
            "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip05Error::PubkeyMismatch { .. }
        ));
    }

    #[test]
    fn test_nip05_response_verify_not_found() {
        let json = r#"{
            "names": {
                "bob": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        let id = Nip05Identifier::parse("alice@example.com").unwrap();

        let result = response.verify(
            &id,
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9",
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip05Error::PubkeyNotFound(_)));
    }

    #[test]
    fn test_nip05_response_verify_invalid_pubkey() {
        let json = r#"{
            "names": {
                "alice": "invalid"
            }
        }"#;

        let response = Nip05Response::from_json(json).unwrap();
        let id = Nip05Identifier::parse("alice@example.com").unwrap();

        let result = response.verify(&id, "invalid");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip05Error::InvalidPubkey(_)));
    }

    #[test]
    fn test_nip05_response_serialization() {
        let mut names = HashMap::new();
        names.insert(
            "alice".to_string(),
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9".to_string(),
        );

        let response = Nip05Response {
            names,
            relays: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("alice"));
        assert!(json.contains("b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"));
    }
}
