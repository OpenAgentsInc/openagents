use super::{BitcoinLayer, DOCUMENT_TYPE, OrderStatus, OrderType};

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
        let now = crate::nip01::unix_now_secs().map_or(0, |timestamp| timestamp);

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
            expires_at: now + 3600,  // 1 hour default
            expiration: now + 86400, // 24 hours default
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
