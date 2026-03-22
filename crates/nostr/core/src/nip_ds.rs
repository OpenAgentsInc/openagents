//! NIP-DS: dataset listings and offers.
//!
//! This module implements the in-repo runtime surface for the draft DS protocol:
//! - `kind:30404` dataset listings
//! - `kind:30405` draft / inactive dataset listings
//! - `kind:30406` dataset offers
//!
//! It intentionally stops at the canonical listing / offer layer. The optional
//! DS-DVM request/result profile remains separate work.

use crate::nip01::{Event, EventTemplate, is_addressable_kind};
use crate::nip99::Price;
use crate::tag_parsing::{
    collect_tag_values, find_tag_value, parse_tag_value, tag_field, tag_name,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Active dataset listing kind.
pub const KIND_DATASET_LISTING: u16 = 30404;

/// Draft / inactive dataset listing kind.
pub const KIND_DRAFT_DATASET_LISTING: u16 = 30405;

/// Dataset offer kind.
pub const KIND_DATASET_OFFER: u16 = 30406;

/// Optional DS-DVM access-request kind.
pub const KIND_DATASET_ACCESS_REQUEST: u16 = 5960;

/// Optional DS-DVM access-result kind.
pub const KIND_DATASET_ACCESS_RESULT: u16 = 6960;

/// Errors that can occur while handling DS events.
#[derive(Debug, Error)]
pub enum NipDsError {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid addressable coordinate: {0}")]
    InvalidCoordinate(String),

    #[error("coordinate must point at an addressable kind: {0}")]
    NonAddressableCoordinate(String),

    #[error("dataset listing references unsupported kind: {0}")]
    InvalidListingReferenceKind(u16),

    #[error("dataset digest must be a lowercase 64-character hex string")]
    InvalidDigest,

    #[error("invalid hex field `{field}`: {value}")]
    InvalidHexField { field: &'static str, value: String },

    #[error("invalid price tag")]
    InvalidPrice,

    #[error("invalid offer status: {0}")]
    InvalidOfferStatus(String),

    #[error("missing delivery tag")]
    MissingDelivery,
}

/// Check whether a kind belongs to the DS core surface.
pub fn is_nip_ds_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_DATASET_LISTING | KIND_DRAFT_DATASET_LISTING | KIND_DATASET_OFFER
    )
}

/// Check whether a kind is a dataset listing.
pub fn is_dataset_listing_kind(kind: u16) -> bool {
    kind == KIND_DATASET_LISTING
}

/// Check whether a kind is a draft dataset listing.
pub fn is_draft_dataset_listing_kind(kind: u16) -> bool {
    kind == KIND_DRAFT_DATASET_LISTING
}

/// Check whether a kind is a dataset offer.
pub fn is_dataset_offer_kind(kind: u16) -> bool {
    kind == KIND_DATASET_OFFER
}

/// Parsed NIP-01 addressable-event coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressableEventCoordinate {
    pub kind: u16,
    pub pubkey: String,
    pub identifier: String,
}

impl AddressableEventCoordinate {
    /// Create a new validated coordinate.
    pub fn new(
        kind: u16,
        pubkey: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Result<Self, NipDsError> {
        let pubkey = pubkey.into();
        let identifier = identifier.into();
        validate_addressable_coordinate_parts(kind, &pubkey, &identifier)?;
        Ok(Self {
            kind,
            pubkey,
            identifier,
        })
    }

    /// Create a dataset-listing coordinate.
    pub fn dataset_listing(
        pubkey: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Result<Self, NipDsError> {
        Self::new(KIND_DATASET_LISTING, pubkey, identifier)
    }

    /// Create a dataset-offer coordinate.
    pub fn dataset_offer(
        pubkey: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Result<Self, NipDsError> {
        Self::new(KIND_DATASET_OFFER, pubkey, identifier)
    }

    /// Parse a `<kind>:<pubkey>:<d-tag>` coordinate.
    pub fn parse(value: &str) -> Result<Self, NipDsError> {
        let mut parts = value.splitn(3, ':');
        let kind = parts
            .next()
            .ok_or_else(|| NipDsError::InvalidCoordinate(value.to_string()))?
            .parse::<u16>()
            .map_err(|_| NipDsError::InvalidCoordinate(value.to_string()))?;
        let pubkey = parts
            .next()
            .ok_or_else(|| NipDsError::InvalidCoordinate(value.to_string()))?;
        let identifier = parts
            .next()
            .ok_or_else(|| NipDsError::InvalidCoordinate(value.to_string()))?;
        Self::new(kind, pubkey, identifier)
    }

    /// Build the DS scope identifier `<coordinate>:<sha256_digest>`.
    pub fn scope_id(&self, digest: &str) -> Result<String, NipDsError> {
        validate_sha256_hex(digest).map_err(|_| NipDsError::InvalidDigest)?;
        Ok(format!("{self}:{digest}"))
    }
}

impl std::fmt::Display for AddressableEventCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.kind, self.pubkey, self.identifier)
    }
}

impl std::str::FromStr for AddressableEventCoordinate {
    type Err = NipDsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// `e` tag with optional relay and marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventReference {
    pub event_id: String,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

impl EventReference {
    pub fn new(event_id: impl Into<String>) -> Result<Self, NipDsError> {
        let event_id = event_id.into();
        validate_lower_hex("e", &event_id)?;
        Ok(Self {
            event_id,
            relay: None,
            marker: None,
        })
    }

    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }

    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["e".to_string(), self.event_id.clone()];
        if self.relay.is_some() || self.marker.is_some() {
            tag.push(self.relay.clone().unwrap_or_default());
        }
        if let Some(marker) = &self.marker {
            tag.push(marker.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Result<Self, NipDsError> {
        if tag_name(tag) != Some("e") {
            return Err(NipDsError::MissingRequiredTag("e"));
        }
        let event_id = tag_field(tag, 1).ok_or(NipDsError::MissingRequiredTag("e"))?;
        validate_lower_hex("e", event_id)?;
        Ok(Self {
            event_id: event_id.to_string(),
            relay: tag_field(tag, 2)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            marker: tag_field(tag, 3).map(str::to_owned),
        })
    }
}

/// `a` tag with optional relay and marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressableEventReference {
    pub coordinate: AddressableEventCoordinate,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

impl AddressableEventReference {
    pub fn new(coordinate: AddressableEventCoordinate) -> Self {
        Self {
            coordinate,
            relay: None,
            marker: None,
        }
    }

    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }

    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["a".to_string(), self.coordinate.to_string()];
        if self.relay.is_some() || self.marker.is_some() {
            tag.push(self.relay.clone().unwrap_or_default());
        }
        if let Some(marker) = &self.marker {
            tag.push(marker.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Result<Self, NipDsError> {
        if tag_name(tag) != Some("a") {
            return Err(NipDsError::MissingRequiredTag("a"));
        }
        let coordinate = AddressableEventCoordinate::parse(
            tag_field(tag, 1).ok_or(NipDsError::MissingRequiredTag("a"))?,
        )?;
        Ok(Self {
            coordinate,
            relay: tag_field(tag, 2)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            marker: tag_field(tag, 3).map(str::to_owned),
        })
    }
}

/// `p` tag with optional relay hint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyReference {
    pub pubkey: String,
    pub relay: Option<String>,
}

impl PublicKeyReference {
    pub fn new(pubkey: impl Into<String>) -> Result<Self, NipDsError> {
        let pubkey = pubkey.into();
        validate_lower_hex("p", &pubkey)?;
        Ok(Self {
            pubkey,
            relay: None,
        })
    }

    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["p".to_string(), self.pubkey.clone()];
        if let Some(relay) = &self.relay {
            tag.push(relay.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Result<Self, NipDsError> {
        if tag_name(tag) != Some("p") {
            return Err(NipDsError::MissingRequiredTag("p"));
        }
        let pubkey = tag_field(tag, 1).ok_or(NipDsError::MissingRequiredTag("p"))?;
        validate_lower_hex("p", pubkey)?;
        Ok(Self {
            pubkey: pubkey.to_string(),
            relay: tag_field(tag, 2)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        })
    }
}

/// Payment rail declared on a dataset offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub rail: String,
    pub details: Vec<String>,
}

impl PaymentMethod {
    pub fn new(rail: impl Into<String>) -> Self {
        Self {
            rail: rail.into(),
            details: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["payment".to_string(), self.rail.clone()];
        tag.extend(self.details.iter().cloned());
        tag
    }

    pub fn from_tag(tag: &[String]) -> Result<Self, NipDsError> {
        if tag_name(tag) != Some("payment") {
            return Err(NipDsError::MissingRequiredTag("payment"));
        }
        let rail = tag_field(tag, 1).ok_or(NipDsError::MissingRequiredTag("payment"))?;
        Ok(Self {
            rail: rail.to_string(),
            details: tag.iter().skip(2).cloned().collect(),
        })
    }
}

/// Dataset-offer lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetOfferStatus {
    Active,
    Inactive,
    Revoked,
    Expired,
}

impl DatasetOfferStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, NipDsError> {
        match value.to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "revoked" => Ok(Self::Revoked),
            "expired" => Ok(Self::Expired),
            _ => Err(NipDsError::InvalidOfferStatus(value.to_string())),
        }
    }
}

/// Canonical dataset listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetListing {
    pub identifier: String,
    pub content: String,
    pub title: String,
    pub digest: String,
    pub published_at: Option<u64>,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub dataset_kind: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub records: Option<u64>,
    pub license: Option<String>,
    pub access: Option<String>,
    pub delivery_modes: Vec<String>,
    pub topics: Vec<String>,
    pub event_refs: Vec<EventReference>,
    pub address_refs: Vec<AddressableEventReference>,
}

impl DatasetListing {
    /// Create a new dataset listing.
    pub fn new(
        identifier: impl Into<String>,
        content: impl Into<String>,
        title: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            content: content.into(),
            title: title.into(),
            digest: digest.into(),
            published_at: None,
            summary: None,
            version: None,
            dataset_kind: None,
            mime_type: None,
            size_bytes: None,
            records: None,
            license: None,
            access: None,
            delivery_modes: Vec::new(),
            topics: Vec::new(),
            event_refs: Vec::new(),
            address_refs: Vec::new(),
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn with_published_at(mut self, published_at: u64) -> Self {
        self.published_at = Some(published_at);
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn with_dataset_kind(mut self, dataset_kind: impl Into<String>) -> Self {
        self.dataset_kind = Some(dataset_kind.into());
        self
    }

    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    pub fn with_records(mut self, records: u64) -> Self {
        self.records = Some(records);
        self
    }

    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    pub fn with_access(mut self, access: impl Into<String>) -> Self {
        self.access = Some(access.into());
        self
    }

    pub fn add_delivery_mode(mut self, delivery_mode: impl Into<String>) -> Self {
        self.delivery_modes.push(delivery_mode.into());
        self
    }

    pub fn add_topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn add_event_ref(mut self, event_ref: EventReference) -> Self {
        self.event_refs.push(event_ref);
        self
    }

    pub fn add_address_ref(mut self, address_ref: AddressableEventReference) -> Self {
        self.address_refs.push(address_ref);
        self
    }

    pub fn coordinate(
        &self,
        publisher_pubkey: impl Into<String>,
    ) -> Result<AddressableEventCoordinate, NipDsError> {
        AddressableEventCoordinate::dataset_listing(publisher_pubkey, self.identifier.clone())
    }

    pub fn validate(&self) -> Result<(), NipDsError> {
        self.validate_for_kind(KIND_DATASET_LISTING)
    }

    pub fn validate_for_draft(&self) -> Result<(), NipDsError> {
        self.validate_for_kind(KIND_DRAFT_DATASET_LISTING)
    }

    pub fn to_tags(&self, kind: u16) -> Result<Vec<Vec<String>>, NipDsError> {
        self.validate_for_kind(kind)?;

        let mut tags = vec![
            vec!["d".to_string(), self.identifier.clone()],
            vec!["title".to_string(), self.title.clone()],
            vec!["x".to_string(), self.digest.clone()],
        ];

        if let Some(published_at) = self.published_at {
            tags.push(vec!["published_at".to_string(), published_at.to_string()]);
        }
        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }
        if let Some(version) = &self.version {
            tags.push(vec!["version".to_string(), version.clone()]);
        }
        if let Some(dataset_kind) = &self.dataset_kind {
            tags.push(vec!["dataset_kind".to_string(), dataset_kind.clone()]);
        }
        if let Some(mime_type) = &self.mime_type {
            tags.push(vec!["m".to_string(), mime_type.clone()]);
        }
        if let Some(size_bytes) = self.size_bytes {
            tags.push(vec!["size".to_string(), size_bytes.to_string()]);
        }
        if let Some(records) = self.records {
            tags.push(vec!["records".to_string(), records.to_string()]);
        }
        if let Some(license) = &self.license {
            tags.push(vec!["license".to_string(), license.clone()]);
        }
        if let Some(access) = &self.access {
            tags.push(vec!["access".to_string(), access.clone()]);
        }
        for delivery_mode in &self.delivery_modes {
            tags.push(vec!["delivery".to_string(), delivery_mode.clone()]);
        }
        for topic in &self.topics {
            tags.push(vec!["t".to_string(), topic.clone()]);
        }
        for event_ref in &self.event_refs {
            tags.push(event_ref.to_tag());
        }
        for address_ref in &self.address_refs {
            tags.push(address_ref.to_tag());
        }

        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipDsError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_DATASET_LISTING,
            tags: self.to_tags(KIND_DATASET_LISTING)?,
            content: self.content.clone(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipDsError> {
        if event.kind != KIND_DATASET_LISTING {
            return Err(NipDsError::InvalidKind {
                expected: KIND_DATASET_LISTING,
                actual: event.kind,
            });
        }
        parse_listing(event, true)
    }

    fn validate_for_kind(&self, kind: u16) -> Result<(), NipDsError> {
        if self.identifier.trim().is_empty() {
            return Err(NipDsError::MissingRequiredTag("d"));
        }
        if self.title.trim().is_empty() {
            return Err(NipDsError::MissingRequiredTag("title"));
        }
        validate_sha256_hex(&self.digest).map_err(|_| NipDsError::InvalidDigest)?;
        if kind == KIND_DATASET_LISTING && self.published_at.is_none() {
            return Err(NipDsError::MissingRequiredTag("published_at"));
        }
        for address_ref in &self.address_refs {
            validate_addressable_coordinate_parts(
                address_ref.coordinate.kind,
                &address_ref.coordinate.pubkey,
                &address_ref.coordinate.identifier,
            )?;
        }
        Ok(())
    }
}

impl TryFrom<&Event> for DatasetListing {
    type Error = NipDsError;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

impl TryFrom<Event> for DatasetListing {
    type Error = NipDsError;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Self::from_event(&event)
    }
}

/// Draft / inactive dataset listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftDatasetListing {
    pub listing: DatasetListing,
}

impl DraftDatasetListing {
    pub fn new(listing: DatasetListing) -> Self {
        Self { listing }
    }

    pub fn coordinate(
        &self,
        publisher_pubkey: impl Into<String>,
    ) -> Result<AddressableEventCoordinate, NipDsError> {
        AddressableEventCoordinate::new(
            KIND_DRAFT_DATASET_LISTING,
            publisher_pubkey,
            self.listing.identifier.clone(),
        )
    }

    pub fn validate(&self) -> Result<(), NipDsError> {
        self.listing.validate_for_draft()
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipDsError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_DRAFT_DATASET_LISTING,
            tags: self.listing.to_tags(KIND_DRAFT_DATASET_LISTING)?,
            content: self.listing.content.clone(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipDsError> {
        if event.kind != KIND_DRAFT_DATASET_LISTING {
            return Err(NipDsError::InvalidKind {
                expected: KIND_DRAFT_DATASET_LISTING,
                actual: event.kind,
            });
        }
        Ok(Self {
            listing: parse_listing(event, false)?,
        })
    }
}

impl TryFrom<&Event> for DraftDatasetListing {
    type Error = NipDsError;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

impl TryFrom<Event> for DraftDatasetListing {
    type Error = NipDsError;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Self::from_event(&event)
    }
}

/// Dataset access terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetOffer {
    pub identifier: String,
    pub content: String,
    pub listing_ref: AddressableEventReference,
    pub status: DatasetOfferStatus,
    pub policy: Option<String>,
    pub price: Option<Price>,
    pub payment_methods: Vec<PaymentMethod>,
    pub delivery_modes: Vec<String>,
    pub license: Option<String>,
    pub targeted_buyers: Vec<PublicKeyReference>,
    pub expiration: Option<String>,
    pub topics: Vec<String>,
    pub related_refs: Vec<AddressableEventReference>,
}

impl DatasetOffer {
    pub fn new(
        identifier: impl Into<String>,
        content: impl Into<String>,
        listing_ref: AddressableEventReference,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            content: content.into(),
            listing_ref,
            status: DatasetOfferStatus::Active,
            policy: None,
            price: None,
            payment_methods: Vec::new(),
            delivery_modes: Vec::new(),
            license: None,
            targeted_buyers: Vec::new(),
            expiration: None,
            topics: Vec::new(),
            related_refs: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: DatasetOfferStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_policy(mut self, policy: impl Into<String>) -> Self {
        self.policy = Some(policy.into());
        self
    }

    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    pub fn add_payment_method(mut self, payment_method: PaymentMethod) -> Self {
        self.payment_methods.push(payment_method);
        self
    }

    pub fn add_delivery_mode(mut self, delivery_mode: impl Into<String>) -> Self {
        self.delivery_modes.push(delivery_mode.into());
        self
    }

    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    pub fn add_targeted_buyer(mut self, buyer: PublicKeyReference) -> Self {
        self.targeted_buyers.push(buyer);
        self
    }

    pub fn with_expiration(mut self, expiration: impl Into<String>) -> Self {
        self.expiration = Some(expiration.into());
        self
    }

    pub fn add_topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn add_related_ref(mut self, related_ref: AddressableEventReference) -> Self {
        self.related_refs.push(related_ref);
        self
    }

    pub fn coordinate(
        &self,
        publisher_pubkey: impl Into<String>,
    ) -> Result<AddressableEventCoordinate, NipDsError> {
        AddressableEventCoordinate::dataset_offer(publisher_pubkey, self.identifier.clone())
    }

    pub fn validate(&self) -> Result<(), NipDsError> {
        if self.identifier.trim().is_empty() {
            return Err(NipDsError::MissingRequiredTag("d"));
        }
        if self.delivery_modes.is_empty() {
            return Err(NipDsError::MissingDelivery);
        }
        if self.listing_ref.coordinate.kind != KIND_DATASET_LISTING {
            return Err(NipDsError::InvalidListingReferenceKind(
                self.listing_ref.coordinate.kind,
            ));
        }
        validate_addressable_coordinate_parts(
            self.listing_ref.coordinate.kind,
            &self.listing_ref.coordinate.pubkey,
            &self.listing_ref.coordinate.identifier,
        )?;
        for related_ref in &self.related_refs {
            validate_addressable_coordinate_parts(
                related_ref.coordinate.kind,
                &related_ref.coordinate.pubkey,
                &related_ref.coordinate.identifier,
            )?;
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, NipDsError> {
        self.validate()?;

        let mut tags = vec![
            vec!["d".to_string(), self.identifier.clone()],
            self.listing_ref.to_tag(),
            vec!["status".to_string(), self.status.as_str().to_string()],
        ];

        if let Some(policy) = &self.policy {
            tags.push(vec!["policy".to_string(), policy.clone()]);
        }
        if let Some(price) = &self.price {
            tags.push(price.to_tag());
        }
        for payment_method in &self.payment_methods {
            tags.push(payment_method.to_tag());
        }
        for delivery_mode in &self.delivery_modes {
            tags.push(vec!["delivery".to_string(), delivery_mode.clone()]);
        }
        if let Some(license) = &self.license {
            tags.push(vec!["license".to_string(), license.clone()]);
        }
        for buyer in &self.targeted_buyers {
            tags.push(buyer.to_tag());
        }
        if let Some(expiration) = &self.expiration {
            tags.push(vec!["expiration".to_string(), expiration.clone()]);
        }
        for topic in &self.topics {
            tags.push(vec!["t".to_string(), topic.clone()]);
        }
        for related_ref in &self.related_refs {
            tags.push(related_ref.to_tag());
        }

        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipDsError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_DATASET_OFFER,
            tags: self.to_tags()?,
            content: self.content.clone(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipDsError> {
        if event.kind != KIND_DATASET_OFFER {
            return Err(NipDsError::InvalidKind {
                expected: KIND_DATASET_OFFER,
                actual: event.kind,
            });
        }

        let identifier = required_tag_value(&event.tags, "d")?;
        let mut address_refs = event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("a"))
            .map(|tag| AddressableEventReference::from_tag(tag))
            .collect::<Result<Vec<_>, _>>()?;
        let listing_ref = address_refs
            .drain(..1)
            .next()
            .ok_or(NipDsError::MissingRequiredTag("a"))?;
        let status = DatasetOfferStatus::parse(&required_tag_value(&event.tags, "status")?)?;
        let policy = find_tag_value(&event.tags, "policy").map(str::to_owned);
        let price = event
            .tags
            .iter()
            .find(|tag| tag_name(tag) == Some("price"))
            .map(|tag| Price::from_tag(tag).map_err(|_| NipDsError::InvalidPrice))
            .transpose()?;
        let payment_methods = event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("payment"))
            .map(|tag| PaymentMethod::from_tag(tag))
            .collect::<Result<Vec<_>, _>>()?;
        let delivery_modes = collect_tag_values(&event.tags, "delivery");
        let license = find_tag_value(&event.tags, "license").map(str::to_owned);
        let targeted_buyers = event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("p"))
            .map(|tag| PublicKeyReference::from_tag(tag))
            .collect::<Result<Vec<_>, _>>()?;
        let expiration = find_tag_value(&event.tags, "expiration").map(str::to_owned);
        let topics = collect_tag_values(&event.tags, "t");

        let offer = Self {
            identifier,
            content: event.content.clone(),
            listing_ref,
            status,
            policy,
            price,
            payment_methods,
            delivery_modes,
            license,
            targeted_buyers,
            expiration,
            topics,
            related_refs: address_refs,
        };
        offer.validate()?;
        Ok(offer)
    }
}

impl TryFrom<&Event> for DatasetOffer {
    type Error = NipDsError;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

impl TryFrom<Event> for DatasetOffer {
    type Error = NipDsError;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Self::from_event(&event)
    }
}

fn parse_listing(event: &Event, require_published_at: bool) -> Result<DatasetListing, NipDsError> {
    let identifier = required_tag_value(&event.tags, "d")?;
    let title = required_tag_value(&event.tags, "title")?;
    let digest = required_tag_value(&event.tags, "x")?;
    validate_sha256_hex(&digest).map_err(|_| NipDsError::InvalidDigest)?;

    let listing = DatasetListing {
        identifier,
        content: event.content.clone(),
        title,
        digest,
        published_at: parse_tag_value::<u64>(&event.tags, "published_at"),
        summary: find_tag_value(&event.tags, "summary").map(str::to_owned),
        version: find_tag_value(&event.tags, "version").map(str::to_owned),
        dataset_kind: find_tag_value(&event.tags, "dataset_kind").map(str::to_owned),
        mime_type: find_tag_value(&event.tags, "m").map(str::to_owned),
        size_bytes: parse_tag_value::<u64>(&event.tags, "size"),
        records: parse_tag_value::<u64>(&event.tags, "records"),
        license: find_tag_value(&event.tags, "license").map(str::to_owned),
        access: find_tag_value(&event.tags, "access").map(str::to_owned),
        delivery_modes: collect_tag_values(&event.tags, "delivery"),
        topics: collect_tag_values(&event.tags, "t"),
        event_refs: event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("e"))
            .map(|tag| EventReference::from_tag(tag))
            .collect::<Result<Vec<_>, _>>()?,
        address_refs: event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("a"))
            .map(|tag| AddressableEventReference::from_tag(tag))
            .collect::<Result<Vec<_>, _>>()?,
    };

    if require_published_at && listing.published_at.is_none() {
        return Err(NipDsError::MissingRequiredTag("published_at"));
    }
    if require_published_at {
        listing.validate()?;
    } else {
        listing.validate_for_draft()?;
    }
    Ok(listing)
}

fn required_tag_value(tags: &[Vec<String>], tag: &'static str) -> Result<String, NipDsError> {
    find_tag_value(tags, tag)
        .map(str::to_owned)
        .ok_or(NipDsError::MissingRequiredTag(tag))
}

fn validate_addressable_coordinate_parts(
    kind: u16,
    pubkey: &str,
    identifier: &str,
) -> Result<(), NipDsError> {
    if !is_addressable_kind(kind) {
        return Err(NipDsError::NonAddressableCoordinate(format!(
            "{kind}:{pubkey}:{identifier}"
        )));
    }
    validate_lower_hex("pubkey", pubkey)?;
    if identifier.trim().is_empty() {
        return Err(NipDsError::InvalidCoordinate(format!(
            "{kind}:{pubkey}:{identifier}"
        )));
    }
    Ok(())
}

fn validate_sha256_hex(value: &str) -> Result<(), NipDsError> {
    validate_lower_hex("x", value)
}

fn validate_lower_hex(field: &'static str, value: &str) -> Result<(), NipDsError> {
    let is_valid = value.len() == 64
        && value == value.to_ascii_lowercase()
        && value.chars().all(|c| c.is_ascii_hexdigit());
    if is_valid {
        Ok(())
    } else {
        Err(NipDsError::InvalidHexField {
            field,
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SELLER_PUBKEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const BUYER_PUBKEY: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const PREVIEW_EVENT_ID: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DISCUSSION_EVENT_ID: &str =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const FAKE_EVENT_ID: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const FAKE_SIG: &str = concat!(
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    );

    #[test]
    fn addressable_coordinate_roundtrips_and_builds_scope_id() {
        let coordinate = AddressableEventCoordinate::dataset_listing(
            SELLER_PUBKEY,
            "bitcoin-policy-transcripts-q1-2026",
        )
        .unwrap();

        assert_eq!(
            coordinate.to_string(),
            "30404:1111111111111111111111111111111111111111111111111111111111111111:bitcoin-policy-transcripts-q1-2026"
        );
        assert_eq!(
            AddressableEventCoordinate::parse(&coordinate.to_string()).unwrap(),
            coordinate
        );
        assert_eq!(
            coordinate.scope_id(DIGEST).unwrap(),
            format!("{coordinate}:{DIGEST}")
        );
    }

    #[test]
    fn dataset_listing_roundtrips_through_event() {
        let listing = DatasetListing::new(
            "bitcoin-policy-transcripts-q1-2026",
            "A cleaned corpus of policy transcripts.",
            "Bitcoin Policy Transcripts Q1 2026",
            DIGEST,
        )
        .with_summary("Redacted transcript corpus with metadata preview.")
        .with_published_at(1_774_080_000)
        .with_version("2026-q1")
        .with_dataset_kind("corpus")
        .with_mime_type("application/x-ndjson")
        .with_size_bytes(28_444_192)
        .with_records(4_127)
        .with_license("seller-license-v1")
        .with_access("paid")
        .add_delivery_mode("nip90")
        .add_delivery_mode("download")
        .add_topic("dataset")
        .add_topic("bitcoin")
        .add_event_ref(
            EventReference::new(PREVIEW_EVENT_ID)
                .unwrap()
                .with_relay("wss://relay.example")
                .with_marker("preview"),
        )
        .add_event_ref(
            EventReference::new(DISCUSSION_EVENT_ID)
                .unwrap()
                .with_relay("wss://relay.example")
                .with_marker("discussion"),
        )
        .add_address_ref(
            AddressableEventReference::new(
                AddressableEventCoordinate::dataset_offer(SELLER_PUBKEY, "open-offer").unwrap(),
            )
            .with_relay("wss://relay.example")
            .with_marker("offer"),
        );

        let parsed = DatasetListing::try_from(build_event(
            SELLER_PUBKEY,
            listing.to_event_template(1_774_080_100).unwrap(),
        ))
        .unwrap();

        assert_eq!(parsed, listing);
    }

    #[test]
    fn draft_dataset_listing_allows_missing_published_at() {
        let draft = DraftDatasetListing::new(
            DatasetListing::new(
                "bitcoin-policy-transcripts-q1-2026",
                "Draft listing for a future release.",
                "Bitcoin Policy Transcripts Q1 2026",
                DIGEST,
            )
            .with_summary("Preview-only draft")
            .with_dataset_kind("corpus"),
        );

        let parsed = DraftDatasetListing::try_from(build_event(
            SELLER_PUBKEY,
            draft.to_event_template(1_774_080_100).unwrap(),
        ))
        .unwrap();

        assert_eq!(parsed, draft);
    }

    #[test]
    fn dataset_offer_roundtrips_through_event() {
        let listing_ref = AddressableEventReference::new(
            AddressableEventCoordinate::dataset_listing(
                SELLER_PUBKEY,
                "bitcoin-policy-transcripts-q1-2026",
            )
            .unwrap(),
        )
        .with_relay("wss://relay.example");
        let offer = DatasetOffer::new(
            "targeted-offer-buyer-1",
            "Targeted access for the full corpus.",
            listing_ref,
        )
        .with_policy("targeted_request")
        .with_price(Price::one_time("5000", "SAT"))
        .add_payment_method(PaymentMethod::new("zap"))
        .add_payment_method(PaymentMethod::new("cashu").with_detail("https://mint.example"))
        .add_delivery_mode("nip90")
        .add_delivery_mode("giftwrap")
        .with_license("seller-license-v1")
        .add_targeted_buyer(
            PublicKeyReference::new(BUYER_PUBKEY)
                .unwrap()
                .with_relay("wss://relay.example"),
        )
        .with_expiration("1774166400")
        .add_topic("dataset");

        let parsed = DatasetOffer::try_from(build_event(
            SELLER_PUBKEY,
            offer.to_event_template(1_774_080_200).unwrap(),
        ))
        .unwrap();

        assert_eq!(parsed, offer);
    }

    #[test]
    fn listing_rejects_non_addressable_reference_tag() {
        let event = Event {
            id: FAKE_EVENT_ID.to_string(),
            pubkey: SELLER_PUBKEY.to_string(),
            created_at: 1_774_080_100,
            kind: KIND_DATASET_LISTING,
            tags: vec![
                vec!["d".to_string(), "invalid-listing".to_string()],
                vec!["title".to_string(), "Invalid Listing".to_string()],
                vec!["x".to_string(), DIGEST.to_string()],
                vec!["published_at".to_string(), "1774080000".to_string()],
                vec![
                    "a".to_string(),
                    format!("1:{SELLER_PUBKEY}:not-addressable"),
                    "wss://relay.example".to_string(),
                    "market".to_string(),
                ],
            ],
            content: "broken".to_string(),
            sig: FAKE_SIG.to_string(),
        };

        assert!(matches!(
            DatasetListing::try_from(event),
            Err(NipDsError::NonAddressableCoordinate(_))
        ));
    }

    fn build_event(pubkey: &str, template: EventTemplate) -> Event {
        Event {
            id: FAKE_EVENT_ID.to_string(),
            pubkey: pubkey.to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: FAKE_SIG.to_string(),
        }
    }
}
