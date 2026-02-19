//! NIP-96: HTTP File Storage Integration
//!
//! Defines types for HTTP file storage servers compatible with Nostr.
//! Note: This NIP is deprecated in favor of NIP-B7, but this implementation
//! provides the types for backward compatibility.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/96.md>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Event kind for file server preference
pub const FILE_SERVER_PREFERENCE_KIND: u16 = 10096;

/// Well-known path for server info
pub const WELL_KNOWN_PATH: &str = "/.well-known/nostr/nip96.json";

/// Errors that can occur during NIP-96 operations
#[derive(Debug, Error)]
pub enum Nip96Error {
    #[error("invalid server info: {0}")]
    InvalidServerInfo(String),

    #[error("upload failed: {0}")]
    UploadFailed(String),

    #[error("file not found")]
    FileNotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("payment required")]
    PaymentRequired,

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("processing error: {0}")]
    ProcessingError(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Server plan information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerPlan {
    /// Plan name
    pub name: String,

    /// Whether NIP-98 auth is required for uploads
    #[serde(default = "default_true")]
    pub is_nip98_required: bool,

    /// Plan landing page URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Maximum file size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_byte_size: Option<u64>,

    /// File expiration range in days [min, max]
    /// [7, 0] means 7 days to unlimited
    /// [0, 0] means no expiration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_expiration: Option<[u32; 2]>,

    /// Supported media transformations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_transformations: Option<HashMap<String, Vec<String>>>,
}

fn default_true() -> bool {
    true
}

/// Server information document from /.well-known/nostr/nip96.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerInfo {
    /// API URL for upload and deletion
    pub api_url: String,

    /// Optional download URL (uses api_url if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,

    /// Optional delegation to another server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegated_to_url: Option<String>,

    /// Supported NIPs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_nips: Option<Vec<u16>>,

    /// Terms of service URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tos_url: Option<String>,

    /// Supported content types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_types: Option<Vec<String>>,

    /// Available plans
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans: Option<HashMap<String, ServerPlan>>,
}

impl ServerInfo {
    /// Get the effective download URL
    pub fn get_download_url(&self) -> &str {
        self.download_url.as_deref().unwrap_or(&self.api_url)
    }

    /// Check if server offers free plan
    pub fn has_free_plan(&self) -> bool {
        self.plans
            .as_ref()
            .map(|plans| plans.contains_key("free"))
            .unwrap_or(false)
    }

    /// Get the free plan if available
    pub fn get_free_plan(&self) -> Option<&ServerPlan> {
        self.plans.as_ref()?.get("free")
    }
}

/// Upload response status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadStatus {
    Success,
    Error,
    Processing,
}

/// NIP-94 event structure (subset for upload response)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Nip94Event {
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

/// Upload response from the server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadResponse {
    pub status: UploadStatus,
    pub message: String,

    /// Optional processing URL for delayed processing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_url: Option<String>,

    /// NIP-94 event with file metadata (absent on error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip94_event: Option<Nip94Event>,
}

impl UploadResponse {
    /// Check if upload was successful
    pub fn is_success(&self) -> bool {
        matches!(self.status, UploadStatus::Success)
    }

    /// Check if upload is being processed
    pub fn is_processing(&self) -> bool {
        matches!(self.status, UploadStatus::Processing)
    }

    /// Get the download URL from the nip94_event tags
    pub fn get_download_url(&self) -> Option<&str> {
        let event = self.nip94_event.as_ref()?;
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "url" {
                return Some(&tag[1]);
            }
        }
        None
    }

    /// Get the original file hash (ox tag)
    pub fn get_original_hash(&self) -> Option<&str> {
        let event = self.nip94_event.as_ref()?;
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "ox" {
                return Some(&tag[1]);
            }
        }
        None
    }

    /// Get the transformed file hash (x tag)
    pub fn get_transformed_hash(&self) -> Option<&str> {
        let event = self.nip94_event.as_ref()?;
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "x" {
                return Some(&tag[1]);
            }
        }
        None
    }
}

/// Processing status response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessingStatus {
    pub status: UploadStatus,
    pub message: String,

    /// Processing percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u8>,
}

/// Delete response from the server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteResponse {
    pub status: String,
    pub message: String,
}

/// File metadata in list response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMetadata {
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub created_at: u64,
}

/// List files response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListFilesResponse {
    /// Number of files in this page
    pub count: u32,

    /// Total number of files
    pub total: u32,

    /// Current page number
    pub page: u32,

    /// Array of file metadata
    pub files: Vec<FileMetadata>,
}

/// Media type for upload
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaType {
    Avatar,
    Banner,
    Normal,
}

impl MediaType {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            MediaType::Avatar => Some("avatar"),
            MediaType::Banner => Some("banner"),
            MediaType::Normal => None,
        }
    }
}

/// Upload request parameters
#[derive(Debug, Clone)]
pub struct UploadRequest {
    /// The file data (handled separately in multipart)
    pub file_name: String,

    /// Loose description
    pub caption: Option<String>,

    /// UNIX timestamp for expiration
    pub expiration: Option<u64>,

    /// File size in bytes
    pub size: Option<u64>,

    /// Strict description for accessibility
    pub alt: Option<String>,

    /// Media type (avatar, banner, or normal)
    pub media_type: MediaType,

    /// MIME type
    pub content_type: Option<String>,

    /// Request no transformation
    pub no_transform: bool,
}

impl Default for UploadRequest {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            caption: None,
            expiration: None,
            size: None,
            alt: None,
            media_type: MediaType::Normal,
            content_type: None,
            no_transform: false,
        }
    }
}

/// Helper to construct download URL
pub fn construct_download_url(base_url: &str, hash: &str, extension: Option<&str>) -> String {
    if let Some(ext) = extension {
        format!("{}/{}.{}", base_url, hash, ext)
    } else {
        format!("{}/{}", base_url, hash)
    }
}

/// Helper to construct delete URL
pub fn construct_delete_url(api_url: &str, hash: &str, extension: Option<&str>) -> String {
    construct_download_url(api_url, hash, extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_basic() {
        let info = ServerInfo {
            api_url: "https://example.com/api".to_string(),
            download_url: None,
            delegated_to_url: None,
            supported_nips: None,
            tos_url: None,
            content_types: None,
            plans: None,
        };

        assert_eq!(info.get_download_url(), "https://example.com/api");
        assert!(!info.has_free_plan());
    }

    #[test]
    fn test_server_info_with_download_url() {
        let info = ServerInfo {
            api_url: "https://example.com/api".to_string(),
            download_url: Some("https://cdn.example.com".to_string()),
            delegated_to_url: None,
            supported_nips: None,
            tos_url: None,
            content_types: None,
            plans: None,
        };

        assert_eq!(info.get_download_url(), "https://cdn.example.com");
    }

    #[test]
    fn test_server_info_with_free_plan() {
        let mut plans = HashMap::new();
        plans.insert(
            "free".to_string(),
            ServerPlan {
                name: "Free Tier".to_string(),
                is_nip98_required: true,
                url: None,
                max_byte_size: Some(10485760),
                file_expiration: Some([14, 90]),
                media_transformations: None,
            },
        );

        let info = ServerInfo {
            api_url: "https://example.com/api".to_string(),
            download_url: None,
            delegated_to_url: None,
            supported_nips: None,
            tos_url: None,
            content_types: None,
            plans: Some(plans),
        };

        assert!(info.has_free_plan());
        let free_plan = info.get_free_plan().unwrap();
        assert_eq!(free_plan.name, "Free Tier");
        assert_eq!(free_plan.max_byte_size, Some(10485760));
    }

    #[test]
    fn test_upload_response_success() {
        let response = UploadResponse {
            status: UploadStatus::Success,
            message: "Upload successful.".to_string(),
            processing_url: None,
            nip94_event: Some(Nip94Event {
                tags: vec![
                    vec![
                        "url".to_string(),
                        "https://example.com/file.png".to_string(),
                    ],
                    vec![
                        "ox".to_string(),
                        "719171db19525d9d08dd69cb716a18158a249b7b3b3ec4bbdec5698dca104b7b"
                            .to_string(),
                    ],
                ],
                content: String::new(),
            }),
        };

        assert!(response.is_success());
        assert!(!response.is_processing());
        assert_eq!(
            response.get_download_url(),
            Some("https://example.com/file.png")
        );
        assert_eq!(
            response.get_original_hash(),
            Some("719171db19525d9d08dd69cb716a18158a249b7b3b3ec4bbdec5698dca104b7b")
        );
    }

    #[test]
    fn test_upload_response_error() {
        let response = UploadResponse {
            status: UploadStatus::Error,
            message: "Upload failed.".to_string(),
            processing_url: None,
            nip94_event: None,
        };

        assert!(!response.is_success());
        assert_eq!(response.get_download_url(), None);
    }

    #[test]
    fn test_upload_response_processing() {
        let response = UploadResponse {
            status: UploadStatus::Processing,
            message: "Processing...".to_string(),
            processing_url: Some("https://example.com/status/123".to_string()),
            nip94_event: None,
        };

        assert!(response.is_processing());
        assert_eq!(
            response.processing_url,
            Some("https://example.com/status/123".to_string())
        );
    }

    #[test]
    fn test_construct_download_url() {
        let hash = "719171db19525d9d08dd69cb716a18158a249b7b3b3ec4bbdec5698dca104b7b";

        assert_eq!(
            construct_download_url("https://example.com/api", hash, Some("png")),
            format!("https://example.com/api/{}.png", hash)
        );

        assert_eq!(
            construct_download_url("https://example.com/api", hash, None),
            format!("https://example.com/api/{}", hash)
        );
    }

    #[test]
    fn test_media_type() {
        assert_eq!(MediaType::Avatar.as_str(), Some("avatar"));
        assert_eq!(MediaType::Banner.as_str(), Some("banner"));
        assert_eq!(MediaType::Normal.as_str(), None);
    }

    #[test]
    fn test_upload_request_default() {
        let req = UploadRequest::default();
        assert_eq!(req.media_type, MediaType::Normal);
        assert!(!req.no_transform);
        assert_eq!(req.caption, None);
    }

    #[test]
    fn test_processing_status() {
        let status = ProcessingStatus {
            status: UploadStatus::Processing,
            message: "Processing...".to_string(),
            percentage: Some(50),
        };

        assert_eq!(status.percentage, Some(50));
    }

    #[test]
    fn test_server_info_serialization() {
        let info = ServerInfo {
            api_url: "https://example.com/api".to_string(),
            download_url: Some("https://cdn.example.com".to_string()),
            delegated_to_url: None,
            supported_nips: Some(vec![60]),
            tos_url: Some("https://example.com/tos".to_string()),
            content_types: Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
            plans: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ServerInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(info, deserialized);
    }

    #[test]
    fn test_list_files_response() {
        let response = ListFilesResponse {
            count: 1,
            total: 1,
            page: 0,
            files: vec![FileMetadata {
                tags: vec![
                    vec![
                        "ox".to_string(),
                        "719171db19525d9d08dd69cb716a18158a249b7b3b3ec4bbdec5698dca104b7b"
                            .to_string(),
                    ],
                    vec!["size".to_string(), "123456".to_string()],
                ],
                content: "a meme".to_string(),
                created_at: 1715691130,
            }],
        };

        assert_eq!(response.count, 1);
        assert_eq!(response.files.len(), 1);
        assert_eq!(response.files[0].content, "a meme");
    }
}
