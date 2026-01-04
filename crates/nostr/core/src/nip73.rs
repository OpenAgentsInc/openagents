//! NIP-73: External Content IDs
//!
//! Defines how to reference external content using globally established identifiers
//! like ISBNs, Podcast GUIDs, ISANs, etc.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/73.md>

use crate::Event;
use thiserror::Error;

/// Tag name for external content identifiers
pub const EXTERNAL_ID_TAG: &str = "i";

/// Tag name for external content kind
pub const EXTERNAL_KIND_TAG: &str = "k";

/// Errors that can occur during NIP-73 operations
#[derive(Debug, Error)]
pub enum Nip73Error {
    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("invalid content ID format: {0}")]
    InvalidFormat(String),

    #[error("unsupported content type: {0}")]
    UnsupportedType(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("missing required field: {0}")]
    MissingField(String),
}

/// Types of external content that can be referenced
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalContentType {
    /// Web URL (normalized, no fragment)
    Web(String),

    /// Book ISBN (without hyphens)
    Isbn(String),

    /// Geohash (lowercase)
    Geohash(String),

    /// Movie ISAN (without version part)
    Isan(String),

    /// Academic paper DOI (lowercase)
    Doi(String),

    /// Hashtag (lowercase)
    Hashtag(String),

    /// Podcast feed GUID
    PodcastFeed(String),

    /// Podcast episode GUID
    PodcastEpisode(String),

    /// Podcast publisher GUID
    PodcastPublisher(String),

    /// Blockchain transaction (blockchain, optional chain ID, transaction ID)
    BlockchainTx {
        blockchain: String,
        chain_id: Option<String>,
        tx_id: String,
    },

    /// Blockchain address (blockchain, optional chain ID, address)
    BlockchainAddress {
        blockchain: String,
        chain_id: Option<String>,
        address: String,
    },
}

impl ExternalContentType {
    /// Get the `i` tag value for this content
    pub fn to_i_tag(&self) -> String {
        match self {
            ExternalContentType::Web(url) => url.clone(),
            ExternalContentType::Isbn(isbn) => format!("isbn:{}", isbn),
            ExternalContentType::Geohash(hash) => format!("geo:{}", hash),
            ExternalContentType::Isan(isan) => format!("isan:{}", isan),
            ExternalContentType::Doi(doi) => format!("doi:{}", doi),
            ExternalContentType::Hashtag(tag) => format!("#{}", tag),
            ExternalContentType::PodcastFeed(guid) => format!("podcast:guid:{}", guid),
            ExternalContentType::PodcastEpisode(guid) => format!("podcast:item:guid:{}", guid),
            ExternalContentType::PodcastPublisher(guid) => {
                format!("podcast:publisher:guid:{}", guid)
            }
            ExternalContentType::BlockchainTx {
                blockchain,
                chain_id,
                tx_id,
            } => {
                if let Some(cid) = chain_id {
                    format!("{}:{}:tx:{}", blockchain, cid, tx_id)
                } else {
                    format!("{}:tx:{}", blockchain, tx_id)
                }
            }
            ExternalContentType::BlockchainAddress {
                blockchain,
                chain_id,
                address,
            } => {
                if let Some(cid) = chain_id {
                    format!("{}:{}:address:{}", blockchain, cid, address)
                } else {
                    format!("{}:address:{}", blockchain, address)
                }
            }
        }
    }

    /// Get the `k` tag value for this content type
    pub fn to_k_tag(&self) -> String {
        match self {
            ExternalContentType::Web(_) => "web".to_string(),
            ExternalContentType::Isbn(_) => "isbn".to_string(),
            ExternalContentType::Geohash(_) => "geo".to_string(),
            ExternalContentType::Isan(_) => "isan".to_string(),
            ExternalContentType::Doi(_) => "doi".to_string(),
            ExternalContentType::Hashtag(_) => "#".to_string(),
            ExternalContentType::PodcastFeed(_) => "podcast:guid".to_string(),
            ExternalContentType::PodcastEpisode(_) => "podcast:item:guid".to_string(),
            ExternalContentType::PodcastPublisher(_) => "podcast:publisher:guid".to_string(),
            ExternalContentType::BlockchainTx { blockchain, .. } => {
                format!("{}:tx", blockchain)
            }
            ExternalContentType::BlockchainAddress { blockchain, .. } => {
                format!("{}:address", blockchain)
            }
        }
    }

    /// Parse an external content reference from an `i` tag value
    pub fn from_i_tag(value: &str) -> Result<Self, Nip73Error> {
        // Check for various prefixes
        if let Some(isbn) = value.strip_prefix("isbn:") {
            Ok(ExternalContentType::Isbn(isbn.to_string()))
        } else if let Some(hash) = value.strip_prefix("geo:") {
            Ok(ExternalContentType::Geohash(hash.to_string()))
        } else if let Some(isan) = value.strip_prefix("isan:") {
            Ok(ExternalContentType::Isan(isan.to_string()))
        } else if let Some(doi) = value.strip_prefix("doi:") {
            Ok(ExternalContentType::Doi(doi.to_string()))
        } else if let Some(tag) = value.strip_prefix('#') {
            Ok(ExternalContentType::Hashtag(tag.to_string()))
        } else if let Some(rest) = value.strip_prefix("podcast:guid:") {
            Ok(ExternalContentType::PodcastFeed(rest.to_string()))
        } else if let Some(rest) = value.strip_prefix("podcast:item:guid:") {
            Ok(ExternalContentType::PodcastEpisode(rest.to_string()))
        } else if let Some(rest) = value.strip_prefix("podcast:publisher:guid:") {
            Ok(ExternalContentType::PodcastPublisher(rest.to_string()))
        } else if value.contains(":tx:") {
            // Blockchain transaction
            Self::parse_blockchain_tx(value)
        } else if value.contains(":address:") {
            // Blockchain address
            Self::parse_blockchain_address(value)
        } else if value.starts_with("http://") || value.starts_with("https://") {
            // Web URL
            Ok(ExternalContentType::Web(value.to_string()))
        } else {
            Err(Nip73Error::InvalidFormat(format!(
                "unrecognized format: {}",
                value
            )))
        }
    }

    fn parse_blockchain_tx(value: &str) -> Result<Self, Nip73Error> {
        let parts: Vec<&str> = value.split(':').collect();

        if parts.len() < 3 {
            return Err(Nip73Error::InvalidFormat(format!(
                "invalid blockchain tx format: {}",
                value
            )));
        }

        let blockchain = parts[0].to_string();

        // Check if there's a chain ID (4 parts) or not (3 parts)
        if parts.len() == 4 && parts[2] == "tx" {
            // Format: blockchain:chainId:tx:txid
            Ok(ExternalContentType::BlockchainTx {
                blockchain,
                chain_id: Some(parts[1].to_string()),
                tx_id: parts[3].to_string(),
            })
        } else if parts.len() == 3 && parts[1] == "tx" {
            // Format: blockchain:tx:txid
            Ok(ExternalContentType::BlockchainTx {
                blockchain,
                chain_id: None,
                tx_id: parts[2].to_string(),
            })
        } else {
            Err(Nip73Error::InvalidFormat(format!(
                "invalid blockchain tx format: {}",
                value
            )))
        }
    }

    fn parse_blockchain_address(value: &str) -> Result<Self, Nip73Error> {
        let parts: Vec<&str> = value.split(':').collect();

        if parts.len() < 3 {
            return Err(Nip73Error::InvalidFormat(format!(
                "invalid blockchain address format: {}",
                value
            )));
        }

        let blockchain = parts[0].to_string();

        // Check if there's a chain ID (4 parts) or not (3 parts)
        if parts.len() == 4 && parts[2] == "address" {
            // Format: blockchain:chainId:address:addr
            Ok(ExternalContentType::BlockchainAddress {
                blockchain,
                chain_id: Some(parts[1].to_string()),
                address: parts[3].to_string(),
            })
        } else if parts.len() == 3 && parts[1] == "address" {
            // Format: blockchain:address:addr
            Ok(ExternalContentType::BlockchainAddress {
                blockchain,
                chain_id: None,
                address: parts[2].to_string(),
            })
        } else {
            Err(Nip73Error::InvalidFormat(format!(
                "invalid blockchain address format: {}",
                value
            )))
        }
    }
}

/// An external content reference with optional URL hint
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalContent {
    pub content: ExternalContentType,
    pub url_hint: Option<String>,
}

impl ExternalContent {
    /// Create a new external content reference
    pub fn new(content: ExternalContentType) -> Self {
        Self {
            content,
            url_hint: None,
        }
    }

    /// Create a new external content reference with a URL hint
    pub fn with_url_hint(content: ExternalContentType, url_hint: String) -> Self {
        Self {
            content,
            url_hint: Some(url_hint),
        }
    }

    /// Convert to tag array (i tag)
    pub fn to_i_tag_array(&self) -> Vec<String> {
        let mut tag = vec![EXTERNAL_ID_TAG.to_string(), self.content.to_i_tag()];
        if let Some(hint) = &self.url_hint {
            tag.push(hint.clone());
        }
        tag
    }

    /// Convert to k tag array
    pub fn to_k_tag_array(&self) -> Vec<String> {
        vec![EXTERNAL_KIND_TAG.to_string(), self.content.to_k_tag()]
    }

    /// Parse from an i tag
    pub fn from_i_tag(tag: &[String]) -> Result<Self, Nip73Error> {
        if tag.is_empty() || tag[0] != EXTERNAL_ID_TAG {
            return Err(Nip73Error::InvalidTag(format!(
                "expected i tag, got: {:?}",
                tag
            )));
        }

        if tag.len() < 2 {
            return Err(Nip73Error::InvalidTag("i tag missing value".to_string()));
        }

        let content = ExternalContentType::from_i_tag(&tag[1])?;
        let url_hint = if tag.len() > 2 {
            Some(tag[2].clone())
        } else {
            None
        };

        Ok(Self { content, url_hint })
    }

    /// Get the content type
    pub fn get_content(&self) -> &ExternalContentType {
        &self.content
    }

    /// Get the URL hint if present
    pub fn get_url_hint(&self) -> Option<&str> {
        self.url_hint.as_deref()
    }
}

/// Extract all external content references from an event
pub fn get_external_content_refs(event: &Event) -> Vec<ExternalContent> {
    let mut refs = Vec::new();

    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == EXTERNAL_ID_TAG {
            if let Ok(content) = ExternalContent::from_i_tag(tag) {
                refs.push(content);
            }
        }
    }

    refs
}

/// Add an external content reference to an event's tags
pub fn add_external_content(tags: &mut Vec<Vec<String>>, content: ExternalContent) {
    tags.push(content.to_i_tag_array());
    tags.push(content.to_k_tag_array());
}

/// Helper function to create a web URL reference
pub fn web(url: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Web(url))
}

/// Helper function to create an ISBN reference
pub fn isbn(isbn: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Isbn(isbn))
}

/// Helper function to create a geohash reference
pub fn geohash(hash: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Geohash(hash))
}

/// Helper function to create an ISAN reference
pub fn isan(isan: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Isan(isan))
}

/// Helper function to create a DOI reference
pub fn doi(doi: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Doi(doi))
}

/// Helper function to create a hashtag reference
pub fn hashtag(tag: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::Hashtag(tag))
}

/// Helper function to create a podcast feed reference
pub fn podcast_feed(guid: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::PodcastFeed(guid))
}

/// Helper function to create a podcast episode reference
pub fn podcast_episode(guid: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::PodcastEpisode(guid))
}

/// Helper function to create a podcast publisher reference
pub fn podcast_publisher(guid: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::PodcastPublisher(guid))
}

/// Helper function to create a Bitcoin transaction reference
pub fn bitcoin_tx(tx_id: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::BlockchainTx {
        blockchain: "bitcoin".to_string(),
        chain_id: None,
        tx_id,
    })
}

/// Helper function to create a Bitcoin address reference
pub fn bitcoin_address(address: String) -> ExternalContent {
    ExternalContent::new(ExternalContentType::BlockchainAddress {
        blockchain: "bitcoin".to_string(),
        chain_id: None,
        address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_url() {
        let content = web("https://myblog.example.com/post/2012-03-27/hello-world".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "https://myblog.example.com/post/2012-03-27/hello-world".to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn test_isbn() {
        let content = isbn("9780765382030".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec!["i".to_string(), "isbn:9780765382030".to_string()]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "isbn".to_string()]
        );
    }

    #[test]
    fn test_podcast_feed() {
        let content = podcast_feed("c90e609a-df1e-596a-bd5e-57bcc8aad6cc".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "podcast:guid:c90e609a-df1e-596a-bd5e-57bcc8aad6cc".to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "podcast:guid".to_string()]
        );
    }

    #[test]
    fn test_podcast_episode() {
        let content = podcast_episode("d98d189b-dc7b-45b1-8720-d4b98690f31f".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f".to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "podcast:item:guid".to_string()]
        );
    }

    #[test]
    fn test_podcast_publisher() {
        let content = podcast_publisher("18bcbf10-6701-4ffb-b255-bc057390d738".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "podcast:publisher:guid:18bcbf10-6701-4ffb-b255-bc057390d738".to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "podcast:publisher:guid".to_string()]
        );
    }

    #[test]
    fn test_isan() {
        let content = isan("0000-0000-401A-0000-7".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec!["i".to_string(), "isan:0000-0000-401A-0000-7".to_string()]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "isan".to_string()]
        );
    }

    #[test]
    fn test_bitcoin_tx() {
        let content = bitcoin_tx(
            "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d".to_string(),
        );
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "bitcoin:tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d"
                    .to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "bitcoin:tx".to_string()]
        );
    }

    #[test]
    fn test_bitcoin_address() {
        let content = bitcoin_address("1HQ3Go3ggs8pFnXuHVHRytPCq5fGG8Hbhx".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "bitcoin:address:1HQ3Go3ggs8pFnXuHVHRytPCq5fGG8Hbhx".to_string()
            ]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "bitcoin:address".to_string()]
        );
    }

    #[test]
    fn test_url_hint() {
        let content = ExternalContent::with_url_hint(
            ExternalContentType::PodcastEpisode("d98d189b-dc7b-45b1-8720-d4b98690f31f".to_string()),
            "https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg".to_string(),
        );
        assert_eq!(
            content.to_i_tag_array(),
            vec![
                "i".to_string(),
                "podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f".to_string(),
                "https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg".to_string()
            ]
        );
        assert_eq!(
            content.get_url_hint(),
            Some("https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg")
        );
    }

    #[test]
    fn test_parse_isbn() {
        let tag = vec!["i".to_string(), "isbn:9780765382030".to_string()];
        let content = ExternalContent::from_i_tag(&tag).unwrap();
        assert!(matches!(
            content.content,
            ExternalContentType::Isbn(ref isbn) if isbn == "9780765382030"
        ));
    }

    #[test]
    fn test_parse_web_url() {
        let tag = vec![
            "i".to_string(),
            "https://myblog.example.com/post".to_string(),
        ];
        let content = ExternalContent::from_i_tag(&tag).unwrap();
        assert!(matches!(
            content.content,
            ExternalContentType::Web(ref url) if url == "https://myblog.example.com/post"
        ));
    }

    #[test]
    fn test_parse_bitcoin_tx() {
        let tag = vec![
            "i".to_string(),
            "bitcoin:tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d"
                .to_string(),
        ];
        let content = ExternalContent::from_i_tag(&tag).unwrap();
        assert!(matches!(
            content.content,
            ExternalContentType::BlockchainTx {
                ref blockchain,
                ref chain_id,
                ref tx_id
            } if blockchain == "bitcoin" && chain_id.is_none() && tx_id == "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d"
        ));
    }

    #[test]
    fn test_geohash() {
        let content = geohash("ezs42".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec!["i".to_string(), "geo:ezs42".to_string()]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "geo".to_string()]
        );
    }

    #[test]
    fn test_doi() {
        let content = doi("10.1000/xyz123".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec!["i".to_string(), "doi:10.1000/xyz123".to_string()]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "doi".to_string()]
        );
    }

    #[test]
    fn test_hashtag() {
        let content = hashtag("bitcoin".to_string());
        assert_eq!(
            content.to_i_tag_array(),
            vec!["i".to_string(), "#bitcoin".to_string()]
        );
        assert_eq!(
            content.to_k_tag_array(),
            vec!["k".to_string(), "#".to_string()]
        );
    }

    #[test]
    fn test_get_external_content_refs() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![
                vec!["i".to_string(), "isbn:9780765382030".to_string()],
                vec!["k".to_string(), "isbn".to_string()],
                vec![
                    "i".to_string(),
                    "https://myblog.example.com/post".to_string(),
                ],
                vec!["k".to_string(), "web".to_string()],
            ],
            content: "test content".to_string(),
            sig: "test_sig".to_string(),
        };

        let refs = get_external_content_refs(&event);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_add_external_content() {
        let mut tags = Vec::new();
        let content = isbn("9780765382030".to_string());
        add_external_content(&mut tags, content);

        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags[0],
            vec!["i".to_string(), "isbn:9780765382030".to_string()]
        );
        assert_eq!(tags[1], vec!["k".to_string(), "isbn".to_string()]);
    }
}
