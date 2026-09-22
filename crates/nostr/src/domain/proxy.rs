//! NIP-48 bridged events.
//!
//! A `proxy` tag on any kind marks the event as bridged from another
//! protocol: `["proxy", <id>, <protocol>]`, where the id is the source
//! object's identifier and the protocol names the bridge —
//! `activitypub`, `atproto`, `rss`, or `web` are defined and the list
//! may extend. Clients use it to reconcile duplicated bridged content
//! or to link the source. NIP-48 is a draft and binds no admission
//! rule, so the tag is stored and is not added to the NIP-11 list.

use super::Event;

/// A bridged event's source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxySource {
    /// The source object's protocol-specific id.
    pub id: String,
    /// The bridge protocol, lowercased: `activitypub`, `atproto`,
    /// `rss`, `web`, or a future extension.
    pub protocol: String,
}

/// Whether the protocol name is one the pinned text defines.
#[must_use]
pub fn known_protocol(protocol: &str) -> bool {
    matches!(protocol, "activitypub" | "atproto" | "rss" | "web")
}

/// Every `proxy` tag on an event, id and protocol both non-empty.
#[must_use]
pub fn proxy_sources(event: &Event) -> Vec<ProxySource> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("proxy"))
        .filter_map(|tag| {
            let id = tag.value()?.to_string();
            let protocol = tag.0.get(2)?.to_string();
            (!id.is_empty() && !protocol.is_empty()).then_some(ProxySource { id, protocol })
        })
        .collect()
}

/// Whether an event declares itself bridged rather than native.
#[must_use]
pub fn is_bridged(event: &Event) -> bool {
    !proxy_sources(event).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, "I'm vegan btw".to_string())
    }

    #[test]
    fn a_proxy_tag_links_the_bridged_source() {
        let bridged = sign(vec![
            Tag::new(vec![
                "proxy".into(),
                "https://gleasonator.com/objects/8f6fac53".into(),
                "activitypub".into(),
            ]),
            Tag::new(vec![
                "proxy".into(),
                "at://did:plc:zhbj/app.bsky.feed.post/3jt".into(),
                "atproto".into(),
            ]),
        ]);
        let sources = proxy_sources(&bridged);
        assert_eq!(sources.len(), 2);
        assert!(is_bridged(&bridged));
        assert!(known_protocol(&sources[0].protocol));
        assert!(known_protocol(&sources[1].protocol));
        assert_eq!(sources[0].id, "https://gleasonator.com/objects/8f6fac53");

        // An unknown protocol still parses — the list may extend.
        assert!(!known_protocol("farcaster"));
        // A malformed tag without a protocol is skipped.
        let bare = sign(vec![Tag::new(vec!["proxy".into(), "id".into()])]);
        assert!(!is_bridged(&bare));
        assert!(!is_bridged(&sign(Vec::new())));
    }
}
