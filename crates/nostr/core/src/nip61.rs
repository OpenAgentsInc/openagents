//! NIP-61: Nutzaps
//!
//! Defines Cashu-based zaps using P2PK-locked tokens. Enables private,
//! anonymous zapping with ecash.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/61.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for nutzap informational event
pub const NUTZAP_INFO_KIND: u16 = 10019;

/// Event kind for nutzap (ecash transfer)
pub const NUTZAP_KIND: u16 = 9321;

/// Errors that can occur during NIP-61 operations
#[derive(Debug, Error)]
pub enum Nip61Error {
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
}

/// Mint information with supported units
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintInfo {
    pub url: String,
    pub units: Vec<String>,
}

impl MintInfo {
    pub fn new(url: String) -> Self {
        Self {
            url,
            units: vec!["sat".to_string()],
        }
    }

    pub fn with_units(mut self, units: Vec<String>) -> Self {
        self.units = units;
        self
    }

    pub fn supports_unit(&self, unit: &str) -> bool {
        self.units.iter().any(|u| u == unit)
    }
}

/// Nutzap informational event (kind 10019)
/// Indicates how a user wants to receive nutzaps
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NutzapInfo {
    pub event: Event,
    pub relays: Vec<String>,
    pub mints: Vec<MintInfo>,
    pub pubkey: String,
}

impl NutzapInfo {
    pub fn from_event(event: Event) -> Result<Self, Nip61Error> {
        if event.kind != NUTZAP_INFO_KIND {
            return Err(Nip61Error::InvalidKind {
                expected: NUTZAP_INFO_KIND,
                actual: event.kind,
            });
        }

        let mut relays = Vec::new();
        let mut mints = Vec::new();
        let mut pubkey = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "relay" if tag.len() >= 2 => {
                    relays.push(tag[1].clone());
                }
                "mint" if tag.len() >= 2 => {
                    let url = tag[1].clone();
                    let units = if tag.len() > 2 {
                        tag[2..].to_vec()
                    } else {
                        vec!["sat".to_string()]
                    };
                    mints.push(MintInfo { url, units });
                }
                "pubkey" if tag.len() >= 2 => {
                    pubkey = Some(tag[1].clone());
                }
                _ => {}
            }
        }

        let pubkey = pubkey.ok_or_else(|| Nip61Error::MissingField("pubkey".to_string()))?;

        Ok(Self {
            event,
            relays,
            mints,
            pubkey,
        })
    }

    /// Check if a mint URL is trusted by this user
    pub fn trusts_mint(&self, mint_url: &str) -> bool {
        self.mints.iter().any(|m| m.url == mint_url)
    }

    /// Get mint info for a specific URL
    pub fn get_mint(&self, mint_url: &str) -> Option<&MintInfo> {
        self.mints.iter().find(|m| m.url == mint_url)
    }

    /// Get the P2PK pubkey for locking tokens
    pub fn get_p2pk_pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Get relays where nutzaps should be sent
    pub fn get_relays(&self) -> &[String] {
        &self.relays
    }
}

/// Cashu proof in nutzap event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NutzapProof {
    pub amount: u64,
    #[serde(rename = "C")]
    pub c: String,
    pub id: String,
    pub secret: String,
}

impl NutzapProof {
    pub fn new(amount: u64, c: String, id: String, secret: String) -> Self {
        Self {
            amount,
            c,
            id,
            secret,
        }
    }
}

/// Nutzap event (kind 9321)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nutzap {
    pub event: Event,
    pub proofs: Vec<NutzapProof>,
    pub mint_url: String,
    pub unit: String,
    pub recipient_pubkey: String,
    pub nutzapped_event: Option<String>,
    pub nutzapped_kind: Option<u16>,
}

impl Nutzap {
    pub fn from_event(event: Event) -> Result<Self, Nip61Error> {
        if event.kind != NUTZAP_KIND {
            return Err(Nip61Error::InvalidKind {
                expected: NUTZAP_KIND,
                actual: event.kind,
            });
        }

        let mut proofs = Vec::new();
        let mut mint_url = None;
        let mut unit = "sat".to_string();
        let mut recipient_pubkey = None;
        let mut nutzapped_event = None;
        let mut nutzapped_kind = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "proof" if tag.len() >= 2 => {
                    let proof: NutzapProof = serde_json::from_str(&tag[1])
                        .map_err(|e| Nip61Error::InvalidProof(e.to_string()))?;
                    proofs.push(proof);
                }
                "u" if tag.len() >= 2 => {
                    mint_url = Some(tag[1].clone());
                }
                "unit" if tag.len() >= 2 => {
                    unit = tag[1].clone();
                }
                "p" if tag.len() >= 2 => {
                    recipient_pubkey = Some(tag[1].clone());
                }
                "e" if tag.len() >= 2 => {
                    nutzapped_event = Some(tag[1].clone());
                }
                "k" if tag.len() >= 2 => {
                    if let Ok(kind) = tag[1].parse::<u16>() {
                        nutzapped_kind = Some(kind);
                    }
                }
                _ => {}
            }
        }

        let mint_url =
            mint_url.ok_or_else(|| Nip61Error::MissingField("u (mint url)".to_string()))?;
        let recipient_pubkey = recipient_pubkey
            .ok_or_else(|| Nip61Error::MissingField("p (recipient pubkey)".to_string()))?;

        if proofs.is_empty() {
            return Err(Nip61Error::MissingField("proof".to_string()));
        }

        Ok(Self {
            event,
            proofs,
            mint_url,
            unit,
            recipient_pubkey,
            nutzapped_event,
            nutzapped_kind,
        })
    }

    /// Get the comment/message for the nutzap
    pub fn get_comment(&self) -> &str {
        &self.event.content
    }

    /// Get the sender's pubkey
    pub fn get_sender(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the total amount of the nutzap
    pub fn get_total_amount(&self) -> u64 {
        self.proofs.iter().map(|p| p.amount).sum()
    }

    /// Get the mint URL
    pub fn get_mint_url(&self) -> &str {
        &self.mint_url
    }

    /// Get the unit (sat, usd, eur, etc.)
    pub fn get_unit(&self) -> &str {
        &self.unit
    }

    /// Get the recipient's Nostr pubkey
    pub fn get_recipient(&self) -> &str {
        &self.recipient_pubkey
    }

    /// Get the nutzapped event ID if present
    pub fn get_nutzapped_event(&self) -> Option<&str> {
        self.nutzapped_event.as_deref()
    }

    /// Get the nutzapped event kind if present
    pub fn get_nutzapped_kind(&self) -> Option<u16> {
        self.nutzapped_kind
    }

    /// Validate the nutzap against recipient's NutzapInfo
    pub fn validate(&self, recipient_info: &NutzapInfo) -> Result<(), Nip61Error> {
        // Check that mint is trusted
        if !recipient_info.trusts_mint(&self.mint_url) {
            return Err(Nip61Error::Parse(format!(
                "mint {} not trusted by recipient",
                self.mint_url
            )));
        }

        // Check that mint supports the unit
        if let Some(mint_info) = recipient_info.get_mint(&self.mint_url)
            && !mint_info.supports_unit(&self.unit)
        {
            return Err(Nip61Error::Parse(format!(
                "mint {} does not support unit {}",
                self.mint_url, self.unit
            )));
        }

        Ok(())
    }
}

/// Create relay tag for nutzap info
pub fn create_relay_tag(relay_url: String) -> Vec<String> {
    vec!["relay".to_string(), relay_url]
}

/// Create mint tag for nutzap info
pub fn create_mint_tag(mint_url: String, units: Vec<String>) -> Vec<String> {
    let mut tag = vec!["mint".to_string(), mint_url];
    tag.extend(units);
    tag
}

/// Create pubkey tag for nutzap info (P2PK pubkey)
pub fn create_pubkey_tag(pubkey: String) -> Vec<String> {
    vec!["pubkey".to_string(), pubkey]
}

/// Create proof tag for nutzap event
pub fn create_proof_tag(proof: &NutzapProof) -> Result<Vec<String>, Nip61Error> {
    let proof_json = serde_json::to_string(proof)?;
    Ok(vec!["proof".to_string(), proof_json])
}

/// Create u tag for nutzap event (mint URL)
pub fn create_u_tag(mint_url: String) -> Vec<String> {
    vec!["u".to_string(), mint_url]
}

/// Create unit tag for nutzap event
pub fn create_unit_tag(unit: String) -> Vec<String> {
    vec!["unit".to_string(), unit]
}

/// Check if an event kind is a nutzap info kind
pub fn is_nutzap_info_kind(kind: u16) -> bool {
    kind == NUTZAP_INFO_KIND
}

/// Check if an event kind is a nutzap kind
pub fn is_nutzap_kind(kind: u16) -> bool {
    kind == NUTZAP_KIND
}

/// Check if an event kind is any NIP-61 kind
pub fn is_nip61_kind(kind: u16) -> bool {
    is_nutzap_info_kind(kind) || is_nutzap_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_mint_info() {
        let mint = MintInfo::new("https://mint.example.com".to_string());
        assert_eq!(mint.url, "https://mint.example.com");
        assert!(mint.supports_unit("sat"));

        let mint = mint.with_units(vec!["sat".to_string(), "usd".to_string()]);
        assert!(mint.supports_unit("sat"));
        assert!(mint.supports_unit("usd"));
        assert!(!mint.supports_unit("eur"));
    }

    #[test]
    fn test_nutzap_info_from_event() {
        let tags = vec![
            vec!["relay".to_string(), "wss://relay1.com".to_string()],
            vec!["relay".to_string(), "wss://relay2.com".to_string()],
            vec![
                "mint".to_string(),
                "https://mint1.com".to_string(),
                "sat".to_string(),
            ],
            vec![
                "mint".to_string(),
                "https://mint2.com".to_string(),
                "usd".to_string(),
                "sat".to_string(),
            ],
            vec!["pubkey".to_string(), "p2pk_pubkey_hex".to_string()],
        ];

        let event = create_test_event(NUTZAP_INFO_KIND, "", tags);
        let info = NutzapInfo::from_event(event).unwrap();

        assert_eq!(info.relays.len(), 2);
        assert_eq!(info.mints.len(), 2);
        assert_eq!(info.get_p2pk_pubkey(), "p2pk_pubkey_hex");
        assert!(info.trusts_mint("https://mint1.com"));
        assert!(info.trusts_mint("https://mint2.com"));
        assert!(!info.trusts_mint("https://mint3.com"));
    }

    #[test]
    fn test_nutzap_info_missing_pubkey() {
        let tags = vec![vec!["relay".to_string(), "wss://relay1.com".to_string()]];

        let event = create_test_event(NUTZAP_INFO_KIND, "", tags);
        let result = NutzapInfo::from_event(event);

        assert!(result.is_err());
    }

    #[test]
    fn test_nutzap_proof() {
        let proof = NutzapProof::new(
            100,
            "C_value".to_string(),
            "proof_id".to_string(),
            "secret".to_string(),
        );

        assert_eq!(proof.amount, 100);
        assert_eq!(proof.c, "C_value");
    }

    #[test]
    fn test_nutzap_from_event() {
        let proof_json = r#"{"amount":1,"C":"02277c66191736eb72fce9d975d08e3191f8f96afb73ab1eec37e4465683066d3f","id":"000a93d6f8a1d2c4","secret":"test_secret"}"#;

        let tags = vec![
            vec!["proof".to_string(), proof_json.to_string()],
            vec!["u".to_string(), "https://mint.example.com".to_string()],
            vec!["unit".to_string(), "sat".to_string()],
            vec!["p".to_string(), "recipient_pubkey".to_string()],
            vec!["e".to_string(), "event_id_123".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];

        let event = create_test_event(NUTZAP_KIND, "Thanks for the post!", tags);
        let nutzap = Nutzap::from_event(event).unwrap();

        assert_eq!(nutzap.get_comment(), "Thanks for the post!");
        assert_eq!(nutzap.proofs.len(), 1);
        assert_eq!(nutzap.get_total_amount(), 1);
        assert_eq!(nutzap.get_mint_url(), "https://mint.example.com");
        assert_eq!(nutzap.get_unit(), "sat");
        assert_eq!(nutzap.get_recipient(), "recipient_pubkey");
        assert_eq!(nutzap.get_nutzapped_event(), Some("event_id_123"));
        assert_eq!(nutzap.get_nutzapped_kind(), Some(1));
    }

    #[test]
    fn test_nutzap_missing_proof() {
        let tags = vec![
            vec!["u".to_string(), "https://mint.example.com".to_string()],
            vec!["p".to_string(), "recipient_pubkey".to_string()],
        ];

        let event = create_test_event(NUTZAP_KIND, "", tags);
        let result = Nutzap::from_event(event);

        assert!(result.is_err());
    }

    #[test]
    fn test_nutzap_validate() {
        let proof_json = r#"{"amount":100,"C":"C_val","id":"id","secret":"secret"}"#;

        let nutzap_tags = vec![
            vec!["proof".to_string(), proof_json.to_string()],
            vec!["u".to_string(), "https://mint1.com".to_string()],
            vec!["unit".to_string(), "sat".to_string()],
            vec!["p".to_string(), "recipient".to_string()],
        ];

        let info_tags = vec![
            vec!["mint".to_string(), "https://mint1.com".to_string()],
            vec!["pubkey".to_string(), "p2pk_key".to_string()],
        ];

        let nutzap_event = create_test_event(NUTZAP_KIND, "", nutzap_tags);
        let info_event = create_test_event(NUTZAP_INFO_KIND, "", info_tags);

        let nutzap = Nutzap::from_event(nutzap_event).unwrap();
        let info = NutzapInfo::from_event(info_event).unwrap();

        assert!(nutzap.validate(&info).is_ok());
    }

    #[test]
    fn test_nutzap_validate_untrusted_mint() {
        let proof_json = r#"{"amount":100,"C":"C_val","id":"id","secret":"secret"}"#;

        let nutzap_tags = vec![
            vec!["proof".to_string(), proof_json.to_string()],
            vec!["u".to_string(), "https://untrusted-mint.com".to_string()],
            vec!["unit".to_string(), "sat".to_string()],
            vec!["p".to_string(), "recipient".to_string()],
        ];

        let info_tags = vec![
            vec!["mint".to_string(), "https://trusted-mint.com".to_string()],
            vec!["pubkey".to_string(), "p2pk_key".to_string()],
        ];

        let nutzap_event = create_test_event(NUTZAP_KIND, "", nutzap_tags);
        let info_event = create_test_event(NUTZAP_INFO_KIND, "", info_tags);

        let nutzap = Nutzap::from_event(nutzap_event).unwrap();
        let info = NutzapInfo::from_event(info_event).unwrap();

        assert!(nutzap.validate(&info).is_err());
    }

    #[test]
    fn test_create_tags() {
        let relay_tag = create_relay_tag("wss://relay.com".to_string());
        assert_eq!(relay_tag, vec!["relay", "wss://relay.com"]);

        let mint_tag = create_mint_tag("https://mint.com".to_string(), vec!["sat".to_string()]);
        assert_eq!(mint_tag, vec!["mint", "https://mint.com", "sat"]);

        let pubkey_tag = create_pubkey_tag("pubkey_hex".to_string());
        assert_eq!(pubkey_tag, vec!["pubkey", "pubkey_hex"]);

        let u_tag = create_u_tag("https://mint.com".to_string());
        assert_eq!(u_tag, vec!["u", "https://mint.com"]);

        let unit_tag = create_unit_tag("sat".to_string());
        assert_eq!(unit_tag, vec!["unit", "sat"]);
    }

    #[test]
    fn test_create_proof_tag() {
        let proof = NutzapProof::new(
            100,
            "C_value".to_string(),
            "id".to_string(),
            "secret".to_string(),
        );

        let proof_tag = create_proof_tag(&proof).unwrap();
        assert_eq!(proof_tag[0], "proof");
        assert!(proof_tag[1].contains("\"amount\":100"));
    }

    #[test]
    fn test_is_nutzap_info_kind() {
        assert!(is_nutzap_info_kind(NUTZAP_INFO_KIND));
        assert!(!is_nutzap_info_kind(1));
    }

    #[test]
    fn test_is_nutzap_kind() {
        assert!(is_nutzap_kind(NUTZAP_KIND));
        assert!(!is_nutzap_kind(1));
    }

    #[test]
    fn test_is_nip61_kind() {
        assert!(is_nip61_kind(NUTZAP_INFO_KIND));
        assert!(is_nip61_kind(NUTZAP_KIND));
        assert!(!is_nip61_kind(1));
    }
}
