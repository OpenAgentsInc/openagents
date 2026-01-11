//! Data consumer and purchase types for accessing contributed training data
//!
//! Enables purchasing and accessing anonymized coding session data, workflow patterns,
//! and aggregated datasets for AI training and research.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when purchasing or accessing data
#[derive(Debug, Error)]
pub enum DataConsumerError {
    #[error("Listing not found: {0}")]
    ListingNotFound(String),

    #[error("Purchase not found: {0}")]
    PurchaseNotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Insufficient funds: available {available}, required {required}")]
    InsufficientFunds { available: u64, required: u64 },

    #[error("Download failed: {0}")]
    DownloadFailed(String),
}

/// Type of data listing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataListingType {
    /// Individual high-quality coding sessions
    Premium,

    /// Curated datasets of multiple sessions
    Standard,

    /// Anonymized aggregate patterns and statistics
    Aggregated,
}

impl DataListingType {
    /// Get a description of this listing type
    pub fn description(&self) -> &'static str {
        match self {
            Self::Premium => "Individual high-quality coding sessions with detailed context",
            Self::Standard => "Curated datasets of multiple sessions",
            Self::Aggregated => "Anonymized aggregate patterns and statistics",
        }
    }

    /// Get typical price range in satoshis
    pub fn typical_price_range(&self) -> (u64, u64) {
        match self {
            Self::Premium => (10000, 100000),  // 10k-100k sats
            Self::Standard => (50000, 500000), // 50k-500k sats
            Self::Aggregated => (5000, 50000), // 5k-50k sats
        }
    }
}

/// Metadata about a dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Source applications that generated the data
    pub sources: Vec<String>,

    /// Programming languages represented in the dataset
    pub languages: Vec<String>,

    /// Date range of the data
    pub date_range: (DateTime<Utc>, DateTime<Utc>),

    /// Quality score (0.0 to 1.0)
    pub quality_score: f32,

    /// Number of contributors (anonymized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_count: Option<u32>,

    /// Average session duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_session_duration: Option<u64>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
}

impl DatasetMetadata {
    /// Create new dataset metadata
    pub fn new(
        sources: Vec<String>,
        languages: Vec<String>,
        date_range: (DateTime<Utc>, DateTime<Utc>),
        quality_score: f32,
    ) -> Self {
        Self {
            sources,
            languages,
            date_range,
            quality_score,
            contributor_count: None,
            avg_session_duration: None,
            custom: None,
        }
    }

    /// Add contributor count
    pub fn with_contributor_count(mut self, count: u32) -> Self {
        self.contributor_count = Some(count);
        self
    }

    /// Add average session duration
    pub fn with_avg_session_duration(mut self, secs: u64) -> Self {
        self.avg_session_duration = Some(secs);
        self
    }

    /// Add custom metadata
    pub fn with_custom(mut self, custom: serde_json::Value) -> Self {
        self.custom = Some(custom);
        self
    }
}

/// A listing for purchasing training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataListing {
    /// Unique listing ID
    pub id: String,

    /// Type of listing
    pub listing_type: DataListingType,

    /// Description of the dataset
    pub description: String,

    /// Whether a sample is available for preview
    pub sample_available: bool,

    /// Price in satoshis
    pub price_sats: u64,

    /// Total size in bytes
    pub size_bytes: u64,

    /// Number of records/sessions in the dataset
    pub record_count: u64,

    /// Dataset metadata
    pub metadata: DatasetMetadata,

    /// When the listing was created
    pub created_at: DateTime<Utc>,

    /// When the listing expires (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl DataListing {
    /// Create a new data listing
    pub fn new(
        id: impl Into<String>,
        listing_type: DataListingType,
        description: impl Into<String>,
        price_sats: u64,
        size_bytes: u64,
        record_count: u64,
        metadata: DatasetMetadata,
    ) -> Self {
        Self {
            id: id.into(),
            listing_type,
            description: description.into(),
            sample_available: false,
            price_sats,
            size_bytes,
            record_count,
            metadata,
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    /// Make sample available
    pub fn with_sample(mut self) -> Self {
        self.sample_available = true;
        self
    }

    /// Set expiration time
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Check if the listing is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Utc::now() > exp)
    }

    /// Get price per record
    pub fn price_per_record(&self) -> u64 {
        if self.record_count == 0 {
            0
        } else {
            self.price_sats / self.record_count
        }
    }

    /// Get price per megabyte
    pub fn price_per_mb(&self) -> u64 {
        if self.size_bytes == 0 {
            0
        } else {
            (self.price_sats * 1_000_000) / self.size_bytes
        }
    }
}

/// A data purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPurchase {
    /// Unique purchase ID
    pub id: String,

    /// Buyer's Nostr public key (hex format)
    pub buyer: String,

    /// ID of the purchased listing
    pub listing_id: String,

    /// Price paid in satoshis
    pub price_paid_sats: u64,

    /// Access token for downloading
    pub access_token: String,

    /// When the purchase was made
    pub purchased_at: DateTime<Utc>,

    /// When the access expires (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,

    /// Whether the data has been downloaded
    #[serde(default)]
    pub downloaded: bool,

    /// When the data was downloaded (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloaded_at: Option<DateTime<Utc>>,
}

impl DataPurchase {
    /// Create a new purchase record
    pub fn new(
        id: impl Into<String>,
        buyer: impl Into<String>,
        listing_id: impl Into<String>,
        price_paid_sats: u64,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            buyer: buyer.into(),
            listing_id: listing_id.into(),
            price_paid_sats,
            access_token: access_token.into(),
            purchased_at: Utc::now(),
            expires_at: None,
            downloaded: false,
            downloaded_at: None,
        }
    }

    /// Set expiration time
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Mark as downloaded
    pub fn mark_downloaded(&mut self) {
        self.downloaded = true;
        self.downloaded_at = Some(Utc::now());
    }

    /// Check if access is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Utc::now() > exp)
    }

    /// Check if download is still available
    pub fn can_download(&self) -> bool {
        !self.is_expired()
    }
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum requests per time window
    pub max_requests: u32,

    /// Time window in seconds
    pub window_secs: u64,

    /// Current request count in this window
    #[serde(default)]
    pub current_count: u32,

    /// When the current window started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_start: Option<DateTime<Utc>>,
}

impl RateLimit {
    /// Create a new rate limit
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            current_count: 0,
            window_start: None,
        }
    }

    /// Check if a request is allowed
    pub fn allow_request(&mut self) -> bool {
        let now = Utc::now();

        // Initialize or reset window if needed
        if let Some(start) = self.window_start {
            let elapsed = (now - start).num_seconds() as u64;
            if elapsed >= self.window_secs {
                // Reset window
                self.current_count = 0;
                self.window_start = Some(now);
            }
        } else {
            self.window_start = Some(now);
        }

        // Check limit
        if self.current_count < self.max_requests {
            self.current_count += 1;
            true
        } else {
            false
        }
    }

    /// Get remaining requests in current window
    pub fn remaining(&self) -> u32 {
        self.max_requests.saturating_sub(self.current_count)
    }
}

/// Permissions for accessing purchased data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPermissions {
    /// Can download the full dataset
    #[serde(default = "default_true")]
    pub can_download: bool,

    /// Can stream the dataset
    #[serde(default)]
    pub can_stream: bool,

    /// Can create derived works
    #[serde(default)]
    pub can_derive: bool,

    /// Attribution required for derived works
    #[serde(default = "default_true")]
    pub attribution_required: bool,
}

fn default_true() -> bool {
    true
}

impl Default for DataPermissions {
    fn default() -> Self {
        Self {
            can_download: true,
            can_stream: false,
            can_derive: false,
            attribution_required: true,
        }
    }
}

impl DataPermissions {
    /// Create permissions with all access
    pub fn full_access() -> Self {
        Self {
            can_download: true,
            can_stream: true,
            can_derive: true,
            attribution_required: true,
        }
    }

    /// Create download-only permissions
    pub fn download_only() -> Self {
        Self {
            can_download: true,
            can_stream: false,
            can_derive: false,
            attribution_required: true,
        }
    }

    /// Enable streaming access
    pub fn with_streaming(mut self) -> Self {
        self.can_stream = true;
        self
    }

    /// Enable derived works
    pub fn with_derive(mut self) -> Self {
        self.can_derive = true;
        self
    }

    /// Remove attribution requirement
    pub fn without_attribution(mut self) -> Self {
        self.attribution_required = false;
        self
    }
}

/// Access token for downloaded data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAccessToken {
    /// The access token string
    pub token: String,

    /// Associated purchase ID
    pub purchase_id: String,

    /// Access permissions
    pub permissions: DataPermissions,

    /// Optional rate limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,

    /// When the token was issued
    pub issued_at: DateTime<Utc>,

    /// When the token expires
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl DataAccessToken {
    /// Create a new access token
    pub fn new(
        token: impl Into<String>,
        purchase_id: impl Into<String>,
        permissions: DataPermissions,
    ) -> Self {
        Self {
            token: token.into(),
            purchase_id: purchase_id.into(),
            permissions,
            rate_limit: None,
            issued_at: Utc::now(),
            expires_at: None,
        }
    }

    /// Set expiration time
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set rate limit
    pub fn with_rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Utc::now() > exp)
    }

    /// Check if a request is allowed (considering rate limits)
    pub fn allow_request(&mut self) -> bool {
        if self.is_expired() {
            return false;
        }

        if let Some(limit) = &mut self.rate_limit {
            limit.allow_request()
        } else {
            true
        }
    }
}

/// Sample data for preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSample {
    /// Listing ID this sample is from
    pub listing_id: String,

    /// Sample records (limited subset)
    pub records: Vec<serde_json::Value>,

    /// Number of records in the sample
    pub sample_size: u32,

    /// Total records in the full dataset
    pub total_records: u64,
}

impl DataSample {
    /// Create a new data sample
    pub fn new(
        listing_id: impl Into<String>,
        records: Vec<serde_json::Value>,
        total_records: u64,
    ) -> Self {
        let sample_size = records.len() as u32;
        Self {
            listing_id: listing_id.into(),
            records,
            sample_size,
            total_records,
        }
    }

    /// Get the sample percentage
    pub fn sample_percentage(&self) -> f32 {
        if self.total_records == 0 {
            0.0
        } else {
            (self.sample_size as f64 / self.total_records as f64 * 100.0) as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listing_type_description() {
        assert_eq!(
            DataListingType::Premium.description(),
            "Individual high-quality coding sessions with detailed context"
        );

        let (min, max) = DataListingType::Standard.typical_price_range();
        assert_eq!(min, 50000);
        assert_eq!(max, 500000);
    }

    #[test]
    fn test_dataset_metadata() {
        let start = Utc::now();
        let end = Utc::now();
        let metadata = DatasetMetadata::new(
            vec!["codex-code".to_string()],
            vec!["rust".to_string(), "python".to_string()],
            (start, end),
            0.85,
        )
        .with_contributor_count(100)
        .with_avg_session_duration(3600);

        assert_eq!(metadata.sources.len(), 1);
        assert_eq!(metadata.languages.len(), 2);
        assert_eq!(metadata.quality_score, 0.85);
        assert_eq!(metadata.contributor_count, Some(100));
        assert_eq!(metadata.avg_session_duration, Some(3600));
    }

    #[test]
    fn test_data_listing() {
        let metadata = DatasetMetadata::new(
            vec!["vscode".to_string()],
            vec!["typescript".to_string()],
            (Utc::now(), Utc::now()),
            0.9,
        );

        let listing = DataListing::new(
            "listing1",
            DataListingType::Standard,
            "High quality TypeScript sessions",
            100000,
            10_000_000, // 10MB
            50,
            metadata,
        )
        .with_sample();

        assert_eq!(listing.id, "listing1");
        assert!(listing.sample_available);
        assert_eq!(listing.price_per_record(), 2000);
        assert!(!listing.is_expired());
    }

    #[test]
    fn test_listing_price_calculations() {
        let metadata = DatasetMetadata::new(
            vec!["source".to_string()],
            vec!["lang".to_string()],
            (Utc::now(), Utc::now()),
            0.8,
        );

        let listing = DataListing::new(
            "listing2",
            DataListingType::Premium,
            "Test",
            50000,
            1_000_000, // 1MB
            100,
            metadata,
        );

        assert_eq!(listing.price_per_record(), 500);
        assert_eq!(listing.price_per_mb(), 50000);
    }

    #[test]
    fn test_data_purchase() {
        let mut purchase =
            DataPurchase::new("purchase1", "buyer123", "listing1", 100000, "token_abc");

        assert!(!purchase.downloaded);
        assert!(purchase.can_download());

        purchase.mark_downloaded();
        assert!(purchase.downloaded);
        assert!(purchase.downloaded_at.is_some());
    }

    #[test]
    fn test_purchase_expiration() {
        let past = Utc::now() - chrono::Duration::hours(1);
        let purchase = DataPurchase::new("purchase2", "buyer456", "listing2", 50000, "token_def")
            .with_expiration(past);

        assert!(purchase.is_expired());
        assert!(!purchase.can_download());
    }

    #[test]
    fn test_rate_limit() {
        let mut limit = RateLimit::new(5, 60);

        // Should allow first 5 requests
        for _ in 0..5 {
            assert!(limit.allow_request());
        }

        // 6th request should be denied
        assert!(!limit.allow_request());
        assert_eq!(limit.remaining(), 0);
    }

    #[test]
    fn test_data_permissions() {
        let default = DataPermissions::default();
        assert!(default.can_download);
        assert!(!default.can_stream);
        assert!(!default.can_derive);
        assert!(default.attribution_required);

        let full = DataPermissions::full_access();
        assert!(full.can_download);
        assert!(full.can_stream);
        assert!(full.can_derive);

        let custom = DataPermissions::download_only()
            .with_streaming()
            .with_derive()
            .without_attribution();
        assert!(custom.can_download);
        assert!(custom.can_stream);
        assert!(custom.can_derive);
        assert!(!custom.attribution_required);
    }

    #[test]
    fn test_data_access_token() {
        let mut token = DataAccessToken::new("abc123", "purchase1", DataPermissions::default())
            .with_rate_limit(RateLimit::new(10, 60));

        assert!(!token.is_expired());
        assert!(token.allow_request());

        let past = Utc::now() - chrono::Duration::hours(1);
        let expired = DataAccessToken::new("def456", "purchase2", DataPermissions::default())
            .with_expiration(past);

        assert!(expired.is_expired());
        assert!(!expired.clone().allow_request());
    }

    #[test]
    fn test_data_sample() {
        let records = vec![
            serde_json::json!({"session": 1}),
            serde_json::json!({"session": 2}),
            serde_json::json!({"session": 3}),
        ];

        let sample = DataSample::new("listing1", records, 100);

        assert_eq!(sample.sample_size, 3);
        assert_eq!(sample.total_records, 100);
        assert_eq!(sample.sample_percentage(), 3.0);
    }

    #[test]
    fn test_data_listing_serde() {
        let metadata = DatasetMetadata::new(
            vec!["source".to_string()],
            vec!["lang".to_string()],
            (Utc::now(), Utc::now()),
            0.8,
        );
        let listing = DataListing::new(
            "listing1",
            DataListingType::Premium,
            "Test",
            10000,
            1000,
            10,
            metadata,
        );

        let json = serde_json::to_string(&listing).unwrap();
        let deserialized: DataListing = serde_json::from_str(&json).unwrap();

        assert_eq!(listing.id, deserialized.id);
        assert_eq!(listing.price_sats, deserialized.price_sats);
    }

    #[test]
    fn test_data_purchase_serde() {
        let purchase = DataPurchase::new("p1", "buyer", "listing1", 5000, "token");
        let json = serde_json::to_string(&purchase).unwrap();
        let deserialized: DataPurchase = serde_json::from_str(&json).unwrap();

        assert_eq!(purchase.id, deserialized.id);
        assert_eq!(purchase.price_paid_sats, deserialized.price_paid_sats);
    }
}
