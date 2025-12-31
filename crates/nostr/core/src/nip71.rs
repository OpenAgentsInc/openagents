//! NIP-71: Video Events
//!
//! This NIP defines video events representing dedicated posts of externally hosted
//! video content. These events are meant for video-specific clients (like YouTube/TikTok).
//!
//! ## Event Types
//!
//! - **Normal Video** (kind 21): Longer, mostly horizontal/landscape videos
//! - **Short Video** (kind 22): Short-form, mostly vertical/portrait videos (stories, reels)
//!
//! ## Example
//!
//! ```
//! use nostr::nip71::{VideoEvent, VideoVariant};
//!
//! // Create a video event
//! let mut video = VideoEvent::new_normal("My Video");
//! video.published_at = Some(1686840000);
//! video.add_variant(VideoVariant {
//!     url: "https://example.com/video.mp4".to_string(),
//!     dimensions: Some("1920x1080".to_string()),
//!     hash: Some("abc123...".to_string()),
//!     mime_type: Some("video/mp4".to_string()),
//!     duration: Some(120.5),
//!     bitrate: Some(3000000),
//!     ..Default::default()
//! });
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for normal video events
pub const KIND_VIDEO: u16 = 21;

/// Kind for short video events (stories, reels, shorts)
pub const KIND_SHORT_VIDEO: u16 = 22;

/// Errors that can occur during NIP-71 operations.
#[derive(Debug, Error)]
pub enum Nip71Error {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid segment format: {0}")]
    InvalidSegmentFormat(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// A variant of a video (different resolution, format, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VideoVariant {
    /// Primary video URL
    pub url: String,
    /// Video dimensions (e.g., "1920x1080")
    pub dimensions: Option<String>,
    /// SHA-256 hash of the video file (hex)
    pub hash: Option<String>,
    /// MIME type (e.g., "video/mp4")
    pub mime_type: Option<String>,
    /// Duration in seconds
    pub duration: Option<f64>,
    /// Average bitrate in bits/sec
    pub bitrate: Option<u64>,
    /// Preview image URLs
    pub images: Vec<String>,
    /// Fallback URLs for the video
    pub fallbacks: Vec<String>,
    /// Service identifier (e.g., "nip96")
    pub service: Option<String>,
}

impl VideoVariant {
    /// Create a new video variant
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set dimensions
    pub fn with_dimensions(mut self, dimensions: impl Into<String>) -> Self {
        self.dimensions = Some(dimensions.into());
        self
    }

    /// Set hash
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = Some(hash.into());
        self
    }

    /// Set MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Set bitrate
    pub fn with_bitrate(mut self, bitrate: u64) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Convert to imeta tag
    pub fn to_imeta_tag(&self) -> Vec<String> {
        let mut tag = vec!["imeta".to_string()];

        if let Some(dim) = &self.dimensions {
            tag.push(format!("dim {}", dim));
        }

        tag.push(format!("url {}", self.url));

        if let Some(hash) = &self.hash {
            tag.push(format!("x {}", hash));
        }

        if let Some(mime_type) = &self.mime_type {
            tag.push(format!("m {}", mime_type));
        }

        for image in &self.images {
            tag.push(format!("image {}", image));
        }

        for fallback in &self.fallbacks {
            tag.push(format!("fallback {}", fallback));
        }

        if let Some(service) = &self.service {
            tag.push(format!("service {}", service));
        }

        if let Some(bitrate) = self.bitrate {
            tag.push(format!("bitrate {}", bitrate));
        }

        if let Some(duration) = self.duration {
            tag.push(format!("duration {}", duration));
        }

        tag
    }
}

/// Video segment (chapter)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSegment {
    /// Start timestamp (HH:MM:SS.sss)
    pub start: String,
    /// End timestamp (HH:MM:SS.sss)
    pub end: String,
    /// Chapter/segment title
    pub title: String,
    /// Optional thumbnail URL
    pub thumbnail: Option<String>,
}

impl VideoSegment {
    /// Create a new video segment
    pub fn new(start: impl Into<String>, end: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            title: title.into(),
            thumbnail: None,
        }
    }

    /// Set thumbnail URL
    pub fn with_thumbnail(mut self, thumbnail: impl Into<String>) -> Self {
        self.thumbnail = Some(thumbnail.into());
        self
    }

    /// Convert to tag
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![
            "segment".to_string(),
            self.start.clone(),
            self.end.clone(),
            self.title.clone(),
        ];

        if let Some(thumbnail) = &self.thumbnail {
            tag.push(thumbnail.clone());
        }

        tag
    }
}

/// Text track (captions, subtitles, chapters, metadata)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextTrack {
    /// Link to WebVTT file
    pub url: String,
    /// Type of supplementary information
    pub track_type: String,
    /// Optional language code
    pub language: Option<String>,
}

impl TextTrack {
    /// Create a new text track
    pub fn new(url: impl Into<String>, track_type: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            track_type: track_type.into(),
            language: None,
        }
    }

    /// Set language code
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Convert to tag
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![
            "text-track".to_string(),
            self.url.clone(),
            self.track_type.clone(),
        ];

        if let Some(language) = &self.language {
            tag.push(language.clone());
        }

        tag
    }
}

/// Video event (kind 21 for normal, kind 22 for short)
///
/// Represents a dedicated post of externally hosted video content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoEvent {
    /// Event kind (21 for normal, 22 for short)
    pub kind: u16,
    /// Title of the video (required)
    pub title: String,
    /// Timestamp when video was first published
    pub published_at: Option<u64>,
    /// Description for accessibility
    pub alt: Option<String>,
    /// Video variants (different resolutions, formats)
    pub variants: Vec<VideoVariant>,
    /// Text tracks (captions, subtitles, etc.)
    pub text_tracks: Vec<TextTrack>,
    /// Content warning
    pub content_warning: Option<String>,
    /// Video segments/chapters
    pub segments: Vec<VideoSegment>,
    /// Hashtags
    pub hashtags: Vec<String>,
    /// Participant pubkeys with optional relays
    pub participants: Vec<(String, Option<String>)>,
    /// Reference URLs
    pub references: Vec<String>,
}

impl VideoEvent {
    /// Create a new normal video event (kind 21)
    pub fn new_normal(title: impl Into<String>) -> Self {
        Self {
            kind: KIND_VIDEO,
            title: title.into(),
            published_at: None,
            alt: None,
            variants: Vec::new(),
            text_tracks: Vec::new(),
            content_warning: None,
            segments: Vec::new(),
            hashtags: Vec::new(),
            participants: Vec::new(),
            references: Vec::new(),
        }
    }

    /// Create a new short video event (kind 22)
    pub fn new_short(title: impl Into<String>) -> Self {
        Self {
            kind: KIND_SHORT_VIDEO,
            title: title.into(),
            published_at: None,
            alt: None,
            variants: Vec::new(),
            text_tracks: Vec::new(),
            content_warning: None,
            segments: Vec::new(),
            hashtags: Vec::new(),
            participants: Vec::new(),
            references: Vec::new(),
        }
    }

    /// Add a video variant
    pub fn add_variant(&mut self, variant: VideoVariant) {
        self.variants.push(variant);
    }

    /// Add a text track
    pub fn add_text_track(&mut self, track: TextTrack) {
        self.text_tracks.push(track);
    }

    /// Add a segment
    pub fn add_segment(&mut self, segment: VideoSegment) {
        self.segments.push(segment);
    }

    /// Add a participant
    pub fn add_participant(&mut self, pubkey: impl Into<String>, relay: Option<String>) {
        self.participants.push((pubkey.into(), relay));
    }

    /// Validate the video event
    pub fn validate(&self) -> Result<(), Nip71Error> {
        if self.title.is_empty() {
            return Err(Nip71Error::MissingField("title".to_string()));
        }
        if self.kind != KIND_VIDEO && self.kind != KIND_SHORT_VIDEO {
            return Err(Nip71Error::MissingField(
                "kind must be 21 or 22".to_string(),
            ));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["title".to_string(), self.title.clone()]];

        if let Some(published_at) = self.published_at {
            tags.push(vec!["published_at".to_string(), published_at.to_string()]);
        }

        if let Some(alt) = &self.alt {
            tags.push(vec!["alt".to_string(), alt.clone()]);
        }

        // Add imeta tags for each variant
        for variant in &self.variants {
            tags.push(variant.to_imeta_tag());
        }

        // Add text tracks
        for track in &self.text_tracks {
            tags.push(track.to_tag());
        }

        if let Some(warning) = &self.content_warning {
            tags.push(vec!["content-warning".to_string(), warning.clone()]);
        }

        // Add segments
        for segment in &self.segments {
            tags.push(segment.to_tag());
        }

        // Add hashtags
        for hashtag in &self.hashtags {
            tags.push(vec!["t".to_string(), hashtag.clone()]);
        }

        // Add participants
        for (pubkey, relay) in &self.participants {
            let mut tag = vec!["p".to_string(), pubkey.clone()];
            if let Some(relay_url) = relay {
                tag.push(relay_url.clone());
            }
            tags.push(tag);
        }

        // Add references
        for reference in &self.references {
            tags.push(vec!["r".to_string(), reference.clone()]);
        }

        tags
    }
}

/// Check if a kind is a video event kind
pub fn is_video_kind(kind: u16) -> bool {
    matches!(kind, KIND_VIDEO | KIND_SHORT_VIDEO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_variant_new() {
        let variant = VideoVariant::new("https://example.com/video.mp4");
        assert_eq!(variant.url, "https://example.com/video.mp4");
    }

    #[test]
    fn test_video_variant_to_imeta_tag() {
        let variant = VideoVariant::new("https://example.com/video.mp4")
            .with_dimensions("1920x1080")
            .with_hash("abc123")
            .with_mime_type("video/mp4")
            .with_duration(120.5)
            .with_bitrate(3000000);

        let tag = variant.to_imeta_tag();
        assert_eq!(tag[0], "imeta");
        assert!(tag.contains(&"dim 1920x1080".to_string()));
        assert!(tag.contains(&"url https://example.com/video.mp4".to_string()));
        assert!(tag.contains(&"x abc123".to_string()));
        assert!(tag.contains(&"m video/mp4".to_string()));
        assert!(tag.contains(&"bitrate 3000000".to_string()));
        assert!(tag.contains(&"duration 120.5".to_string()));
    }

    #[test]
    fn test_video_segment() {
        let segment = VideoSegment::new("00:00:00.000", "00:01:30.000", "Introduction")
            .with_thumbnail("https://example.com/thumb.jpg");

        let tag = segment.to_tag();
        assert_eq!(tag[0], "segment");
        assert_eq!(tag[1], "00:00:00.000");
        assert_eq!(tag[2], "00:01:30.000");
        assert_eq!(tag[3], "Introduction");
        assert_eq!(tag[4], "https://example.com/thumb.jpg");
    }

    #[test]
    fn test_text_track() {
        let track =
            TextTrack::new("https://example.com/captions.vtt", "captions").with_language("en");

        let tag = track.to_tag();
        assert_eq!(tag[0], "text-track");
        assert_eq!(tag[1], "https://example.com/captions.vtt");
        assert_eq!(tag[2], "captions");
        assert_eq!(tag[3], "en");
    }

    #[test]
    fn test_video_event_new_normal() {
        let video = VideoEvent::new_normal("My Video");
        assert_eq!(video.kind, KIND_VIDEO);
        assert_eq!(video.title, "My Video");
    }

    #[test]
    fn test_video_event_new_short() {
        let video = VideoEvent::new_short("My Short");
        assert_eq!(video.kind, KIND_SHORT_VIDEO);
        assert_eq!(video.title, "My Short");
    }

    #[test]
    fn test_video_event_validate() {
        let video = VideoEvent::new_normal("Test Video");
        assert!(video.validate().is_ok());

        let mut invalid = VideoEvent::new_normal("");
        assert!(invalid.validate().is_err());

        invalid.title = "Valid".to_string();
        invalid.kind = 999;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_video_event_to_tags() {
        let mut video = VideoEvent::new_normal("Test Video");
        video.published_at = Some(1686840000);
        video.alt = Some("A test video".to_string());
        video.add_variant(
            VideoVariant::new("https://example.com/video.mp4").with_dimensions("1920x1080"),
        );
        video.add_text_track(TextTrack::new("https://example.com/subs.vtt", "subtitles"));
        video.add_segment(VideoSegment::new("00:00:00", "00:01:00", "Intro"));
        video.hashtags.push("test".to_string());
        video.add_participant("pubkey123", Some("wss://relay.example.com".to_string()));
        video.references.push("https://example.com".to_string());

        let tags = video.to_tags();

        assert!(tags.contains(&vec!["title".to_string(), "Test Video".to_string()]));
        assert!(tags.contains(&vec!["published_at".to_string(), "1686840000".to_string()]));
        assert!(tags.contains(&vec!["alt".to_string(), "A test video".to_string()]));
        assert!(tags.contains(&vec!["t".to_string(), "test".to_string()]));
        assert!(tags.contains(&vec!["r".to_string(), "https://example.com".to_string()]));

        // Check imeta tag exists
        assert!(tags.iter().any(|tag| tag[0] == "imeta"));

        // Check text-track tag
        assert!(tags.iter().any(|tag| tag[0] == "text-track"));

        // Check segment tag
        assert!(tags.iter().any(|tag| tag[0] == "segment"));
    }

    #[test]
    fn test_is_video_kind() {
        assert!(is_video_kind(KIND_VIDEO));
        assert!(is_video_kind(KIND_SHORT_VIDEO));
        assert!(!is_video_kind(1));
        assert!(!is_video_kind(999));
    }

    #[test]
    fn test_video_variant_with_images_and_fallbacks() {
        let mut variant = VideoVariant::new("https://example.com/video.mp4");
        variant
            .images
            .push("https://example.com/thumb1.jpg".to_string());
        variant
            .images
            .push("https://example.com/thumb2.jpg".to_string());
        variant
            .fallbacks
            .push("https://backup.example.com/video.mp4".to_string());
        variant.service = Some("nip96".to_string());

        let tag = variant.to_imeta_tag();
        assert!(tag.contains(&"image https://example.com/thumb1.jpg".to_string()));
        assert!(tag.contains(&"image https://example.com/thumb2.jpg".to_string()));
        assert!(tag.contains(&"fallback https://backup.example.com/video.mp4".to_string()));
        assert!(tag.contains(&"service nip96".to_string()));
    }

    #[test]
    fn test_video_event_content_warning() {
        let mut video = VideoEvent::new_normal("NSFW Video");
        video.content_warning = Some("adult content".to_string());

        let tags = video.to_tags();
        assert!(tags.contains(&vec![
            "content-warning".to_string(),
            "adult content".to_string()
        ]));
    }
}
