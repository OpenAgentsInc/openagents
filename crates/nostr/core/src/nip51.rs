//! NIP-51: Lists
//!
//! This module implements NIP-51, which defines lists of things that users can create.
//! Lists can contain references to anything, and these references can be public or private.
//!
//! ## List Types
//!
//! **Standard Lists** (Replaceable events - one per user):
//! - Follow list (kind 3)
//! - Mute list (kind 10000)
//! - Pinned notes (kind 10001)
//! - Bookmarks (kind 10003)
//! - Communities (kind 10004)
//! - Public chats (kind 10005)
//! - Blocked relays (kind 10006)
//! - Search relays (kind 10007)
//! - Interests (kind 10015)
//! - Emojis (kind 10030)
//!
//! **Sets** (Addressable events - multiple per user with 'd' identifier):
//! - Follow sets (kind 30000)
//! - Relay sets (kind 30002)
//! - Bookmark sets (kind 30003)
//! - Curation sets (kind 30004)
//! - Interest sets (kind 30015)
//! - Emoji sets (kind 30030)
//!
//! ## Public vs Private
//!
//! - **Public items**: Stored in event tags
//! - **Private items**: Encrypted using NIP-44 and stored in event content
//!
//! # Example
//!
//! ```
//! use nostr_core::nip51::{is_list_event, get_list_type, ListType};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! if is_list_event(event) {
//!     if let Some(list_type) = get_list_type(event) {
//!         println!("List type: {:?}", list_type);
//!     }
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

// Standard Lists (Replaceable Events)
/// Follow list (kind 3) - See NIP-02
pub const KIND_FOLLOW_LIST: u16 = 3;

/// Mute list (kind 10000)
pub const KIND_MUTE_LIST: u16 = 10000;

/// Pinned notes (kind 10001)
pub const KIND_PINNED_NOTES: u16 = 10001;

/// Read/write relays (kind 10002) - See NIP-65
pub const KIND_RELAY_LIST: u16 = 10002;

/// Bookmarks (kind 10003)
pub const KIND_BOOKMARKS: u16 = 10003;

/// Communities (kind 10004) - See NIP-72
pub const KIND_COMMUNITIES: u16 = 10004;

/// Public chats (kind 10005) - See NIP-28
pub const KIND_PUBLIC_CHATS: u16 = 10005;

/// Blocked relays (kind 10006)
pub const KIND_BLOCKED_RELAYS: u16 = 10006;

/// Search relays (kind 10007)
pub const KIND_SEARCH_RELAYS: u16 = 10007;

/// Simple groups (kind 10009) - See NIP-29
pub const KIND_SIMPLE_GROUPS: u16 = 10009;

/// Relay feeds (kind 10012)
pub const KIND_RELAY_FEEDS: u16 = 10012;

/// Interests (kind 10015)
pub const KIND_INTERESTS: u16 = 10015;

/// Media follows (kind 10020)
pub const KIND_MEDIA_FOLLOWS: u16 = 10020;

/// Emojis (kind 10030) - See NIP-30
pub const KIND_EMOJIS: u16 = 10030;

/// DM relays (kind 10050) - See NIP-17
pub const KIND_DM_RELAYS: u16 = 10050;

/// Wiki authors (kind 10101) - See NIP-54
pub const KIND_WIKI_AUTHORS: u16 = 10101;

/// Wiki relays (kind 10102) - See NIP-54
pub const KIND_WIKI_RELAYS: u16 = 10102;

// Sets (Addressable Events)
/// Follow sets (kind 30000)
pub const KIND_FOLLOW_SETS: u16 = 30000;

/// Relay sets (kind 30002)
pub const KIND_RELAY_SETS: u16 = 30002;

/// Bookmark sets (kind 30003)
pub const KIND_BOOKMARK_SETS: u16 = 30003;

/// Curation sets (kind 30004)
pub const KIND_CURATION_SETS: u16 = 30004;

/// Video curation sets (kind 30005)
pub const KIND_VIDEO_CURATION: u16 = 30005;

/// Kind mute sets (kind 30007)
pub const KIND_KIND_MUTE_SETS: u16 = 30007;

/// Interest sets (kind 30015)
pub const KIND_INTEREST_SETS: u16 = 30015;

/// Emoji sets (kind 30030)
pub const KIND_EMOJI_SETS: u16 = 30030;

/// Release artifact sets (kind 30063)
pub const KIND_RELEASE_ARTIFACTS: u16 = 30063;

/// App curation sets (kind 30267)
pub const KIND_APP_CURATION: u16 = 30267;

/// Calendar event sets (kind 31924)
pub const KIND_CALENDAR: u16 = 31924;

/// Starter packs (kind 39089)
pub const KIND_STARTER_PACKS: u16 = 39089;

/// Media starter packs (kind 39092)
pub const KIND_MEDIA_STARTER_PACKS: u16 = 39092;

/// Errors that can occur during NIP-51 operations.
#[derive(Debug, Error)]
pub enum Nip51Error {
    #[error("invalid list event: not a valid list kind")]
    InvalidListKind,

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid encrypted content")]
    InvalidEncryptedContent,

    #[error("encryption/decryption error: {0}")]
    EncryptionError(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Types of lists defined in NIP-51
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListType {
    // Standard lists
    FollowList,
    MuteList,
    PinnedNotes,
    RelayList,
    Bookmarks,
    Communities,
    PublicChats,
    BlockedRelays,
    SearchRelays,
    SimpleGroups,
    RelayFeeds,
    Interests,
    MediaFollows,
    Emojis,
    DmRelays,
    WikiAuthors,
    WikiRelays,

    // Sets
    FollowSets,
    RelaySets,
    BookmarkSets,
    CurationSets,
    VideoCuration,
    KindMuteSets,
    InterestSets,
    EmojiSets,
    ReleaseArtifacts,
    AppCuration,
    Calendar,
    StarterPacks,
    MediaStarterPacks,
}

impl ListType {
    /// Get the event kind for this list type
    pub fn kind(&self) -> u16 {
        match self {
            ListType::FollowList => KIND_FOLLOW_LIST,
            ListType::MuteList => KIND_MUTE_LIST,
            ListType::PinnedNotes => KIND_PINNED_NOTES,
            ListType::RelayList => KIND_RELAY_LIST,
            ListType::Bookmarks => KIND_BOOKMARKS,
            ListType::Communities => KIND_COMMUNITIES,
            ListType::PublicChats => KIND_PUBLIC_CHATS,
            ListType::BlockedRelays => KIND_BLOCKED_RELAYS,
            ListType::SearchRelays => KIND_SEARCH_RELAYS,
            ListType::SimpleGroups => KIND_SIMPLE_GROUPS,
            ListType::RelayFeeds => KIND_RELAY_FEEDS,
            ListType::Interests => KIND_INTERESTS,
            ListType::MediaFollows => KIND_MEDIA_FOLLOWS,
            ListType::Emojis => KIND_EMOJIS,
            ListType::DmRelays => KIND_DM_RELAYS,
            ListType::WikiAuthors => KIND_WIKI_AUTHORS,
            ListType::WikiRelays => KIND_WIKI_RELAYS,
            ListType::FollowSets => KIND_FOLLOW_SETS,
            ListType::RelaySets => KIND_RELAY_SETS,
            ListType::BookmarkSets => KIND_BOOKMARK_SETS,
            ListType::CurationSets => KIND_CURATION_SETS,
            ListType::VideoCuration => KIND_VIDEO_CURATION,
            ListType::KindMuteSets => KIND_KIND_MUTE_SETS,
            ListType::InterestSets => KIND_INTEREST_SETS,
            ListType::EmojiSets => KIND_EMOJI_SETS,
            ListType::ReleaseArtifacts => KIND_RELEASE_ARTIFACTS,
            ListType::AppCuration => KIND_APP_CURATION,
            ListType::Calendar => KIND_CALENDAR,
            ListType::StarterPacks => KIND_STARTER_PACKS,
            ListType::MediaStarterPacks => KIND_MEDIA_STARTER_PACKS,
        }
    }

    /// Check if this list type is a set (addressable event)
    pub fn is_set(&self) -> bool {
        matches!(
            self,
            ListType::FollowSets
                | ListType::RelaySets
                | ListType::BookmarkSets
                | ListType::CurationSets
                | ListType::VideoCuration
                | ListType::KindMuteSets
                | ListType::InterestSets
                | ListType::EmojiSets
                | ListType::ReleaseArtifacts
                | ListType::AppCuration
                | ListType::Calendar
                | ListType::StarterPacks
                | ListType::MediaStarterPacks
        )
    }
}

/// Check if an event is a list event
pub fn is_list_event(event: &Event) -> bool {
    matches!(
        event.kind,
        KIND_FOLLOW_LIST
            | KIND_MUTE_LIST
            | KIND_PINNED_NOTES
            | KIND_RELAY_LIST
            | KIND_BOOKMARKS
            | KIND_COMMUNITIES
            | KIND_PUBLIC_CHATS
            | KIND_BLOCKED_RELAYS
            | KIND_SEARCH_RELAYS
            | KIND_SIMPLE_GROUPS
            | KIND_RELAY_FEEDS
            | KIND_INTERESTS
            | KIND_MEDIA_FOLLOWS
            | KIND_EMOJIS
            | KIND_DM_RELAYS
            | KIND_WIKI_AUTHORS
            | KIND_WIKI_RELAYS
            | KIND_FOLLOW_SETS
            | KIND_RELAY_SETS
            | KIND_BOOKMARK_SETS
            | KIND_CURATION_SETS
            | KIND_VIDEO_CURATION
            | KIND_KIND_MUTE_SETS
            | KIND_INTEREST_SETS
            | KIND_EMOJI_SETS
            | KIND_RELEASE_ARTIFACTS
            | KIND_APP_CURATION
            | KIND_CALENDAR
            | KIND_STARTER_PACKS
            | KIND_MEDIA_STARTER_PACKS
    )
}

/// Get the list type for an event
pub fn get_list_type(event: &Event) -> Option<ListType> {
    match event.kind {
        KIND_FOLLOW_LIST => Some(ListType::FollowList),
        KIND_MUTE_LIST => Some(ListType::MuteList),
        KIND_PINNED_NOTES => Some(ListType::PinnedNotes),
        KIND_RELAY_LIST => Some(ListType::RelayList),
        KIND_BOOKMARKS => Some(ListType::Bookmarks),
        KIND_COMMUNITIES => Some(ListType::Communities),
        KIND_PUBLIC_CHATS => Some(ListType::PublicChats),
        KIND_BLOCKED_RELAYS => Some(ListType::BlockedRelays),
        KIND_SEARCH_RELAYS => Some(ListType::SearchRelays),
        KIND_SIMPLE_GROUPS => Some(ListType::SimpleGroups),
        KIND_RELAY_FEEDS => Some(ListType::RelayFeeds),
        KIND_INTERESTS => Some(ListType::Interests),
        KIND_MEDIA_FOLLOWS => Some(ListType::MediaFollows),
        KIND_EMOJIS => Some(ListType::Emojis),
        KIND_DM_RELAYS => Some(ListType::DmRelays),
        KIND_WIKI_AUTHORS => Some(ListType::WikiAuthors),
        KIND_WIKI_RELAYS => Some(ListType::WikiRelays),
        KIND_FOLLOW_SETS => Some(ListType::FollowSets),
        KIND_RELAY_SETS => Some(ListType::RelaySets),
        KIND_BOOKMARK_SETS => Some(ListType::BookmarkSets),
        KIND_CURATION_SETS => Some(ListType::CurationSets),
        KIND_VIDEO_CURATION => Some(ListType::VideoCuration),
        KIND_KIND_MUTE_SETS => Some(ListType::KindMuteSets),
        KIND_INTEREST_SETS => Some(ListType::InterestSets),
        KIND_EMOJI_SETS => Some(ListType::EmojiSets),
        KIND_RELEASE_ARTIFACTS => Some(ListType::ReleaseArtifacts),
        KIND_APP_CURATION => Some(ListType::AppCuration),
        KIND_CALENDAR => Some(ListType::Calendar),
        KIND_STARTER_PACKS => Some(ListType::StarterPacks),
        KIND_MEDIA_STARTER_PACKS => Some(ListType::MediaStarterPacks),
        _ => None,
    }
}

/// Get the public items from a list event
///
/// Returns all tags that are not metadata tags (title, description, image, d)
pub fn get_public_items(event: &Event) -> Vec<Vec<String>> {
    event
        .tags
        .iter()
        .filter(|tag| {
            let tag_name = tag.get(0).map(|s| s.as_str());
            !matches!(
                tag_name,
                Some("title") | Some("description") | Some("image") | Some("d")
            )
        })
        .cloned()
        .collect()
}

/// Get the title of a list/set
pub fn get_title(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("title"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the description of a list/set
pub fn get_description(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("description"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the image URL of a list/set
pub fn get_image(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("image"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the 'd' identifier for a set
pub fn get_set_identifier(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("d"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Check if the encrypted content uses NIP-04 or NIP-44
///
/// Returns true if NIP-04 (legacy), false if NIP-44 (current)
pub fn is_nip04_encryption(content: &str) -> bool {
    content.contains("?iv=")
}

/// Create metadata tags for a list/set
///
/// # Arguments
///
/// * `title` - Optional title
/// * `description` - Optional description
/// * `image` - Optional image URL
///
/// # Returns
///
/// Vector of metadata tags
pub fn create_metadata_tags(
    title: Option<&str>,
    description: Option<&str>,
    image: Option<&str>,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    if let Some(t) = title {
        tags.push(vec!["title".to_string(), t.to_string()]);
    }

    if let Some(d) = description {
        tags.push(vec!["description".to_string(), d.to_string()]);
    }

    if let Some(i) = image {
        tags.push(vec!["image".to_string(), i.to_string()]);
    }

    tags
}

/// Create a set identifier tag
pub fn create_set_identifier_tag(identifier: &str) -> Vec<String> {
    vec!["d".to_string(), identifier.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "0".repeat(64),
            pubkey: "0".repeat(64),
            created_at: 1000000,
            kind,
            tags,
            content: String::new(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_is_list_event() {
        assert!(is_list_event(&create_test_event(KIND_FOLLOW_LIST, vec![])));
        assert!(is_list_event(&create_test_event(KIND_MUTE_LIST, vec![])));
        assert!(is_list_event(&create_test_event(KIND_BOOKMARKS, vec![])));
        assert!(is_list_event(&create_test_event(KIND_FOLLOW_SETS, vec![])));
        assert!(!is_list_event(&create_test_event(1, vec![])));
    }

    #[test]
    fn test_get_list_type() {
        assert_eq!(
            get_list_type(&create_test_event(KIND_FOLLOW_LIST, vec![])),
            Some(ListType::FollowList)
        );
        assert_eq!(
            get_list_type(&create_test_event(KIND_BOOKMARKS, vec![])),
            Some(ListType::Bookmarks)
        );
        assert_eq!(
            get_list_type(&create_test_event(KIND_FOLLOW_SETS, vec![])),
            Some(ListType::FollowSets)
        );
        assert_eq!(get_list_type(&create_test_event(1, vec![])), None);
    }

    #[test]
    fn test_list_type_kind() {
        assert_eq!(ListType::FollowList.kind(), KIND_FOLLOW_LIST);
        assert_eq!(ListType::Bookmarks.kind(), KIND_BOOKMARKS);
        assert_eq!(ListType::FollowSets.kind(), KIND_FOLLOW_SETS);
    }

    #[test]
    fn test_list_type_is_set() {
        assert!(!ListType::FollowList.is_set());
        assert!(!ListType::Bookmarks.is_set());
        assert!(ListType::FollowSets.is_set());
        assert!(ListType::BookmarkSets.is_set());
    }

    #[test]
    fn test_get_public_items() {
        let event = create_test_event(
            KIND_BOOKMARKS,
            vec![
                vec!["e".to_string(), "event1".to_string()],
                vec!["title".to_string(), "My Bookmarks".to_string()],
                vec!["a".to_string(), "30023:pubkey:article".to_string()],
            ],
        );

        let items = get_public_items(&event);
        assert_eq!(items.len(), 2);
        assert!(items.contains(&vec!["e".to_string(), "event1".to_string()]));
        assert!(items.contains(&vec!["a".to_string(), "30023:pubkey:article".to_string()]));
    }

    #[test]
    fn test_get_title() {
        let event = create_test_event(
            KIND_BOOKMARKS,
            vec![vec!["title".to_string(), "My Bookmarks".to_string()]],
        );
        assert_eq!(get_title(&event), Some("My Bookmarks".to_string()));
    }

    #[test]
    fn test_get_description() {
        let event = create_test_event(
            KIND_BOOKMARKS,
            vec![vec![
                "description".to_string(),
                "Saved articles".to_string(),
            ]],
        );
        assert_eq!(get_description(&event), Some("Saved articles".to_string()));
    }

    #[test]
    fn test_get_image() {
        let event = create_test_event(
            KIND_BOOKMARKS,
            vec![vec![
                "image".to_string(),
                "https://example.com/img.png".to_string(),
            ]],
        );
        assert_eq!(
            get_image(&event),
            Some("https://example.com/img.png".to_string())
        );
    }

    #[test]
    fn test_get_set_identifier() {
        let event = create_test_event(
            KIND_FOLLOW_SETS,
            vec![vec!["d".to_string(), "tech-people".to_string()]],
        );
        assert_eq!(get_set_identifier(&event), Some("tech-people".to_string()));
    }

    #[test]
    fn test_is_nip04_encryption() {
        assert!(is_nip04_encryption("base64content?iv=base64iv"));
        assert!(!is_nip04_encryption("base64content"));
    }

    #[test]
    fn test_create_metadata_tags() {
        let tags = create_metadata_tags(
            Some("My List"),
            Some("A cool list"),
            Some("https://example.com/img.png"),
        );

        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&vec!["title".to_string(), "My List".to_string()]));
        assert!(tags.contains(&vec!["description".to_string(), "A cool list".to_string()]));
        assert!(tags.contains(&vec![
            "image".to_string(),
            "https://example.com/img.png".to_string()
        ]));
    }

    #[test]
    fn test_create_set_identifier_tag() {
        let tag = create_set_identifier_tag("my-set");
        assert_eq!(tag, vec!["d", "my-set"]);
    }
}
