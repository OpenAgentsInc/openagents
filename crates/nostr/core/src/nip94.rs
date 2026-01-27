//! NIP-94: File Metadata
//!
//! Implements file metadata events (kind 1063) for organizing and classifying shared files.
//! Supports various file types with metadata including:
//! - URL, MIME type, hash (SHA-256)
//! - Size, dimensions, thumbnails
//! - Torrent/magnet links
//! - Accessibility descriptions
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/94.md>

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for file metadata
pub const FILE_METADATA_KIND: u16 = 1063;

/// Errors that can occur during NIP-94 operations
#[derive(Debug, Error)]
pub enum Nip94Error {
    #[error("missing required field: {0}")]
    MissingRequired(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),

    #[error("invalid dimensions: {0}")]
    InvalidDimensions(String),

    #[error("invalid hash: {0}")]
    InvalidHash(String),
}

/// Represents file dimensions in pixels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    /// Create new dimensions
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Parse dimensions from string format "WxH"
    ///
    /// # Example
    /// ```ignore
    /// let dim = Dimensions::parse("1920x1080")?;
    /// assert_eq!(dim.width, 1920);
    /// assert_eq!(dim.height, 1080);
    /// ```
    pub fn parse(s: &str) -> Result<Self, Nip94Error> {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() != 2 {
            return Err(Nip94Error::InvalidDimensions(format!(
                "expected format 'WxH', got '{}'",
                s
            )));
        }

        let width = parts[0]
            .parse()
            .map_err(|e| Nip94Error::InvalidDimensions(format!("invalid width: {}", e)))?;
        let height = parts[1]
            .parse()
            .map_err(|e| Nip94Error::InvalidDimensions(format!("invalid height: {}", e)))?;

        Ok(Self { width, height })
    }

}

impl std::fmt::Display for Dimensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Represents a file thumbnail or preview image
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileImage {
    pub url: String,
    pub hash: Option<String>,
}

impl FileImage {
    /// Create new file image
    pub fn new(url: String) -> Self {
        Self { url, hash: None }
    }

    /// Create new file image with hash
    pub fn with_hash(url: String, hash: String) -> Self {
        Self {
            url,
            hash: Some(hash),
        }
    }
}

/// Complete file metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Description/caption of the file
    pub content: String,

    /// URL to download the file
    pub url: String,

    /// MIME type (lowercase)
    pub mime_type: String,

    /// SHA-256 hash of the file (hex)
    pub hash: String,

    /// SHA-256 hash of original file before transformations (hex)
    pub original_hash: Option<String>,

    /// Size in bytes
    pub size: Option<u64>,

    /// Dimensions in pixels (for images/videos)
    pub dimensions: Option<Dimensions>,

    /// Magnet URI for torrents
    pub magnet: Option<String>,

    /// Torrent infohash
    pub infohash: Option<String>,

    /// Blurhash for preview
    pub blurhash: Option<String>,

    /// Thumbnail URL with optional hash
    pub thumbnail: Option<FileImage>,

    /// Preview image URL with optional hash
    pub image: Option<FileImage>,

    /// Text excerpt/summary
    pub summary: Option<String>,

    /// Accessibility description
    pub alt: Option<String>,

    /// Fallback file URLs
    pub fallbacks: Vec<String>,

    /// Service type (e.g., NIP-96 server)
    pub service: Option<String>,
}

impl FileMetadata {
    /// Create new file metadata with required fields
    ///
    /// # Arguments
    /// * `url` - URL to download the file
    /// * `mime_type` - MIME type (will be converted to lowercase)
    /// * `hash` - SHA-256 hash (hex string)
    pub fn new(url: String, mime_type: String, hash: String) -> Self {
        Self {
            content: String::new(),
            url,
            mime_type: mime_type.to_lowercase(),
            hash,
            original_hash: None,
            size: None,
            dimensions: None,
            magnet: None,
            infohash: None,
            blurhash: None,
            thumbnail: None,
            image: None,
            summary: None,
            alt: None,
            fallbacks: Vec::new(),
            service: None,
        }
    }

    /// Set content/caption
    pub fn with_content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    /// Set original hash
    pub fn with_original_hash(mut self, hash: String) -> Self {
        self.original_hash = Some(hash);
        self
    }

    /// Set file size in bytes
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Set dimensions
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.dimensions = Some(Dimensions::new(width, height));
        self
    }

    /// Set magnet URI
    pub fn with_magnet(mut self, magnet: String) -> Self {
        self.magnet = Some(magnet);
        self
    }

    /// Set torrent infohash
    pub fn with_infohash(mut self, infohash: String) -> Self {
        self.infohash = Some(infohash);
        self
    }

    /// Set blurhash
    pub fn with_blurhash(mut self, blurhash: String) -> Self {
        self.blurhash = Some(blurhash);
        self
    }

    /// Set thumbnail
    pub fn with_thumbnail(mut self, url: String, hash: Option<String>) -> Self {
        self.thumbnail = Some(if let Some(h) = hash {
            FileImage::with_hash(url, h)
        } else {
            FileImage::new(url)
        });
        self
    }

    /// Set preview image
    pub fn with_image(mut self, url: String, hash: Option<String>) -> Self {
        self.image = Some(if let Some(h) = hash {
            FileImage::with_hash(url, h)
        } else {
            FileImage::new(url)
        });
        self
    }

    /// Set summary
    pub fn with_summary(mut self, summary: String) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Set alt text
    pub fn with_alt(mut self, alt: String) -> Self {
        self.alt = Some(alt);
        self
    }

    /// Add fallback URL
    pub fn add_fallback(mut self, url: String) -> Self {
        self.fallbacks.push(url);
        self
    }

    /// Set service type
    pub fn with_service(mut self, service: String) -> Self {
        self.service = Some(service);
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Required tags
        tags.push(vec!["url".to_string(), self.url.clone()]);
        tags.push(vec!["m".to_string(), self.mime_type.clone()]);
        tags.push(vec!["x".to_string(), self.hash.clone()]);

        // Optional tags
        if let Some(ref oh) = self.original_hash {
            tags.push(vec!["ox".to_string(), oh.clone()]);
        }

        if let Some(size) = self.size {
            tags.push(vec!["size".to_string(), size.to_string()]);
        }

        if let Some(ref dim) = self.dimensions {
            tags.push(vec!["dim".to_string(), dim.to_string()]);
        }

        if let Some(ref magnet) = self.magnet {
            tags.push(vec!["magnet".to_string(), magnet.clone()]);
        }

        if let Some(ref infohash) = self.infohash {
            tags.push(vec!["i".to_string(), infohash.clone()]);
        }

        if let Some(ref blurhash) = self.blurhash {
            tags.push(vec!["blurhash".to_string(), blurhash.clone()]);
        }

        if let Some(ref thumb) = self.thumbnail {
            let mut tag = vec!["thumb".to_string(), thumb.url.clone()];
            if let Some(ref hash) = thumb.hash {
                tag.push(hash.clone());
            }
            tags.push(tag);
        }

        if let Some(ref img) = self.image {
            let mut tag = vec!["image".to_string(), img.url.clone()];
            if let Some(ref hash) = img.hash {
                tag.push(hash.clone());
            }
            tags.push(tag);
        }

        if let Some(ref summary) = self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(ref alt) = self.alt {
            tags.push(vec!["alt".to_string(), alt.clone()]);
        }

        for fallback in &self.fallbacks {
            tags.push(vec!["fallback".to_string(), fallback.clone()]);
        }

        if let Some(ref service) = self.service {
            tags.push(vec!["service".to_string(), service.clone()]);
        }

        tags
    }

    /// Parse from event tags
    pub fn from_tags(tags: &[Vec<String>], content: &str) -> Result<Self, Nip94Error> {
        let mut url: Option<String> = None;
        let mut mime_type: Option<String> = None;
        let mut hash: Option<String> = None;
        let mut original_hash: Option<String> = None;
        let mut size: Option<u64> = None;
        let mut dimensions: Option<Dimensions> = None;
        let mut magnet: Option<String> = None;
        let mut infohash: Option<String> = None;
        let mut blurhash: Option<String> = None;
        let mut thumbnail: Option<FileImage> = None;
        let mut image: Option<FileImage> = None;
        let mut summary: Option<String> = None;
        let mut alt: Option<String> = None;
        let mut fallbacks: Vec<String> = Vec::new();
        let mut service: Option<String> = None;

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "url" if tag.len() >= 2 => url = Some(tag[1].clone()),
                "m" if tag.len() >= 2 => mime_type = Some(tag[1].to_lowercase()),
                "x" if tag.len() >= 2 => hash = Some(tag[1].clone()),
                "ox" if tag.len() >= 2 => original_hash = Some(tag[1].clone()),
                "size" if tag.len() >= 2 => {
                    size = tag[1].parse().ok();
                }
                "dim" if tag.len() >= 2 => {
                    dimensions = Dimensions::parse(&tag[1]).ok();
                }
                "magnet" if tag.len() >= 2 => magnet = Some(tag[1].clone()),
                "i" if tag.len() >= 2 => infohash = Some(tag[1].clone()),
                "blurhash" if tag.len() >= 2 => blurhash = Some(tag[1].clone()),
                "thumb" if tag.len() >= 2 => {
                    thumbnail = Some(if tag.len() >= 3 {
                        FileImage::with_hash(tag[1].clone(), tag[2].clone())
                    } else {
                        FileImage::new(tag[1].clone())
                    });
                }
                "image" if tag.len() >= 2 => {
                    image = Some(if tag.len() >= 3 {
                        FileImage::with_hash(tag[1].clone(), tag[2].clone())
                    } else {
                        FileImage::new(tag[1].clone())
                    });
                }
                "summary" if tag.len() >= 2 => summary = Some(tag[1].clone()),
                "alt" if tag.len() >= 2 => alt = Some(tag[1].clone()),
                "fallback" if tag.len() >= 2 => fallbacks.push(tag[1].clone()),
                "service" if tag.len() >= 2 => service = Some(tag[1].clone()),
                _ => {} // Ignore unknown tags
            }
        }

        // Validate required fields
        let url = url.ok_or_else(|| Nip94Error::MissingRequired("url".to_string()))?;
        let mime_type =
            mime_type.ok_or_else(|| Nip94Error::MissingRequired("mime_type".to_string()))?;
        let hash = hash.ok_or_else(|| Nip94Error::MissingRequired("hash".to_string()))?;

        Ok(Self {
            content: content.to_string(),
            url,
            mime_type,
            hash,
            original_hash,
            size,
            dimensions,
            magnet,
            infohash,
            blurhash,
            thumbnail,
            image,
            summary,
            alt,
            fallbacks,
            service,
        })
    }
}

/// Check if a kind is a file metadata event
pub fn is_file_metadata_kind(kind: u16) -> bool {
    kind == FILE_METADATA_KIND
}

/// Create an EventTemplate for file metadata.
///
/// This is a convenience function that converts FileMetadata into an EventTemplate
/// ready to be signed and published. The content field comes from the metadata's
/// content (description/caption).
///
/// # Example
///
/// ```
/// use nostr::nip94::{FileMetadata, create_file_metadata_event};
///
/// let metadata = FileMetadata::new(
///     "https://example.com/dataset.csv".to_string(),
///     "text/csv".to_string(),
///     "abc123def456".to_string(),
/// )
/// .with_content("Sales data Q4 2024".to_string())
/// .with_size(1024000);
///
/// let event_template = create_file_metadata_event(&metadata);
/// assert_eq!(event_template.kind, 1063);
/// ```
pub fn create_file_metadata_event(metadata: &FileMetadata) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: FILE_METADATA_KIND,
        tags: metadata.to_tags(),
        content: metadata.content.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_parse() {
        let dim = Dimensions::parse("1920x1080").unwrap();
        assert_eq!(dim.width, 1920);
        assert_eq!(dim.height, 1080);
        assert_eq!(dim.to_string(), "1920x1080");
    }

    #[test]
    fn test_dimensions_parse_invalid() {
        assert!(Dimensions::parse("1920").is_err());
        assert!(Dimensions::parse("1920x").is_err());
        assert!(Dimensions::parse("x1080").is_err());
        assert!(Dimensions::parse("abc x def").is_err());
    }

    #[test]
    fn test_file_image() {
        let img = FileImage::new("https://example.com/image.jpg".to_string());
        assert_eq!(img.url, "https://example.com/image.jpg");
        assert_eq!(img.hash, None);

        let img_with_hash = FileImage::with_hash(
            "https://example.com/image.jpg".to_string(),
            "abc123".to_string(),
        );
        assert_eq!(img_with_hash.url, "https://example.com/image.jpg");
        assert_eq!(img_with_hash.hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_file_metadata_minimal() {
        let metadata = FileMetadata::new(
            "https://example.com/file.pdf".to_string(),
            "application/PDF".to_string(), // Test case conversion
            "abc123def456".to_string(),
        );

        assert_eq!(metadata.url, "https://example.com/file.pdf");
        assert_eq!(metadata.mime_type, "application/pdf"); // Should be lowercase
        assert_eq!(metadata.hash, "abc123def456");
        assert_eq!(metadata.content, "");
    }

    #[test]
    fn test_file_metadata_builder() {
        let metadata = FileMetadata::new(
            "https://example.com/image.jpg".to_string(),
            "image/jpeg".to_string(),
            "abc123".to_string(),
        )
        .with_content("A beautiful sunset".to_string())
        .with_size(1024000)
        .with_dimensions(1920, 1080)
        .with_alt("Sunset over mountains".to_string())
        .add_fallback("https://backup.example.com/image.jpg".to_string());

        assert_eq!(metadata.content, "A beautiful sunset");
        assert_eq!(metadata.size, Some(1024000));
        assert_eq!(metadata.dimensions, Some(Dimensions::new(1920, 1080)));
        assert_eq!(metadata.alt, Some("Sunset over mountains".to_string()));
        assert_eq!(metadata.fallbacks.len(), 1);
    }

    #[test]
    fn test_file_metadata_to_tags() {
        let metadata = FileMetadata::new(
            "https://example.com/file.pdf".to_string(),
            "application/pdf".to_string(),
            "abc123".to_string(),
        )
        .with_size(500000)
        .with_original_hash("def456".to_string());

        let tags = metadata.to_tags();

        assert!(tags.contains(&vec![
            "url".to_string(),
            "https://example.com/file.pdf".to_string()
        ]));
        assert!(tags.contains(&vec!["m".to_string(), "application/pdf".to_string()]));
        assert!(tags.contains(&vec!["x".to_string(), "abc123".to_string()]));
        assert!(tags.contains(&vec!["ox".to_string(), "def456".to_string()]));
        assert!(tags.contains(&vec!["size".to_string(), "500000".to_string()]));
    }

    #[test]
    fn test_file_metadata_from_tags() {
        let tags = vec![
            vec![
                "url".to_string(),
                "https://example.com/file.pdf".to_string(),
            ],
            vec!["m".to_string(), "application/pdf".to_string()],
            vec!["x".to_string(), "abc123".to_string()],
            vec!["size".to_string(), "500000".to_string()],
            vec!["dim".to_string(), "1920x1080".to_string()],
        ];

        let metadata = FileMetadata::from_tags(&tags, "Test file").unwrap();

        assert_eq!(metadata.content, "Test file");
        assert_eq!(metadata.url, "https://example.com/file.pdf");
        assert_eq!(metadata.mime_type, "application/pdf");
        assert_eq!(metadata.hash, "abc123");
        assert_eq!(metadata.size, Some(500000));
        assert_eq!(metadata.dimensions, Some(Dimensions::new(1920, 1080)));
    }

    #[test]
    fn test_file_metadata_from_tags_missing_required() {
        let tags = vec![
            vec![
                "url".to_string(),
                "https://example.com/file.pdf".to_string(),
            ],
            vec!["m".to_string(), "application/pdf".to_string()],
            // Missing 'x' tag
        ];

        let result = FileMetadata::from_tags(&tags, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_file_metadata_roundtrip() {
        let original = FileMetadata::new(
            "https://example.com/video.mp4".to_string(),
            "video/mp4".to_string(),
            "videohash123".to_string(),
        )
        .with_content("Cool video".to_string())
        .with_size(10485760)
        .with_dimensions(1280, 720)
        .with_thumbnail(
            "https://example.com/thumb.jpg".to_string(),
            Some("thumbhash".to_string()),
        )
        .add_fallback("https://cdn.example.com/video.mp4".to_string());

        let tags = original.to_tags();
        let reconstructed = FileMetadata::from_tags(&tags, &original.content).unwrap();

        assert_eq!(reconstructed.url, original.url);
        assert_eq!(reconstructed.mime_type, original.mime_type);
        assert_eq!(reconstructed.hash, original.hash);
        assert_eq!(reconstructed.size, original.size);
        assert_eq!(reconstructed.dimensions, original.dimensions);
        assert_eq!(reconstructed.thumbnail, original.thumbnail);
        assert_eq!(reconstructed.fallbacks, original.fallbacks);
    }

    #[test]
    fn test_create_file_metadata_event() {
        let metadata = FileMetadata::new(
            "https://example.com/data.csv".to_string(),
            "text/csv".to_string(),
            "hash123".to_string(),
        )
        .with_content("Dataset description".to_string())
        .with_size(500000);

        let event = create_file_metadata_event(&metadata);

        assert_eq!(event.kind, FILE_METADATA_KIND);
        assert!(event.tags.iter().any(|t| t[0] == "url"));
        assert!(event.tags.iter().any(|t| t[0] == "m" && t[1] == "text/csv"));
        assert_eq!(event.content, "Dataset description");
    }

    #[test]
    fn test_is_file_metadata_kind() {
        assert!(is_file_metadata_kind(1063));
        assert!(!is_file_metadata_kind(1));
        assert!(!is_file_metadata_kind(1064));
    }
}
