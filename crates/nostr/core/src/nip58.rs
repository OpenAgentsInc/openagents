//! NIP-58: Badges
//!
//! Implements badge definitions, awards, and profile badge display.
//!
//! Features:
//! - Badge Definition (kind 30009): Define reusable badges with images
//! - Badge Award (kind 8): Award badges to users
//! - Profile Badges (kind 30008): Display selected badges on profile
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/58.md>

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for badge definitions (addressable)
pub const KIND_BADGE_DEFINITION: u16 = 30009;

/// Event kind for badge awards
pub const KIND_BADGE_AWARD: u16 = 8;

/// Event kind for profile badges (addressable)
pub const KIND_PROFILE_BADGES: u16 = 30008;

/// Fixed d tag value for profile badges
pub const PROFILE_BADGES_D_TAG: &str = "profile_badges";

/// Errors that can occur during NIP-58 operations
#[derive(Debug, Error)]
pub enum Nip58Error {
    #[error("invalid badge definition: {0}")]
    InvalidDefinition(String),

    #[error("invalid badge award: {0}")]
    InvalidAward(String),

    #[error("invalid profile badges: {0}")]
    InvalidProfileBadges(String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag format: {0}")]
    InvalidTagFormat(String),

    #[error("invalid dimensions: {0}")]
    InvalidDimensions(String),
}

/// Image dimensions (width x height in pixels)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

impl ImageDimensions {
    /// Create new dimensions
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Parse from "widthxheight" string
    pub fn parse(s: &str) -> Result<Self, Nip58Error> {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() != 2 {
            return Err(Nip58Error::InvalidDimensions(format!(
                "expected WIDTHxHEIGHT format, got '{}'",
                s
            )));
        }

        let width: u32 = parts[0]
            .parse()
            .map_err(|_| Nip58Error::InvalidDimensions(format!("invalid width: {}", parts[0])))?;

        let height: u32 = parts[1]
            .parse()
            .map_err(|_| Nip58Error::InvalidDimensions(format!("invalid height: {}", parts[1])))?;

        Ok(Self { width, height })
    }
}

impl std::fmt::Display for ImageDimensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Badge thumbnail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeThumbnail {
    /// URL to thumbnail image
    pub url: String,

    /// Optional dimensions
    pub dimensions: Option<ImageDimensions>,
}

impl BadgeThumbnail {
    /// Create new thumbnail
    pub fn new(url: String) -> Self {
        Self {
            url,
            dimensions: None,
        }
    }

    /// Set dimensions
    pub fn with_dimensions(mut self, dimensions: ImageDimensions) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

/// Badge definition (kind 30009)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeDefinition {
    /// Unique identifier (d tag)
    pub identifier: String,

    /// Optional short name
    pub name: Option<String>,

    /// Optional description
    pub description: Option<String>,

    /// Optional high-resolution image URL
    pub image: Option<String>,

    /// Optional image dimensions
    pub image_dimensions: Option<ImageDimensions>,

    /// Optional thumbnails
    pub thumbnails: Vec<BadgeThumbnail>,
}

impl BadgeDefinition {
    /// Create new badge definition
    pub fn new(identifier: String) -> Self {
        Self {
            identifier,
            name: None,
            description: None,
            image: None,
            image_dimensions: None,
            thumbnails: Vec::new(),
        }
    }

    /// Set name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set image
    pub fn with_image(mut self, url: String) -> Self {
        self.image = Some(url);
        self
    }

    /// Set image dimensions
    pub fn with_image_dimensions(mut self, dimensions: ImageDimensions) -> Self {
        self.image_dimensions = Some(dimensions);
        self
    }

    /// Add thumbnail
    pub fn add_thumbnail(mut self, thumbnail: BadgeThumbnail) -> Self {
        self.thumbnails.push(thumbnail);
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // d tag (identifier) - REQUIRED
        tags.push(vec!["d".to_string(), self.identifier.clone()]);

        // name tag (optional)
        if let Some(ref name) = self.name {
            tags.push(vec!["name".to_string(), name.clone()]);
        }

        // description tag (optional)
        if let Some(ref desc) = self.description {
            tags.push(vec!["description".to_string(), desc.clone()]);
        }

        // image tag (optional)
        if let Some(ref url) = self.image {
            let mut tag = vec!["image".to_string(), url.clone()];
            if let Some(ref dims) = self.image_dimensions {
                tag.push(dims.to_string());
            }
            tags.push(tag);
        }

        // thumb tags (optional)
        for thumb in &self.thumbnails {
            let mut tag = vec!["thumb".to_string(), thumb.url.clone()];
            if let Some(ref dims) = thumb.dimensions {
                tag.push(dims.to_string());
            }
            tags.push(tag);
        }

        tags
    }

    /// Parse from event tags
    pub fn from_tags(tags: &[Vec<String>]) -> Result<Self, Nip58Error> {
        let mut identifier = None;
        let mut name = None;
        let mut description = None;
        let mut image = None;
        let mut image_dimensions = None;
        let mut thumbnails = Vec::new();

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "d" => {
                    if tag.len() < 2 {
                        return Err(Nip58Error::InvalidTagFormat(
                            "d tag requires identifier".to_string(),
                        ));
                    }
                    identifier = Some(tag[1].clone());
                }
                "name" => {
                    if tag.len() >= 2 {
                        name = Some(tag[1].clone());
                    }
                }
                "description" => {
                    if tag.len() >= 2 {
                        description = Some(tag[1].clone());
                    }
                }
                "image" => {
                    if tag.len() >= 2 {
                        image = Some(tag[1].clone());
                        if tag.len() >= 3 {
                            image_dimensions = Some(ImageDimensions::parse(&tag[2])?);
                        }
                    }
                }
                "thumb" => {
                    if tag.len() >= 2 {
                        let url = tag[1].clone();
                        let dims = if tag.len() >= 3 {
                            Some(ImageDimensions::parse(&tag[2])?)
                        } else {
                            None
                        };
                        let mut thumb = BadgeThumbnail::new(url);
                        if let Some(d) = dims {
                            thumb = thumb.with_dimensions(d);
                        }
                        thumbnails.push(thumb);
                    }
                }
                _ => {}
            }
        }

        let identifier =
            identifier.ok_or_else(|| Nip58Error::MissingTag("d tag required".to_string()))?;

        Ok(Self {
            identifier,
            name,
            description,
            image,
            image_dimensions,
            thumbnails,
        })
    }
}

/// Badge award (kind 8)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeAward {
    /// Badge definition coordinate (30009:pubkey:identifier)
    pub badge_definition: String,

    /// Awarded pubkeys with optional relay hints
    pub awarded_pubkeys: Vec<(String, Option<String>)>,
}

impl BadgeAward {
    /// Create new badge award
    pub fn new(badge_definition: String) -> Self {
        Self {
            badge_definition,
            awarded_pubkeys: Vec::new(),
        }
    }

    /// Add awarded pubkey
    pub fn add_pubkey(mut self, pubkey: String) -> Self {
        self.awarded_pubkeys.push((pubkey, None));
        self
    }

    /// Add awarded pubkey with relay hint
    pub fn add_pubkey_with_relay(mut self, pubkey: String, relay: String) -> Self {
        self.awarded_pubkeys.push((pubkey, Some(relay)));
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // a tag (badge definition) - REQUIRED
        tags.push(vec!["a".to_string(), self.badge_definition.clone()]);

        // p tags (awarded pubkeys) - REQUIRED (at least one)
        for (pubkey, relay) in &self.awarded_pubkeys {
            let mut tag = vec!["p".to_string(), pubkey.clone()];
            if let Some(r) = relay {
                tag.push(r.clone());
            }
            tags.push(tag);
        }

        tags
    }

    /// Parse from event tags
    pub fn from_tags(tags: &[Vec<String>]) -> Result<Self, Nip58Error> {
        let mut badge_definition = None;
        let mut awarded_pubkeys = Vec::new();

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "a" => {
                    if tag.len() < 2 {
                        return Err(Nip58Error::InvalidTagFormat(
                            "a tag requires badge definition coordinate".to_string(),
                        ));
                    }
                    badge_definition = Some(tag[1].clone());
                }
                "p" => {
                    if tag.len() < 2 {
                        return Err(Nip58Error::InvalidTagFormat(
                            "p tag requires pubkey".to_string(),
                        ));
                    }
                    let pubkey = tag[1].clone();
                    let relay = tag.get(2).cloned();
                    awarded_pubkeys.push((pubkey, relay));
                }
                _ => {}
            }
        }

        let badge_definition =
            badge_definition.ok_or_else(|| Nip58Error::MissingTag("a tag required".to_string()))?;

        if awarded_pubkeys.is_empty() {
            return Err(Nip58Error::MissingTag(
                "at least one p tag required".to_string(),
            ));
        }

        Ok(Self {
            badge_definition,
            awarded_pubkeys,
        })
    }
}

/// Profile badge pair (badge definition + award)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBadgePair {
    /// Badge definition coordinate (30009:pubkey:identifier)
    pub badge_definition: String,

    /// Badge award event ID
    pub award_event_id: String,

    /// Optional relay hint for award event
    pub relay_hint: Option<String>,
}

impl ProfileBadgePair {
    /// Create new profile badge pair
    pub fn new(badge_definition: String, award_event_id: String) -> Self {
        Self {
            badge_definition,
            award_event_id,
            relay_hint: None,
        }
    }

    /// Set relay hint
    pub fn with_relay(mut self, relay: String) -> Self {
        self.relay_hint = Some(relay);
        self
    }
}

/// Profile badges (kind 30008)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBadges {
    /// Ordered list of badge pairs to display
    pub badges: Vec<ProfileBadgePair>,
}

impl ProfileBadges {
    /// Create new profile badges
    pub fn new() -> Self {
        Self { badges: Vec::new() }
    }

    /// Add badge pair
    pub fn add_badge(mut self, pair: ProfileBadgePair) -> Self {
        self.badges.push(pair);
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // d tag with fixed value "profile_badges" - REQUIRED
        tags.push(vec!["d".to_string(), PROFILE_BADGES_D_TAG.to_string()]);

        // Pairs of a and e tags
        for pair in &self.badges {
            tags.push(vec!["a".to_string(), pair.badge_definition.clone()]);

            let mut e_tag = vec!["e".to_string(), pair.award_event_id.clone()];
            if let Some(ref relay) = pair.relay_hint {
                e_tag.push(relay.clone());
            }
            tags.push(e_tag);
        }

        tags
    }

    /// Parse from event tags
    pub fn from_tags(tags: &[Vec<String>]) -> Result<Self, Nip58Error> {
        let mut has_d_tag = false;
        let mut badges = Vec::new();
        let mut current_a: Option<String> = None;

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "d" => {
                    if tag.len() >= 2 && tag[1] == PROFILE_BADGES_D_TAG {
                        has_d_tag = true;
                    }
                }
                "a" => {
                    if tag.len() >= 2 {
                        // Save previous pair if we have a pending 'a' tag
                        if current_a.is_some() {
                            // Unpaired 'a' tag - ignore per spec
                        }
                        current_a = Some(tag[1].clone());
                    }
                }
                "e" => {
                    if tag.len() >= 2
                        && let Some(a_tag) = current_a.take()
                    {
                        let award_event_id = tag[1].clone();
                        let relay_hint = tag.get(2).cloned();
                        let mut pair = ProfileBadgePair::new(a_tag, award_event_id);
                        if let Some(relay) = relay_hint {
                            pair = pair.with_relay(relay);
                        }
                        badges.push(pair);
                    }
                    // else: unpaired 'e' tag - ignore per spec
                }
                _ => {}
            }
        }

        if !has_d_tag {
            return Err(Nip58Error::MissingTag(
                "d tag with value 'profile_badges' required".to_string(),
            ));
        }

        Ok(Self { badges })
    }
}

impl Default for ProfileBadges {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a kind is a badge definition kind (30009)
pub fn is_badge_definition_kind(kind: u16) -> bool {
    kind == KIND_BADGE_DEFINITION
}

/// Check if a kind is a badge award kind (8)
pub fn is_badge_award_kind(kind: u16) -> bool {
    kind == KIND_BADGE_AWARD
}

/// Check if a kind is a profile badges kind (30008)
pub fn is_profile_badges_kind(kind: u16) -> bool {
    kind == KIND_PROFILE_BADGES
}

/// Check if a kind is any NIP-58 kind
pub fn is_nip58_kind(kind: u16) -> bool {
    is_badge_definition_kind(kind) || is_badge_award_kind(kind) || is_profile_badges_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_dimensions_parse() {
        let dims = ImageDimensions::parse("1024x1024").unwrap();
        assert_eq!(dims.width, 1024);
        assert_eq!(dims.height, 1024);

        let dims = ImageDimensions::parse("256x512").unwrap();
        assert_eq!(dims.width, 256);
        assert_eq!(dims.height, 512);

        assert!(ImageDimensions::parse("invalid").is_err());
        assert!(ImageDimensions::parse("1024").is_err());
        assert!(ImageDimensions::parse("1024x").is_err());
    }

    #[test]
    fn test_image_dimensions_to_string() {
        let dims = ImageDimensions::new(1024, 1024);
        assert_eq!(dims.to_string(), "1024x1024");

        let dims = ImageDimensions::new(256, 512);
        assert_eq!(dims.to_string(), "256x512");
    }

    #[test]
    fn test_badge_thumbnail() {
        let thumb = BadgeThumbnail::new("https://example.com/thumb.png".to_string())
            .with_dimensions(ImageDimensions::new(256, 256));

        assert_eq!(thumb.url, "https://example.com/thumb.png");
        assert_eq!(thumb.dimensions.unwrap().width, 256);
    }

    #[test]
    fn test_badge_definition_to_tags() {
        let def = BadgeDefinition::new("bravery".to_string())
            .with_name("Medal of Bravery".to_string())
            .with_description("Awarded to users demonstrating bravery".to_string())
            .with_image("https://example.com/bravery.png".to_string())
            .with_image_dimensions(ImageDimensions::new(1024, 1024))
            .add_thumbnail(
                BadgeThumbnail::new("https://example.com/bravery_256.png".to_string())
                    .with_dimensions(ImageDimensions::new(256, 256)),
            );

        let tags = def.to_tags();

        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "bravery"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "name" && t[1] == "Medal of Bravery")
        );
        assert!(tags.iter().any(|t| t[0] == "description"));
        assert!(tags.iter().any(|t| t[0] == "image" && t.len() == 3));
        assert!(tags.iter().any(|t| t[0] == "thumb" && t.len() == 3));
    }

    #[test]
    fn test_badge_definition_from_tags() {
        let tags = vec![
            vec!["d".to_string(), "bravery".to_string()],
            vec!["name".to_string(), "Medal of Bravery".to_string()],
            vec![
                "description".to_string(),
                "Awarded to users demonstrating bravery".to_string(),
            ],
            vec![
                "image".to_string(),
                "https://example.com/bravery.png".to_string(),
                "1024x1024".to_string(),
            ],
            vec![
                "thumb".to_string(),
                "https://example.com/bravery_256.png".to_string(),
                "256x256".to_string(),
            ],
        ];

        let def = BadgeDefinition::from_tags(&tags).unwrap();

        assert_eq!(def.identifier, "bravery");
        assert_eq!(def.name, Some("Medal of Bravery".to_string()));
        assert!(def.description.is_some());
        assert_eq!(
            def.image,
            Some("https://example.com/bravery.png".to_string())
        );
        assert_eq!(def.image_dimensions.unwrap().width, 1024);
        assert_eq!(def.thumbnails.len(), 1);
    }

    #[test]
    fn test_badge_definition_missing_d_tag() {
        let tags = vec![vec!["name".to_string(), "Test".to_string()]];
        let result = BadgeDefinition::from_tags(&tags);
        assert!(result.is_err());
    }

    #[test]
    fn test_badge_award_to_tags() {
        let award = BadgeAward::new("30009:alice:bravery".to_string())
            .add_pubkey_with_relay("bob".to_string(), "wss://relay1".to_string())
            .add_pubkey_with_relay("charlie".to_string(), "wss://relay2".to_string());

        let tags = award.to_tags();

        assert_eq!(tags[0], vec!["a", "30009:alice:bravery"]);
        assert_eq!(tags[1], vec!["p", "bob", "wss://relay1"]);
        assert_eq!(tags[2], vec!["p", "charlie", "wss://relay2"]);
    }

    #[test]
    fn test_badge_award_from_tags() {
        let tags = vec![
            vec!["a".to_string(), "30009:alice:bravery".to_string()],
            vec![
                "p".to_string(),
                "bob".to_string(),
                "wss://relay1".to_string(),
            ],
            vec![
                "p".to_string(),
                "charlie".to_string(),
                "wss://relay2".to_string(),
            ],
        ];

        let award = BadgeAward::from_tags(&tags).unwrap();

        assert_eq!(award.badge_definition, "30009:alice:bravery");
        assert_eq!(award.awarded_pubkeys.len(), 2);
        assert_eq!(award.awarded_pubkeys[0].0, "bob");
        assert_eq!(award.awarded_pubkeys[1].0, "charlie");
    }

    #[test]
    fn test_badge_award_missing_a_tag() {
        let tags = vec![vec!["p".to_string(), "bob".to_string()]];
        let result = BadgeAward::from_tags(&tags);
        assert!(result.is_err());
    }

    #[test]
    fn test_badge_award_missing_p_tag() {
        let tags = vec![vec!["a".to_string(), "30009:alice:bravery".to_string()]];
        let result = BadgeAward::from_tags(&tags);
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_badge_pair() {
        let pair = ProfileBadgePair::new("30009:alice:bravery".to_string(), "event123".to_string())
            .with_relay("wss://relay".to_string());

        assert_eq!(pair.badge_definition, "30009:alice:bravery");
        assert_eq!(pair.award_event_id, "event123");
        assert_eq!(pair.relay_hint, Some("wss://relay".to_string()));
    }

    #[test]
    fn test_profile_badges_to_tags() {
        let badges = ProfileBadges::new()
            .add_badge(
                ProfileBadgePair::new("30009:alice:bravery".to_string(), "event1".to_string())
                    .with_relay("wss://relay1".to_string()),
            )
            .add_badge(ProfileBadgePair::new(
                "30009:alice:honor".to_string(),
                "event2".to_string(),
            ));

        let tags = badges.to_tags();

        assert_eq!(tags[0], vec!["d", "profile_badges"]);
        assert_eq!(tags[1], vec!["a", "30009:alice:bravery"]);
        assert_eq!(tags[2], vec!["e", "event1", "wss://relay1"]);
        assert_eq!(tags[3], vec!["a", "30009:alice:honor"]);
        assert_eq!(tags[4], vec!["e", "event2"]);
    }

    #[test]
    fn test_profile_badges_from_tags() {
        let tags = vec![
            vec!["d".to_string(), "profile_badges".to_string()],
            vec!["a".to_string(), "30009:alice:bravery".to_string()],
            vec![
                "e".to_string(),
                "event1".to_string(),
                "wss://relay1".to_string(),
            ],
            vec!["a".to_string(), "30009:alice:honor".to_string()],
            vec!["e".to_string(), "event2".to_string()],
        ];

        let badges = ProfileBadges::from_tags(&tags).unwrap();

        assert_eq!(badges.badges.len(), 2);
        assert_eq!(badges.badges[0].badge_definition, "30009:alice:bravery");
        assert_eq!(badges.badges[0].award_event_id, "event1");
        assert_eq!(badges.badges[1].badge_definition, "30009:alice:honor");
        assert_eq!(badges.badges[1].award_event_id, "event2");
    }

    #[test]
    fn test_profile_badges_unpaired_tags() {
        let tags = vec![
            vec!["d".to_string(), "profile_badges".to_string()],
            vec!["a".to_string(), "30009:alice:bravery".to_string()],
            // Missing 'e' tag for first 'a'
            vec!["a".to_string(), "30009:alice:honor".to_string()],
            vec!["e".to_string(), "event2".to_string()],
            // Extra 'e' tag without 'a'
            vec!["e".to_string(), "event3".to_string()],
        ];

        let badges = ProfileBadges::from_tags(&tags).unwrap();

        // Should only include the properly paired badge
        assert_eq!(badges.badges.len(), 1);
        assert_eq!(badges.badges[0].badge_definition, "30009:alice:honor");
    }

    #[test]
    fn test_profile_badges_missing_d_tag() {
        let tags = vec![
            vec!["a".to_string(), "30009:alice:bravery".to_string()],
            vec!["e".to_string(), "event1".to_string()],
        ];
        let result = ProfileBadges::from_tags(&tags);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_badge_definition_kind() {
        assert!(is_badge_definition_kind(30009));
        assert!(!is_badge_definition_kind(8));
        assert!(!is_badge_definition_kind(30008));
    }

    #[test]
    fn test_is_badge_award_kind() {
        assert!(is_badge_award_kind(8));
        assert!(!is_badge_award_kind(30009));
        assert!(!is_badge_award_kind(30008));
    }

    #[test]
    fn test_is_profile_badges_kind() {
        assert!(is_profile_badges_kind(30008));
        assert!(!is_profile_badges_kind(8));
        assert!(!is_profile_badges_kind(30009));
    }

    #[test]
    fn test_is_nip58_kind() {
        assert!(is_nip58_kind(30009));
        assert!(is_nip58_kind(8));
        assert!(is_nip58_kind(30008));
        assert!(!is_nip58_kind(1));
    }
}
