//! Data marketplace - NIP-94/95 dataset publishing and access

pub mod discover;
pub mod publish;
pub mod purchase;
pub mod serve;

// Re-export NIP-94/95 types
pub use nostr::{Dimensions, FILE_METADATA_KIND, FileImage, FileMetadata};

// Re-export discover functionality
pub use discover::{
    DatasetBrowser, DatasetCategory, DatasetListing, DiscoverError, SearchFilters, SortBy,
};
