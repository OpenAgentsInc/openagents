//! NIP-72: Moderated Communities (Reddit Style)
//!
//! This NIP enables public communities with moderation. Communities are defined
//! with kind 34550 events, posts use kind 1111, and moderators issue approval
//! events with kind 4550.
//!
//! ## Event Types
//!
//! - **Community Definition** (kind 34550): Defines community and moderators
//! - **Community Post** (kind 1111): Posts to a community (NIP-22)
//! - **Approval Event** (kind 4550): Moderator approval of posts
//!
//! ## Example
//!
//! ```
//! use nostr::nip72::{Community, CommunityModerator, CommunityPost};
//!
//! // Create a community
//! let mut community = Community::new("rust-programming", "Rust Programming");
//! community.description = Some("A community for Rust developers".to_string());
//! community.add_moderator(CommunityModerator::new("mod-pubkey"));
//!
//! // Create a post
//! let post = CommunityPost::new_top_level(
//!     "34550:community-author:rust-programming"
//! );
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for community definition
pub const KIND_COMMUNITY_DEFINITION: u16 = 34550;

/// Kind for community posts (NIP-22)
pub const KIND_COMMUNITY_POST: u16 = 1111;

/// Kind for approval events
pub const KIND_COMMUNITY_APPROVAL: u16 = 4550;

/// Errors that can occur during NIP-72 operations.
#[derive(Debug, Error)]
pub enum Nip72Error {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Community moderator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityModerator {
    /// Pubkey of the moderator (32-bytes hex)
    pub pubkey: String,
    /// Optional recommended relay URL
    pub relay: Option<String>,
}

impl CommunityModerator {
    /// Create a new moderator
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay: None,
        }
    }

    /// Set the relay URL
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }
}

/// Community relay with optional marker
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityRelay {
    /// Relay URL
    pub url: String,
    /// Optional marker (author, requests, approvals)
    pub marker: Option<String>,
}

impl CommunityRelay {
    /// Create a new community relay
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            marker: None,
        }
    }

    /// Set the marker
    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }
}

/// Community definition (kind 34550)
///
/// An addressable event that defines the community and its moderators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Community {
    /// Unique identifier (d tag)
    pub d: String,
    /// Community name (if not provided, d is used)
    pub name: Option<String>,
    /// Community description
    pub description: Option<String>,
    /// Community image URL with optional dimensions
    pub image: Option<(String, Option<String>)>,
    /// Moderators
    pub moderators: Vec<CommunityModerator>,
    /// Relays used by the community
    pub relays: Vec<CommunityRelay>,
}

impl Community {
    /// Create a new community
    pub fn new(d: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            d: d.into(),
            name: Some(name.into()),
            description: None,
            image: None,
            moderators: Vec::new(),
            relays: Vec::new(),
        }
    }

    /// Add a moderator
    pub fn add_moderator(&mut self, moderator: CommunityModerator) {
        self.moderators.push(moderator);
    }

    /// Add a relay
    pub fn add_relay(&mut self, relay: CommunityRelay) {
        self.relays.push(relay);
    }

    /// Validate the community
    pub fn validate(&self) -> Result<(), Nip72Error> {
        if self.d.is_empty() {
            return Err(Nip72Error::MissingField("d".to_string()));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.d.clone()]];

        if let Some(name) = &self.name {
            tags.push(vec!["name".to_string(), name.clone()]);
        }

        if let Some(description) = &self.description {
            tags.push(vec!["description".to_string(), description.clone()]);
        }

        if let Some((image_url, dimensions)) = &self.image {
            let mut tag = vec!["image".to_string(), image_url.clone()];
            if let Some(dim) = dimensions {
                tag.push(dim.clone());
            }
            tags.push(tag);
        }

        for moderator in &self.moderators {
            let mut tag = vec!["p".to_string(), moderator.pubkey.clone()];
            if let Some(relay) = &moderator.relay {
                tag.push(relay.clone());
            } else {
                tag.push(String::new());
            }
            tag.push("moderator".to_string());
            tags.push(tag);
        }

        for relay in &self.relays {
            let mut tag = vec!["relay".to_string(), relay.url.clone()];
            if let Some(marker) = &relay.marker {
                tag.push(marker.clone());
            }
            tags.push(tag);
        }

        tags
    }
}

/// Community post (kind 1111)
///
/// A post to a community, using NIP-22 with uppercase/lowercase tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityPost {
    /// Community reference (uppercase A tag)
    pub community_ref: String,
    /// Optional relay for community
    pub community_relay: Option<String>,
    /// Community author pubkey (uppercase P tag)
    pub community_author: String,
    /// Optional relay for community author
    pub community_author_relay: Option<String>,
    /// Community kind (uppercase K tag, always "34550")
    pub community_kind: String,
    /// Parent reference (lowercase a tag for top-level, e tag for replies)
    pub parent_ref: Option<String>,
    /// Parent type (e for event id, a for address)
    pub parent_ref_type: Option<String>,
    /// Optional relay for parent
    pub parent_relay: Option<String>,
    /// Parent author pubkey (lowercase p tag)
    pub parent_author: Option<String>,
    /// Optional relay for parent author
    pub parent_author_relay: Option<String>,
    /// Parent kind (lowercase k tag)
    pub parent_kind: Option<String>,
}

impl CommunityPost {
    /// Create a new top-level post
    ///
    /// For top-level posts, both uppercase and lowercase tags refer to the community.
    pub fn new_top_level(community_ref: impl Into<String>) -> Self {
        let community_ref_str = community_ref.into();

        // Parse community reference to extract author
        let parts: Vec<&str> = community_ref_str.split(':').collect();
        let community_author = if parts.len() >= 2 {
            parts[1].to_string()
        } else {
            String::new()
        };

        Self {
            community_ref: community_ref_str.clone(),
            community_relay: None,
            community_author,
            community_author_relay: None,
            community_kind: "34550".to_string(),
            parent_ref: Some(community_ref_str),
            parent_ref_type: Some("a".to_string()),
            parent_relay: None,
            parent_author: None,
            parent_author_relay: None,
            parent_kind: Some("34550".to_string()),
        }
    }

    /// Create a nested reply
    ///
    /// For nested replies, uppercase tags refer to community, lowercase to parent.
    pub fn new_reply(
        community_ref: impl Into<String>,
        parent_event_id: impl Into<String>,
        parent_author: impl Into<String>,
        parent_kind: impl Into<String>,
    ) -> Self {
        let community_ref_str = community_ref.into();

        // Parse community reference to extract author
        let parts: Vec<&str> = community_ref_str.split(':').collect();
        let community_author = if parts.len() >= 2 {
            parts[1].to_string()
        } else {
            String::new()
        };

        Self {
            community_ref: community_ref_str,
            community_relay: None,
            community_author,
            community_author_relay: None,
            community_kind: "34550".to_string(),
            parent_ref: Some(parent_event_id.into()),
            parent_ref_type: Some("e".to_string()),
            parent_relay: None,
            parent_author: Some(parent_author.into()),
            parent_author_relay: None,
            parent_kind: Some(parent_kind.into()),
        }
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Uppercase A tag (community)
        let mut a_tag = vec!["A".to_string(), self.community_ref.clone()];
        if let Some(relay) = &self.community_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        // Uppercase P tag (community author)
        let mut p_tag = vec!["P".to_string(), self.community_author.clone()];
        if let Some(relay) = &self.community_author_relay {
            p_tag.push(relay.clone());
        }
        tags.push(p_tag);

        // Uppercase K tag (community kind)
        tags.push(vec!["K".to_string(), self.community_kind.clone()]);

        // Lowercase tags for parent
        if let Some(parent_ref) = &self.parent_ref {
            if let Some(ref_type) = &self.parent_ref_type {
                let mut parent_tag = vec![ref_type.clone(), parent_ref.clone()];
                if let Some(relay) = &self.parent_relay {
                    parent_tag.push(relay.clone());
                }
                tags.push(parent_tag);
            }
        }

        if let Some(parent_author) = &self.parent_author {
            let mut p_tag = vec!["p".to_string(), parent_author.clone()];
            if let Some(relay) = &self.parent_author_relay {
                p_tag.push(relay.clone());
            }
            tags.push(p_tag);
        }

        if let Some(parent_kind) = &self.parent_kind {
            tags.push(vec!["k".to_string(), parent_kind.clone()]);
        }

        tags
    }
}

/// Approval event (kind 4550)
///
/// Moderator approval of a post in a community.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityApproval {
    /// Community reference (a tag, starts with 34550)
    pub community_ref: String,
    /// Optional relay for community
    pub community_relay: Option<String>,
    /// Post event ID (e tag) or address (a tag)
    pub post_ref: String,
    /// Post reference type (e or a)
    pub post_ref_type: String,
    /// Optional relay for post
    pub post_relay: Option<String>,
    /// Post author pubkey (p tag)
    pub post_author: String,
    /// Optional relay for post author
    pub post_author_relay: Option<String>,
    /// Post kind (k tag)
    pub post_kind: String,
}

impl CommunityApproval {
    /// Create a new approval
    pub fn new(
        community_ref: impl Into<String>,
        post_event_id: impl Into<String>,
        post_author: impl Into<String>,
        post_kind: impl Into<String>,
    ) -> Self {
        Self {
            community_ref: community_ref.into(),
            community_relay: None,
            post_ref: post_event_id.into(),
            post_ref_type: "e".to_string(),
            post_relay: None,
            post_author: post_author.into(),
            post_author_relay: None,
            post_kind: post_kind.into(),
        }
    }

    /// Create approval for a replaceable event (a tag)
    pub fn new_for_replaceable(
        community_ref: impl Into<String>,
        post_address: impl Into<String>,
        post_author: impl Into<String>,
        post_kind: impl Into<String>,
    ) -> Self {
        Self {
            community_ref: community_ref.into(),
            community_relay: None,
            post_ref: post_address.into(),
            post_ref_type: "a".to_string(),
            post_relay: None,
            post_author: post_author.into(),
            post_author_relay: None,
            post_kind: post_kind.into(),
        }
    }

    /// Validate the approval
    pub fn validate(&self) -> Result<(), Nip72Error> {
        if self.community_ref.is_empty() {
            return Err(Nip72Error::MissingField("community_ref".to_string()));
        }
        if self.post_ref.is_empty() {
            return Err(Nip72Error::MissingField("post_ref".to_string()));
        }
        if self.post_author.is_empty() {
            return Err(Nip72Error::MissingField("post_author".to_string()));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Community a tag
        let mut a_tag = vec!["a".to_string(), self.community_ref.clone()];
        if let Some(relay) = &self.community_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        // Post reference (e or a tag)
        let mut post_tag = vec![self.post_ref_type.clone(), self.post_ref.clone()];
        if let Some(relay) = &self.post_relay {
            post_tag.push(relay.clone());
        }
        tags.push(post_tag);

        // Post author p tag
        let mut p_tag = vec!["p".to_string(), self.post_author.clone()];
        if let Some(relay) = &self.post_author_relay {
            p_tag.push(relay.clone());
        }
        tags.push(p_tag);

        // Post kind k tag
        tags.push(vec!["k".to_string(), self.post_kind.clone()]);

        tags
    }
}

/// Check if a kind is a NIP-72 kind
pub fn is_nip72_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_COMMUNITY_DEFINITION | KIND_COMMUNITY_POST | KIND_COMMUNITY_APPROVAL
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_community_moderator() {
        let mod1 = CommunityModerator::new("pubkey123").with_relay("wss://relay.example.com");

        assert_eq!(mod1.pubkey, "pubkey123");
        assert_eq!(mod1.relay, Some("wss://relay.example.com".to_string()));
    }

    #[test]
    fn test_community_relay() {
        let relay = CommunityRelay::new("wss://relay.example.com").with_marker("requests");

        assert_eq!(relay.url, "wss://relay.example.com");
        assert_eq!(relay.marker, Some("requests".to_string()));
    }

    #[test]
    fn test_community_new() {
        let community = Community::new("rust-programming", "Rust Programming");
        assert_eq!(community.d, "rust-programming");
        assert_eq!(community.name, Some("Rust Programming".to_string()));
    }

    #[test]
    fn test_community_validate() {
        let community = Community::new("test", "Test Community");
        assert!(community.validate().is_ok());

        let invalid = Community::default();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_community_to_tags() {
        let mut community = Community::new("rust", "Rust Community");
        community.description = Some("A place for Rustaceans".to_string());
        community.image = Some((
            "https://example.com/rust.png".to_string(),
            Some("512x512".to_string()),
        ));
        community.add_moderator(CommunityModerator::new("mod1"));
        community.add_relay(CommunityRelay::new("wss://relay.example.com").with_marker("requests"));

        let tags = community.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "rust".to_string()]));
        assert!(tags.contains(&vec!["name".to_string(), "Rust Community".to_string()]));
        assert!(tags.contains(&vec![
            "description".to_string(),
            "A place for Rustaceans".to_string()
        ]));
    }

    #[test]
    fn test_community_post_top_level() {
        let post = CommunityPost::new_top_level("34550:author-pubkey:rust-programming");

        let tags = post.to_tags();
        assert_eq!(tags[0][0], "A");
        assert_eq!(tags[0][1], "34550:author-pubkey:rust-programming");
        assert_eq!(tags[1][0], "P");
        assert_eq!(tags[1][1], "author-pubkey");
        assert_eq!(tags[2][0], "K");
        assert_eq!(tags[2][1], "34550");

        // Lowercase tags should match community for top-level
        assert!(
            tags.iter()
                .any(|t| t[0] == "a" && t[1] == "34550:author-pubkey:rust-programming")
        );
        assert!(tags.iter().any(|t| t[0] == "k" && t[1] == "34550"));
    }

    #[test]
    fn test_community_post_reply() {
        let post = CommunityPost::new_reply(
            "34550:author-pubkey:rust-programming",
            "parent-event-id",
            "parent-author",
            "1111",
        );

        let tags = post.to_tags();

        // Uppercase tags for community
        assert_eq!(tags[0][0], "A");
        assert_eq!(tags[0][1], "34550:author-pubkey:rust-programming");
        assert_eq!(tags[1][0], "P");
        assert_eq!(tags[2][0], "K");
        assert_eq!(tags[2][1], "34550");

        // Lowercase tags for parent
        assert!(
            tags.iter()
                .any(|t| t[0] == "e" && t[1] == "parent-event-id")
        );
        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "parent-author"));
        assert!(tags.iter().any(|t| t[0] == "k" && t[1] == "1111"));
    }

    #[test]
    fn test_community_approval_new() {
        let approval = CommunityApproval::new(
            "34550:community-author:rust",
            "post-event-id",
            "post-author",
            "1111",
        );

        assert_eq!(approval.community_ref, "34550:community-author:rust");
        assert_eq!(approval.post_ref, "post-event-id");
        assert_eq!(approval.post_ref_type, "e");
        assert_eq!(approval.post_author, "post-author");
        assert_eq!(approval.post_kind, "1111");
    }

    #[test]
    fn test_community_approval_for_replaceable() {
        let approval = CommunityApproval::new_for_replaceable(
            "34550:community-author:rust",
            "30023:author:article-id",
            "author",
            "30023",
        );

        assert_eq!(approval.post_ref_type, "a");
        assert_eq!(approval.post_ref, "30023:author:article-id");
    }

    #[test]
    fn test_community_approval_validate() {
        let approval =
            CommunityApproval::new("34550:community-author:rust", "post-id", "author", "1111");
        assert!(approval.validate().is_ok());

        let invalid = CommunityApproval::new("", "post-id", "author", "1111");
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_community_approval_to_tags() {
        let approval = CommunityApproval::new(
            "34550:community-author:rust",
            "post-event-id",
            "post-author",
            "1111",
        );

        let tags = approval.to_tags();
        assert!(tags.contains(&vec![
            "a".to_string(),
            "34550:community-author:rust".to_string()
        ]));
        assert!(tags.contains(&vec!["e".to_string(), "post-event-id".to_string()]));
        assert!(tags.contains(&vec!["p".to_string(), "post-author".to_string()]));
        assert!(tags.contains(&vec!["k".to_string(), "1111".to_string()]));
    }

    #[test]
    fn test_is_nip72_kind() {
        assert!(is_nip72_kind(KIND_COMMUNITY_DEFINITION));
        assert!(is_nip72_kind(KIND_COMMUNITY_POST));
        assert!(is_nip72_kind(KIND_COMMUNITY_APPROVAL));
        assert!(!is_nip72_kind(1));
    }
}
