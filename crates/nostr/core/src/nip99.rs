//! NIP-99: Classified Listings
//!
//! This module implements kind `30402` classified listings and kind `30403`
//! draft listings. OpenAgents uses this surface as an optional public catalog
//! wrapper around canonical DS listings and offers.

use crate::nip01::{Event, EventTemplate, is_addressable_kind};
use crate::tag_parsing::{
    collect_tag_values, find_tag_value, parse_tag_value, tag_field, tag_name,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for active classified listings.
pub const KIND_CLASSIFIED_LISTING: u16 = 30402;

/// Kind number for draft/inactive classified listings.
pub const KIND_DRAFT_LISTING: u16 = 30403;

/// Errors that can occur during NIP-99 operations.
#[derive(Debug, Error)]
pub enum Nip99Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("listing must have a title")]
    MissingTitle,

    #[error("listing must have a d tag identifier")]
    MissingDTag,

    #[error("invalid price format")]
    InvalidPrice,

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("invalid coordinate kind: {0}")]
    InvalidCoordinateKind(u16),

    #[error("invalid lowercase hex field `{field}`: {value}")]
    InvalidHexField { field: &'static str, value: String },
}

/// Check if a kind is a NIP-99 classified listing kind.
pub fn is_nip99_kind(kind: u16) -> bool {
    kind == KIND_CLASSIFIED_LISTING || kind == KIND_DRAFT_LISTING
}

/// Check if a kind is an active classified listing.
pub fn is_classified_listing_kind(kind: u16) -> bool {
    kind == KIND_CLASSIFIED_LISTING
}

/// Check if a kind is a draft listing.
pub fn is_draft_listing_kind(kind: u16) -> bool {
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
    /// Convert status to string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Sold => "sold",
        }
    }
}

impl std::str::FromStr for ListingStatus {
    type Err = Nip99Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "sold" => Ok(Self::Sold),
            _ => Err(Nip99Error::InvalidStatus(s.to_string())),
        }
    }
}

/// Price information for a listing.
///
/// Format: `["price", amount, currency, frequency?]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Amount as a string (numeric format)
    pub amount: String,
    /// Currency code (ISO 4217 or crypto like `SAT`)
    pub currency: String,
    /// Optional frequency for recurring payments (hour, day, week, month, year)
    pub frequency: Option<String>,
}

impl Price {
    /// Create a one-time price.
    pub fn one_time(amount: impl Into<String>, currency: impl Into<String>) -> Self {
        Self {
            amount: amount.into(),
            currency: currency.into(),
            frequency: None,
        }
    }

    /// Create a recurring price.
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
        if tag.len() < 3 || tag_name(tag) != Some("price") {
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
    /// Optional dimensions (e.g. `256x256`)
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

    /// Parse from tag format.
    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag_name(tag) != Some("image") {
            return None;
        }
        let url = tag_field(tag, 1)?.trim();
        if url.is_empty() {
            return None;
        }
        Some(Self {
            url: url.to_string(),
            dimensions: tag_field(tag, 2)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
    }
}

/// A classified listing (kind `30402`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifiedListing {
    /// The `d` tag identifier (required for addressable events)
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
    /// Categories / `t` tags
    pub tags: Vec<String>,
    /// Related addressable-event references from `a` tags
    pub address_refs: Vec<String>,
    /// Related event references from `e` tags
    pub event_refs: Vec<String>,
}

impl ClassifiedListing {
    /// Create a new classified listing.
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
            address_refs: Vec::new(),
            event_refs: Vec::new(),
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

    /// Add an `a`-tag reference.
    pub fn add_address_ref(&mut self, coordinate: impl Into<String>) {
        self.address_refs.push(coordinate.into());
    }

    /// Add an `e`-tag reference.
    pub fn add_event_ref(&mut self, event_id: impl Into<String>) {
        self.event_refs.push(event_id.into());
    }

    /// Validate the listing.
    pub fn validate(&self) -> Result<(), Nip99Error> {
        if self.title.trim().is_empty() {
            return Err(Nip99Error::MissingTitle);
        }
        if self.identifier.trim().is_empty() {
            return Err(Nip99Error::MissingDTag);
        }
        for event_id in &self.event_refs {
            validate_lower_hex("e", event_id)?;
        }
        Ok(())
    }

    /// Derive the addressable coordinate string for this listing.
    pub fn coordinate(&self, publisher_pubkey: impl Into<String>) -> Result<String, Nip99Error> {
        coordinate_for_kind(
            KIND_CLASSIFIED_LISTING,
            publisher_pubkey.into().as_str(),
            self.identifier.as_str(),
        )
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        tags.push(vec!["d".to_string(), self.identifier.clone()]);
        tags.push(vec!["title".to_string(), self.title.clone()]);

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }
        if let Some(published_at) = self.published_at {
            tags.push(vec!["published_at".to_string(), published_at.to_string()]);
        }
        if let Some(location) = &self.location {
            tags.push(vec!["location".to_string(), location.clone()]);
        }
        if let Some(geohash) = &self.geohash {
            tags.push(vec!["g".to_string(), geohash.clone()]);
        }
        if let Some(price) = &self.price {
            tags.push(price.to_tag());
        }
        if let Some(status) = &self.status {
            tags.push(vec!["status".to_string(), status.as_str().to_string()]);
        }
        for image in &self.images {
            tags.push(image.to_tag());
        }
        for tag in &self.tags {
            tags.push(vec!["t".to_string(), tag.clone()]);
        }
        for address_ref in &self.address_refs {
            tags.push(vec!["a".to_string(), address_ref.clone()]);
        }
        for event_ref in &self.event_refs {
            tags.push(vec!["e".to_string(), event_ref.clone()]);
        }

        tags
    }

    /// Convert into an active kind `30402` event template.
    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, Nip99Error> {
        self.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CLASSIFIED_LISTING,
            tags: self.to_tags(),
            content: self.content.clone(),
        })
    }

    /// Parse an active kind `30402` event.
    pub fn from_event(event: &Event) -> Result<Self, Nip99Error> {
        if event.kind != KIND_CLASSIFIED_LISTING {
            return Err(Nip99Error::InvalidKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: event.kind,
            });
        }
        parse_classified_listing(event)
    }
}

impl TryFrom<&Event> for ClassifiedListing {
    type Error = Nip99Error;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

impl TryFrom<Event> for ClassifiedListing {
    type Error = Nip99Error;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Self::from_event(&event)
    }
}

/// A draft or inactive listing (kind `30403`).
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

    /// Derive the addressable coordinate string for this draft.
    pub fn coordinate(&self, publisher_pubkey: impl Into<String>) -> Result<String, Nip99Error> {
        coordinate_for_kind(
            KIND_DRAFT_LISTING,
            publisher_pubkey.into().as_str(),
            self.listing.identifier.as_str(),
        )
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        self.listing.to_tags()
    }

    /// Convert into a kind `30403` event template.
    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, Nip99Error> {
        self.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_DRAFT_LISTING,
            tags: self.listing.to_tags(),
            content: self.listing.content.clone(),
        })
    }

    /// Parse a draft kind `30403` event.
    pub fn from_event(event: &Event) -> Result<Self, Nip99Error> {
        if event.kind != KIND_DRAFT_LISTING {
            return Err(Nip99Error::InvalidKind {
                expected: KIND_DRAFT_LISTING,
                actual: event.kind,
            });
        }
        Ok(Self {
            listing: parse_classified_listing(event)?,
        })
    }
}

impl TryFrom<&Event> for DraftListing {
    type Error = Nip99Error;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

impl TryFrom<Event> for DraftListing {
    type Error = Nip99Error;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Self::from_event(&event)
    }
}

fn parse_classified_listing(event: &Event) -> Result<ClassifiedListing, Nip99Error> {
    let identifier = find_tag_value(&event.tags, "d")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(Nip99Error::MissingDTag)?
        .to_string();
    let title = find_tag_value(&event.tags, "title")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(Nip99Error::MissingTitle)?
        .to_string();
    let price = event
        .tags
        .iter()
        .find(|tag| tag_name(tag) == Some("price"))
        .map(|tag| Price::from_tag(tag))
        .transpose()?;
    let status = find_tag_value(&event.tags, "status")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::parse::<ListingStatus>)
        .transpose()?;
    let images = event
        .tags
        .iter()
        .filter_map(|tag| ListingImage::from_tag(tag))
        .collect();
    let event_refs = collect_tag_values(&event.tags, "e");
    for event_id in &event_refs {
        validate_lower_hex("e", event_id)?;
    }

    Ok(ClassifiedListing {
        identifier,
        content: event.content.clone(),
        title,
        summary: find_tag_value(&event.tags, "summary")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        published_at: parse_tag_value(&event.tags, "published_at"),
        location: find_tag_value(&event.tags, "location")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        geohash: find_tag_value(&event.tags, "g")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        price,
        status,
        images,
        tags: collect_tag_values(&event.tags, "t"),
        address_refs: collect_tag_values(&event.tags, "a"),
        event_refs,
    })
}

fn coordinate_for_kind(kind: u16, pubkey: &str, identifier: &str) -> Result<String, Nip99Error> {
    if !is_addressable_kind(kind) {
        return Err(Nip99Error::InvalidCoordinateKind(kind));
    }
    validate_lower_hex("pubkey", pubkey)?;
    if identifier.trim().is_empty() {
        return Err(Nip99Error::MissingDTag);
    }
    Ok(format!("{kind}:{pubkey}:{identifier}"))
}

fn validate_lower_hex(field: &'static str, value: &str) -> Result<(), Nip99Error> {
    let trimmed = value.trim();
    if trimmed.len() != 64
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(Nip99Error::InvalidHexField {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_template(identity: &crate::NostrIdentity, template: &EventTemplate) -> Event {
        let key_bytes = hex::decode(identity.private_key_hex.as_str()).expect("private key hex");
        let mut private_key = [0_u8; 32];
        private_key.copy_from_slice(key_bytes.as_slice());
        crate::finalize_event(template, &private_key).expect("sign event")
    }

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

        assert!(matches!(
            "active".parse::<ListingStatus>(),
            Ok(ListingStatus::Active)
        ));
        assert!(matches!(
            "SOLD".parse::<ListingStatus>(),
            Ok(ListingStatus::Sold)
        ));
        assert!("invalid".parse::<ListingStatus>().is_err());
    }

    #[test]
    fn test_price_one_time() {
        let price = Price::one_time("50", "USD");
        assert_eq!(price.amount, "50");
        assert_eq!(price.currency, "USD");
        assert_eq!(price.frequency, None);
        assert_eq!(price.to_tag(), vec!["price", "50", "USD"]);
    }

    #[test]
    fn test_price_recurring() {
        let price = Price::recurring("15", "EUR", "month");
        assert_eq!(price.amount, "15");
        assert_eq!(price.currency, "EUR");
        assert_eq!(price.frequency, Some("month".to_string()));
        assert_eq!(price.to_tag(), vec!["price", "15", "EUR", "month"]);
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
        assert_eq!(image.to_tag(), vec!["image", "https://example.com/img.jpg"]);
    }

    #[test]
    fn test_listing_image_with_dimensions() {
        let image = ListingImage::with_dimensions("https://example.com/img.jpg", "256x256");
        assert_eq!(image.url, "https://example.com/img.jpg");
        assert_eq!(image.dimensions, Some("256x256".to_string()));
        assert_eq!(
            image.to_tag(),
            vec!["image", "https://example.com/img.jpg", "256x256"]
        );
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
    fn test_classified_listing_add_image_and_refs() {
        let mut listing = ClassifiedListing::new("id", "content", "title");
        listing.add_image(ListingImage::new("https://example.com/img1.jpg"));
        listing.add_image(ListingImage::with_dimensions(
            "https://example.com/img2.jpg",
            "512x512",
        ));
        listing.add_tag("electronics");
        listing.add_tag("headphones");
        listing.add_address_ref(
            "30404:1111111111111111111111111111111111111111111111111111111111111111:dataset-1",
        );
        listing.add_event_ref("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

        assert_eq!(listing.images.len(), 2);
        assert_eq!(listing.tags.len(), 2);
        assert_eq!(listing.address_refs.len(), 1);
        assert_eq!(listing.event_refs.len(), 1);
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
        listing.add_address_ref(
            "30404:1111111111111111111111111111111111111111111111111111111111111111:dataset-1",
        );
        listing.add_event_ref("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

        let tags = listing.to_tags();

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
        assert!(
            tags.iter().any(|tag| tag[0] == "a"
                && tag[1]
                    == "30404:1111111111111111111111111111111111111111111111111111111111111111:dataset-1")
        );
        assert!(tags.iter().any(|tag| tag[0] == "e"
            && tag[1] == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn test_classified_listing_round_trip_from_event_preserves_ds_refs() {
        let identity = crate::regenerate_identity().expect("identity");
        let mut listing = ClassifiedListing::new(
            "dataset-alpha-classified",
            "Public market wrapper for a dataset.",
            "Dataset Alpha",
        )
        .with_summary("Catalog wrapper")
        .with_published_at(1_774_160_000)
        .with_price(Price::one_time("42", "SAT"))
        .with_status(ListingStatus::Active);
        listing.add_tag("dataset");
        listing.add_address_ref(format!("30404:{}:dataset-alpha", identity.public_key_hex));
        listing.add_address_ref(format!(
            "30406:{}:dataset-alpha-offer",
            identity.public_key_hex
        ));
        listing.add_event_ref("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        let template = listing.to_event_template(1_774_160_001).expect("template");
        let event = sign_template(&identity, &template);
        let parsed = ClassifiedListing::from_event(&event).expect("parsed");

        assert_eq!(parsed.identifier, listing.identifier);
        assert_eq!(parsed.title, listing.title);
        assert_eq!(parsed.summary, listing.summary);
        assert_eq!(parsed.price, listing.price);
        assert_eq!(parsed.tags, listing.tags);
        assert_eq!(parsed.address_refs, listing.address_refs);
        assert_eq!(parsed.event_refs, listing.event_refs);
        assert_eq!(
            parsed.coordinate(identity.public_key_hex.clone()).unwrap(),
            format!("30402:{}:dataset-alpha-classified", identity.public_key_hex)
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
    fn test_draft_listing_round_trip_from_event() {
        let identity = crate::regenerate_identity().expect("identity");
        let listing = ClassifiedListing::new("draft-id", "draft content", "Draft Title");
        let draft = DraftListing::new(listing.clone());
        let template = draft.to_event_template(1_774_160_100).expect("template");
        let event = sign_template(&identity, &template);
        let parsed = DraftListing::from_event(&event).expect("parsed draft");

        assert_eq!(parsed.listing.identifier, "draft-id");
        assert_eq!(parsed.listing.title, "Draft Title");
        assert_eq!(
            parsed.coordinate(identity.public_key_hex.clone()).unwrap(),
            format!("30403:{}:draft-id", identity.public_key_hex)
        );
    }
}
