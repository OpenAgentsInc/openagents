//! Lenient NIP-69 event parsing with validation tracking
//!
//! Parses kind 38383 events with graceful handling of missing or malformed tags.

use crate::market::MarketKey;
use crate::state::OrderCoord;
use nostr::Event;

/// NIP-69 order kind
pub const P2P_ORDER_KIND: u16 = 38383;

/// Parsed order with validation notes
#[derive(Debug, Clone)]
pub struct ParsedOrder {
    /// Event ID (hex)
    pub event_id: String,
    /// Order coordinate for deduplication
    pub coord: OrderCoord,
    /// Event creation timestamp
    pub created_at: u64,
    /// Relay URL where this event was received
    pub relay_url: String,

    // NIP-69 fields (all optional for lenient parsing)
    /// Order side: "buy" or "sell"
    pub side: Option<String>,
    /// Fiat currency (ISO 4217)
    pub currency: Option<String>,
    /// Order status: pending, canceled, in-progress, success, expired
    pub status: Option<String>,
    /// Amount in satoshis (0 means amount determined later)
    pub amount_sats: Option<u64>,
    /// Fiat amount (single value or [min, max] range)
    pub fiat_amount: Vec<u64>,
    /// Premium percentage (can be negative for discount)
    pub premium: Option<f64>,
    /// Accepted payment methods
    pub payment_methods: Vec<String>,
    /// Bitcoin network: mainnet, testnet, signet
    pub network: Option<String>,
    /// Bitcoin layer: onchain, lightning, liquid
    pub layer: Option<String>,
    /// When order should expire (for pending status)
    pub expires_at: Option<u64>,
    /// NIP-40 event expiration (relay deletion hint)
    pub expiration: Option<u64>,
    /// Platform name (e.g., mostro, lnp2pbot)
    pub platform: Option<String>,
    /// Source URL
    pub source: Option<String>,
    /// Maker's display name
    pub name: Option<String>,
    /// Geohash location
    pub geohash: Option<String>,
    /// Bond amount in sats
    pub bond: Option<u64>,

    // Validation
    /// Validation errors encountered during parsing
    pub validation_errors: Vec<String>,
    /// Whether the order has all required fields
    pub is_valid: bool,
}

impl ParsedOrder {
    /// Get the market key for this order
    pub fn market_key(&self) -> MarketKey {
        MarketKey::from_optional(
            self.currency.as_deref(),
            self.network.as_deref(),
            self.layer.as_deref(),
        )
    }

    /// Check if this is a buy order
    pub fn is_buy(&self) -> bool {
        self.side.as_deref() == Some("buy")
    }

    /// Check if this is a sell order
    pub fn is_sell(&self) -> bool {
        self.side.as_deref() == Some("sell")
    }

    /// Check if this is a range order (amt=0)
    pub fn is_range_order(&self) -> bool {
        self.amount_sats == Some(0)
    }

    /// Check if order is active (pending status)
    pub fn is_active(&self) -> bool {
        self.status.as_deref() == Some("pending")
    }

    /// Get fiat amount as display string
    pub fn fiat_display(&self) -> String {
        match self.fiat_amount.len() {
            0 => "?".to_string(),
            1 => format!("{}", self.fiat_amount[0]),
            _ => format!(
                "{}-{}",
                self.fiat_amount[0],
                self.fiat_amount.last().unwrap()
            ),
        }
    }

    /// Get premium as display string
    pub fn premium_display(&self) -> String {
        match self.premium {
            Some(p) if p > 0.0 => format!("+{:.1}%", p),
            Some(p) if p < 0.0 => format!("{:.1}%", p),
            Some(_) => "0%".to_string(),
            None => "?".to_string(),
        }
    }
}

/// Extract a single tag value from event tags
fn extract_tag(tags: &[Vec<String>], name: &str) -> Option<String> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == name)
        .map(|t| t[1].clone())
}

/// Extract all values for a tag (for multi-value tags like "fa" and "pm")
fn extract_tag_values(tags: &[Vec<String>], name: &str) -> Vec<String> {
    tags.iter()
        .find(|t| !t.is_empty() && t[0] == name)
        .map(|t| t[1..].to_vec())
        .unwrap_or_default()
}

/// Parse a NIP-69 event leniently, capturing validation errors
pub fn parse_order_lenient(event: &Event, relay_url: &str) -> ParsedOrder {
    let mut errors = Vec::new();

    // Validate event kind
    if event.kind != P2P_ORDER_KIND {
        errors.push(format!(
            "Wrong event kind: expected {}, got {}",
            P2P_ORDER_KIND, event.kind
        ));
    }

    // Extract d-tag (required for coordinate)
    let d_tag = extract_tag(&event.tags, "d");
    if d_tag.is_none() {
        errors.push("Missing required 'd' tag (order_id)".to_string());
    }

    // Extract and validate side (k tag)
    let side = extract_tag(&event.tags, "k");
    if side.is_none() {
        errors.push("Missing 'k' tag (order type)".to_string());
    } else if !matches!(side.as_deref(), Some("buy") | Some("sell")) {
        errors.push(format!(
            "Invalid 'k' tag: expected 'buy' or 'sell', got {:?}",
            side
        ));
    }

    // Extract currency (f tag)
    let currency = extract_tag(&event.tags, "f");
    if currency.is_none() {
        errors.push("Missing 'f' tag (currency)".to_string());
    }

    // Extract status (s tag)
    let status = extract_tag(&event.tags, "s");
    if status.is_none() {
        errors.push("Missing 's' tag (status)".to_string());
    }

    // Extract amount (amt tag)
    let amount_sats = extract_tag(&event.tags, "amt").and_then(|s| s.parse::<u64>().ok());
    if amount_sats.is_none() {
        errors.push("Missing or invalid 'amt' tag".to_string());
    }

    // Extract fiat amount (fa tag) - can be single value or range
    let fa_values = extract_tag_values(&event.tags, "fa");
    let fiat_amount: Vec<u64> = fa_values
        .iter()
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    if fiat_amount.is_empty() {
        errors.push("Missing or invalid 'fa' tag (fiat amount)".to_string());
    }

    // Extract premium
    let premium = extract_tag(&event.tags, "premium").and_then(|s| s.parse::<f64>().ok());

    // Extract payment methods (pm tag)
    let payment_methods = extract_tag_values(&event.tags, "pm");
    if payment_methods.is_empty() {
        errors.push("Missing 'pm' tag (payment methods)".to_string());
    }

    // Extract network
    let network = extract_tag(&event.tags, "network");

    // Extract layer
    let layer = extract_tag(&event.tags, "layer");

    // Extract expiration times
    let expires_at = extract_tag(&event.tags, "expires_at").and_then(|s| s.parse::<u64>().ok());
    let expiration = extract_tag(&event.tags, "expiration").and_then(|s| s.parse::<u64>().ok());

    // Extract platform (y tag)
    let platform = extract_tag(&event.tags, "y");

    // Extract optional fields
    let source = extract_tag(&event.tags, "source");
    let name = extract_tag(&event.tags, "name");
    let geohash = extract_tag(&event.tags, "g");
    let bond = extract_tag(&event.tags, "bond").and_then(|s| s.parse::<u64>().ok());

    // Determine if valid (has all required fields)
    let is_valid = d_tag.is_some()
        && side.is_some()
        && currency.is_some()
        && status.is_some()
        && amount_sats.is_some()
        && !fiat_amount.is_empty()
        && !payment_methods.is_empty();

    ParsedOrder {
        event_id: event.id.clone(),
        coord: OrderCoord::new(event.kind, event.pubkey.clone(), d_tag.unwrap_or_default()),
        created_at: event.created_at,
        relay_url: relay_url.to_string(),
        side,
        currency,
        status,
        amount_sats,
        fiat_amount,
        premium,
        payment_methods,
        network,
        layer,
        expires_at,
        expiration,
        platform,
        source,
        name,
        geohash,
        bond,
        validation_errors: errors,
        is_valid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_event_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1735400000,
            kind,
            tags,
            content: String::new(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_parse_valid_order() {
        let tags = vec![
            vec!["d".to_string(), "order-123".to_string()],
            vec!["k".to_string(), "sell".to_string()],
            vec!["f".to_string(), "USD".to_string()],
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), "10000".to_string()],
            vec!["fa".to_string(), "100".to_string()],
            vec![
                "pm".to_string(),
                "cashu".to_string(),
                "lightning".to_string(),
            ],
            vec!["premium".to_string(), "2.5".to_string()],
            vec!["network".to_string(), "mainnet".to_string()],
            vec!["layer".to_string(), "lightning".to_string()],
            vec!["y".to_string(), "mostro".to_string()],
        ];

        let event = make_test_event(P2P_ORDER_KIND, tags);
        let order = parse_order_lenient(&event, "wss://test.relay");

        assert!(order.is_valid);
        assert!(order.validation_errors.is_empty());
        assert_eq!(order.coord.d_tag, "order-123");
        assert_eq!(order.side, Some("sell".to_string()));
        assert_eq!(order.currency, Some("USD".to_string()));
        assert_eq!(order.amount_sats, Some(10000));
        assert_eq!(order.fiat_amount, vec![100]);
        assert_eq!(order.premium, Some(2.5));
        assert_eq!(order.payment_methods, vec!["cashu", "lightning"]);
        assert_eq!(order.platform, Some("mostro".to_string()));
    }

    #[test]
    fn test_parse_range_order() {
        let tags = vec![
            vec!["d".to_string(), "order-456".to_string()],
            vec!["k".to_string(), "buy".to_string()],
            vec!["f".to_string(), "EUR".to_string()],
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), "0".to_string()],
            vec!["fa".to_string(), "100".to_string(), "500".to_string()],
            vec!["pm".to_string(), "sepa".to_string()],
        ];

        let event = make_test_event(P2P_ORDER_KIND, tags);
        let order = parse_order_lenient(&event, "wss://test.relay");

        assert!(order.is_valid);
        assert!(order.is_range_order());
        assert_eq!(order.fiat_amount, vec![100, 500]);
        assert_eq!(order.fiat_display(), "100-500");
    }

    #[test]
    fn test_parse_missing_required_tags() {
        let tags = vec![
            vec!["d".to_string(), "order-789".to_string()],
            // Missing k, f, s, amt, fa, pm
        ];

        let event = make_test_event(P2P_ORDER_KIND, tags);
        let order = parse_order_lenient(&event, "wss://test.relay");

        assert!(!order.is_valid);
        assert!(!order.validation_errors.is_empty());
        assert!(
            order
                .validation_errors
                .iter()
                .any(|e| e.contains("'k' tag"))
        );
        assert!(
            order
                .validation_errors
                .iter()
                .any(|e| e.contains("'f' tag"))
        );
    }

    #[test]
    fn test_parse_wrong_kind() {
        let tags = vec![vec!["d".to_string(), "note-123".to_string()]];

        let event = make_test_event(1, tags); // Kind 1, not 38383
        let order = parse_order_lenient(&event, "wss://test.relay");

        assert!(!order.is_valid);
        assert!(
            order
                .validation_errors
                .iter()
                .any(|e| e.contains("Wrong event kind"))
        );
    }

    #[test]
    fn test_market_key_extraction() {
        let tags = vec![
            vec!["d".to_string(), "order-123".to_string()],
            vec!["k".to_string(), "sell".to_string()],
            vec!["f".to_string(), "usd".to_string()], // lowercase
            vec!["s".to_string(), "pending".to_string()],
            vec!["amt".to_string(), "10000".to_string()],
            vec!["fa".to_string(), "100".to_string()],
            vec!["pm".to_string(), "cashu".to_string()],
            vec!["network".to_string(), "mainnet".to_string()],
            vec!["layer".to_string(), "onchain".to_string()],
        ];

        let event = make_test_event(P2P_ORDER_KIND, tags);
        let order = parse_order_lenient(&event, "wss://test.relay");

        let market = order.market_key();
        assert_eq!(market.currency, "USD"); // Should be uppercased
        assert_eq!(market.network, "mainnet");
        assert_eq!(market.layer, "onchain");
    }

    #[test]
    fn test_premium_display() {
        let mut order = ParsedOrder {
            event_id: "test".to_string(),
            coord: OrderCoord::new(38383, "pk".to_string(), "d".to_string()),
            created_at: 0,
            relay_url: "wss://test".to_string(),
            side: None,
            currency: None,
            status: None,
            amount_sats: None,
            fiat_amount: vec![],
            premium: None,
            payment_methods: vec![],
            network: None,
            layer: None,
            expires_at: None,
            expiration: None,
            platform: None,
            source: None,
            name: None,
            geohash: None,
            bond: None,
            validation_errors: vec![],
            is_valid: false,
        };

        order.premium = Some(2.5);
        assert_eq!(order.premium_display(), "+2.5%");

        order.premium = Some(-1.0);
        assert_eq!(order.premium_display(), "-1.0%");

        order.premium = Some(0.0);
        assert_eq!(order.premium_display(), "0%");

        order.premium = None;
        assert_eq!(order.premium_display(), "?");
    }
}
