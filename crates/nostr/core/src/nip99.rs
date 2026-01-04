//! NIP-99: Classified Listings
//!
//! This NIP defines kind 30402 for classified listings - marketplace ads for products,
//! services, rentals, job opportunities, and other offerings. Kind 30403 is used for
//! draft or inactive listings.
//!
//! ## Features
//!
//! - Structured metadata (title, summary, price, location)
//! - Flexible price format with currency and frequency
//! - Status tracking (active, sold)
//! - Image support (NIP-58 format)
//! - Categories/tags and geohash location
//!
//! ## Examples
//!
//! ```
//! use nostr::nip99::{ClassifiedListing, Price, ListingStatus};
//!
//! // Create a listing for a physical product
//! let listing = ClassifiedListing::new(
//!     "unique-listing-id",
//!     "Premium headphones in excellent condition",
//!     "Premium Headphones"
//! )
//! .with_summary("High-quality wireless headphones")
//! .with_price(Price::one_time("100", "USD"))
//! .with_location("NYC")
//! .with_status(ListingStatus::Active);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for active classified listings.
pub const KIND_CLASSIFIED_LISTING: u64 = 30402;

/// Kind number for draft/inactive classified listings.
pub const KIND_DRAFT_LISTING: u64 = 30403;

/// Errors that can occur during NIP-99 operations.
#[derive(Debug, Error)]
pub enum Nip99Error {
    #[error("listing must have a title")]
    MissingTitle,

    #[error("listing must have a d tag identifier")]
    MissingDTag,

    #[error("invalid price format")]
    InvalidPrice,

    #[error("invalid status: {0}")]
    InvalidStatus(String),
}

/// Check if a kind is a NIP-99 classified listing kind.
pub fn is_nip99_kind(kind: u64) -> bool {
    kind == KIND_CLASSIFIED_LISTING || kind == KIND_DRAFT_LISTING
}

/// Check if a kind is an active classified listing.
pub fn is_classified_listing_kind(kind: u64) -> bool {
    kind == KIND_CLASSIFIED_LISTING
}

/// Check if a kind is a draft listing.
pub fn is_draft_listing_kind(kind: u64) -> bool {
    kind == KIND_DRAFT_LISTING
}

/// Status of a classified listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListingStatus {
    Active,
    Sold,
}

impl ListingStatus {
    /// Parse a status from a string.
    pub fn from_str(s: &str) -> Result<Self, Nip99Error> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "sold" => Ok(Self::Sold),
            _ => Err(Nip99Error::InvalidStatus(s.to_string())),
        }
    }

    /// Convert status to string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Sold => "sold",
        }
    }
}

/// Price information for a listing.
///
/// Format: ["price", amount, currency, frequency?]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Amount as a string (numeric format)
    pub amount: String,
    /// Currency code (ISO 4217 or crypto like "btc")
    pub currency: String,
    /// Optional frequency for recurring payments (hour, day, week, month, year)
    pub frequency: Option<String>,
}

impl Price {
    /// Create a one-time price.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip99::Price;
    ///
    /// let price = Price::one_time("50", "USD");
    /// assert_eq!(price.amount, "50");
    /// assert_eq!(price.currency, "USD");
    /// assert_eq!(price.frequency, None);
    /// ```
    pub fn one_time(amount: impl Into<String>, currency: impl Into<String>) -> Self {
        Self {
            amount: amount.into(),
            currency: currency.into(),
            frequency: None,
        }
    }

    /// Create a recurring price.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip99::Price;
    ///
    /// let price = Price::recurring("15", "EUR", "month");
    /// assert_eq!(price.amount, "15");
    /// assert_eq!(price.currency, "EUR");
    /// assert_eq!(price.frequency, Some("month".to_string()));
    /// ```
    pub fn recurring(
        amount: impl Into<String>,
        currency: impl Into<String>,
        frequency: impl Into<String>,
    ) -> Self {
        Self {
            amount: amount.into(),
            currency: currency.into(),
            frequency: Some(frequency.into()),
        }
    }

    /// Convert to tag format.
    ///
    /// Returns: ["price", amount, currency] or ["price", amount, currency, frequency]
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![
            "price".to_string(),
            self.amount.clone(),
            self.currency.clone(),
        ];
        if let Some(freq) = &self.frequency {
            tag.push(freq.clone());
        }
        tag
    }

    /// Parse from tag format.
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip99Error> {
        if tag.len() < 3 || tag[0] != "price" {
            return Err(Nip99Error::InvalidPrice);
        }

        Ok(Self {
            amount: tag[1].clone(),
            currency: tag[2].clone(),
            frequency: tag.get(3).cloned(),
        })
    }
}

/// An image for a listing (NIP-58 format).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListingImage {
    /// Image URL
    pub url: String,
    /// Optional dimensions (e.g., "256x256")
    pub dimensions: Option<String>,
}

impl ListingImage {
    /// Create a new image.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            dimensions: None,
        }
    }

    /// Create an image with dimensions.
    pub fn with_dimensions(url: impl Into<String>, dimensions: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            dimensions: Some(dimensions.into()),
        }
    }

    /// Convert to tag format.
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["image".to_string(), self.url.clone()];
        if let Some(dim) = &self.dimensions {
            tag.push(dim.clone());
        }
        tag
    }
}

/// A classified listing (kind 30402).
///
/// Used for active marketplace listings of products, services, rentals, etc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifiedListing {
    /// The "d" tag identifier (required for addressable events)
    pub identifier: String,

    /// Markdown description of what is being offered
    pub content: String,

    /// Title of the listing
    pub title: String,

    /// Short summary or tagline
    pub summary: Option<String>,

    /// Published timestamp (unix seconds)
    pub published_at: Option<u64>,

    /// Location (free-form text)
    pub location: Option<String>,

    /// Geohash for precise location
    pub geohash: Option<String>,

    /// Price information
    pub price: Option<Price>,

    /// Status (active or sold)
    pub status: Option<ListingStatus>,

    /// Images
    pub images: Vec<ListingImage>,

    /// Categories/tags
    pub tags: Vec<String>,
}

impl ClassifiedListing {
    /// Create a new classified listing.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip99::ClassifiedListing;
    ///
    /// let listing = ClassifiedListing::new(
    ///     "premium-headphones",
    ///     "Premium wireless headphones in excellent condition. Barely used.",
    ///     "Premium Headphones"
    /// );
    /// ```
    pub fn new(
        identifier: impl Into<String>,
        content: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            content: content.into(),
            title: title.into(),
            summary: None,
            published_at: None,
            location: None,
            geohash: None,
            price: None,
            status: None,
            images: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Set the summary.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Set the published timestamp.
    pub fn with_published_at(mut self, timestamp: u64) -> Self {
        self.published_at = Some(timestamp);
        self
    }

    /// Set the location.
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set the geohash.
    pub fn with_geohash(mut self, geohash: impl Into<String>) -> Self {
        self.geohash = Some(geohash.into());
        self
    }

    /// Set the price.
    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Set the status.
    pub fn with_status(mut self, status: ListingStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Add an image.
    pub fn add_image(&mut self, image: ListingImage) {
        self.images.push(image);
    }

    /// Add a tag/category.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    /// Validate the listing.
    pub fn validate(&self) -> Result<(), Nip99Error> {
        if self.title.is_empty() {
            return Err(Nip99Error::MissingTitle);
        }
        if self.identifier.is_empty() {
            return Err(Nip99Error::MissingDTag);
        }
        Ok(())
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // d tag (required for addressable events)
        tags.push(vec!["d".to_string(), self.identifier.clone()]);

        // title
        tags.push(vec!["title".to_string(), self.title.clone()]);

        // summary
        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        // published_at
        if let Some(published_at) = self.published_at {
            tags.push(vec!["published_at".to_string(), published_at.to_string()]);
        }

        // location
        if let Some(location) = &self.location {
            tags.push(vec!["location".to_string(), location.clone()]);
        }

        // geohash
        if let Some(geohash) = &self.geohash {
            tags.push(vec!["g".to_string(), geohash.clone()]);
        }

        // price
        if let Some(price) = &self.price {
            tags.push(price.to_tag());
        }

        // status
        if let Some(status) = &self.status {
            tags.push(vec!["status".to_string(), status.as_str().to_string()]);
        }

        // images
        for image in &self.images {
            tags.push(image.to_tag());
        }

        // tags/categories
        for tag in &self.tags {
            tags.push(vec!["t".to_string(), tag.clone()]);
        }

        tags
    }
}

/// A draft or inactive listing (kind 30403).
///
/// Same structure as ClassifiedListing but used for drafts or inactive listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftListing {
    /// The underlying listing data
    pub listing: ClassifiedListing,
}

impl DraftListing {
    /// Create a new draft listing.
    pub fn new(listing: ClassifiedListing) -> Self {
        Self { listing }
    }

    /// Convert to active listing.
    pub fn into_active(self) -> ClassifiedListing {
        self.listing
    }

    /// Validate the draft listing.
    pub fn validate(&self) -> Result<(), Nip99Error> {
        self.listing.validate()
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        self.listing.to_tags()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nip99_kind() {
        assert!(is_nip99_kind(30402));
        assert!(is_nip99_kind(30403));
        assert!(!is_nip99_kind(1));
        assert!(!is_nip99_kind(30404));
    }

    #[test]
    fn test_is_classified_listing_kind() {
        assert!(is_classified_listing_kind(30402));
        assert!(!is_classified_listing_kind(30403));
    }

    #[test]
    fn test_is_draft_listing_kind() {
        assert!(is_draft_listing_kind(30403));
        assert!(!is_draft_listing_kind(30402));
    }

    #[test]
    fn test_listing_status() {
        assert_eq!(ListingStatus::Active.as_str(), "active");
        assert_eq!(ListingStatus::Sold.as_str(), "sold");

        assert_eq!(
            ListingStatus::from_str("active").unwrap(),
            ListingStatus::Active
        );
        assert_eq!(
            ListingStatus::from_str("SOLD").unwrap(),
            ListingStatus::Sold
        );
        assert!(ListingStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_price_one_time() {
        let price = Price::one_time("50", "USD");
        assert_eq!(price.amount, "50");
        assert_eq!(price.currency, "USD");
        assert_eq!(price.frequency, None);

        let tag = price.to_tag();
        assert_eq!(tag, vec!["price", "50", "USD"]);
    }

    #[test]
    fn test_price_recurring() {
        let price = Price::recurring("15", "EUR", "month");
        assert_eq!(price.amount, "15");
        assert_eq!(price.currency, "EUR");
        assert_eq!(price.frequency, Some("month".to_string()));

        let tag = price.to_tag();
        assert_eq!(tag, vec!["price", "15", "EUR", "month"]);
    }

    #[test]
    fn test_price_from_tag() {
        let tag = vec!["price".to_string(), "100".to_string(), "BTC".to_string()];
        let price = Price::from_tag(&tag).unwrap();
        assert_eq!(price.amount, "100");
        assert_eq!(price.currency, "BTC");
        assert_eq!(price.frequency, None);

        let tag = vec![
            "price".to_string(),
            "50000".to_string(),
            "GBP".to_string(),
            "year".to_string(),
        ];
        let price = Price::from_tag(&tag).unwrap();
        assert_eq!(price.amount, "50000");
        assert_eq!(price.currency, "GBP");
        assert_eq!(price.frequency, Some("year".to_string()));
    }

    #[test]
    fn test_price_from_tag_invalid() {
        let tag = vec!["wrong".to_string(), "50".to_string()];
        assert!(Price::from_tag(&tag).is_err());

        let tag = vec!["price".to_string(), "50".to_string()];
        assert!(Price::from_tag(&tag).is_err());
    }

    #[test]
    fn test_listing_image() {
        let image = ListingImage::new("https://example.com/img.jpg");
        assert_eq!(image.url, "https://example.com/img.jpg");
        assert_eq!(image.dimensions, None);

        let tag = image.to_tag();
        assert_eq!(tag, vec!["image", "https://example.com/img.jpg"]);
    }

    #[test]
    fn test_listing_image_with_dimensions() {
        let image = ListingImage::with_dimensions("https://example.com/img.jpg", "256x256");
        assert_eq!(image.url, "https://example.com/img.jpg");
        assert_eq!(image.dimensions, Some("256x256".to_string()));

        let tag = image.to_tag();
        assert_eq!(tag, vec!["image", "https://example.com/img.jpg", "256x256"]);
    }

    #[test]
    fn test_classified_listing_new() {
        let listing = ClassifiedListing::new(
            "headphones-123",
            "Premium wireless headphones",
            "Premium Headphones",
        );

        assert_eq!(listing.identifier, "headphones-123");
        assert_eq!(listing.content, "Premium wireless headphones");
        assert_eq!(listing.title, "Premium Headphones");
        assert_eq!(listing.summary, None);
        assert_eq!(listing.price, None);
        assert_eq!(listing.status, None);
    }

    #[test]
    fn test_classified_listing_builder() {
        let listing = ClassifiedListing::new("id", "content", "title")
            .with_summary("A great product")
            .with_published_at(1675642635)
            .with_location("NYC")
            .with_geohash("dr5regw")
            .with_price(Price::one_time("100", "USD"))
            .with_status(ListingStatus::Active);

        assert_eq!(listing.summary, Some("A great product".to_string()));
        assert_eq!(listing.published_at, Some(1675642635));
        assert_eq!(listing.location, Some("NYC".to_string()));
        assert_eq!(listing.geohash, Some("dr5regw".to_string()));
        assert!(listing.price.is_some());
        assert_eq!(listing.status, Some(ListingStatus::Active));
    }

    #[test]
    fn test_classified_listing_add_image() {
        let mut listing = ClassifiedListing::new("id", "content", "title");
        listing.add_image(ListingImage::new("https://example.com/img1.jpg"));
        listing.add_image(ListingImage::with_dimensions(
            "https://example.com/img2.jpg",
            "512x512",
        ));

        assert_eq!(listing.images.len(), 2);
        assert_eq!(listing.images[0].url, "https://example.com/img1.jpg");
        assert_eq!(listing.images[1].dimensions, Some("512x512".to_string()));
    }

    #[test]
    fn test_classified_listing_add_tag() {
        let mut listing = ClassifiedListing::new("id", "content", "title");
        listing.add_tag("electronics");
        listing.add_tag("headphones");

        assert_eq!(listing.tags.len(), 2);
        assert_eq!(listing.tags[0], "electronics");
        assert_eq!(listing.tags[1], "headphones");
    }

    #[test]
    fn test_classified_listing_validate() {
        let listing = ClassifiedListing::new("id", "content", "title");
        assert!(listing.validate().is_ok());

        let listing = ClassifiedListing::new("id", "content", "");
        assert!(listing.validate().is_err());

        let listing = ClassifiedListing::new("", "content", "title");
        assert!(listing.validate().is_err());
    }

    #[test]
    fn test_classified_listing_to_tags() {
        let mut listing = ClassifiedListing::new("test-id", "Test content", "Test Title")
            .with_summary("Test summary")
            .with_published_at(1675642635)
            .with_location("NYC")
            .with_geohash("dr5regw")
            .with_price(Price::one_time("100", "USD"))
            .with_status(ListingStatus::Active);

        listing.add_image(ListingImage::with_dimensions(
            "https://example.com/img.jpg",
            "256x256",
        ));
        listing.add_tag("electronics");

        let tags = listing.to_tags();

        // Check for required tags
        assert!(tags.iter().any(|tag| tag[0] == "d" && tag[1] == "test-id"));
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "title" && tag[1] == "Test Title")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "summary" && tag[1] == "Test summary")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "published_at" && tag[1] == "1675642635")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "location" && tag[1] == "NYC")
        );
        assert!(tags.iter().any(|tag| tag[0] == "g" && tag[1] == "dr5regw"));
        assert!(
            tags.iter().any(|tag| tag.len() == 3
                && tag[0] == "price"
                && tag[1] == "100"
                && tag[2] == "USD")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "status" && tag[1] == "active")
        );
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "image"
            && tag[1] == "https://example.com/img.jpg"
            && tag[2] == "256x256"));
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "t" && tag[1] == "electronics")
        );
    }

    #[test]
    fn test_draft_listing() {
        let listing = ClassifiedListing::new("id", "content", "title");
        let draft = DraftListing::new(listing.clone());

        assert_eq!(draft.listing, listing);
        assert!(draft.validate().is_ok());

        let active = draft.into_active();
        assert_eq!(active, listing);
    }

    #[test]
    fn test_draft_listing_to_tags() {
        let listing = ClassifiedListing::new("id", "content", "title");
        let draft = DraftListing::new(listing.clone());

        assert_eq!(draft.to_tags(), listing.to_tags());
    }
}
