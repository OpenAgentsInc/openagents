//! NIP-48: Proxy Tags
//!
//! Defines proxy tags for linking Nostr events back to their source objects
//! when bridged from other protocols like ActivityPub, AT Protocol, RSS, etc.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/48.md>

use crate::Event;
use thiserror::Error;

/// Tag name for proxy tags
pub const PROXY_TAG: &str = "proxy";

/// Errors that can occur during NIP-48 operations
#[derive(Debug, Error)]
pub enum Nip48Error {
    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("invalid protocol: {0}")]
    InvalidProtocol(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Supported protocols for proxy tags
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProtocol {
    /// ActivityPub protocol (ID format: URL)
    ActivityPub,

    /// AT Protocol / Bluesky (ID format: AT URI)
    AtProto,

    /// RSS feeds (ID format: URL with guid fragment)
    Rss,

    /// Web/generic URL (ID format: URL)
    Web,

    /// Other/unknown protocol
    Other(String),
}

impl ProxyProtocol {
    /// Convert protocol to string representation
    pub fn as_str(&self) -> &str {
        match self {
            ProxyProtocol::ActivityPub => "activitypub",
            ProxyProtocol::AtProto => "atproto",
            ProxyProtocol::Rss => "rss",
            ProxyProtocol::Web => "web",
            ProxyProtocol::Other(s) => s.as_str(),
        }
    }

    /// Parse protocol from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "activitypub" => ProxyProtocol::ActivityPub,
            "atproto" => ProxyProtocol::AtProto,
            "rss" => ProxyProtocol::Rss,
            "web" => ProxyProtocol::Web,
            _ => ProxyProtocol::Other(s.to_string()),
        }
    }
}

impl std::fmt::Display for ProxyProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A proxy tag linking to a source object from another protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTag {
    /// The ID of the source object (format varies by protocol)
    pub id: String,

    /// The protocol name
    pub protocol: ProxyProtocol,
}

impl ProxyTag {
    /// Create a new proxy tag
    pub fn new(id: String, protocol: ProxyProtocol) -> Self {
        Self { id, protocol }
    }

    /// Create an ActivityPub proxy tag
    pub fn activitypub(url: String) -> Self {
        Self {
            id: url,
            protocol: ProxyProtocol::ActivityPub,
        }
    }

    /// Create an AT Protocol proxy tag
    pub fn atproto(at_uri: String) -> Self {
        Self {
            id: at_uri,
            protocol: ProxyProtocol::AtProto,
        }
    }

    /// Create an RSS proxy tag
    pub fn rss(url_with_fragment: String) -> Self {
        Self {
            id: url_with_fragment,
            protocol: ProxyProtocol::Rss,
        }
    }

    /// Create a Web proxy tag
    pub fn web(url: String) -> Self {
        Self {
            id: url,
            protocol: ProxyProtocol::Web,
        }
    }

    /// Convert to tag array
    pub fn to_tag(&self) -> Vec<String> {
        vec![
            PROXY_TAG.to_string(),
            self.id.clone(),
            self.protocol.as_str().to_string(),
        ]
    }

    /// Parse from a tag array
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip48Error> {
        if tag.is_empty() || tag[0] != PROXY_TAG {
            return Err(Nip48Error::InvalidTag(format!(
                "expected proxy tag, got: {:?}",
                tag
            )));
        }

        if tag.len() < 3 {
            return Err(Nip48Error::InvalidTag(
                "proxy tag must have id and protocol".to_string(),
            ));
        }

        Ok(Self {
            id: tag[1].clone(),
            protocol: ProxyProtocol::from_str(&tag[2]),
        })
    }

    /// Get the source object ID
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Get the protocol
    pub fn get_protocol(&self) -> &ProxyProtocol {
        &self.protocol
    }

    /// Check if this is an ActivityPub proxy
    pub fn is_activitypub(&self) -> bool {
        matches!(self.protocol, ProxyProtocol::ActivityPub)
    }

    /// Check if this is an AT Protocol proxy
    pub fn is_atproto(&self) -> bool {
        matches!(self.protocol, ProxyProtocol::AtProto)
    }

    /// Check if this is an RSS proxy
    pub fn is_rss(&self) -> bool {
        matches!(self.protocol, ProxyProtocol::Rss)
    }

    /// Check if this is a Web proxy
    pub fn is_web(&self) -> bool {
        matches!(self.protocol, ProxyProtocol::Web)
    }
}

/// Extract proxy tag from an event
pub fn get_proxy_tag(event: &Event) -> Option<ProxyTag> {
    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == PROXY_TAG {
            if let Ok(proxy) = ProxyTag::from_tag(tag) {
                return Some(proxy);
            }
        }
    }
    None
}

/// Extract all proxy tags from an event
pub fn get_proxy_tags(event: &Event) -> Vec<ProxyTag> {
    let mut proxies = Vec::new();

    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == PROXY_TAG {
            if let Ok(proxy) = ProxyTag::from_tag(tag) {
                proxies.push(proxy);
            }
        }
    }

    proxies
}

/// Add a proxy tag to an event's tags
pub fn add_proxy_tag(tags: &mut Vec<Vec<String>>, proxy: ProxyTag) {
    tags.push(proxy.to_tag());
}

/// Check if an event has a proxy tag
pub fn has_proxy_tag(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| !tag.is_empty() && tag[0] == PROXY_TAG)
}

/// Check if an event is bridged (has a proxy tag)
pub fn is_bridged_event(event: &Event) -> bool {
    has_proxy_tag(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_protocol_from_str() {
        assert_eq!(
            ProxyProtocol::from_str("activitypub"),
            ProxyProtocol::ActivityPub
        );
        assert_eq!(ProxyProtocol::from_str("atproto"), ProxyProtocol::AtProto);
        assert_eq!(ProxyProtocol::from_str("rss"), ProxyProtocol::Rss);
        assert_eq!(ProxyProtocol::from_str("web"), ProxyProtocol::Web);

        // Case insensitive
        assert_eq!(
            ProxyProtocol::from_str("ActivityPub"),
            ProxyProtocol::ActivityPub
        );

        // Unknown protocol
        match ProxyProtocol::from_str("unknown") {
            ProxyProtocol::Other(s) => assert_eq!(s, "unknown"),
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_proxy_protocol_as_str() {
        assert_eq!(ProxyProtocol::ActivityPub.as_str(), "activitypub");
        assert_eq!(ProxyProtocol::AtProto.as_str(), "atproto");
        assert_eq!(ProxyProtocol::Rss.as_str(), "rss");
        assert_eq!(ProxyProtocol::Web.as_str(), "web");
        assert_eq!(
            ProxyProtocol::Other("custom".to_string()).as_str(),
            "custom"
        );
    }

    #[test]
    fn test_proxy_protocol_display() {
        assert_eq!(ProxyProtocol::ActivityPub.to_string(), "activitypub");
        assert_eq!(ProxyProtocol::AtProto.to_string(), "atproto");
    }

    #[test]
    fn test_proxy_tag_activitypub() {
        let proxy = ProxyTag::activitypub(
            "https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79".to_string(),
        );

        assert_eq!(
            proxy.get_id(),
            "https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79"
        );
        assert!(proxy.is_activitypub());
        assert!(!proxy.is_atproto());
    }

    #[test]
    fn test_proxy_tag_atproto() {
        let proxy = ProxyTag::atproto(
            "at://did:plc:zhbjlbmir5dganqhueg7y4i3/app.bsky.feed.post/3jt5hlibeol2i".to_string(),
        );

        assert_eq!(
            proxy.get_id(),
            "at://did:plc:zhbjlbmir5dganqhueg7y4i3/app.bsky.feed.post/3jt5hlibeol2i"
        );
        assert!(proxy.is_atproto());
        assert!(!proxy.is_activitypub());
    }

    #[test]
    fn test_proxy_tag_rss() {
        let proxy = ProxyTag::rss(
            "https://soapbox.pub/rss/feed.xml#https%3A%2F%2Fsoapbox.pub%2Fblog%2Fmostr-fediverse-nostr-bridge".to_string(),
        );

        assert!(proxy.is_rss());
    }

    #[test]
    fn test_proxy_tag_web() {
        let proxy = ProxyTag::web("https://twitter.com/jack/status/20".to_string());

        assert_eq!(proxy.get_id(), "https://twitter.com/jack/status/20");
        assert!(proxy.is_web());
    }

    #[test]
    fn test_proxy_tag_to_tag() {
        let proxy = ProxyTag::activitypub("https://example.com/object/123".to_string());
        let tag = proxy.to_tag();

        assert_eq!(tag.len(), 3);
        assert_eq!(tag[0], "proxy");
        assert_eq!(tag[1], "https://example.com/object/123");
        assert_eq!(tag[2], "activitypub");
    }

    #[test]
    fn test_proxy_tag_from_tag() {
        let tag = vec![
            "proxy".to_string(),
            "https://example.com/object/123".to_string(),
            "activitypub".to_string(),
        ];

        let proxy = ProxyTag::from_tag(&tag).unwrap();

        assert_eq!(proxy.get_id(), "https://example.com/object/123");
        assert!(proxy.is_activitypub());
    }

    #[test]
    fn test_proxy_tag_from_tag_invalid() {
        let tag = vec!["other".to_string(), "value".to_string()];

        let result = ProxyTag::from_tag(&tag);
        assert!(result.is_err());
    }

    #[test]
    fn test_proxy_tag_from_tag_missing_fields() {
        let tag = vec!["proxy".to_string(), "id".to_string()];

        let result = ProxyTag::from_tag(&tag);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_proxy_tag() {
        let event = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![vec![
                "proxy".to_string(),
                "https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79".to_string(),
                "activitypub".to_string(),
            ]],
            content: "I'm vegan btw".to_string(),
            sig: "test_sig".to_string(),
        };

        let proxy = get_proxy_tag(&event).unwrap();
        assert!(proxy.is_activitypub());
        assert_eq!(
            proxy.get_id(),
            "https://gleasonator.com/objects/8f6fac53-4f66-4c6e-ac7d-92e5e78c3e79"
        );
    }

    #[test]
    fn test_get_proxy_tags_multiple() {
        let event = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![
                vec![
                    "proxy".to_string(),
                    "https://example.com/object/1".to_string(),
                    "activitypub".to_string(),
                ],
                vec![
                    "proxy".to_string(),
                    "https://example.com/post/2".to_string(),
                    "web".to_string(),
                ],
            ],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let proxies = get_proxy_tags(&event);
        assert_eq!(proxies.len(), 2);
        assert!(proxies[0].is_activitypub());
        assert!(proxies[1].is_web());
    }

    #[test]
    fn test_add_proxy_tag() {
        let mut tags = Vec::new();
        let proxy = ProxyTag::activitypub("https://example.com/object/123".to_string());

        add_proxy_tag(&mut tags, proxy);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "proxy");
        assert_eq!(tags[0][1], "https://example.com/object/123");
        assert_eq!(tags[0][2], "activitypub");
    }

    #[test]
    fn test_has_proxy_tag() {
        let event_with_proxy = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![vec![
                "proxy".to_string(),
                "https://example.com/object/1".to_string(),
                "activitypub".to_string(),
            ]],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let event_without_proxy = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        assert!(has_proxy_tag(&event_with_proxy));
        assert!(!has_proxy_tag(&event_without_proxy));
    }

    #[test]
    fn test_is_bridged_event() {
        let bridged = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![vec![
                "proxy".to_string(),
                "https://example.com/object/1".to_string(),
                "activitypub".to_string(),
            ]],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let native = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1691091365,
            kind: 1,
            tags: vec![],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        assert!(is_bridged_event(&bridged));
        assert!(!is_bridged_event(&native));
    }

    #[test]
    fn test_proxy_tag_new() {
        let proxy = ProxyTag::new(
            "https://example.com/object/1".to_string(),
            ProxyProtocol::ActivityPub,
        );

        assert_eq!(proxy.get_id(), "https://example.com/object/1");
        assert_eq!(proxy.get_protocol(), &ProxyProtocol::ActivityPub);
    }
}
