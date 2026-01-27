//! NIP-87: Ecash Mint Discoverability
//!
//! This module implements NIP-87 which provides a way to discover ecash mints,
//! their capabilities, and user recommendations.
//!
//! # Overview
//!
//! NIP-87 enables three types of actors:
//! - Mint operators who announce their mint capabilities
//! - Users who recommend mints they trust
//! - Users seeking mint recommendations
//!
//! # Event Types
//!
//! ## Cashu Mint Announcement (kind 38172)
//! - Announces a Cashu mint with URL and supported nuts
//! - Uses `d` tag with mint pubkey as identifier
//! - Lists capabilities via `nuts` tag
//! - Network specification via `n` tag
//!
//! ## Fedimint Announcement (kind 38173)
//! - Announces a Fedimint with invite codes
//! - Uses `d` tag with federation ID as identifier
//! - Lists modules via `modules` tag
//! - Network specification via `n` tag
//!
//! ## Mint Recommendation (kind 38000)
//! - User recommends a mint
//! - References mint announcement via `a` tag
//! - Optional invite codes/URLs via `u` tags
//! - Review content
//!
//! # Example
//!
//! ```
//! use nostr::nip87::{CashuMintInfo, MintRecommendation, MintNetwork};
//!
//! // Create Cashu mint info
//! let mint = CashuMintInfo {
//!     mint_pubkey: "mint123".to_string(),
//!     url: "https://mint.example.com".to_string(),
//!     nuts: vec![1, 2, 3, 4, 5],
//!     network: MintNetwork::Mainnet,
//!     metadata: None,
//! };
//!
//! // Create recommendation
//! let rec = MintRecommendation {
//!     mint_kind: 38172,
//!     d_tag: "mint123".to_string(),
//!     urls: vec!["https://mint.example.com".to_string()],
//!     mint_event_ref: Some(("38172:pubkey:mint123".to_string(), Some("wss://relay".to_string()))),
//!     review: Some("Great mint!".to_string()),
//! };
//! ```

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// Event kind for Cashu mint announcements
pub const KIND_CASHU_MINT: u16 = 38172;

/// Event kind for Fedimint announcements
pub const KIND_FEDIMINT: u16 = 38173;

/// Event kind for mint recommendations
pub const KIND_MINT_RECOMMENDATION: u16 = 38000;

/// Tag name for mint URL or invite code
pub const URL_TAG: &str = "u";

/// Tag name for mint kind reference
pub const KIND_TAG: &str = "k";

/// Tag name for event reference
pub const EVENT_REF_TAG: &str = "a";

/// Tag name for d-tag identifier
pub const D_TAG: &str = "d";

/// Tag name for network
pub const NETWORK_TAG: &str = "n";

/// Tag name for Cashu nuts
pub const NUTS_TAG: &str = "nuts";

/// Tag name for Fedimint modules
pub const MODULES_TAG: &str = "modules";

/// NIP-87 error types
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Nip87Error {
    /// Missing required d-tag
    #[error("missing required d-tag")]
    MissingDTag,

    /// Missing required URL
    #[error("missing required URL (u tag)")]
    MissingUrl,

    /// Invalid network value
    #[error("invalid network: {0}")]
    InvalidNetwork(String),

    /// Invalid nuts format
    #[error("invalid nuts format: {0}")]
    InvalidNuts(String),

    /// Missing kind tag
    #[error("missing kind tag (k)")]
    MissingKindTag,

    /// Wrong event kind
    #[error("expected kind {expected}, got {actual}")]
    WrongKind { expected: u16, actual: u16 },
}

/// Network type for ecash mints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MintNetwork {
    /// Mainnet
    Mainnet,
    /// Testnet
    Testnet,
    /// Signet
    Signet,
    /// Regtest
    Regtest,
}

impl MintNetwork {
    /// Convert network to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            MintNetwork::Mainnet => "mainnet",
            MintNetwork::Testnet => "testnet",
            MintNetwork::Signet => "signet",
            MintNetwork::Regtest => "regtest",
        }
    }

}

impl std::str::FromStr for MintNetwork {
    type Err = Nip87Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(MintNetwork::Mainnet),
            "testnet" => Ok(MintNetwork::Testnet),
            "signet" => Ok(MintNetwork::Signet),
            "regtest" => Ok(MintNetwork::Regtest),
            _ => Err(Nip87Error::InvalidNetwork(s.to_string())),
        }
    }
}

/// Cashu mint information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashuMintInfo {
    /// Mint pubkey (d-tag identifier)
    pub mint_pubkey: String,
    /// Mint URL
    pub url: String,
    /// Supported nuts (NUT numbers)
    pub nuts: Vec<u8>,
    /// Network
    pub network: MintNetwork,
    /// Optional metadata (NIP-01 style)
    pub metadata: Option<String>,
}

/// Fedimint information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FedimintInfo {
    /// Federation ID (d-tag identifier)
    pub federation_id: String,
    /// Invite codes
    pub invite_codes: Vec<String>,
    /// Supported modules
    pub modules: Vec<String>,
    /// Network
    pub network: MintNetwork,
    /// Optional metadata (NIP-01 style)
    pub metadata: Option<String>,
}

/// Mint recommendation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MintRecommendation {
    /// Kind of mint being recommended (38172 or 38173)
    pub mint_kind: u16,
    /// D-tag identifier
    pub d_tag: String,
    /// URLs/invite codes
    pub urls: Vec<String>,
    /// Optional event reference (a-tag): (event_id, relay_hint)
    pub mint_event_ref: Option<(String, Option<String>)>,
    /// Optional review content
    pub review: Option<String>,
}

/// Create tags for a Cashu mint announcement
///
/// # Example
///
/// ```
/// use nostr::nip87::{MintNetwork, create_cashu_mint_tags};
///
/// let tags = create_cashu_mint_tags(
///     "mint_pubkey",
///     "https://mint.example.com",
///     &[1, 2, 3, 4, 5],
///     MintNetwork::Mainnet,
/// );
///
/// assert!(tags.len() >= 4); // d, u, nuts, n
/// ```
pub fn create_cashu_mint_tags(
    mint_pubkey: &str,
    url: &str,
    nuts: &[u8],
    network: MintNetwork,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add d-tag with mint pubkey
    tags.push(vec![D_TAG.to_string(), mint_pubkey.to_string()]);

    // Add URL
    tags.push(vec![URL_TAG.to_string(), url.to_string()]);

    // Add nuts
    let nuts_str = nuts
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");
    tags.push(vec![NUTS_TAG.to_string(), nuts_str]);

    // Add network
    tags.push(vec![NETWORK_TAG.to_string(), network.as_str().to_string()]);

    tags
}

/// Create tags for a Fedimint announcement
///
/// # Example
///
/// ```
/// use nostr::nip87::{MintNetwork, create_fedimint_tags};
///
/// let tags = create_fedimint_tags(
///     "fed_id",
///     &["fed11abc".to_string(), "fed11xyz".to_string()],
///     &["lightning".to_string(), "wallet".to_string()],
///     MintNetwork::Signet,
/// );
///
/// assert!(tags.len() >= 5); // d, 2x u, modules, n
/// ```
pub fn create_fedimint_tags(
    federation_id: &str,
    invite_codes: &[String],
    modules: &[String],
    network: MintNetwork,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add d-tag with federation ID
    tags.push(vec![D_TAG.to_string(), federation_id.to_string()]);

    // Add invite codes
    for code in invite_codes {
        tags.push(vec![URL_TAG.to_string(), code.clone()]);
    }

    // Add modules
    let modules_str = modules.join(",");
    tags.push(vec![MODULES_TAG.to_string(), modules_str]);

    // Add network
    tags.push(vec![NETWORK_TAG.to_string(), network.as_str().to_string()]);

    tags
}

/// Create tags for a mint recommendation
///
/// # Example
///
/// ```
/// use nostr::nip87::create_recommendation_tags;
///
/// let tags = create_recommendation_tags(
///     38172,
///     "mint_pubkey",
///     &["https://mint.example.com".to_string()],
///     Some(("38172:pubkey:mint_pubkey".to_string(), Some("wss://relay".to_string()))),
/// );
///
/// assert!(tags.len() >= 3); // k, d, u, a
/// ```
pub fn create_recommendation_tags(
    mint_kind: u16,
    d_tag: &str,
    urls: &[String],
    mint_event_ref: Option<(String, Option<String>)>,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add k-tag with mint kind
    tags.push(vec![KIND_TAG.to_string(), mint_kind.to_string()]);

    // Add d-tag
    tags.push(vec![D_TAG.to_string(), d_tag.to_string()]);

    // Add URLs
    for url in urls {
        tags.push(vec![URL_TAG.to_string(), url.clone()]);
    }

    // Add event reference if provided
    if let Some((event_id, relay_hint)) = mint_event_ref {
        let mut a_tag = vec![EVENT_REF_TAG.to_string(), event_id];
        if let Some(relay) = relay_hint {
            a_tag.push(relay);
        }
        tags.push(a_tag);
    }

    tags
}

/// Parse Cashu mint info from event
pub fn parse_cashu_mint(
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> Result<CashuMintInfo, Nip87Error> {
    if kind != KIND_CASHU_MINT {
        return Err(Nip87Error::WrongKind {
            expected: KIND_CASHU_MINT,
            actual: kind,
        });
    }

    // Get d-tag (mint pubkey)
    let mint_pubkey = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(D_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip87Error::MissingDTag)?
        .clone();

    // Get URL
    let url = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(URL_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip87Error::MissingUrl)?
        .clone();

    // Get nuts
    let nuts_str = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(NUTS_TAG))
        .and_then(|tag| tag.get(1))
        .unwrap_or(&String::new())
        .clone();

    let nuts: Vec<u8> = if !nuts_str.is_empty() {
        nuts_str
            .split(',')
            .map(|s| s.trim().parse::<u8>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Nip87Error::InvalidNuts(e.to_string()))?
    } else {
        Vec::new()
    };

    // Get network
    let network = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(NETWORK_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| MintNetwork::from_str(s))
        .transpose()?
        .unwrap_or(MintNetwork::Mainnet);

    // Get metadata from content
    let metadata = if !content.is_empty() {
        Some(content.to_string())
    } else {
        None
    };

    Ok(CashuMintInfo {
        mint_pubkey,
        url,
        nuts,
        network,
        metadata,
    })
}

/// Parse Fedimint info from event
pub fn parse_fedimint(
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> Result<FedimintInfo, Nip87Error> {
    if kind != KIND_FEDIMINT {
        return Err(Nip87Error::WrongKind {
            expected: KIND_FEDIMINT,
            actual: kind,
        });
    }

    // Get d-tag (federation ID)
    let federation_id = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(D_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip87Error::MissingDTag)?
        .clone();

    // Get invite codes (all u tags)
    let invite_codes: Vec<String> = tags
        .iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some(URL_TAG))
        .filter_map(|tag| tag.get(1).cloned())
        .collect();

    // Get modules
    let modules_str = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(MODULES_TAG))
        .and_then(|tag| tag.get(1))
        .unwrap_or(&String::new())
        .clone();

    let modules: Vec<String> = if !modules_str.is_empty() {
        modules_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    };

    // Get network
    let network = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(NETWORK_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| MintNetwork::from_str(s))
        .transpose()?
        .unwrap_or(MintNetwork::Mainnet);

    // Get metadata from content
    let metadata = if !content.is_empty() {
        Some(content.to_string())
    } else {
        None
    };

    Ok(FedimintInfo {
        federation_id,
        invite_codes,
        modules,
        network,
        metadata,
    })
}

/// Parse mint recommendation from event
pub fn parse_recommendation(
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> Result<MintRecommendation, Nip87Error> {
    if kind != KIND_MINT_RECOMMENDATION {
        return Err(Nip87Error::WrongKind {
            expected: KIND_MINT_RECOMMENDATION,
            actual: kind,
        });
    }

    // Get k-tag (mint kind)
    let mint_kind = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(KIND_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip87Error::MissingKindTag)?
        .parse::<u16>()
        .map_err(|_| Nip87Error::MissingKindTag)?;

    // Get d-tag
    let d_tag = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(D_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip87Error::MissingDTag)?
        .clone();

    // Get URLs (all u tags)
    let urls: Vec<String> = tags
        .iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some(URL_TAG))
        .filter_map(|tag| tag.get(1).cloned())
        .collect();

    // Get event reference (a tag)
    let mint_event_ref = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(EVENT_REF_TAG))
        .and_then(|tag| {
            tag.get(1)
                .map(|event_id| (event_id.clone(), tag.get(2).cloned()))
        });

    // Get review from content
    let review = if !content.is_empty() {
        Some(content.to_string())
    } else {
        None
    };

    Ok(MintRecommendation {
        mint_kind,
        d_tag,
        urls,
        mint_event_ref,
        review,
    })
}

/// Check if an event is a Cashu mint announcement
pub fn is_cashu_mint(kind: u16) -> bool {
    kind == KIND_CASHU_MINT
}

/// Check if an event is a Fedimint announcement
pub fn is_fedimint(kind: u16) -> bool {
    kind == KIND_FEDIMINT
}

/// Check if an event is a mint recommendation
pub fn is_mint_recommendation(kind: u16) -> bool {
    kind == KIND_MINT_RECOMMENDATION
}

/// Check if an event is any NIP-87 mint-related event
pub fn is_nip87_kind(kind: u16) -> bool {
    is_cashu_mint(kind) || is_fedimint(kind) || is_mint_recommendation(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_network_conversion() {
        assert_eq!(MintNetwork::Mainnet.as_str(), "mainnet");
        assert_eq!(MintNetwork::Testnet.as_str(), "testnet");
        assert_eq!(MintNetwork::Signet.as_str(), "signet");
        assert_eq!(MintNetwork::Regtest.as_str(), "regtest");

        assert!(matches!(
            MintNetwork::from_str("mainnet"),
            Ok(MintNetwork::Mainnet)
        ));
        assert!(matches!(
            MintNetwork::from_str("TESTNET"),
            Ok(MintNetwork::Testnet)
        ));
        assert!(MintNetwork::from_str("invalid").is_err());
    }

    #[test]
    fn test_create_cashu_mint_tags() {
        let tags = create_cashu_mint_tags(
            "mint_pubkey",
            "https://mint.example.com",
            &[1, 2, 3, 4, 5],
            MintNetwork::Mainnet,
        );

        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], vec!["d", "mint_pubkey"]);
        assert_eq!(tags[1], vec!["u", "https://mint.example.com"]);
        assert_eq!(tags[2], vec!["nuts", "1,2,3,4,5"]);
        assert_eq!(tags[3], vec!["n", "mainnet"]);
    }

    #[test]
    fn test_create_fedimint_tags() {
        let tags = create_fedimint_tags(
            "fed_id",
            &["fed11abc".to_string(), "fed11xyz".to_string()],
            &["lightning".to_string(), "wallet".to_string()],
            MintNetwork::Signet,
        );

        assert_eq!(tags.len(), 5);
        assert_eq!(tags[0], vec!["d", "fed_id"]);
        assert_eq!(tags[1], vec!["u", "fed11abc"]);
        assert_eq!(tags[2], vec!["u", "fed11xyz"]);
        assert_eq!(tags[3], vec!["modules", "lightning,wallet"]);
        assert_eq!(tags[4], vec!["n", "signet"]);
    }

    #[test]
    fn test_create_recommendation_tags() {
        let tags = create_recommendation_tags(
            38172,
            "mint_pubkey",
            &["https://mint.example.com".to_string()],
            Some((
                "38172:pubkey:mint_pubkey".to_string(),
                Some("wss://relay".to_string()),
            )),
        );

        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], vec!["k", "38172"]);
        assert_eq!(tags[1], vec!["d", "mint_pubkey"]);
        assert_eq!(tags[2], vec!["u", "https://mint.example.com"]);
        assert_eq!(
            tags[3],
            vec!["a", "38172:pubkey:mint_pubkey", "wss://relay"]
        );
    }

    #[test]
    fn test_parse_cashu_mint() {
        let tags = vec![
            vec!["d".to_string(), "mint_pubkey".to_string()],
            vec!["u".to_string(), "https://mint.example.com".to_string()],
            vec!["nuts".to_string(), "1,2,3,4,5".to_string()],
            vec!["n".to_string(), "mainnet".to_string()],
        ];

        let mint = parse_cashu_mint(KIND_CASHU_MINT, &tags, "").unwrap();
        assert_eq!(mint.mint_pubkey, "mint_pubkey");
        assert_eq!(mint.url, "https://mint.example.com");
        assert_eq!(mint.nuts, vec![1, 2, 3, 4, 5]);
        assert_eq!(mint.network, MintNetwork::Mainnet);
        assert_eq!(mint.metadata, None);
    }

    #[test]
    fn test_parse_fedimint() {
        let tags = vec![
            vec!["d".to_string(), "fed_id".to_string()],
            vec!["u".to_string(), "fed11abc".to_string()],
            vec!["u".to_string(), "fed11xyz".to_string()],
            vec!["modules".to_string(), "lightning,wallet,mint".to_string()],
            vec!["n".to_string(), "signet".to_string()],
        ];

        let fed = parse_fedimint(KIND_FEDIMINT, &tags, "metadata").unwrap();
        assert_eq!(fed.federation_id, "fed_id");
        assert_eq!(fed.invite_codes, vec!["fed11abc", "fed11xyz"]);
        assert_eq!(fed.modules, vec!["lightning", "wallet", "mint"]);
        assert_eq!(fed.network, MintNetwork::Signet);
        assert_eq!(fed.metadata, Some("metadata".to_string()));
    }

    #[test]
    fn test_parse_recommendation() {
        let tags = vec![
            vec!["k".to_string(), "38172".to_string()],
            vec!["d".to_string(), "mint_pubkey".to_string()],
            vec!["u".to_string(), "https://mint.example.com".to_string()],
            vec![
                "a".to_string(),
                "38172:pubkey:mint_pubkey".to_string(),
                "wss://relay".to_string(),
            ],
        ];

        let rec = parse_recommendation(KIND_MINT_RECOMMENDATION, &tags, "Great mint!").unwrap();
        assert_eq!(rec.mint_kind, 38172);
        assert_eq!(rec.d_tag, "mint_pubkey");
        assert_eq!(rec.urls, vec!["https://mint.example.com"]);
        assert_eq!(
            rec.mint_event_ref,
            Some((
                "38172:pubkey:mint_pubkey".to_string(),
                Some("wss://relay".to_string())
            ))
        );
        assert_eq!(rec.review, Some("Great mint!".to_string()));
    }

    #[test]
    fn test_is_functions() {
        assert!(is_cashu_mint(KIND_CASHU_MINT));
        assert!(is_cashu_mint(38172));
        assert!(!is_cashu_mint(1));

        assert!(is_fedimint(KIND_FEDIMINT));
        assert!(is_fedimint(38173));
        assert!(!is_fedimint(1));

        assert!(is_mint_recommendation(KIND_MINT_RECOMMENDATION));
        assert!(is_mint_recommendation(38000));
        assert!(!is_mint_recommendation(1));

        assert!(is_nip87_kind(38172));
        assert!(is_nip87_kind(38173));
        assert!(is_nip87_kind(38000));
        assert!(!is_nip87_kind(1));
    }
}
