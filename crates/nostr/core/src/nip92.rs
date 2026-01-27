//! NIP-92: Media Attachments
//!
//! Defines inline media metadata tags for attaching rich media information
//! to URLs in event content.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/92.md>

use crate::Event;
use std::collections::HashMap;
use thiserror::Error;

/// Tag name for inline media metadata
pub const IMETA_TAG: &str = "imeta";

/// Errors that can occur during NIP-92 operations
#[derive(Debug, Error)]
pub enum Nip92Error {
    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid field format: {0}")]
    InvalidField(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Inline media metadata for a URL in event content
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAttachment {
    /// The media URL (required)
    pub url: String,

    /// Additional metadata fields
    pub fields: HashMap<String, String>,
}

impl MediaAttachment {
    /// Create a new media attachment with a URL
    pub fn new(url: String) -> Self {
        Self {
            url,
            fields: HashMap::new(),
        }
    }

    /// Set the MIME type (m field)
    pub fn with_mime_type(mut self, mime_type: String) -> Self {
        self.fields.insert("m".to_string(), mime_type);
        self
    }

    /// Set the dimensions (dim field)
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.fields
            .insert("dim".to_string(), format!("{}x{}", width, height));
        self
    }

    /// Set the blurhash
    pub fn with_blurhash(mut self, blurhash: String) -> Self {
        self.fields.insert("blurhash".to_string(), blurhash);
        self
    }

    /// Set the alt text
    pub fn with_alt(mut self, alt: String) -> Self {
        self.fields.insert("alt".to_string(), alt);
        self
    }

    /// Set the hash (x field from NIP-94)
    pub fn with_hash(mut self, hash: String) -> Self {
        self.fields.insert("x".to_string(), hash);
        self
    }

    /// Add a fallback URL
    pub fn add_fallback(mut self, fallback_url: String) -> Self {
        // For fallbacks, we'll store them with a key like "fallback_0", "fallback_1", etc.
        let idx = self
            .fields
            .keys()
            .filter(|k| k.starts_with("fallback_"))
            .count();
        self.fields
            .insert(format!("fallback_{}", idx), fallback_url);
        self
    }

    /// Add a custom field
    pub fn with_field(mut self, key: String, value: String) -> Self {
        self.fields.insert(key, value);
        self
    }

    /// Get the MIME type if present
    pub fn get_mime_type(&self) -> Option<&str> {
        self.fields.get("m").map(|s| s.as_str())
    }

    /// Get the dimensions if present (returns (width, height))
    pub fn get_dimensions(&self) -> Option<(u32, u32)> {
        let dim = self.fields.get("dim")?;
        let parts: Vec<&str> = dim.split('x').collect();
        if parts.len() != 2 {
            return None;
        }
        let width = parts[0].parse().ok()?;
        let height = parts[1].parse().ok()?;
        Some((width, height))
    }

    /// Get the blurhash if present
    pub fn get_blurhash(&self) -> Option<&str> {
        self.fields.get("blurhash").map(|s| s.as_str())
    }

    /// Get the alt text if present
    pub fn get_alt(&self) -> Option<&str> {
        self.fields.get("alt").map(|s| s.as_str())
    }

    /// Get the hash if present
    pub fn get_hash(&self) -> Option<&str> {
        self.fields.get("x").map(|s| s.as_str())
    }

    /// Get all fallback URLs
    pub fn get_fallbacks(&self) -> Vec<&str> {
        let mut fallbacks: Vec<(usize, &str)> = self
            .fields
            .iter()
            .filter_map(|(k, v)| {
                if let Some(idx_str) = k.strip_prefix("fallback_")
                    && let Ok(idx) = idx_str.parse::<usize>()
                {
                    return Some((idx, v.as_str()));
                }
                None
            })
            .collect();
        fallbacks.sort_by_key(|(idx, _)| *idx);
        fallbacks.into_iter().map(|(_, url)| url).collect()
    }

    /// Convert to imeta tag format
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![IMETA_TAG.to_string()];

        // URL is always first after the tag name
        tag.push(format!("url {}", self.url));

        // Add all other fields
        for (key, value) in &self.fields {
            if key.starts_with("fallback_") {
                // Extract the actual fallback URL
                tag.push(format!("fallback {}", value));
            } else {
                tag.push(format!("{} {}", key, value));
            }
        }

        tag
    }

    /// Parse from an imeta tag
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip92Error> {
        if tag.is_empty() || tag[0] != IMETA_TAG {
            return Err(Nip92Error::InvalidTag(format!(
                "expected imeta tag, got: {:?}",
                tag
            )));
        }

        if tag.len() < 3 {
            return Err(Nip92Error::InvalidTag(
                "imeta tag must have url and at least one other field".to_string(),
            ));
        }

        let mut url = None;
        let mut fields = HashMap::new();
        let mut fallback_idx = 0;

        // Parse all fields (skipping the tag name)
        for item in &tag[1..] {
            let parts: Vec<&str> = item.splitn(2, ' ').collect();
            if parts.len() != 2 {
                return Err(Nip92Error::InvalidField(format!("invalid field: {}", item)));
            }

            let key = parts[0];
            let value = parts[1].to_string();

            if key == "url" {
                url = Some(value);
            } else if key == "fallback" {
                fields.insert(format!("fallback_{}", fallback_idx), value);
                fallback_idx += 1;
            } else {
                fields.insert(key.to_string(), value);
            }
        }

        let url = url.ok_or_else(|| Nip92Error::MissingField("url".to_string()))?;

        if fields.is_empty() {
            return Err(Nip92Error::InvalidTag(
                "imeta must have at least one field besides url".to_string(),
            ));
        }

        Ok(Self { url, fields })
    }
}

/// Extract all media attachments from an event
pub fn get_media_attachments(event: &Event) -> Vec<MediaAttachment> {
    let mut attachments = Vec::new();

    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == IMETA_TAG
            && let Ok(attachment) = MediaAttachment::from_tag(tag)
        {
            attachments.push(attachment);
        }
    }

    attachments
}

/// Add a media attachment to an event's tags
pub fn add_media_attachment(tags: &mut Vec<Vec<String>>, attachment: MediaAttachment) {
    tags.push(attachment.to_tag());
}

/// Check if an event has media attachments
pub fn has_media_attachments(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| !tag.is_empty() && tag[0] == IMETA_TAG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_attachment_basic() {
        let attachment = MediaAttachment::new("https://example.com/image.jpg".to_string())
            .with_mime_type("image/jpeg".to_string())
            .with_dimensions(1920, 1080)
            .with_alt("A test image".to_string());

        assert_eq!(attachment.url, "https://example.com/image.jpg");
        assert_eq!(attachment.get_mime_type(), Some("image/jpeg"));
        assert_eq!(attachment.get_dimensions(), Some((1920, 1080)));
        assert_eq!(attachment.get_alt(), Some("A test image"));
    }

    #[test]
    fn test_media_attachment_with_blurhash() {
        let attachment = MediaAttachment::new("https://example.com/image.jpg".to_string())
            .with_blurhash("eVF$^OI:${M{o#*0-nNF".to_string());

        assert_eq!(attachment.get_blurhash(), Some("eVF$^OI:${M{o#*0-nNF"));
    }

    #[test]
    fn test_media_attachment_with_hash() {
        let attachment = MediaAttachment::new("https://example.com/image.jpg".to_string())
            .with_hash("abc123def456".to_string());

        assert_eq!(attachment.get_hash(), Some("abc123def456"));
    }

    #[test]
    fn test_media_attachment_with_fallbacks() {
        let attachment = MediaAttachment::new("https://example.com/image.jpg".to_string())
            .with_mime_type("image/jpeg".to_string())
            .add_fallback("https://fallback1.com/image.jpg".to_string())
            .add_fallback("https://fallback2.com/image.jpg".to_string());

        let fallbacks = attachment.get_fallbacks();
        assert_eq!(fallbacks.len(), 2);
        assert_eq!(fallbacks[0], "https://fallback1.com/image.jpg");
        assert_eq!(fallbacks[1], "https://fallback2.com/image.jpg");
    }

    #[test]
    fn test_media_attachment_to_tag() {
        let attachment = MediaAttachment::new("https://nostr.build/i/my-image.jpg".to_string())
            .with_mime_type("image/jpeg".to_string())
            .with_dimensions(3024, 4032)
            .with_alt("A scenic photo overlooking the coast of Costa Rica".to_string());

        let tag = attachment.to_tag();

        assert_eq!(tag[0], "imeta");
        assert_eq!(tag[1], "url https://nostr.build/i/my-image.jpg");
        assert!(tag.contains(&"m image/jpeg".to_string()));
        assert!(tag.contains(&"dim 3024x4032".to_string()));
        assert!(
            tag.contains(&"alt A scenic photo overlooking the coast of Costa Rica".to_string())
        );
    }

    #[test]
    fn test_media_attachment_from_tag() {
        let tag = vec![
            "imeta".to_string(),
            "url https://nostr.build/i/my-image.jpg".to_string(),
            "m image/jpeg".to_string(),
            "dim 3024x4032".to_string(),
            "alt A scenic photo".to_string(),
        ];

        let attachment = MediaAttachment::from_tag(&tag).unwrap();

        assert_eq!(attachment.url, "https://nostr.build/i/my-image.jpg");
        assert_eq!(attachment.get_mime_type(), Some("image/jpeg"));
        assert_eq!(attachment.get_dimensions(), Some((3024, 4032)));
        assert_eq!(attachment.get_alt(), Some("A scenic photo"));
    }

    #[test]
    fn test_media_attachment_from_tag_with_fallbacks() {
        let tag = vec![
            "imeta".to_string(),
            "url https://example.com/image.jpg".to_string(),
            "m image/jpeg".to_string(),
            "fallback https://fallback1.com/image.jpg".to_string(),
            "fallback https://fallback2.com/image.jpg".to_string(),
        ];

        let attachment = MediaAttachment::from_tag(&tag).unwrap();

        let fallbacks = attachment.get_fallbacks();
        assert_eq!(fallbacks.len(), 2);
        assert_eq!(fallbacks[0], "https://fallback1.com/image.jpg");
        assert_eq!(fallbacks[1], "https://fallback2.com/image.jpg");
    }

    #[test]
    fn test_media_attachment_from_tag_missing_url() {
        let tag = vec![
            "imeta".to_string(),
            "m image/jpeg".to_string(),
            "dim 1920x1080".to_string(),
        ];

        let result = MediaAttachment::from_tag(&tag);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip92Error::MissingField(ref field) if field == "url"
        ));
    }

    #[test]
    fn test_media_attachment_from_tag_no_extra_fields() {
        let tag = vec![
            "imeta".to_string(),
            "url https://example.com/image.jpg".to_string(),
        ];

        let result = MediaAttachment::from_tag(&tag);
        assert!(result.is_err());
    }

    #[test]
    fn test_media_attachment_from_tag_invalid() {
        let tag = vec!["other".to_string(), "value".to_string()];

        let result = MediaAttachment::from_tag(&tag);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_media_attachments() {
        let event = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![
                vec![
                    "imeta".to_string(),
                    "url https://example.com/image1.jpg".to_string(),
                    "m image/jpeg".to_string(),
                ],
                vec![
                    "imeta".to_string(),
                    "url https://example.com/image2.jpg".to_string(),
                    "m image/png".to_string(),
                ],
            ],
            content: "Check out these images!".to_string(),
            sig: "test_sig".to_string(),
        };

        let attachments = get_media_attachments(&event);
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0].url, "https://example.com/image1.jpg");
        assert_eq!(attachments[1].url, "https://example.com/image2.jpg");
    }

    #[test]
    fn test_add_media_attachment() {
        let mut tags = Vec::new();
        let attachment = MediaAttachment::new("https://example.com/image.jpg".to_string())
            .with_mime_type("image/jpeg".to_string());

        add_media_attachment(&mut tags, attachment);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "imeta");
        assert_eq!(tags[0][1], "url https://example.com/image.jpg");
    }

    #[test]
    fn test_has_media_attachments() {
        let event_with_media = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec![
                "imeta".to_string(),
                "url https://example.com/image.jpg".to_string(),
                "m image/jpeg".to_string(),
            ]],
            content: "Image!".to_string(),
            sig: "test_sig".to_string(),
        };

        let event_without_media = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "No images".to_string(),
            sig: "test_sig".to_string(),
        };

        assert!(has_media_attachments(&event_with_media));
        assert!(!has_media_attachments(&event_without_media));
    }

    #[test]
    fn test_media_attachment_custom_field() {
        let attachment = MediaAttachment::new("https://example.com/video.mp4".to_string())
            .with_field("duration".to_string(), "120".to_string())
            .with_field("codec".to_string(), "h264".to_string());

        assert_eq!(attachment.fields.get("duration"), Some(&"120".to_string()));
        assert_eq!(attachment.fields.get("codec"), Some(&"h264".to_string()));
    }
}
