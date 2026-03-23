//! NIP-15: Nostr Marketplace
//!
//! This module intentionally implements only the publication and parsing
//! surface for marketplace stalls and products:
//! - `kind:30017` stall metadata
//! - `kind:30018` product metadata
//!
//! OpenAgents uses this as an optional storefront wrapper around canonical
//! NIP-DS listings and offers. Checkout and direct-message order flow remain
//! separate work.

use crate::nip01::{Event, EventTemplate, is_addressable_kind};
use crate::tag_parsing::{collect_tag_values, find_tag_value, tag_name};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for marketplace stalls.
pub const KIND_STALL: u16 = 30017;

/// Kind number for marketplace products.
pub const KIND_PRODUCT: u16 = 30018;

/// Errors that can occur while handling NIP-15 marketplace events.
#[derive(Debug, Error)]
pub enum Nip15Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid content json: {0}")]
    InvalidContent(String),

    #[error("missing required d tag")]
    MissingDTag,

    #[error("missing stall name")]
    MissingName,

    #[error("missing stall currency")]
    MissingCurrency,

    #[error("missing product stall_id")]
    MissingStallId,

    #[error("invalid addressable coordinate kind: {0}")]
    InvalidCoordinateKind(u16),

    #[error("invalid lowercase hex field `{field}`: {value}")]
    InvalidHexField { field: &'static str, value: String },

    #[error("invalid product price: {0}")]
    InvalidPrice(String),
}

/// Check if a kind belongs to the NIP-15 storefront surface used by OpenAgents.
pub fn is_nip15_kind(kind: u16) -> bool {
    matches!(kind, KIND_STALL | KIND_PRODUCT)
}

/// Check whether a kind is a marketplace stall.
pub fn is_stall_kind(kind: u16) -> bool {
    kind == KIND_STALL
}

/// Check whether a kind is a marketplace product.
pub fn is_product_kind(kind: u16) -> bool {
    kind == KIND_PRODUCT
}

/// Shipping zone metadata published on a marketplace stall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StallShippingZone {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub cost: f64,
    #[serde(default)]
    pub regions: Vec<String>,
}

impl StallShippingZone {
    pub fn new(id: impl Into<String>, cost: f64) -> Result<Self, Nip15Error> {
        validate_price(cost)?;
        Ok(Self {
            id: id.into(),
            name: None,
            cost,
            regions: Vec::new(),
        })
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn add_region(mut self, region: impl Into<String>) -> Self {
        self.regions.push(region.into());
        self
    }
}

/// Marketplace stall metadata (`kind:30017`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceStall {
    pub identifier: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub currency: String,
    #[serde(default)]
    pub shipping: Vec<StallShippingZone>,
}

impl MarketplaceStall {
    pub fn new(
        identifier: impl Into<String>,
        name: impl Into<String>,
        currency: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            name: name.into(),
            description: None,
            currency: currency.into(),
            shipping: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn add_shipping_zone(mut self, zone: StallShippingZone) -> Self {
        self.shipping.push(zone);
        self
    }

    pub fn validate(&self) -> Result<(), Nip15Error> {
        if self.identifier.trim().is_empty() {
            return Err(Nip15Error::MissingDTag);
        }
        if self.name.trim().is_empty() {
            return Err(Nip15Error::MissingName);
        }
        if self.currency.trim().is_empty() {
            return Err(Nip15Error::MissingCurrency);
        }
        for zone in &self.shipping {
            validate_price(zone.cost)?;
        }
        Ok(())
    }

    pub fn coordinate(&self, publisher_pubkey: impl Into<String>) -> Result<String, Nip15Error> {
        coordinate_for_kind(
            KIND_STALL,
            publisher_pubkey.into().as_str(),
            self.identifier.as_str(),
        )
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        vec![vec!["d".to_string(), self.identifier.clone()]]
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, Nip15Error> {
        self.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_STALL,
            tags: self.to_tags(),
            content: serde_json::to_string(&StallContent::from(self))
                .map_err(|error| Nip15Error::InvalidContent(error.to_string()))?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, Nip15Error> {
        if event.kind != KIND_STALL {
            return Err(Nip15Error::InvalidKind {
                expected: KIND_STALL,
                actual: event.kind,
            });
        }
        parse_stall(event)
    }
}

impl TryFrom<&Event> for MarketplaceStall {
    type Error = Nip15Error;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

/// Per-product shipping surcharge metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductShippingCost {
    pub id: String,
    pub cost: f64,
}

impl ProductShippingCost {
    pub fn new(id: impl Into<String>, cost: f64) -> Result<Self, Nip15Error> {
        validate_price(cost)?;
        Ok(Self {
            id: id.into(),
            cost,
        })
    }
}

/// Marketplace product metadata (`kind:30018`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceProduct {
    pub identifier: String,
    pub stall_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    pub currency: String,
    pub price: f64,
    #[serde(default)]
    pub quantity: Option<u64>,
    #[serde(default)]
    pub specs: Vec<(String, String)>,
    #[serde(default)]
    pub shipping: Vec<ProductShippingCost>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub address_refs: Vec<String>,
}

impl MarketplaceProduct {
    pub fn new(
        identifier: impl Into<String>,
        stall_id: impl Into<String>,
        name: impl Into<String>,
        currency: impl Into<String>,
        price: f64,
    ) -> Result<Self, Nip15Error> {
        validate_price(price)?;
        Ok(Self {
            identifier: identifier.into(),
            stall_id: stall_id.into(),
            name: name.into(),
            description: None,
            images: Vec::new(),
            currency: currency.into(),
            price,
            quantity: None,
            specs: Vec::new(),
            shipping: Vec::new(),
            tags: Vec::new(),
            address_refs: Vec::new(),
        })
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn add_image(mut self, image_url: impl Into<String>) -> Self {
        self.images.push(image_url.into());
        self
    }

    pub fn with_quantity(mut self, quantity: Option<u64>) -> Self {
        self.quantity = quantity;
        self
    }

    pub fn add_spec(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.specs.push((key.into(), value.into()));
        self
    }

    pub fn add_shipping_cost(mut self, shipping_cost: ProductShippingCost) -> Self {
        self.shipping.push(shipping_cost);
        self
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    pub fn add_address_ref(&mut self, coordinate: impl Into<String>) {
        self.address_refs.push(coordinate.into());
    }

    pub fn validate(&self) -> Result<(), Nip15Error> {
        if self.identifier.trim().is_empty() {
            return Err(Nip15Error::MissingDTag);
        }
        if self.stall_id.trim().is_empty() {
            return Err(Nip15Error::MissingStallId);
        }
        if self.name.trim().is_empty() {
            return Err(Nip15Error::MissingName);
        }
        if self.currency.trim().is_empty() {
            return Err(Nip15Error::MissingCurrency);
        }
        validate_price(self.price)?;
        for shipping_cost in &self.shipping {
            validate_price(shipping_cost.cost)?;
        }
        Ok(())
    }

    pub fn coordinate(&self, publisher_pubkey: impl Into<String>) -> Result<String, Nip15Error> {
        coordinate_for_kind(
            KIND_PRODUCT,
            publisher_pubkey.into().as_str(),
            self.identifier.as_str(),
        )
    }

    pub fn stall_coordinate(
        &self,
        publisher_pubkey: impl Into<String>,
    ) -> Result<String, Nip15Error> {
        coordinate_for_kind(
            KIND_STALL,
            publisher_pubkey.into().as_str(),
            self.stall_id.as_str(),
        )
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.identifier.clone()]];
        for tag in &self.tags {
            tags.push(vec!["t".to_string(), tag.clone()]);
        }
        for address_ref in &self.address_refs {
            tags.push(vec!["a".to_string(), address_ref.clone()]);
        }
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, Nip15Error> {
        self.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_PRODUCT,
            tags: self.to_tags(),
            content: serde_json::to_string(&ProductContent::from(self))
                .map_err(|error| Nip15Error::InvalidContent(error.to_string()))?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, Nip15Error> {
        if event.kind != KIND_PRODUCT {
            return Err(Nip15Error::InvalidKind {
                expected: KIND_PRODUCT,
                actual: event.kind,
            });
        }
        parse_product(event)
    }
}

impl TryFrom<&Event> for MarketplaceProduct {
    type Error = Nip15Error;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        Self::from_event(event)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StallContent {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    currency: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shipping: Vec<StallShippingZone>,
}

impl From<&MarketplaceStall> for StallContent {
    fn from(stall: &MarketplaceStall) -> Self {
        Self {
            id: stall.identifier.clone(),
            name: stall.name.clone(),
            description: stall.description.clone(),
            currency: stall.currency.clone(),
            shipping: stall.shipping.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ProductContent {
    id: String,
    stall_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
    currency: String,
    price: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quantity: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    specs: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shipping: Vec<ProductShippingCost>,
}

impl From<&MarketplaceProduct> for ProductContent {
    fn from(product: &MarketplaceProduct) -> Self {
        Self {
            id: product.identifier.clone(),
            stall_id: product.stall_id.clone(),
            name: product.name.clone(),
            description: product.description.clone(),
            images: product.images.clone(),
            currency: product.currency.clone(),
            price: product.price,
            quantity: product.quantity,
            specs: product.specs.clone(),
            shipping: product.shipping.clone(),
        }
    }
}

fn parse_stall(event: &Event) -> Result<MarketplaceStall, Nip15Error> {
    let content = serde_json::from_str::<StallContent>(&event.content)
        .map_err(|error| Nip15Error::InvalidContent(error.to_string()))?;
    let identifier = find_tag_value(&event.tags, "d").ok_or(Nip15Error::MissingDTag)?;
    let stall = MarketplaceStall {
        identifier: identifier.to_string(),
        name: content.name,
        description: content.description,
        currency: content.currency,
        shipping: content.shipping,
    };
    stall.validate()?;
    Ok(stall)
}

fn parse_product(event: &Event) -> Result<MarketplaceProduct, Nip15Error> {
    let content = serde_json::from_str::<ProductContent>(&event.content)
        .map_err(|error| Nip15Error::InvalidContent(error.to_string()))?;
    let identifier = find_tag_value(&event.tags, "d").ok_or(Nip15Error::MissingDTag)?;
    let product = MarketplaceProduct {
        identifier: identifier.to_string(),
        stall_id: content.stall_id,
        name: content.name,
        description: content.description,
        images: content.images,
        currency: content.currency,
        price: content.price,
        quantity: content.quantity,
        specs: content.specs,
        shipping: content.shipping,
        tags: collect_tag_values(&event.tags, "t"),
        address_refs: event
            .tags
            .iter()
            .filter(|tag| tag_name(tag) == Some("a"))
            .filter_map(|tag| tag.get(1).cloned())
            .collect(),
    };
    product.validate()?;
    Ok(product)
}

fn coordinate_for_kind(kind: u16, pubkey: &str, identifier: &str) -> Result<String, Nip15Error> {
    if !is_addressable_kind(kind) {
        return Err(Nip15Error::InvalidCoordinateKind(kind));
    }
    validate_lower_hex("pubkey", pubkey)?;
    Ok(format!("{kind}:{pubkey}:{identifier}"))
}

fn validate_lower_hex(field: &'static str, value: &str) -> Result<(), Nip15Error> {
    if !value.chars().all(|character| character.is_ascii_hexdigit())
        || value
            .chars()
            .any(|character| character.is_ascii_uppercase())
    {
        return Err(Nip15Error::InvalidHexField {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

fn validate_price(price: f64) -> Result<(), Nip15Error> {
    if price.is_finite() && price >= 0.0 {
        Ok(())
    } else {
        Err(Nip15Error::InvalidPrice(price.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KIND_PRODUCT, KIND_STALL, MarketplaceProduct, MarketplaceStall, Nip15Error,
        ProductShippingCost, StallShippingZone, is_nip15_kind,
    };
    use crate::{NostrIdentity, finalize_event, regenerate_identity};

    fn sign_template(identity: &NostrIdentity, template: &crate::EventTemplate) -> crate::Event {
        let key_bytes = hex::decode(identity.private_key_hex.as_str()).expect("private key hex");
        let mut private_key = [0_u8; 32];
        private_key.copy_from_slice(key_bytes.as_slice());
        finalize_event(template, &private_key).expect("sign event")
    }

    #[test]
    fn test_marketplace_kind_checks() {
        assert!(is_nip15_kind(KIND_STALL));
        assert!(is_nip15_kind(KIND_PRODUCT));
        assert!(!is_nip15_kind(1));
    }

    #[test]
    fn test_stall_roundtrip() {
        let identity = regenerate_identity().expect("identity");
        let stall = MarketplaceStall::new("datasets.sat", "Dataset Stall", "SAT")
            .with_description("Public storefront for datasets.")
            .add_shipping_zone(
                StallShippingZone::new("digital", 0.0)
                    .expect("shipping zone")
                    .with_name("Digital")
                    .add_region("global"),
            );

        let event = sign_template(
            &identity,
            &stall
                .to_event_template(1_762_700_000)
                .expect("stall template"),
        );
        let parsed = MarketplaceStall::from_event(&event).expect("parse stall");

        assert_eq!(parsed.identifier, "datasets.sat");
        assert_eq!(parsed.name, "Dataset Stall");
        assert_eq!(parsed.currency, "SAT");
        assert_eq!(parsed.shipping.len(), 1);
        assert_eq!(
            stall
                .coordinate(identity.public_key_hex.clone())
                .expect("coordinate"),
            format!("30017:{}:datasets.sat", identity.public_key_hex)
        );
    }

    #[test]
    fn test_product_roundtrip_with_ds_refs() {
        let identity = regenerate_identity().expect("identity");
        let mut product =
            MarketplaceProduct::new("dataset.alpha", "datasets.sat", "Dataset Alpha", "SAT", 5.0)
                .expect("product");
        product = product
            .with_description("Dataset storefront product")
            .with_quantity(None)
            .add_spec("dataset_kind", "conversation_bundle")
            .add_shipping_cost(ProductShippingCost::new("digital", 0.0).expect("shipping"));
        product.add_tag("dataset");
        product.add_tag("nip-ds");
        product.add_address_ref(format!(
            "30404:{}:data_asset.example.corpus.001",
            identity.public_key_hex
        ));
        product.add_address_ref(format!(
            "30406:{}:grant.example.corpus.001",
            identity.public_key_hex
        ));

        let event = sign_template(
            &identity,
            &product
                .to_event_template(1_762_700_010)
                .expect("product template"),
        );
        let parsed = MarketplaceProduct::from_event(&event).expect("parse product");

        assert_eq!(parsed.identifier, "dataset.alpha");
        assert_eq!(parsed.stall_id, "datasets.sat");
        assert_eq!(parsed.currency, "SAT");
        assert_eq!(parsed.price, 5.0);
        assert!(parsed.tags.iter().any(|tag| tag == "dataset"));
        assert_eq!(parsed.address_refs.len(), 2);
        assert_eq!(
            product
                .coordinate(identity.public_key_hex.clone())
                .expect("coordinate"),
            format!("30018:{}:dataset.alpha", identity.public_key_hex)
        );
        assert_eq!(
            product
                .stall_coordinate(identity.public_key_hex.clone())
                .expect("stall coordinate"),
            format!("30017:{}:datasets.sat", identity.public_key_hex)
        );
    }

    #[test]
    fn test_marketplace_product_requires_price() {
        let error = MarketplaceProduct::new(
            "dataset.alpha",
            "datasets.sat",
            "Dataset Alpha",
            "SAT",
            f64::NAN,
        )
        .expect_err("invalid price");
        assert!(matches!(error, Nip15Error::InvalidPrice(_)));
    }
}
