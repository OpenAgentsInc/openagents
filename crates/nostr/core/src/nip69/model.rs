use crate::Event;
use crate::tag_parsing::{parse_tag_field, tag_field, tag_name};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
}

impl std::str::FromStr for OrderType {
    type Err = Nip69Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
}

impl std::str::FromStr for OrderStatus {
    type Err = Nip69Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
}

impl std::str::FromStr for BitcoinLayer {
    type Err = Nip69Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
            match tag_name(tag) {
                Some("d") => {
                    if let Some(id) = tag_field(tag, 1) {
                        order_id = Some(id.to_string());
                    }
                }
                Some("k") => {
                    if let Some(value) = tag_field(tag, 1) {
                        order_type = Some(OrderType::from_str(value)?);
                    }
                }
                Some("f") => {
                    if let Some(value) = tag_field(tag, 1) {
                        currency = Some(value.to_string());
                    }
                }
                Some("s") => {
                    if let Some(value) = tag_field(tag, 1) {
                        status = Some(OrderStatus::from_str(value)?);
                    }
                }
                Some("amt") => {
                    let amount = parse_tag_field::<u64>(tag, 1)
                        .ok_or_else(|| Nip69Error::Parse("invalid amt value".to_string()))?;
                    amount_sats = Some(amount);
                }
                Some("fa") if tag.len() >= 2 => {
                    // Can have 1 or 2 values (single amount or range)
                    for value in &tag[1..] {
                        let amount = value
                            .parse::<u64>()
                            .map_err(|_| Nip69Error::Parse("invalid fa value".to_string()))?;
                        fiat_amount.push(amount);
                    }
                }
                Some("pm") if tag.len() >= 2 => {
                    payment_methods = tag[1..]
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>();
                }
                Some("premium") => {
                    premium =
                        Some(parse_tag_field::<f64>(tag, 1).ok_or_else(|| {
                            Nip69Error::Parse("invalid premium value".to_string())
                        })?);
                }
                Some("network") => {
                    if let Some(value) = tag_field(tag, 1) {
                        network = Some(value.to_string());
                    }
                }
                Some("layer") => {
                    if let Some(value) = tag_field(tag, 1) {
                        layer = Some(BitcoinLayer::from_str(value)?);
                    }
                }
                Some("expires_at") => {
                    expires_at = Some(parse_tag_field::<u64>(tag, 1).ok_or_else(|| {
                        Nip69Error::Parse("invalid expires_at value".to_string())
                    })?);
                }
                Some("expiration") => {
                    expiration = Some(parse_tag_field::<u64>(tag, 1).ok_or_else(|| {
                        Nip69Error::Parse("invalid expiration value".to_string())
                    })?);
                }
                Some("y") => {
                    if let Some(value) = tag_field(tag, 1) {
                        platform = Some(value.to_string());
                    }
                }
                Some("source") => {
                    if let Some(value) = tag_field(tag, 1) {
                        source = Some(value.to_string());
                    }
                }
                Some("rating") => {
                    if let Some(value) = tag_field(tag, 1) {
                        rating = Some(
                            serde_json::from_str(value)
                                .map_err(|error| Nip69Error::Parse(error.to_string()))?,
                        );
                    }
                }
                Some("name") => {
                    if let Some(value) = tag_field(tag, 1) {
                        name = Some(value.to_string());
                    }
                }
                Some("g") => {
                    if let Some(value) = tag_field(tag, 1) {
                        geohash = Some(value.to_string());
                    }
                }
                Some("bond") => {
                    bond = Some(
                        parse_tag_field::<u64>(tag, 1)
                            .ok_or_else(|| Nip69Error::Parse("invalid bond value".to_string()))?,
                    );
                }
                _ => {}
            }
        }

        let order_id =
            order_id.ok_or_else(|| Nip69Error::MissingField("d (order_id)".to_string()))?;
        let order_type =
            order_type.ok_or_else(|| Nip69Error::MissingField("k (order_type)".to_string()))?;
        let currency =
            currency.ok_or_else(|| Nip69Error::MissingField("f (currency)".to_string()))?;
        let status = status.ok_or_else(|| Nip69Error::MissingField("s (status)".to_string()))?;
        let amount_sats =
            amount_sats.ok_or_else(|| Nip69Error::MissingField("amt (amount)".to_string()))?;
        let premium = premium.ok_or_else(|| Nip69Error::MissingField("premium".to_string()))?;
        let network = network.ok_or_else(|| Nip69Error::MissingField("network".to_string()))?;
        let layer = layer.ok_or_else(|| Nip69Error::MissingField("layer".to_string()))?;
        let expires_at =
            expires_at.ok_or_else(|| Nip69Error::MissingField("expires_at".to_string()))?;
        let expiration =
            expiration.ok_or_else(|| Nip69Error::MissingField("expiration".to_string()))?;
        let platform =
            platform.ok_or_else(|| Nip69Error::MissingField("y (platform)".to_string()))?;

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
