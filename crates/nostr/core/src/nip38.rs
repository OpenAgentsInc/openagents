//! NIP-38: User Statuses
//!
//! This NIP defines kind 30315 for sharing live user statuses like what music they're
//! listening to or what they're currently doing.
//!
//! ## Features
//!
//! - General statuses (working, hiking, etc.)
//! - Music statuses (currently playing track)
//! - Custom status types
//! - Optional expiration (especially for music)
//! - Links to URLs, profiles, notes, or addressable events
//! - Emoji support
//!
//! ## Examples
//!
//! ```
//! use nostr::nip38::{UserStatus, StatusType};
//!
//! // General status
//! let status = UserStatus::new(StatusType::General, "Working on NIP-38");
//!
//! // Music status with expiration
//! let music = UserStatus::new(
//!     StatusType::Music,
//!     "Intergalactic - Beastie Boys"
//! )
//! .with_link("spotify:search:Intergalactic%20-%20Beastie%20Boys")
//! .with_expiration(1692845589);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for user status events.
pub const KIND_USER_STATUS: u64 = 30315;

/// D tag value for general statuses.
pub const STATUS_GENERAL: &str = "general";

/// D tag value for music statuses.
pub const STATUS_MUSIC: &str = "music";

/// Errors that can occur during NIP-38 operations.
#[derive(Debug, Error)]
pub enum Nip38Error {
    #[error("status type cannot be empty")]
    EmptyStatusType,
}

/// Check if a kind is a user status kind.
pub fn is_user_status_kind(kind: u64) -> bool {
    kind == KIND_USER_STATUS
}

/// Status type for user statuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusType {
    /// General status (working, hiking, etc.)
    General,
    /// Music status (currently playing)
    Music,
    /// Custom status type
    Custom(String),
}

impl StatusType {
    /// Get the d tag value for this status type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::General => STATUS_GENERAL,
            Self::Music => STATUS_MUSIC,
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Parse from a d tag value.
    pub fn from_str(s: &str) -> Self {
        match s {
            STATUS_GENERAL => Self::General,
            STATUS_MUSIC => Self::Music,
            _ => Self::Custom(s.to_string()),
        }
    }

    /// Check if this is a music status.
    pub fn is_music(&self) -> bool {
        matches!(self, Self::Music)
    }

    /// Check if this is a general status.
    pub fn is_general(&self) -> bool {
        matches!(self, Self::General)
    }
}

/// Reference link for a status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusLink {
    /// URL reference
    Url(String),
    /// Profile reference
    Profile {
        pubkey: String,
        relay: Option<String>,
    },
    /// Event reference
    Event { id: String, relay: Option<String> },
    /// Addressable event reference
    Address {
        address: String,
        relay: Option<String>,
    },
}

impl StatusLink {
    /// Create a URL link.
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

    /// Create a profile link.
    pub fn profile(pubkey: impl Into<String>, relay: Option<String>) -> Self {
        Self::Profile {
            pubkey: pubkey.into(),
            relay,
        }
    }

    /// Create an event link.
    pub fn event(id: impl Into<String>, relay: Option<String>) -> Self {
        Self::Event {
            id: id.into(),
            relay,
        }
    }

    /// Create an addressable event link.
    pub fn address(address: impl Into<String>, relay: Option<String>) -> Self {
        Self::Address {
            address: address.into(),
            relay,
        }
    }

    /// Convert to tag format.
    pub fn to_tag(&self) -> Vec<String> {
        match self {
            Self::Url(url) => vec!["r".to_string(), url.clone()],
            Self::Profile { pubkey, relay } => {
                let mut tag = vec!["p".to_string(), pubkey.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
            Self::Event { id, relay } => {
                let mut tag = vec!["e".to_string(), id.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
            Self::Address { address, relay } => {
                let mut tag = vec!["a".to_string(), address.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
        }
    }
}

/// A user status event (kind 30315).
///
/// Represents what a user is currently doing or listening to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserStatus {
    /// Type of status (general, music, or custom)
    pub status_type: StatusType,

    /// Status content (can be empty to clear status)
    /// Can include emoji or custom emoji (NIP-30)
    pub content: String,

    /// Optional link (URL, profile, event, or address)
    pub link: Option<StatusLink>,

    /// Optional expiration timestamp (unix seconds)
    /// For music: when the track stops playing
    pub expiration: Option<u64>,
}

impl UserStatus {
    /// Create a new user status.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip38::{UserStatus, StatusType};
    ///
    /// let status = UserStatus::new(StatusType::General, "Working on code");
    /// assert_eq!(status.content, "Working on code");
    /// ```
    pub fn new(status_type: StatusType, content: impl Into<String>) -> Self {
        Self {
            status_type,
            content: content.into(),
            link: None,
            expiration: None,
        }
    }

    /// Create a general status.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip38::UserStatus;
    ///
    /// let status = UserStatus::general("In a meeting");
    /// ```
    pub fn general(content: impl Into<String>) -> Self {
        Self::new(StatusType::General, content)
    }

    /// Create a music status.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip38::UserStatus;
    ///
    /// let status = UserStatus::music("Bohemian Rhapsody - Queen");
    /// ```
    pub fn music(content: impl Into<String>) -> Self {
        Self::new(StatusType::Music, content)
    }

    /// Create a custom status.
    pub fn custom(status_type: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(StatusType::Custom(status_type.into()), content)
    }

    /// Create an empty status (clears the status).
    pub fn clear(status_type: StatusType) -> Self {
        Self::new(status_type, "")
    }

    /// Set a link (builder pattern).
    pub fn with_link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(StatusLink::url(url));
        self
    }

    /// Set a link with StatusLink (builder pattern).
    pub fn with_status_link(mut self, link: StatusLink) -> Self {
        self.link = Some(link);
        self
    }

    /// Set expiration (builder pattern).
    pub fn with_expiration(mut self, expiration: u64) -> Self {
        self.expiration = Some(expiration);
        self
    }

    /// Check if this status is empty (cleared).
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Check if this status has expired.
    pub fn is_expired(&self, current_time: u64) -> bool {
        self.expiration
            .map(|exp| current_time > exp)
            .unwrap_or(false)
    }

    /// Validate the status.
    pub fn validate(&self) -> Result<(), Nip38Error> {
        if self.status_type.as_str().is_empty() {
            return Err(Nip38Error::EmptyStatusType);
        }
        Ok(())
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // d tag (status type)
        tags.push(vec!["d".to_string(), self.status_type.as_str().to_string()]);

        // Link tag (r, p, e, or a)
        if let Some(link) = &self.link {
            tags.push(link.to_tag());
        }

        // Expiration tag
        if let Some(expiration) = self.expiration {
            tags.push(vec!["expiration".to_string(), expiration.to_string()]);
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_user_status_kind() {
        assert!(is_user_status_kind(30315));
        assert!(!is_user_status_kind(1));
        assert!(!is_user_status_kind(30314));
    }

    #[test]
    fn test_status_type_general() {
        let status_type = StatusType::General;
        assert_eq!(status_type.as_str(), "general");
        assert!(status_type.is_general());
        assert!(!status_type.is_music());
    }

    #[test]
    fn test_status_type_music() {
        let status_type = StatusType::Music;
        assert_eq!(status_type.as_str(), "music");
        assert!(status_type.is_music());
        assert!(!status_type.is_general());
    }

    #[test]
    fn test_status_type_custom() {
        let status_type = StatusType::Custom("gaming".to_string());
        assert_eq!(status_type.as_str(), "gaming");
        assert!(!status_type.is_general());
        assert!(!status_type.is_music());
    }

    #[test]
    fn test_status_type_from_str() {
        assert_eq!(StatusType::from_str("general"), StatusType::General);
        assert_eq!(StatusType::from_str("music"), StatusType::Music);
        assert_eq!(
            StatusType::from_str("custom"),
            StatusType::Custom("custom".to_string())
        );
    }

    #[test]
    fn test_status_link_url() {
        let link = StatusLink::url("https://example.com");
        let tag = link.to_tag();
        assert_eq!(tag, vec!["r", "https://example.com"]);
    }

    #[test]
    fn test_status_link_profile() {
        let link = StatusLink::profile("pubkey123", Some("wss://relay.example.com".to_string()));
        let tag = link.to_tag();
        assert_eq!(tag, vec!["p", "pubkey123", "wss://relay.example.com"]);
    }

    #[test]
    fn test_status_link_event() {
        let link = StatusLink::event("event-id", None);
        let tag = link.to_tag();
        assert_eq!(tag, vec!["e", "event-id"]);
    }

    #[test]
    fn test_status_link_address() {
        let link = StatusLink::address("30023:pubkey:d-tag", None);
        let tag = link.to_tag();
        assert_eq!(tag, vec!["a", "30023:pubkey:d-tag"]);
    }

    #[test]
    fn test_user_status_new() {
        let status = UserStatus::new(StatusType::General, "Working on code");
        assert_eq!(status.status_type, StatusType::General);
        assert_eq!(status.content, "Working on code");
        assert_eq!(status.link, None);
        assert_eq!(status.expiration, None);
    }

    #[test]
    fn test_user_status_general() {
        let status = UserStatus::general("In a meeting");
        assert_eq!(status.status_type, StatusType::General);
        assert_eq!(status.content, "In a meeting");
    }

    #[test]
    fn test_user_status_music() {
        let status = UserStatus::music("Bohemian Rhapsody - Queen");
        assert_eq!(status.status_type, StatusType::Music);
        assert_eq!(status.content, "Bohemian Rhapsody - Queen");
    }

    #[test]
    fn test_user_status_custom() {
        let status = UserStatus::custom("gaming", "Playing chess");
        assert_eq!(status.status_type, StatusType::Custom("gaming".to_string()));
        assert_eq!(status.content, "Playing chess");
    }

    #[test]
    fn test_user_status_clear() {
        let status = UserStatus::clear(StatusType::General);
        assert!(status.is_empty());
    }

    #[test]
    fn test_user_status_with_link() {
        let status = UserStatus::general("Check this out").with_link("https://example.com");

        assert!(status.link.is_some());
        match status.link.unwrap() {
            StatusLink::Url(url) => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected URL link"),
        }
    }

    #[test]
    fn test_user_status_with_expiration() {
        let status = UserStatus::music("Song Title").with_expiration(1692845589);

        assert_eq!(status.expiration, Some(1692845589));
    }

    #[test]
    fn test_user_status_is_expired() {
        let status = UserStatus::music("Song").with_expiration(1000);

        assert!(!status.is_expired(500));
        assert!(!status.is_expired(1000));
        assert!(status.is_expired(1001));

        let status_no_exp = UserStatus::music("Song");
        assert!(!status_no_exp.is_expired(9999999));
    }

    #[test]
    fn test_user_status_validate() {
        let status = UserStatus::general("Working");
        assert!(status.validate().is_ok());

        let status = UserStatus::music("Song Title");
        assert!(status.validate().is_ok());
    }

    #[test]
    fn test_user_status_to_tags() {
        let status = UserStatus::general("Working")
            .with_link("https://nostr.world")
            .with_expiration(1692845589);

        let tags = status.to_tags();

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], vec!["d", "general"]);
        assert_eq!(tags[1], vec!["r", "https://nostr.world"]);
        assert_eq!(tags[2], vec!["expiration", "1692845589"]);
    }

    #[test]
    fn test_user_status_music_with_spotify() {
        let status = UserStatus::music("Intergalactic - Beastie Boys")
            .with_link("spotify:search:Intergalactic%20-%20Beastie%20Boys")
            .with_expiration(1692845589);

        let tags = status.to_tags();

        assert!(
            tags.iter()
                .any(|tag| tag[0] == "r" && tag[1].starts_with("spotify:"))
        );
        assert!(tags.iter().any(|tag| tag[0] == "expiration"));
    }

    #[test]
    fn test_user_status_with_emoji() {
        let status = UserStatus::general("Working 💻");
        assert_eq!(status.content, "Working 💻");
    }

    #[test]
    fn test_user_status_empty_content() {
        let status = UserStatus::new(StatusType::General, "");
        assert!(status.is_empty());
    }

    #[test]
    fn test_user_status_with_profile_link() {
        let link = StatusLink::profile("pubkey123", None);
        let status = UserStatus::general("Talking with Alice").with_status_link(link);

        let tags = status.to_tags();
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "p" && tag[1] == "pubkey123")
        );
    }

    #[test]
    fn test_user_status_with_event_link() {
        let link = StatusLink::event("event-id", Some("wss://relay.example.com".to_string()));
        let status = UserStatus::general("Reading this note").with_status_link(link);

        let tags = status.to_tags();
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "e"
            && tag[1] == "event-id"
            && tag[2] == "wss://relay.example.com"));
    }

    #[test]
    fn test_user_status_use_case_calendar() {
        // Calendar app updating status when in a meeting
        let status = UserStatus::general("In a meeting 📅")
            .with_link("https://calendar.example.com/meeting/123");

        assert_eq!(status.status_type, StatusType::General);
        assert!(status.content.contains("meeting"));
    }

    #[test]
    fn test_user_status_use_case_music_streaming() {
        // Music streaming service updating now playing
        let status = UserStatus::music("Stairway to Heaven - Led Zeppelin")
            .with_link("spotify:track:abcd1234")
            .with_expiration(1692845589);

        assert!(status.status_type.is_music());
        assert!(status.expiration.is_some());
    }

    #[test]
    fn test_user_status_use_case_podcast() {
        // Podcasting app with link to listen
        let link = StatusLink::address("30023:pubkey:podcast-episode-1", None);
        let status = UserStatus::music("Listening to Podcast Episode 1").with_status_link(link);

        let tags = status.to_tags();
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "a" && tag[1].contains("podcast"))
        );
    }
}
