//! Data marketplace - NIP-94/95 dataset publishing and access

pub mod publish;
pub mod discover;
pub mod purchase;
pub mod serve;

// Re-export NIP-94/95 types
pub use nostr::{FileMetadata, Dimensions, FileImage, FILE_METADATA_KIND};

// Re-export discover functionality
pub use discover::{
    DatasetBrowser, DatasetCategory, DatasetListing, DiscoverError, SearchFilters, SortBy,
};
