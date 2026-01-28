//! NIP-35: Torrents
//!
//! Defines torrent index events for BitTorrent file sharing via Nostr.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/35.md>

use crate::Event;
use thiserror::Error;

/// Event kind for torrent index
pub const TORRENT_KIND: u16 = 2003;

/// Event kind for torrent comments
pub const TORRENT_COMMENT_KIND: u16 = 2004;

/// Tag name for BitTorrent info hash
pub const INFO_HASH_TAG: &str = "x";

/// Tag name for file entries
pub const FILE_TAG: &str = "file";

/// Tag name for trackers
pub const TRACKER_TAG: &str = "tracker";

/// Tag name for title
pub const TITLE_TAG: &str = "title";

/// Errors that can occur during NIP-35 operations
#[derive(Debug, Error)]
pub enum Nip35Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// A file entry in a torrent
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    /// File path within the torrent
    pub path: String,

    /// File size in bytes
    pub size: Option<u64>,
}

impl TorrentFile {
    /// Create a new torrent file entry
    pub fn new(path: String) -> Self {
        Self { path, size: None }
    }

    /// Create a torrent file with size
    pub fn with_size(path: String, size: u64) -> Self {
        Self {
            path,
            size: Some(size),
        }
    }

    /// Convert to tag array
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![FILE_TAG.to_string(), self.path.clone()];
        if let Some(size) = self.size {
            tag.push(size.to_string());
        }
        tag
    }

    /// Parse from a file tag
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip35Error> {
        if tag.is_empty() || tag[0] != FILE_TAG {
            return Err(Nip35Error::InvalidTag(format!(
                "expected file tag, got: {:?}",
                tag
            )));
        }

        if tag.len() < 2 {
            return Err(Nip35Error::InvalidTag(
                "file tag must have path".to_string(),
            ));
        }

        let path = tag[1].clone();
        let size = if tag.len() > 2 {
            Some(
                tag[2]
                    .parse()
                    .map_err(|_| Nip35Error::Parse(format!("invalid file size: {}", tag[2])))?,
            )
        } else {
            None
        };

        Ok(Self { path, size })
    }
}

/// A torrent event (kind 2003)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Torrent {
    pub event: Event,
    pub title: Option<String>,
    pub info_hash: String,
    pub files: Vec<TorrentFile>,
    pub trackers: Vec<String>,
}

impl Torrent {
    /// Create a torrent from an event
    pub fn from_event(event: Event) -> Result<Self, Nip35Error> {
        if event.kind != TORRENT_KIND {
            return Err(Nip35Error::InvalidKind {
                expected: TORRENT_KIND,
                actual: event.kind,
            });
        }

        // Find info hash (required)
        let mut info_hash = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == INFO_HASH_TAG && tag.len() > 1 {
                info_hash = Some(tag[1].clone());
                break;
            }
        }

        let info_hash =
            info_hash.ok_or_else(|| Nip35Error::MissingTag("info hash (x)".to_string()))?;

        // Find title (optional)
        let mut title = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == TITLE_TAG && tag.len() > 1 {
                title = Some(tag[1].clone());
                break;
            }
        }

        // Find all files
        let mut files = Vec::new();
        for tag in &event.tags {
            if !tag.is_empty()
                && tag[0] == FILE_TAG
                && let Ok(file) = TorrentFile::from_tag(tag)
            {
                files.push(file);
            }
        }

        // Find all trackers
        let mut trackers = Vec::new();
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == TRACKER_TAG && tag.len() > 1 {
                trackers.push(tag[1].clone());
            }
        }

        Ok(Self {
            event,
            title,
            info_hash,
            files,
            trackers,
        })
    }

    /// Get the torrent title
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Get the BitTorrent info hash
    pub fn get_info_hash(&self) -> &str {
        &self.info_hash
    }

    /// Get the description from event content
    pub fn get_description(&self) -> &str {
        &self.event.content
    }

    /// Get all files in the torrent
    pub fn get_files(&self) -> &[TorrentFile] {
        &self.files
    }

    /// Get all trackers
    pub fn get_trackers(&self) -> &[String] {
        &self.trackers
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Construct magnet link
    pub fn get_magnet_link(&self) -> String {
        let mut magnet = format!("magnet:?xt=urn:btih:{}", self.info_hash);

        // Add display name if available
        if let Some(title) = &self.title {
            magnet.push_str(&format!("&dn={}", urlencoding::encode(title)));
        }

        // Add trackers
        for tracker in &self.trackers {
            magnet.push_str(&format!("&tr={}", urlencoding::encode(tracker)));
        }

        magnet
    }

    /// Validate the torrent structure
    pub fn validate(&self) -> Result<(), Nip35Error> {
        if self.event.kind != TORRENT_KIND {
            return Err(Nip35Error::InvalidKind {
                expected: TORRENT_KIND,
                actual: self.event.kind,
            });
        }

        if self.info_hash.is_empty() {
            return Err(Nip35Error::MissingTag("info hash (x)".to_string()));
        }

        Ok(())
    }
}

/// Extract external content IDs from a torrent event
pub fn get_external_ids(event: &Event) -> Vec<(String, String)> {
    let mut ids = Vec::new();

    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "i" {
            ids.push((tag[1].clone(), tag.get(2).cloned().unwrap_or_default()));
        }
    }

    ids
}

/// Check if an event kind is a torrent
pub fn is_torrent_kind(kind: u16) -> bool {
    kind == TORRENT_KIND
}

/// Check if an event kind is a torrent comment
pub fn is_torrent_comment_kind(kind: u16) -> bool {
    kind == TORRENT_COMMENT_KIND
}

/// Check if an event kind is NIP-35 related
pub fn is_nip35_kind(kind: u16) -> bool {
    is_torrent_kind(kind) || is_torrent_comment_kind(kind)
}

/// Helper to create info hash tag
pub fn create_info_hash_tag(hash: String) -> Vec<String> {
    vec![INFO_HASH_TAG.to_string(), hash]
}

/// Helper to create title tag
pub fn create_title_tag(title: String) -> Vec<String> {
    vec![TITLE_TAG.to_string(), title]
}

/// Helper to create tracker tag
pub fn create_tracker_tag(tracker: String) -> Vec<String> {
    vec![TRACKER_TAG.to_string(), tracker]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_file_basic() {
        let file = TorrentFile::new("info/example.txt".to_string());
        assert_eq!(file.path, "info/example.txt");
        assert_eq!(file.size, None);
    }

    #[test]
    fn test_torrent_file_with_size() {
        let file = TorrentFile::with_size("video/movie.mkv".to_string(), 1073741824);
        assert_eq!(file.path, "video/movie.mkv");
        assert_eq!(file.size, Some(1073741824));
    }

    #[test]
    fn test_torrent_file_to_tag() {
        let file = TorrentFile::with_size("video/movie.mkv".to_string(), 1073741824);
        let tag = file.to_tag();

        assert_eq!(tag[0], "file");
        assert_eq!(tag[1], "video/movie.mkv");
        assert_eq!(tag[2], "1073741824");
    }

    #[test]
    fn test_torrent_file_from_tag() {
        let tag = vec![
            "file".to_string(),
            "video/movie.mkv".to_string(),
            "1073741824".to_string(),
        ];

        let file = TorrentFile::from_tag(&tag).unwrap();
        assert_eq!(file.path, "video/movie.mkv");
        assert_eq!(file.size, Some(1073741824));
    }

    #[test]
    fn test_torrent_file_from_tag_no_size() {
        let tag = vec!["file".to_string(), "info/readme.txt".to_string()];

        let file = TorrentFile::from_tag(&tag).unwrap();
        assert_eq!(file.path, "info/readme.txt");
        assert_eq!(file.size, None);
    }

    fn create_test_torrent_event(
        title: Option<&str>,
        info_hash: &str,
        files: Vec<TorrentFile>,
        trackers: Vec<&str>,
        description: &str,
    ) -> Event {
        let mut tags = vec![create_info_hash_tag(info_hash.to_string())];

        if let Some(t) = title {
            tags.push(create_title_tag(t.to_string()));
        }

        for file in files {
            tags.push(file.to_tag());
        }

        for tracker in trackers {
            tags.push(create_tracker_tag(tracker.to_string()));
        }

        Event {
            id: "torrent_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: TORRENT_KIND,
            tags,
            content: description.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_torrent_from_event() {
        let files = vec![
            TorrentFile::with_size("movie.mkv".to_string(), 1073741824),
            TorrentFile::with_size("subtitle.srt".to_string(), 50000),
        ];

        let event = create_test_torrent_event(
            Some("My Movie"),
            "abc123def456",
            files,
            vec!["udp://tracker.example.com:1337"],
            "A great movie!",
        );

        let torrent = Torrent::from_event(event).unwrap();

        assert_eq!(torrent.get_title(), Some("My Movie"));
        assert_eq!(torrent.get_info_hash(), "abc123def456");
        assert_eq!(torrent.get_description(), "A great movie!");
        assert_eq!(torrent.get_files().len(), 2);
        assert_eq!(torrent.get_trackers().len(), 1);
        assert_eq!(torrent.get_trackers()[0], "udp://tracker.example.com:1337");
    }

    #[test]
    fn test_torrent_magnet_link() {
        let event = create_test_torrent_event(
            Some("Test Movie"),
            "abc123",
            vec![],
            vec!["udp://tracker1.com:1337", "http://tracker2.com/announce"],
            "Description",
        );

        let torrent = Torrent::from_event(event).unwrap();
        let magnet = torrent.get_magnet_link();

        assert!(magnet.starts_with("magnet:?xt=urn:btih:abc123"));
        assert!(magnet.contains("&dn=Test%20Movie"));
        assert!(magnet.contains("&tr=udp%3A%2F%2Ftracker1.com%3A1337"));
        assert!(magnet.contains("&tr=http%3A%2F%2Ftracker2.com%2Fannounce"));
    }

    #[test]
    fn test_torrent_missing_info_hash() {
        let event = Event {
            id: "torrent_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: TORRENT_KIND,
            tags: vec![],
            content: "Description".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = Torrent::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip35Error::MissingTag(ref tag) if tag.contains("info hash")
        ));
    }

    #[test]
    fn test_torrent_invalid_kind() {
        let mut event =
            create_test_torrent_event(Some("Movie"), "abc123", vec![], vec![], "Description");
        event.kind = 1;

        let result = Torrent::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip35Error::InvalidKind {
                expected: TORRENT_KIND,
                actual: 1
            }
        ));
    }

    #[test]
    fn test_torrent_validate() {
        let event =
            create_test_torrent_event(Some("Movie"), "abc123", vec![], vec![], "Description");

        let torrent = Torrent::from_event(event).unwrap();
        assert!(torrent.validate().is_ok());
    }

    #[test]
    fn test_get_external_ids() {
        let event = Event {
            id: "torrent_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: TORRENT_KIND,
            tags: vec![
                vec!["x".to_string(), "abc123".to_string()],
                vec!["i".to_string(), "imdb:tt15239678".to_string()],
                vec!["i".to_string(), "tmdb:movie:693134".to_string()],
            ],
            content: "Description".to_string(),
            sig: "test_sig".to_string(),
        };

        let ids = get_external_ids(&event);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].0, "imdb:tt15239678");
        assert_eq!(ids[1].0, "tmdb:movie:693134");
    }

    #[test]
    fn test_is_torrent_kind() {
        assert!(is_torrent_kind(TORRENT_KIND));
        assert!(!is_torrent_kind(1));
        assert!(!is_torrent_kind(TORRENT_COMMENT_KIND));
    }

    #[test]
    fn test_is_torrent_comment_kind() {
        assert!(is_torrent_comment_kind(TORRENT_COMMENT_KIND));
        assert!(!is_torrent_comment_kind(1));
        assert!(!is_torrent_comment_kind(TORRENT_KIND));
    }

    #[test]
    fn test_is_nip35_kind() {
        assert!(is_nip35_kind(TORRENT_KIND));
        assert!(is_nip35_kind(TORRENT_COMMENT_KIND));
        assert!(!is_nip35_kind(1));
    }

    #[test]
    fn test_torrent_get_author() {
        let event = create_test_torrent_event(None, "abc123", vec![], vec![], "Description");

        let torrent = Torrent::from_event(event).unwrap();
        assert_eq!(torrent.get_author(), "author_pubkey");
    }

    #[test]
    fn test_torrent_get_created_at() {
        let event = create_test_torrent_event(None, "abc123", vec![], vec![], "Description");

        let torrent = Torrent::from_event(event).unwrap();
        assert_eq!(torrent.get_created_at(), 1234567890);
    }
}
