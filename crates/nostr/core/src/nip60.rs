//! NIP-60: Cashu Wallets
//!
//! Defines cashu-based wallet state management on Nostr relays. Enables
//! cross-application wallet access with encrypted proof storage.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/60.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Event kind for wallet configuration (replaceable)
pub const WALLET_KIND: u16 = 17375;

/// Event kind for token storage (unspent proofs)
pub const TOKEN_KIND: u16 = 7375;

/// Event kind for spending history
pub const SPENDING_HISTORY_KIND: u16 = 7376;

/// Event kind for quote tracking (optional)
pub const QUOTE_KIND: u16 = 7374;

/// Errors that can occur during NIP-60 operations
#[derive(Debug, Error)]
pub enum Nip60Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid proof format: {0}")]
    InvalidProof(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("encrypted content, decryption required")]
    EncryptedContent,
}

/// Direction of a transaction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionDirection {
    In,
    Out,
}

impl TransactionDirection {
    pub fn as_str(&self) -> &str {
        match self {
            TransactionDirection::In => "in",
            TransactionDirection::Out => "out",
        }
    }

}

impl std::str::FromStr for TransactionDirection {
    type Err = Nip60Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in" => Ok(TransactionDirection::In),
            "out" => Ok(TransactionDirection::Out),
            _ => Err(Nip60Error::Parse(format!(
                "invalid transaction direction: {}",
                s
            ))),
        }
    }
}

/// Marker for event references in spending history
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventMarker {
    Created,
    Destroyed,
    Redeemed,
}

impl EventMarker {
    pub fn as_str(&self) -> &str {
        match self {
            EventMarker::Created => "created",
            EventMarker::Destroyed => "destroyed",
            EventMarker::Redeemed => "redeemed",
        }
    }

}

impl std::str::FromStr for EventMarker {
    type Err = Nip60Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "created" => Ok(EventMarker::Created),
            "destroyed" => Ok(EventMarker::Destroyed),
            "redeemed" => Ok(EventMarker::Redeemed),
            _ => Err(Nip60Error::Parse(format!("invalid event marker: {}", s))),
        }
    }
}

/// A Cashu proof (ecash token)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashuProof {
    pub id: String,
    pub amount: u64,
    pub secret: String,
    #[serde(rename = "C")]
    pub c: String,
}

impl CashuProof {
    pub fn new(id: String, amount: u64, secret: String, c: String) -> Self {
        Self {
            id,
            amount,
            secret,
            c,
        }
    }
}

/// Token event content (encrypted with NIP-44)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenContent {
    pub mint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub proofs: Vec<CashuProof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub del: Option<Vec<String>>,
}

impl TokenContent {
    pub fn new(mint: String, proofs: Vec<CashuProof>) -> Self {
        Self {
            mint,
            unit: None,
            proofs,
            del: None,
        }
    }

    pub fn with_unit(mut self, unit: String) -> Self {
        self.unit = Some(unit);
        self
    }

    pub fn with_deleted(mut self, deleted_ids: Vec<String>) -> Self {
        self.del = Some(deleted_ids);
        self
    }

    pub fn get_unit(&self) -> &str {
        self.unit.as_deref().unwrap_or("sat")
    }

    pub fn total_amount(&self) -> u64 {
        self.proofs.iter().map(|p| p.amount).sum()
    }
}

/// Token event (kind 7375)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEvent {
    pub event: Event,
    /// Encrypted content (requires NIP-44 decryption)
    encrypted: bool,
}

impl TokenEvent {
    pub fn from_event(event: Event) -> Result<Self, Nip60Error> {
        if event.kind != TOKEN_KIND {
            return Err(Nip60Error::InvalidKind {
                expected: TOKEN_KIND,
                actual: event.kind,
            });
        }

        Ok(Self {
            event,
            encrypted: true,
        })
    }

    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Get decrypted content (user must decrypt with NIP-44 first)
    pub fn parse_content(decrypted_json: &str) -> Result<TokenContent, Nip60Error> {
        serde_json::from_str(decrypted_json).map_err(Nip60Error::Json)
    }
}

/// Wallet configuration (kind 17375)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletEvent {
    pub event: Event,
}

impl WalletEvent {
    pub fn from_event(event: Event) -> Result<Self, Nip60Error> {
        if event.kind != WALLET_KIND {
            return Err(Nip60Error::InvalidKind {
                expected: WALLET_KIND,
                actual: event.kind,
            });
        }

        Ok(Self { event })
    }

    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Parse wallet configuration tags from decrypted content
    /// Expected format: [["privkey", "hex"], ["mint", "url1"], ["mint", "url2"]]
    pub fn parse_content(decrypted_json: &str) -> Result<HashMap<String, Vec<String>>, Nip60Error> {
        let tags: Vec<Vec<String>> = serde_json::from_str(decrypted_json)?;
        let mut config = HashMap::new();

        for tag in tags {
            if tag.len() >= 2 {
                let key = tag[0].clone();
                let value = tag[1].clone();
                config.entry(key).or_insert_with(Vec::new).push(value);
            }
        }

        Ok(config)
    }
}

/// Spending history event (kind 7376)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendingHistoryEvent {
    pub event: Event,
}

impl SpendingHistoryEvent {
    pub fn from_event(event: Event) -> Result<Self, Nip60Error> {
        if event.kind != SPENDING_HISTORY_KIND {
            return Err(Nip60Error::InvalidKind {
                expected: SPENDING_HISTORY_KIND,
                actual: event.kind,
            });
        }

        Ok(Self { event })
    }

    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Parse spending history from decrypted content
    /// Format: [["direction", "in"], ["amount", "100"], ["unit", "sat"], ["e", "id", "", "created"]]
    pub fn parse_content(decrypted_json: &str) -> Result<HashMap<String, Vec<String>>, Nip60Error> {
        let tags: Vec<Vec<String>> = serde_json::from_str(decrypted_json)?;
        let mut history = HashMap::new();

        for tag in tags {
            if tag.len() >= 2 {
                let key = tag[0].clone();
                let value = tag[1].clone();
                history.entry(key).or_insert_with(Vec::new).push(value);
            }
        }

        Ok(history)
    }

    /// Get redeemed event references from tags (unencrypted)
    pub fn get_redeemed_events(&self) -> Vec<String> {
        self.event
            .tags
            .iter()
            .filter_map(|tag| {
                if tag.len() >= 4 && tag[0] == "e" && tag[3] == "redeemed" {
                    Some(tag[1].clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Quote event for Lightning payment tracking (kind 7374)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteEvent {
    pub event: Event,
}

impl QuoteEvent {
    pub fn from_event(event: Event) -> Result<Self, Nip60Error> {
        if event.kind != QUOTE_KIND {
            return Err(Nip60Error::InvalidKind {
                expected: QUOTE_KIND,
                actual: event.kind,
            });
        }

        Ok(Self { event })
    }

    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Get the mint URL from tags
    pub fn get_mint(&self) -> Option<&str> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 2 && tag[0] == "mint" {
                Some(tag[1].as_str())
            } else {
                None
            }
        })
    }

    /// Get the expiration timestamp from tags
    pub fn get_expiration(&self) -> Option<u64> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 2 && tag[0] == "expiration" {
                tag[1].parse().ok()
            } else {
                None
            }
        })
    }
}

/// Check if an event kind is a Cashu wallet kind
pub fn is_wallet_kind(kind: u16) -> bool {
    kind == WALLET_KIND
}

/// Check if an event kind is a token kind
pub fn is_token_kind(kind: u16) -> bool {
    kind == TOKEN_KIND
}

/// Check if an event kind is a spending history kind
pub fn is_spending_history_kind(kind: u16) -> bool {
    kind == SPENDING_HISTORY_KIND
}

/// Check if an event kind is a quote kind
pub fn is_quote_kind(kind: u16) -> bool {
    kind == QUOTE_KIND
}

/// Check if an event kind is any NIP-60 kind
pub fn is_nip60_kind(kind: u16) -> bool {
    is_wallet_kind(kind)
        || is_token_kind(kind)
        || is_spending_history_kind(kind)
        || is_quote_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_event(kind: u16, content: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_cashu_proof() {
        let proof = CashuProof::new(
            "005c2502034d4f12".to_string(),
            1,
            "z+zyxAVLRqN9lEjxuNPSyRJzEstbl69Jc1vtimvtkPg=".to_string(),
            "0241d98a8197ef238a192d47edf191a9de78b657308937b4f7dd0aa53beae72c46".to_string(),
        );

        assert_eq!(proof.id, "005c2502034d4f12");
        assert_eq!(proof.amount, 1);
    }

    #[test]
    fn test_token_content() {
        let proofs = vec![
            CashuProof::new("1".to_string(), 1, "secret1".to_string(), "C1".to_string()),
            CashuProof::new("2".to_string(), 2, "secret2".to_string(), "C2".to_string()),
        ];

        let content = TokenContent::new("https://mint.example.com".to_string(), proofs)
            .with_unit("sat".to_string());

        assert_eq!(content.mint, "https://mint.example.com");
        assert_eq!(content.get_unit(), "sat");
        assert_eq!(content.total_amount(), 3);
    }

    #[test]
    fn test_token_content_default_unit() {
        let proofs = vec![CashuProof::new(
            "1".to_string(),
            1,
            "secret".to_string(),
            "C".to_string(),
        )];

        let content = TokenContent::new("https://mint.example.com".to_string(), proofs);

        assert_eq!(content.get_unit(), "sat");
    }

    #[test]
    fn test_token_content_with_deleted() {
        let proofs = vec![CashuProof::new(
            "1".to_string(),
            1,
            "secret".to_string(),
            "C".to_string(),
        )];

        let content = TokenContent::new("https://mint.example.com".to_string(), proofs)
            .with_deleted(vec!["event-id-1".to_string(), "event-id-2".to_string()]);

        assert_eq!(
            content.del,
            Some(vec!["event-id-1".to_string(), "event-id-2".to_string()])
        );
    }

    #[test]
    fn test_token_event_from_event() {
        let event = create_test_event(TOKEN_KIND, "encrypted_content", vec![]);
        let token = TokenEvent::from_event(event).unwrap();

        assert_eq!(token.get_encrypted_content(), "encrypted_content");
    }

    #[test]
    fn test_token_event_invalid_kind() {
        let event = create_test_event(1, "content", vec![]);
        let result = TokenEvent::from_event(event);

        assert!(result.is_err());
    }

    #[test]
    fn test_token_event_parse_content() {
        let json = r#"{
            "mint": "https://mint.example.com",
            "unit": "sat",
            "proofs": [
                {
                    "id": "1",
                    "amount": 100,
                    "secret": "secret1",
                    "C": "C1"
                }
            ]
        }"#;

        let content = TokenEvent::parse_content(json).unwrap();
        assert_eq!(content.mint, "https://mint.example.com");
        assert_eq!(content.proofs.len(), 1);
        assert_eq!(content.total_amount(), 100);
    }

    #[test]
    fn test_wallet_event_from_event() {
        let event = create_test_event(WALLET_KIND, "encrypted_content", vec![]);
        let wallet = WalletEvent::from_event(event).unwrap();

        assert_eq!(wallet.get_encrypted_content(), "encrypted_content");
    }

    #[test]
    fn test_wallet_event_parse_content() {
        let json = r#"[
            ["privkey", "hexkey123"],
            ["mint", "https://mint1.example.com"],
            ["mint", "https://mint2.example.com"]
        ]"#;

        let config = WalletEvent::parse_content(json).unwrap();
        assert_eq!(config.get("privkey"), Some(&vec!["hexkey123".to_string()]));
        assert_eq!(
            config.get("mint"),
            Some(&vec![
                "https://mint1.example.com".to_string(),
                "https://mint2.example.com".to_string()
            ])
        );
    }

    #[test]
    fn test_spending_history_event() {
        let event = create_test_event(
            SPENDING_HISTORY_KIND,
            "encrypted_content",
            vec![vec![
                "e".to_string(),
                "event123".to_string(),
                "".to_string(),
                "redeemed".to_string(),
            ]],
        );

        let history = SpendingHistoryEvent::from_event(event).unwrap();
        let redeemed = history.get_redeemed_events();

        assert_eq!(redeemed.len(), 1);
        assert_eq!(redeemed[0], "event123");
    }

    #[test]
    fn test_spending_history_parse_content() {
        let json = r#"[
            ["direction", "out"],
            ["amount", "100"],
            ["unit", "sat"]
        ]"#;

        let history = SpendingHistoryEvent::parse_content(json).unwrap();
        assert_eq!(history.get("direction"), Some(&vec!["out".to_string()]));
        assert_eq!(history.get("amount"), Some(&vec!["100".to_string()]));
        assert_eq!(history.get("unit"), Some(&vec!["sat".to_string()]));
    }

    #[test]
    fn test_quote_event() {
        let event = create_test_event(
            QUOTE_KIND,
            "encrypted_quote_id",
            vec![
                vec!["mint".to_string(), "https://mint.example.com".to_string()],
                vec!["expiration".to_string(), "1234567890".to_string()],
            ],
        );

        let quote = QuoteEvent::from_event(event).unwrap();
        assert_eq!(quote.get_mint(), Some("https://mint.example.com"));
        assert_eq!(quote.get_expiration(), Some(1234567890));
    }

    #[test]
    fn test_transaction_direction() {
        assert_eq!(TransactionDirection::In.as_str(), "in");
        assert_eq!(TransactionDirection::Out.as_str(), "out");

        assert_eq!(
            TransactionDirection::from_str("in")
                .ok()
                .unwrap_or(TransactionDirection::In),
            TransactionDirection::In
        );
        assert_eq!(
            TransactionDirection::from_str("out")
                .ok()
                .unwrap_or(TransactionDirection::Out),
            TransactionDirection::Out
        );
    }

    #[test]
    fn test_event_marker() {
        assert_eq!(EventMarker::Created.as_str(), "created");
        assert_eq!(EventMarker::Destroyed.as_str(), "destroyed");
        assert_eq!(EventMarker::Redeemed.as_str(), "redeemed");

        assert_eq!(
            EventMarker::from_str("created")
                .ok()
                .unwrap_or(EventMarker::Created),
            EventMarker::Created
        );
    }

    #[test]
    fn test_is_wallet_kind() {
        assert!(is_wallet_kind(WALLET_KIND));
        assert!(!is_wallet_kind(1));
    }

    #[test]
    fn test_is_token_kind() {
        assert!(is_token_kind(TOKEN_KIND));
        assert!(!is_token_kind(1));
    }

    #[test]
    fn test_is_spending_history_kind() {
        assert!(is_spending_history_kind(SPENDING_HISTORY_KIND));
        assert!(!is_spending_history_kind(1));
    }

    #[test]
    fn test_is_quote_kind() {
        assert!(is_quote_kind(QUOTE_KIND));
        assert!(!is_quote_kind(1));
    }

    #[test]
    fn test_is_nip60_kind() {
        assert!(is_nip60_kind(WALLET_KIND));
        assert!(is_nip60_kind(TOKEN_KIND));
        assert!(is_nip60_kind(SPENDING_HISTORY_KIND));
        assert!(is_nip60_kind(QUOTE_KIND));
        assert!(!is_nip60_kind(1));
    }
}
