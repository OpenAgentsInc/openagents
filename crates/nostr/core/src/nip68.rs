//! NIP-68: Picture-first Feeds
//!
//! Defines kind 20 events for picture-first clients (Instagram-like feeds).
//! Images are self-contained and referenced using imeta tags.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/68.md>

use crate::Event;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

/// Event kind for picture posts
pub const PICTURE_KIND: u16 = 20;

/// Allowed image media types for picture events
pub const ALLOWED_MEDIA_TYPES: &[&str] = &[
    "image/apng", // Animated Portable Network Graphics
    "image/avif", // AV1 Image File Format
    "image/gif",  // Graphics Interchange Format
    "image/jpeg", // Joint Photographic Expert Group
    "image/png",  // Portable Network Graphics
    "image/webp", // Web Picture format
];

/// Errors that can occur during NIP-68 operations
#[derive(Debug, Error)]
pub enum Nip68Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid media type: {0}")]
    InvalidMediaType(String),

    #[error("invalid annotation format: {0}")]
    InvalidAnnotation(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// User annotation with position in image
#[derive(Debug, Clone, PartialEq)]
pub struct UserAnnotation {
    pub pubkey: String,
    pub pos_x: f64,
    pub pos_y: f64,
}

impl UserAnnotation {
    pub fn new(pubkey: String, pos_x: f64, pos_y: f64) -> Self {
        Self {
            pubkey,
            pos_x,
            pos_y,
        }
    }

}

impl std::str::FromStr for UserAnnotation {
    type Err = Nip68Error;

    /// Parse from annotate-user tag value: `<pubkey>:<posX>:<posY>`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(Nip68Error::InvalidAnnotation(format!(
                "expected format <pubkey>:<posX>:<posY>, got: {}",
                s
            )));
        }

        let pubkey = parts[0].to_string();
        let pos_x = parts[1]
            .parse::<f64>()
            .map_err(|_| Nip68Error::InvalidAnnotation(format!("invalid posX: {}", parts[1])))?;
        let pos_y = parts[2]
            .parse::<f64>()
            .map_err(|_| Nip68Error::InvalidAnnotation(format!("invalid posY: {}", parts[2])))?;

        Ok(Self {
            pubkey,
            pos_x,
            pos_y,
        })
    }
}

impl std::fmt::Display for UserAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.pubkey, self.pos_x, self.pos_y)
    }
}

/// Picture event (kind 20)
#[derive(Debug, Clone, PartialEq)]
pub struct PictureEvent {
    pub event: Event,
    pub title: Option<String>,
    pub description: String,
    pub images: Vec<HashMap<String, String>>,
    pub annotations: Vec<UserAnnotation>,
}

impl PictureEvent {
    pub fn from_event(event: Event) -> Result<Self, Nip68Error> {
        if event.kind != PICTURE_KIND {
            return Err(Nip68Error::InvalidKind {
                expected: PICTURE_KIND,
                actual: event.kind,
            });
        }

        let description = event.content.clone();
        let mut title = None;
        let mut images = Vec::new();
        let mut annotations = Vec::new();

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "title" if tag.len() >= 2 => {
                    title = Some(tag[1].clone());
                }
                "imeta" => {
                    // Parse imeta tag fields
                    let mut image_data = HashMap::new();
                    for item in &tag[1..] {
                        if let Some((key, value)) = item.split_once(' ') {
                            image_data.insert(key.to_string(), value.to_string());
                        } else if item.starts_with("annotate-user ")
                            && let Some(annotation_str) = item.strip_prefix("annotate-user ")
                            && let Ok(annotation) = UserAnnotation::from_str(annotation_str)
                        {
                            annotations.push(annotation);
                        }
                    }
                    if !image_data.is_empty() {
                        images.push(image_data);
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            event,
            title,
            description,
            images,
            annotations,
        })
    }

    /// Get the title of the picture post
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Get the description
    pub fn get_description(&self) -> &str {
        &self.description
    }

    /// Get all images
    pub fn get_images(&self) -> &[HashMap<String, String>] {
        &self.images
    }

    /// Get user annotations
    pub fn get_annotations(&self) -> &[UserAnnotation] {
        &self.annotations
    }

    /// Get content warning if present
    pub fn get_content_warning(&self) -> Option<&str> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 2 && tag[0] == "content-warning" {
                Some(tag[1].as_str())
            } else {
                None
            }
        })
    }

    /// Get hashtags
    pub fn get_hashtags(&self) -> Vec<&str> {
        self.event
            .tags
            .iter()
            .filter_map(|tag| {
                if tag.len() >= 2 && tag[0] == "t" {
                    Some(tag[1].as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get location
    pub fn get_location(&self) -> Option<&str> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 2 && tag[0] == "location" {
                Some(tag[1].as_str())
            } else {
                None
            }
        })
    }

    /// Get geohash
    pub fn get_geohash(&self) -> Option<&str> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 2 && tag[0] == "g" {
                Some(tag[1].as_str())
            } else {
                None
            }
        })
    }

    /// Get tagged users (p tags)
    pub fn get_tagged_users(&self) -> Vec<&str> {
        self.event
            .tags
            .iter()
            .filter_map(|tag| {
                if tag.len() >= 2 && tag[0] == "p" {
                    Some(tag[1].as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get media types (m tags)
    pub fn get_media_types(&self) -> Vec<&str> {
        self.event
            .tags
            .iter()
            .filter_map(|tag| {
                if tag.len() >= 2 && tag[0] == "m" {
                    Some(tag[1].as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get language (ISO-639-1)
    pub fn get_language(&self) -> Option<&str> {
        self.event.tags.iter().find_map(|tag| {
            if tag.len() >= 3 && tag[0] == "l" && tag[2] == "ISO-639-1" {
                Some(tag[1].as_str())
            } else {
                None
            }
        })
    }

    /// Validate media types are allowed
    pub fn validate_media_types(&self) -> Result<(), Nip68Error> {
        let media_types = self.get_media_types();
        for media_type in media_types {
            if !is_allowed_media_type(media_type) {
                return Err(Nip68Error::InvalidMediaType(media_type.to_string()));
            }
        }
        Ok(())
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }
}

/// Check if a media type is allowed
pub fn is_allowed_media_type(media_type: &str) -> bool {
    ALLOWED_MEDIA_TYPES.contains(&media_type)
}

/// Check if an event kind is a picture kind
pub fn is_picture_kind(kind: u16) -> bool {
    kind == PICTURE_KIND
}

/// Create title tag
#[allow(dead_code)]
pub fn create_title_tag(title: String) -> Vec<String> {
    vec!["title".to_string(), title]
}

/// Create content-warning tag
pub fn create_content_warning_tag(reason: String) -> Vec<String> {
    vec!["content-warning".to_string(), reason]
}

/// Create location tag
pub fn create_location_tag(location: String) -> Vec<String> {
    vec!["location".to_string(), location]
}

/// Create geohash tag
pub fn create_geohash_tag(geohash: String) -> Vec<String> {
    vec!["g".to_string(), geohash]
}

/// Create language tags (L and l)
pub fn create_language_tags(language: String) -> Vec<Vec<String>> {
    vec![
        vec!["L".to_string(), "ISO-639-1".to_string()],
        vec!["l".to_string(), language, "ISO-639-1".to_string()],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, content: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_user_annotation() {
        let annotation = UserAnnotation::new("pubkey123".to_string(), 100.5, 200.75);
        assert_eq!(annotation.pubkey, "pubkey123");
        assert_eq!(annotation.pos_x, 100.5);
        assert_eq!(annotation.pos_y, 200.75);
    }

    #[test]
    fn test_user_annotation_from_str() {
        assert!(matches!(
            UserAnnotation::from_str("pubkey123:100.5:200.75"),
            Ok(annotation)
                if annotation.pubkey == "pubkey123"
                    && annotation.pos_x == 100.5
                    && annotation.pos_y == 200.75
        ));
    }

    #[test]
    fn test_user_annotation_to_string() {
        let annotation = UserAnnotation::new("pubkey123".to_string(), 100.5, 200.75);
        assert_eq!(annotation.to_string(), "pubkey123:100.5:200.75");
    }

    #[test]
    fn test_user_annotation_invalid() {
        assert!(UserAnnotation::from_str("invalid").is_err());
        assert!(UserAnnotation::from_str("pubkey:abc:200").is_err());
    }

    #[test]
    fn test_picture_event_basic() {
        let tags = vec![
            vec!["title".to_string(), "My Picture".to_string()],
            vec![
                "imeta".to_string(),
                "url https://example.com/image.jpg".to_string(),
                "m image/jpeg".to_string(),
            ],
        ];

        let event = create_test_event(PICTURE_KIND, "A beautiful sunset", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert_eq!(picture.get_title(), Some("My Picture"));
        assert_eq!(picture.get_description(), "A beautiful sunset");
        assert_eq!(picture.get_images().len(), 1);
    }

    #[test]
    fn test_picture_event_multiple_images() {
        let tags = vec![
            vec!["title".to_string(), "Gallery".to_string()],
            vec![
                "imeta".to_string(),
                "url https://example.com/image1.jpg".to_string(),
                "m image/jpeg".to_string(),
            ],
            vec![
                "imeta".to_string(),
                "url https://example.com/image2.png".to_string(),
                "m image/png".to_string(),
            ],
        ];

        let event = create_test_event(PICTURE_KIND, "Photo gallery", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert_eq!(picture.get_images().len(), 2);
    }

    #[test]
    fn test_picture_event_content_warning() {
        let tags = vec![vec!["content-warning".to_string(), "NSFW".to_string()]];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert_eq!(picture.get_content_warning(), Some("NSFW"));
    }

    #[test]
    fn test_picture_event_hashtags() {
        let tags = vec![
            vec!["t".to_string(), "photography".to_string()],
            vec!["t".to_string(), "nature".to_string()],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        let hashtags = picture.get_hashtags();
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"photography"));
        assert!(hashtags.contains(&"nature"));
    }

    #[test]
    fn test_picture_event_location() {
        let tags = vec![
            vec!["location".to_string(), "San Francisco, CA".to_string()],
            vec!["g".to_string(), "9q8yy".to_string()],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert_eq!(picture.get_location(), Some("San Francisco, CA"));
        assert_eq!(picture.get_geohash(), Some("9q8yy"));
    }

    #[test]
    fn test_picture_event_tagged_users() {
        let tags = vec![
            vec!["p".to_string(), "pubkey1".to_string()],
            vec![
                "p".to_string(),
                "pubkey2".to_string(),
                "wss://relay.com".to_string(),
            ],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        let users = picture.get_tagged_users();
        assert_eq!(users.len(), 2);
        assert!(users.contains(&"pubkey1"));
        assert!(users.contains(&"pubkey2"));
    }

    #[test]
    fn test_picture_event_media_types() {
        let tags = vec![
            vec!["m".to_string(), "image/jpeg".to_string()],
            vec!["m".to_string(), "image/png".to_string()],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        let media_types = picture.get_media_types();
        assert_eq!(media_types.len(), 2);
        assert!(media_types.contains(&"image/jpeg"));
        assert!(media_types.contains(&"image/png"));
    }

    #[test]
    fn test_picture_event_language() {
        let tags = vec![
            vec!["L".to_string(), "ISO-639-1".to_string()],
            vec!["l".to_string(), "en".to_string(), "ISO-639-1".to_string()],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert_eq!(picture.get_language(), Some("en"));
    }

    #[test]
    fn test_picture_event_validate_media_types() {
        let tags = vec![
            vec!["m".to_string(), "image/jpeg".to_string()],
            vec!["m".to_string(), "image/png".to_string()],
        ];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert!(picture.validate_media_types().is_ok());
    }

    #[test]
    fn test_picture_event_validate_invalid_media_type() {
        let tags = vec![vec!["m".to_string(), "image/invalid".to_string()]];

        let event = create_test_event(PICTURE_KIND, "Content", tags);
        let picture = PictureEvent::from_event(event).unwrap();

        assert!(picture.validate_media_types().is_err());
    }

    #[test]
    fn test_picture_event_invalid_kind() {
        let event = create_test_event(1, "Content", vec![]);
        let result = PictureEvent::from_event(event);

        assert!(result.is_err());
    }

    #[test]
    fn test_is_allowed_media_type() {
        assert!(is_allowed_media_type("image/jpeg"));
        assert!(is_allowed_media_type("image/png"));
        assert!(is_allowed_media_type("image/gif"));
        assert!(is_allowed_media_type("image/webp"));
        assert!(is_allowed_media_type("image/avif"));
        assert!(is_allowed_media_type("image/apng"));
        assert!(!is_allowed_media_type("image/invalid"));
        assert!(!is_allowed_media_type("video/mp4"));
    }

    #[test]
    fn test_is_picture_kind() {
        assert!(is_picture_kind(PICTURE_KIND));
        assert!(!is_picture_kind(1));
    }

    #[test]
    fn test_create_tags() {
        let title_tag = create_title_tag("My Picture".to_string());
        assert_eq!(title_tag, vec!["title", "My Picture"]);

        let cw_tag = create_content_warning_tag("NSFW".to_string());
        assert_eq!(cw_tag, vec!["content-warning", "NSFW"]);

        let location_tag = create_location_tag("New York".to_string());
        assert_eq!(location_tag, vec!["location", "New York"]);

        let geohash_tag = create_geohash_tag("9q8yy".to_string());
        assert_eq!(geohash_tag, vec!["g", "9q8yy"]);

        let language_tags = create_language_tags("en".to_string());
        assert_eq!(language_tags.len(), 2);
        assert_eq!(language_tags[0], vec!["L", "ISO-639-1"]);
        assert_eq!(language_tags[1], vec!["l", "en", "ISO-639-1"]);
    }

    #[test]
    fn test_picture_event_get_author() {
        let event = create_test_event(PICTURE_KIND, "Content", vec![]);
        let picture = PictureEvent::from_event(event).unwrap();
        assert_eq!(picture.get_author(), "test_pubkey");
    }

    #[test]
    fn test_picture_event_get_created_at() {
        let event = create_test_event(PICTURE_KIND, "Content", vec![]);
        let picture = PictureEvent::from_event(event).unwrap();
        assert_eq!(picture.get_created_at(), 1234567890);
    }
}
