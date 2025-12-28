//! NIP-69: Peer-to-peer Order Events
//!
//! Defines peer-to-peer order events for decentralized marketplace trading.
//! Creates a unified liquidity pool across P2P platforms.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/69.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for P2P orders (addressable)
pub const P2P_ORDER_KIND: u16 = 38383;

/// Document type tag value
pub const DOCUMENT_TYPE: &str = "order";

/// Errors that can occur during NIP-69 operations
#[derive(Debug, Error)]
pub enum Nip69Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid order type: {0}")]
    InvalidOrderType(String),

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("invalid layer: {0}")]
    InvalidLayer(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Order type (buy or sell)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Buy,
    Sell,
}

impl OrderType {
    pub fn as_str(&self) -> &str {
        match self {
            OrderType::Buy => "buy",
            OrderType::Sell => "sell",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip69Error> {
        match s {
            "buy" => Ok(OrderType::Buy),
            "sell" => Ok(OrderType::Sell),
            _ => Err(Nip69Error::InvalidOrderType(s.to_string())),
        }
    }
}

/// Order status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderStatus {
    Pending,
    Canceled,
    InProgress,
    Success,
    Expired,
}

impl OrderStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Canceled => "canceled",
            OrderStatus::InProgress => "in-progress",
            OrderStatus::Success => "success",
            OrderStatus::Expired => "expired",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip69Error> {
        match s {
            "pending" => Ok(OrderStatus::Pending),
            "canceled" => Ok(OrderStatus::Canceled),
            "in-progress" => Ok(OrderStatus::InProgress),
            "success" => Ok(OrderStatus::Success),
            "expired" => Ok(OrderStatus::Expired),
            _ => Err(Nip69Error::InvalidStatus(s.to_string())),
        }
    }
}

/// Bitcoin layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitcoinLayer {
    Onchain,
    Lightning,
    Liquid,
}

impl BitcoinLayer {
    pub fn as_str(&self) -> &str {
        match self {
            BitcoinLayer::Onchain => "onchain",
            BitcoinLayer::Lightning => "lightning",
            BitcoinLayer::Liquid => "liquid",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip69Error> {
        match s {
            "onchain" => Ok(BitcoinLayer::Onchain),
            "lightning" => Ok(BitcoinLayer::Lightning),
            "liquid" => Ok(BitcoinLayer::Liquid),
            _ => Err(Nip69Error::InvalidLayer(s.to_string())),
        }
    }
}

/// Rating information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub total_reviews: u32,
    pub total_rating: f64,
    pub last_rating: u32,
    pub max_rate: u32,
    pub min_rate: u32,
}

/// P2P Order event (kind 38383)
#[derive(Debug, Clone, PartialEq)]
pub struct P2POrder {
    pub event: Event,
    pub order_id: String,
    pub order_type: OrderType,
    pub currency: String,
    pub status: OrderStatus,
    pub amount_sats: u64,
    pub fiat_amount: Vec<u64>,
    pub payment_methods: Vec<String>,
    pub premium: f64,
    pub network: String,
    pub layer: BitcoinLayer,
    pub expires_at: u64,
    pub expiration: u64,
    pub platform: String,
    pub source: Option<String>,
    pub rating: Option<Rating>,
    pub name: Option<String>,
    pub geohash: Option<String>,
    pub bond: Option<u64>,
}

impl P2POrder {
    pub fn from_event(event: Event) -> Result<Self, Nip69Error> {
        if event.kind != P2P_ORDER_KIND {
            return Err(Nip69Error::InvalidKind {
                expected: P2P_ORDER_KIND,
                actual: event.kind,
            });
        }

        let mut order_id = None;
        let mut order_type = None;
        let mut currency = None;
        let mut status = None;
        let mut amount_sats = None;
        let mut fiat_amount = Vec::new();
        let mut payment_methods = Vec::new();
        let mut premium = None;
        let mut network = None;
        let mut layer = None;
        let mut expires_at = None;
        let mut expiration = None;
        let mut platform = None;
        let mut source = None;
        let mut rating = None;
        let mut name = None;
        let mut geohash = None;
        let mut bond = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "d" if tag.len() >= 2 => {
                    order_id = Some(tag[1].clone());
                }
                "k" if tag.len() >= 2 => {
                    order_type = Some(OrderType::from_str(&tag[1])?);
                }
                "f" if tag.len() >= 2 => {
                    currency = Some(tag[1].clone());
                }
                "s" if tag.len() >= 2 => {
                    status = Some(OrderStatus::from_str(&tag[1])?);
                }
                "amt" if tag.len() >= 2 => {
                    amount_sats = Some(tag[1].parse().unwrap_or(0));
                }
                "fa" if tag.len() >= 2 => {
                    // Can have 1 or 2 values (single amount or range)
                    for value in &tag[1..] {
                        if let Ok(amt) = value.parse::<u64>() {
                            fiat_amount.push(amt);
                        }
                    }
                }
                "pm" if tag.len() >= 2 => {
                    payment_methods = tag[1..].to_vec();
                }
                "premium" if tag.len() >= 2 => {
                    premium = tag[1].parse().ok();
                }
                "network" if tag.len() >= 2 => {
                    network = Some(tag[1].clone());
                }
                "layer" if tag.len() >= 2 => {
                    layer = Some(BitcoinLayer::from_str(&tag[1])?);
                }
                "expires_at" if tag.len() >= 2 => {
                    expires_at = tag[1].parse().ok();
                }
                "expiration" if tag.len() >= 2 => {
                    expiration = tag[1].parse().ok();
                }
                "y" if tag.len() >= 2 => {
                    platform = Some(tag[1].clone());
                }
                "source" if tag.len() >= 2 => {
                    source = Some(tag[1].clone());
                }
                "rating" if tag.len() >= 2 => {
                    rating = serde_json::from_str(&tag[1]).ok();
                }
                "name" if tag.len() >= 2 => {
                    name = Some(tag[1].clone());
                }
                "g" if tag.len() >= 2 => {
                    geohash = Some(tag[1].clone());
                }
                "bond" if tag.len() >= 2 => {
                    bond = tag[1].parse().ok();
                }
                _ => {}
            }
        }

        let order_id = order_id.ok_or_else(|| Nip69Error::MissingField("d (order_id)".to_string()))?;
        let order_type = order_type.ok_or_else(|| Nip69Error::MissingField("k (order_type)".to_string()))?;
        let currency = currency.ok_or_else(|| Nip69Error::MissingField("f (currency)".to_string()))?;
        let status = status.ok_or_else(|| Nip69Error::MissingField("s (status)".to_string()))?;
        let amount_sats = amount_sats.ok_or_else(|| Nip69Error::MissingField("amt (amount)".to_string()))?;
        let premium = premium.ok_or_else(|| Nip69Error::MissingField("premium".to_string()))?;
        let network = network.ok_or_else(|| Nip69Error::MissingField("network".to_string()))?;
        let layer = layer.ok_or_else(|| Nip69Error::MissingField("layer".to_string()))?;
        let expires_at = expires_at.ok_or_else(|| Nip69Error::MissingField("expires_at".to_string()))?;
        let expiration = expiration.ok_or_else(|| Nip69Error::MissingField("expiration".to_string()))?;
        let platform = platform.ok_or_else(|| Nip69Error::MissingField("y (platform)".to_string()))?;

        if fiat_amount.is_empty() {
            return Err(Nip69Error::MissingField("fa (fiat_amount)".to_string()));
        }

        if payment_methods.is_empty() {
            return Err(Nip69Error::MissingField("pm (payment_methods)".to_string()));
        }

        Ok(Self {
            event,
            order_id,
            order_type,
            currency,
            status,
            amount_sats,
            fiat_amount,
            payment_methods,
            premium,
            network,
            layer,
            expires_at,
            expiration,
            platform,
            source,
            rating,
            name,
            geohash,
            bond,
        })
    }

    /// Get the order ID
    pub fn get_order_id(&self) -> &str {
        &self.order_id
    }

    /// Get the order type
    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    /// Get the currency (ISO 4217)
    pub fn get_currency(&self) -> &str {
        &self.currency
    }

    /// Get the order status
    pub fn get_status(&self) -> &OrderStatus {
        &self.status
    }

    /// Get the amount in satoshis (0 means amount will be determined later)
    pub fn get_amount_sats(&self) -> u64 {
        self.amount_sats
    }

    /// Get the fiat amount (single value or range)
    pub fn get_fiat_amount(&self) -> &[u64] {
        &self.fiat_amount
    }

    /// Check if this is a range order
    pub fn is_range_order(&self) -> bool {
        self.fiat_amount.len() > 1
    }

    /// Get payment methods
    pub fn get_payment_methods(&self) -> &[String] {
        &self.payment_methods
    }

    /// Get premium percentage
    pub fn get_premium(&self) -> f64 {
        self.premium
    }

    /// Get network (mainnet, testnet, signet, etc.)
    pub fn get_network(&self) -> &str {
        &self.network
    }

    /// Get layer
    pub fn get_layer(&self) -> &BitcoinLayer {
        &self.layer
    }

    /// Get expiration timestamp for pending status
    pub fn get_expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Get event expiration (NIP-40)
    pub fn get_expiration(&self) -> u64 {
        self.expiration
    }

    /// Get platform name
    pub fn get_platform(&self) -> &str {
        &self.platform
    }

    /// Get maker's public key
    pub fn get_maker(&self) -> &str {
        &self.event.pubkey
    }

    /// Get creation timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }
}

/// Check if an event kind is a P2P order kind
pub fn is_p2p_order_kind(kind: u16) -> bool {
    kind == P2P_ORDER_KIND
}

/// Create order ID tag
#[allow(dead_code)]
pub fn create_order_id_tag(order_id: String) -> Vec<String> {
    vec!["d".to_string(), order_id]
}

/// Create order type tag
#[allow(dead_code)]
pub fn create_order_type_tag(order_type: OrderType) -> Vec<String> {
    vec!["k".to_string(), order_type.as_str().to_string()]
}

/// Create currency tag (ISO 4217)
#[allow(dead_code)]
pub fn create_currency_tag(currency: String) -> Vec<String> {
    vec!["f".to_string(), currency]
}

/// Create status tag
#[allow(dead_code)]
pub fn create_status_tag(status: OrderStatus) -> Vec<String> {
    vec!["s".to_string(), status.as_str().to_string()]
}

/// Create amount tag
#[allow(dead_code)]
pub fn create_amount_tag(amount_sats: u64) -> Vec<String> {
    vec!["amt".to_string(), amount_sats.to_string()]
}

/// Create fiat amount tag
#[allow(dead_code)]
pub fn create_fiat_amount_tag(amounts: Vec<u64>) -> Vec<String> {
    let mut tag = vec!["fa".to_string()];
    tag.extend(amounts.iter().map(|a| a.to_string()));
    tag
}

/// Create payment method tag
#[allow(dead_code)]
pub fn create_payment_method_tag(methods: Vec<String>) -> Vec<String> {
    let mut tag = vec!["pm".to_string()];
    tag.extend(methods);
    tag
}

/// Create layer tag
#[allow(dead_code)]
pub fn create_layer_tag(layer: BitcoinLayer) -> Vec<String> {
    vec!["layer".to_string(), layer.as_str().to_string()]
}

/// Create platform tag
#[allow(dead_code)]
pub fn create_platform_tag(platform: String) -> Vec<String> {
    vec!["y".to_string(), platform]
}

/// Create document type tag
#[allow(dead_code)]
pub fn create_document_tag() -> Vec<String> {
    vec!["z".to_string(), DOCUMENT_TYPE.to_string()]
}

/// Builder for creating P2P order events
#[derive(Debug, Clone)]
pub struct P2POrderBuilder {
    order_id: String,
    order_type: OrderType,
    currency: String,
    amount_sats: u64,
    fiat_amount: Vec<u64>,
    payment_methods: Vec<String>,
    premium: f64,
    network: String,
    layer: BitcoinLayer,
    expires_at: u64,
    expiration: u64,
    platform: String,
    source: Option<String>,
    name: Option<String>,
    geohash: Option<String>,
    bond: Option<u64>,
}

impl P2POrderBuilder {
    /// Create a new order builder
    pub fn new(order_id: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            order_id: order_id.into(),
            order_type: OrderType::Sell,
            currency: "USD".to_string(),
            amount_sats: 0,
            fiat_amount: vec![],
            payment_methods: vec!["cashu".to_string()],
            premium: 0.0,
            network: "mainnet".to_string(),
            layer: BitcoinLayer::Lightning,
            expires_at: now + 3600,      // 1 hour default
            expiration: now + 86400,     // 24 hours default
            platform: "openagents".to_string(),
            source: None,
            name: None,
            geohash: None,
            bond: None,
        }
    }

    /// Set order type (buy or sell)
    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = order_type;
        self
    }

    /// Set currency (ISO 4217 code)
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
        self
    }

    /// Set amount in satoshis
    pub fn amount_sats(mut self, amount: u64) -> Self {
        self.amount_sats = amount;
        self
    }

    /// Set fiat amount (single value)
    pub fn fiat_amount(mut self, amount: u64) -> Self {
        self.fiat_amount = vec![amount];
        self
    }

    /// Set fiat amount range (min, max)
    pub fn fiat_amount_range(mut self, min: u64, max: u64) -> Self {
        self.fiat_amount = vec![min, max];
        self
    }

    /// Set payment methods
    pub fn payment_methods(mut self, methods: Vec<String>) -> Self {
        self.payment_methods = methods;
        self
    }

    /// Set premium percentage
    pub fn premium(mut self, premium: f64) -> Self {
        self.premium = premium;
        self
    }

    /// Set Bitcoin network
    pub fn network(mut self, network: impl Into<String>) -> Self {
        self.network = network.into();
        self
    }

    /// Set Bitcoin layer
    pub fn layer(mut self, layer: BitcoinLayer) -> Self {
        self.layer = layer;
        self
    }

    /// Set order expiration timestamp
    pub fn expires_at(mut self, timestamp: u64) -> Self {
        self.expires_at = timestamp;
        self
    }

    /// Set event expiration (NIP-40)
    pub fn expiration(mut self, timestamp: u64) -> Self {
        self.expiration = timestamp;
        self
    }

    /// Set platform name
    pub fn platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = platform.into();
        self
    }

    /// Set source URL
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set maker name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set geohash location
    pub fn geohash(mut self, geohash: impl Into<String>) -> Self {
        self.geohash = Some(geohash.into());
        self
    }

    /// Set bond amount
    pub fn bond(mut self, bond: u64) -> Self {
        self.bond = Some(bond);
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.order_id.clone()],
            vec!["k".to_string(), self.order_type.as_str().to_string()],
            vec!["f".to_string(), self.currency.clone()],
            vec!["s".to_string(), OrderStatus::Pending.as_str().to_string()],
            vec!["amt".to_string(), self.amount_sats.to_string()],
        ];

        // Fiat amount
        let mut fa_tag = vec!["fa".to_string()];
        for amt in &self.fiat_amount {
            fa_tag.push(amt.to_string());
        }
        tags.push(fa_tag);

        // Payment methods
        let mut pm_tag = vec!["pm".to_string()];
        pm_tag.extend(self.payment_methods.clone());
        tags.push(pm_tag);

        tags.push(vec!["premium".to_string(), self.premium.to_string()]);
        tags.push(vec!["network".to_string(), self.network.clone()]);
        tags.push(vec!["layer".to_string(), self.layer.as_str().to_string()]);
        tags.push(vec!["expires_at".to_string(), self.expires_at.to_string()]);
        tags.push(vec!["expiration".to_string(), self.expiration.to_string()]);
        tags.push(vec!["y".to_string(), self.platform.clone()]);
        tags.push(vec!["z".to_string(), DOCUMENT_TYPE.to_string()]);

        // Optional tags
        if let Some(ref source) = self.source {
            tags.push(vec!["source".to_string(), source.clone()]);
        }
        if let Some(ref name) = self.name {
            tags.push(vec!["name".to_string(), name.clone()]);
        }
        if let Some(ref geohash) = self.geohash {
            tags.push(vec!["g".to_string(), geohash.clone()]);
        }
        if let Some(bond) = self.bond {
            tags.push(vec!["bond".to_string(), bond.to_string()]);
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_maker_pubkey".to_string(),
            created_at: 1702548701,
            kind,
            tags,
            content: String::new(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_order_type() {
        assert_eq!(OrderType::Buy.as_str(), "buy");
        assert_eq!(OrderType::Sell.as_str(), "sell");
        assert_eq!(OrderType::from_str("buy").unwrap(), OrderType::Buy);
        assert_eq!(OrderType::from_str("sell").unwrap(), OrderType::Sell);
    }

    #[test]
    fn test_order_status() {
        assert_eq!(OrderStatus::Pending.as_str(), "pending");
        assert_eq!(OrderStatus::Canceled.as_str(), "canceled");
        assert_eq!(OrderStatus::InProgress.as_str(), "in-progress");
        assert_eq!(OrderStatus::Success.as_str(), "success");
        assert_eq!(OrderStatus::Expired.as_str(), "expired");
    }

    #[test]
    fn test_bitcoin_layer() {
        assert_eq!(BitcoinLayer::Onchain.as_str(), "onchain");
        assert_eq!(BitcoinLayer::Lightning.as_str(), "lightning");
        assert_eq!(BitcoinLayer::Liquid.as_str(), "liquid");
    }

    #[test]
    fn test_p2p_order_from_event() {
        let tags = vec![
            vec!["d".to_string(), "order-123".to_string()],
            vec!["k".to_string(), "sell".to_string()],
            vec!["f".to_string(), "VES".to_string()],
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), "10000".to_string()],
            vec!["fa".to_string(), "100".to_string()],
            vec!["pm".to_string(), "face to face".to_string(), "bank transfer".to_string()],
            vec!["premium".to_string(), "1.5".to_string()],
            vec!["network".to_string(), "mainnet".to_string()],
            vec!["layer".to_string(), "lightning".to_string()],
            vec!["expires_at".to_string(), "1719391096".to_string()],
            vec!["expiration".to_string(), "1719995896".to_string()],
            vec!["y".to_string(), "lnp2pbot".to_string()],
            vec!["z".to_string(), "order".to_string()],
        ];

        let event = create_test_event(P2P_ORDER_KIND, tags);
        let order = P2POrder::from_event(event).unwrap();

        assert_eq!(order.get_order_id(), "order-123");
        assert_eq!(order.get_order_type(), &OrderType::Sell);
        assert_eq!(order.get_currency(), "VES");
        assert_eq!(order.get_status(), &OrderStatus::Pending);
        assert_eq!(order.get_amount_sats(), 10000);
        assert_eq!(order.get_fiat_amount(), &[100]);
        assert_eq!(order.get_payment_methods().len(), 2);
        assert_eq!(order.get_premium(), 1.5);
        assert_eq!(order.get_network(), "mainnet");
        assert_eq!(order.get_layer(), &BitcoinLayer::Lightning);
        assert_eq!(order.get_platform(), "lnp2pbot");
        assert!(!order.is_range_order());
    }

    #[test]
    fn test_p2p_order_range() {
        let tags = vec![
            vec!["d".to_string(), "order-456".to_string()],
            vec!["k".to_string(), "buy".to_string()],
            vec!["f".to_string(), "USD".to_string()],
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), "0".to_string()],
            vec!["fa".to_string(), "100".to_string(), "500".to_string()],
            vec!["pm".to_string(), "bank transfer".to_string()],
            vec!["premium".to_string(), "2.0".to_string()],
            vec!["network".to_string(), "mainnet".to_string()],
            vec!["layer".to_string(), "onchain".to_string()],
            vec!["expires_at".to_string(), "1719391096".to_string()],
            vec!["expiration".to_string(), "1719995896".to_string()],
            vec!["y".to_string(), "mostro".to_string()],
            vec!["z".to_string(), "order".to_string()],
        ];

        let event = create_test_event(P2P_ORDER_KIND, tags);
        let order = P2POrder::from_event(event).unwrap();

        assert_eq!(order.get_order_type(), &OrderType::Buy);
        assert_eq!(order.get_fiat_amount(), &[100, 500]);
        assert!(order.is_range_order());
        assert_eq!(order.get_layer(), &BitcoinLayer::Onchain);
    }

    #[test]
    fn test_p2p_order_missing_field() {
        let tags = vec![
            vec!["d".to_string(), "order-123".to_string()],
            vec!["k".to_string(), "sell".to_string()],
        ];

        let event = create_test_event(P2P_ORDER_KIND, tags);
        let result = P2POrder::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_p2p_order_invalid_kind() {
        let event = create_test_event(1, vec![]);
        let result = P2POrder::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_p2p_order_kind() {
        assert!(is_p2p_order_kind(P2P_ORDER_KIND));
        assert!(!is_p2p_order_kind(1));
    }

    #[test]
    fn test_create_tags() {
        let order_id_tag = create_order_id_tag("order-123".to_string());
        assert_eq!(order_id_tag, vec!["d", "order-123"]);

        let type_tag = create_order_type_tag(OrderType::Sell);
        assert_eq!(type_tag, vec!["k", "sell"]);

        let currency_tag = create_currency_tag("USD".to_string());
        assert_eq!(currency_tag, vec!["f", "USD"]);

        let status_tag = create_status_tag(OrderStatus::Pending);
        assert_eq!(status_tag, vec!["s", "pending"]);

        let amount_tag = create_amount_tag(10000);
        assert_eq!(amount_tag, vec!["amt", "10000"]);

        let fiat_tag = create_fiat_amount_tag(vec![100, 500]);
        assert_eq!(fiat_tag, vec!["fa", "100", "500"]);

        let pm_tag = create_payment_method_tag(vec!["bank".to_string(), "cash".to_string()]);
        assert_eq!(pm_tag, vec!["pm", "bank", "cash"]);

        let layer_tag = create_layer_tag(BitcoinLayer::Lightning);
        assert_eq!(layer_tag, vec!["layer", "lightning"]);

        let platform_tag = create_platform_tag("mostro".to_string());
        assert_eq!(platform_tag, vec!["y", "mostro"]);

        let doc_tag = create_document_tag();
        assert_eq!(doc_tag, vec!["z", "order"]);
    }

    #[test]
    fn test_rating_serialization() {
        let rating = Rating {
            total_reviews: 10,
            total_rating: 4.5,
            last_rating: 5,
            max_rate: 5,
            min_rate: 1,
        };

        let json = serde_json::to_string(&rating).unwrap();
        let deserialized: Rating = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_reviews, 10);
        assert_eq!(deserialized.total_rating, 4.5);
    }

    #[test]
    fn test_p2p_order_builder() {
        let builder = P2POrderBuilder::new("order-789")
            .order_type(OrderType::Sell)
            .currency("USD")
            .amount_sats(10000)
            .fiat_amount(100)
            .payment_methods(vec!["cashu".to_string(), "lightning".to_string()])
            .premium(0.5)
            .network("mainnet")
            .layer(BitcoinLayer::Lightning)
            .name("Treasury Agent");

        let tags = builder.build_tags();

        // Verify key tags
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "d" && t[1] == "order-789"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "k" && t[1] == "sell"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "f" && t[1] == "USD"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "s" && t[1] == "pending"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "amt" && t[1] == "10000"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "fa" && t[1] == "100"));
        assert!(tags.iter().any(|t| t.len() >= 3 && t[0] == "pm" && t[1] == "cashu" && t[2] == "lightning"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "premium" && t[1] == "0.5"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "name" && t[1] == "Treasury Agent"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "y" && t[1] == "openagents"));
        assert!(tags.iter().any(|t| t.len() >= 2 && t[0] == "z" && t[1] == "order"));
    }

    #[test]
    fn test_p2p_order_builder_range() {
        let builder = P2POrderBuilder::new("order-range")
            .order_type(OrderType::Buy)
            .fiat_amount_range(50, 200);

        let tags = builder.build_tags();

        // Verify range fiat amount
        assert!(tags.iter().any(|t| t.len() >= 3 && t[0] == "fa" && t[1] == "50" && t[2] == "200"));
    }

    #[test]
    fn test_p2p_order_builder_roundtrip() {
        let builder = P2POrderBuilder::new("roundtrip-test")
            .order_type(OrderType::Sell)
            .currency("USD")
            .amount_sats(50000)
            .fiat_amount(500)
            .payment_methods(vec!["cashu".to_string()])
            .premium(1.0)
            .name("Test Maker");

        let tags = builder.build_tags();
        let event = create_test_event(P2P_ORDER_KIND, tags);
        let order = P2POrder::from_event(event).unwrap();

        assert_eq!(order.get_order_id(), "roundtrip-test");
        assert_eq!(order.get_order_type(), &OrderType::Sell);
        assert_eq!(order.get_currency(), "USD");
        assert_eq!(order.get_amount_sats(), 50000);
        assert_eq!(order.get_fiat_amount(), &[500]);
        assert_eq!(order.get_premium(), 1.0);
        assert_eq!(order.name, Some("Test Maker".to_string()));
    }
}
