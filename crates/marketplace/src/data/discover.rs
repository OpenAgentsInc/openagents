//! Dataset discovery and browsing via NIP-94/95
//!
//! This module implements dataset discovery using the Nostr network via NIP-94.

use nostr::FileMetadata;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during dataset browsing operations
#[derive(Debug, Error)]
pub enum DiscoverError {
    #[error("network error: {0}")]
    Network(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("no datasets found matching criteria")]
    NoDatasetsFound,
}

/// Sort order for dataset listings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Most recently published first
    Recent,
    /// Alphabetical by name
    Name,
    /// Largest size first
    Size,
    /// Lowest price first
    Price,
}

/// Category filter for datasets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatasetCategory {
    /// Code embeddings
    Embeddings,
    /// Source code repositories
    Code,
    /// Documentation
    Documentation,
    /// Training data
    Training,
    /// Benchmark datasets
    Benchmarks,
    /// Research data
    Research,
    /// Other category
    Other(String),
}

impl DatasetCategory {
    pub fn as_str(&self) -> &str {
        match self {
            DatasetCategory::Embeddings => "embeddings",
            DatasetCategory::Code => "code",
            DatasetCategory::Documentation => "documentation",
            DatasetCategory::Training => "training",
            DatasetCategory::Benchmarks => "benchmarks",
            DatasetCategory::Research => "research",
            DatasetCategory::Other(s) => s.as_str(),
        }
    }
}

impl FromStr for DatasetCategory {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "embeddings" => DatasetCategory::Embeddings,
            "code" => DatasetCategory::Code,
            "documentation" => DatasetCategory::Documentation,
            "training" => DatasetCategory::Training,
            "benchmarks" => DatasetCategory::Benchmarks,
            "research" => DatasetCategory::Research,
            other => DatasetCategory::Other(other.to_string()),
        })
    }
}

/// Search filters for browsing datasets
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by category
    pub category: Option<DatasetCategory>,
    /// Filter by MIME type prefix (e.g., "text/", "application/json")
    pub mime_type: Option<String>,
    /// Maximum price in sats (None = any price)
    pub max_price_sats: Option<u64>,
    /// Minimum size in bytes
    pub min_size_bytes: Option<u64>,
    /// Maximum size in bytes
    pub max_size_bytes: Option<u64>,
    /// Only show free datasets
    pub free_only: bool,
    /// Text search query
    pub query: Option<String>,
}

impl SearchFilters {
    /// Create empty filters
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by category
    pub fn with_category(mut self, category: DatasetCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Filter by MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Filter by maximum price
    pub fn with_max_price(mut self, max_price_sats: u64) -> Self {
        self.max_price_sats = Some(max_price_sats);
        self
    }

    /// Filter by minimum size
    pub fn with_min_size(mut self, min_size_bytes: u64) -> Self {
        self.min_size_bytes = Some(min_size_bytes);
        self
    }

    /// Filter by maximum size
    pub fn with_max_size(mut self, max_size_bytes: u64) -> Self {
        self.max_size_bytes = Some(max_size_bytes);
        self
    }

    /// Only show free datasets
    pub fn free_only(mut self) -> Self {
        self.free_only = true;
        self
    }

    /// Add text search query
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }
}

/// Dataset listing from marketplace discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetListing {
    /// Dataset ID
    pub id: String,
    /// Dataset name/title
    pub name: String,
    /// Description
    pub description: String,
    /// Creator pubkey
    pub creator_pubkey: String,
    /// Download URL
    pub url: String,
    /// MIME type
    pub mime_type: String,
    /// SHA-256 hash
    pub hash: String,
    /// Size in bytes
    pub size_bytes: Option<u64>,
    /// Price in sats (None = free)
    pub price_sats: Option<u64>,
    /// Preview/thumbnail URL
    pub preview_url: Option<String>,
    /// Summary/excerpt
    pub summary: Option<String>,
}

impl DatasetListing {
    /// Convert from NIP-94 FileMetadata
    pub fn from_file_metadata(metadata: FileMetadata, id: String, creator_pubkey: String) -> Self {
        Self {
            id,
            name: metadata
                .summary
                .clone()
                .unwrap_or_else(|| "Unnamed Dataset".to_string()),
            description: metadata.content.clone(),
            creator_pubkey,
            url: metadata.url.clone(),
            mime_type: metadata.mime_type.clone(),
            hash: metadata.hash.clone(),
            size_bytes: metadata.size,
            price_sats: None, // Will be set from marketplace metadata
            preview_url: metadata.thumbnail.map(|t| t.url),
            summary: metadata.summary,
        }
    }

    /// Check if dataset matches the given filters
    pub fn matches_filters(&self, filters: &SearchFilters) -> bool {
        // Check free-only filter
        if filters.free_only && self.price_sats.is_some() {
            return false;
        }

        // Check max price filter
        if let Some(max_price) = filters.max_price_sats {
            if let Some(price) = self.price_sats {
                if price > max_price {
                    return false;
                }
            }
        }

        // Check MIME type filter
        if let Some(mime_filter) = &filters.mime_type {
            if !self.mime_type.starts_with(mime_filter) {
                return false;
            }
        }

        // Check size filters
        if let Some(size) = self.size_bytes {
            if let Some(min_size) = filters.min_size_bytes {
                if size < min_size {
                    return false;
                }
            }
            if let Some(max_size) = filters.max_size_bytes {
                if size > max_size {
                    return false;
                }
            }
        }

        // Check text query (search in name and description)
        if let Some(query) = &filters.query {
            let query_lower = query.to_lowercase();
            let matches = self.name.to_lowercase().contains(&query_lower)
                || self.description.to_lowercase().contains(&query_lower);
            if !matches {
                return false;
            }
        }

        true
    }

    /// Format size as human-readable string
    pub fn format_size(&self) -> String {
        match self.size_bytes {
            None => "Unknown".to_string(),
            Some(bytes) => {
                if bytes < 1024 {
                    format!("{} B", bytes)
                } else if bytes < 1024 * 1024 {
                    format!("{:.1} KB", bytes as f64 / 1024.0)
                } else if bytes < 1024 * 1024 * 1024 {
                    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
                } else {
                    format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
                }
            }
        }
    }
}

/// Dataset browser for NIP-94/95 discovery
pub struct DatasetBrowser {
    // Future: add relay pool connection
}

impl DatasetBrowser {
    /// Create a new dataset browser
    pub fn new() -> Self {
        Self {}
    }

    /// Browse all datasets
    ///
    /// This will fetch all dataset metadata from relays and return them as listings.
    pub async fn browse(
        &self,
        _filters: SearchFilters,
        _sort_by: SortBy,
    ) -> Result<Vec<DatasetListing>, DiscoverError> {
        // Dataset browsing requires Nostr relay integration which is not yet implemented.
        // Per d-012 (No Stubs), we return an explicit error instead of an empty list.
        Err(DiscoverError::Network(
            "Dataset browsing not yet implemented. Requires Nostr relay client integration for fetching NIP-94 dataset metadata events.".to_string()
        ))
    }

    /// Search for datasets by query
    pub async fn search(
        &self,
        query: impl Into<String>,
        sort_by: SortBy,
    ) -> Result<Vec<DatasetListing>, DiscoverError> {
        let filters = SearchFilters::new().with_query(query);
        self.browse(filters, sort_by).await
    }

    /// Get a specific dataset by ID
    pub async fn get_dataset(&self, _dataset_id: &str) -> Result<DatasetListing, DiscoverError> {
        // Dataset fetching requires Nostr relay integration which is not yet implemented.
        // Per d-012 (No Stubs), we return an explicit error.
        Err(DiscoverError::Network(
            "Dataset fetching not yet implemented. Requires Nostr relay client integration for fetching NIP-94 dataset metadata events.".to_string()
        ))
    }

    /// Sort listings according to the specified order
    #[allow(dead_code)]
    fn sort_listings(&self, listings: &mut [DatasetListing], sort_by: SortBy) {
        match sort_by {
            SortBy::Recent => {
                // Sorting by publication timestamp requires adding created_at field
                // to DatasetListing and populating it from contribution events.
                // When implemented:
                // listings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            }
            SortBy::Name => {
                listings.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortBy::Size => {
                listings.sort_by(|a, b| {
                    match (a.size_bytes, b.size_bytes) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, Some(_)) => std::cmp::Ordering::Less,
                        (Some(_), None) => std::cmp::Ordering::Greater,
                        (Some(a_size), Some(b_size)) => b_size.cmp(&a_size), // Largest first
                    }
                });
            }
            SortBy::Price => {
                listings.sort_by(|a, b| {
                    match (a.price_sats, b.price_sats) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, Some(_)) => std::cmp::Ordering::Less, // Free comes first
                        (Some(_), None) => std::cmp::Ordering::Greater,
                        (Some(a_price), Some(b_price)) => a_price.cmp(&b_price),
                    }
                });
            }
        }
    }
}

impl Default for DatasetBrowser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_category_conversions() {
        assert_eq!(DatasetCategory::Embeddings.as_str(), "embeddings");
        assert_eq!(
            DatasetCategory::from_str("embeddings"),
            Ok(DatasetCategory::Embeddings)
        );

        let custom = DatasetCategory::Other("custom".to_string());
        assert_eq!(custom.as_str(), "custom");
        assert_eq!(DatasetCategory::from_str("custom"), Ok(custom));
    }

    #[test]
    fn test_search_filters_builder() {
        let filters = SearchFilters::new()
            .with_category(DatasetCategory::Embeddings)
            .with_mime_type("application/json")
            .with_max_price(5000)
            .with_query("rust");

        assert_eq!(filters.category, Some(DatasetCategory::Embeddings));
        assert_eq!(filters.mime_type, Some("application/json".to_string()));
        assert_eq!(filters.max_price_sats, Some(5000));
        assert_eq!(filters.query, Some("rust".to_string()));
    }

    #[test]
    fn test_dataset_listing_matches_filters_free_only() {
        let dataset = DatasetListing {
            id: "test".to_string(),
            name: "Test Dataset".to_string(),
            description: "Test description".to_string(),
            creator_pubkey: "creator".to_string(),
            url: "https://example.com/data".to_string(),
            mime_type: "application/json".to_string(),
            hash: "abc123".to_string(),
            size_bytes: Some(1000),
            price_sats: Some(100),
            preview_url: None,
            summary: None,
        };

        let filters = SearchFilters::new().free_only();
        assert!(!dataset.matches_filters(&filters));

        let free_dataset = DatasetListing {
            price_sats: None,
            ..dataset.clone()
        };
        assert!(free_dataset.matches_filters(&filters));
    }

    #[test]
    fn test_dataset_listing_matches_filters_size() {
        let dataset = DatasetListing {
            id: "test".to_string(),
            name: "Test Dataset".to_string(),
            description: "Test description".to_string(),
            creator_pubkey: "creator".to_string(),
            url: "https://example.com/data".to_string(),
            mime_type: "application/json".to_string(),
            hash: "abc123".to_string(),
            size_bytes: Some(5000),
            price_sats: None,
            preview_url: None,
            summary: None,
        };

        let filters = SearchFilters::new().with_min_size(6000);
        assert!(!dataset.matches_filters(&filters));

        let filters = SearchFilters::new().with_min_size(4000);
        assert!(dataset.matches_filters(&filters));

        let filters = SearchFilters::new().with_max_size(4000);
        assert!(!dataset.matches_filters(&filters));

        let filters = SearchFilters::new().with_max_size(6000);
        assert!(dataset.matches_filters(&filters));
    }

    #[test]
    fn test_dataset_listing_format_size() {
        let dataset = DatasetListing {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            creator_pubkey: "creator".to_string(),
            url: "https://example.com/data".to_string(),
            mime_type: "application/json".to_string(),
            hash: "abc123".to_string(),
            size_bytes: Some(500),
            price_sats: None,
            preview_url: None,
            summary: None,
        };

        assert_eq!(dataset.format_size(), "500 B");

        let dataset = DatasetListing {
            size_bytes: Some(2048),
            ..dataset.clone()
        };
        assert_eq!(dataset.format_size(), "2.0 KB");

        let dataset = DatasetListing {
            size_bytes: Some(5 * 1024 * 1024),
            ..dataset.clone()
        };
        assert_eq!(dataset.format_size(), "5.0 MB");

        let dataset = DatasetListing {
            size_bytes: None,
            ..dataset
        };
        assert_eq!(dataset.format_size(), "Unknown");
    }

    #[test]
    fn test_dataset_listing_includes_preview_and_summary() {
        let metadata = FileMetadata::new(
            "https://example.com/data.json".to_string(),
            "application/json".to_string(),
            "abc123".to_string(),
        )
        .with_content("Dataset description".to_string())
        .with_thumbnail("https://example.com/preview.png".to_string(), None)
        .with_summary("Preview excerpt".to_string());

        let listing = DatasetListing::from_file_metadata(
            metadata,
            "dataset-1".to_string(),
            "creator".to_string(),
        );

        assert_eq!(
            listing.preview_url.as_deref(),
            Some("https://example.com/preview.png")
        );
        assert_eq!(listing.summary.as_deref(), Some("Preview excerpt"));
    }

    #[tokio::test]
    async fn test_dataset_browser_empty() {
        let browser = DatasetBrowser::new();
        let result = browser.browse(SearchFilters::new(), SortBy::Name).await;
        // Browser returns error when not implemented per d-012 (No Stubs)
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DiscoverError::Network(_)));
    }
}
