//! Terminal display for orderbook data
//!
//! Provides raw feed streaming and aggregated orderbook views.

use crate::market::MarketKey;
use crate::parser::ParsedOrder;
use crate::state::OrderbookState;
use chrono::{DateTime, Local, TimeZone};
use std::io::{self, Write};

/// Display mode for the viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Stream events as they arrive
    RawFeed,
    /// Show aggregated orderbook view
    Aggregated,
    /// Output JSON (one event per line)
    Json,
}

/// Format a timestamp for display
fn format_timestamp(ts: u64) -> String {
    Local
        .timestamp_opt(ts as i64, 0)
        .single()
        .map(|dt: DateTime<Local>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// Truncate a string with ellipsis (UTF-8 safe)
fn truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        "...".to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    }
}

/// Print a single order in raw feed format
pub fn print_order_raw(order: &ParsedOrder) {
    let status_indicator = match order.status.as_deref() {
        Some("pending") => "NEW",
        Some("in-progress") => "ACTIVE",
        Some("success") => "DONE",
        Some("canceled") => "CANCEL",
        Some("expired") => "EXPIRED",
        _ => "???",
    };

    let side_display = match order.side.as_deref() {
        Some("buy") => "BUY ",
        Some("sell") => "SELL",
        _ => "??? ",
    };

    println!();
    println!(
        "[{}] {} ORDER",
        format_timestamp(order.created_at),
        status_indicator
    );
    println!(
        "  Event: {} | Relay: {}",
        truncate(&order.event_id, 12),
        order.relay_url
    );
    println!("  Coord: {}", order.coord.to_addr());
    println!(
        "  {} | Status: {} | Currency: {}",
        side_display,
        order.status.as_deref().unwrap_or("?"),
        order.currency.as_deref().unwrap_or("?")
    );

    // Amount info
    let sats_display = order
        .amount_sats
        .map(|a| {
            if a == 0 {
                "range".to_string()
            } else {
                format!("{} sats", a)
            }
        })
        .unwrap_or_else(|| "?".to_string());

    println!(
        "  Amount: {} | Fiat: {} | Premium: {}",
        sats_display,
        order.fiat_display(),
        order.premium_display()
    );

    // Payment and network info
    let payments = if order.payment_methods.is_empty() {
        "?".to_string()
    } else {
        order.payment_methods.join(", ")
    };

    println!(
        "  Payment: {} | Layer: {} | Network: {}",
        truncate(&payments, 30),
        order.layer.as_deref().unwrap_or("?"),
        order.network.as_deref().unwrap_or("?")
    );

    // Platform and expiration
    if let Some(platform) = &order.platform {
        print!("  Platform: {}", platform);
    }
    if let Some(exp) = order.expires_at {
        print!(" | Expires: {}", format_timestamp(exp));
    }
    if order.platform.is_some() || order.expires_at.is_some() {
        println!();
    }

    // Validation warnings
    if !order.validation_errors.is_empty() {
        println!("  Warnings: {}", order.validation_errors.join("; "));
    }
}

/// Print order as JSON
pub fn print_order_json(order: &ParsedOrder) {
    let json = serde_json::json!({
        "event_id": order.event_id,
        "coord": order.coord.to_addr(),
        "pubkey": order.coord.pubkey,
        "created_at": order.created_at,
        "relay_url": order.relay_url,
        "side": order.side,
        "currency": order.currency,
        "status": order.status,
        "amount_sats": order.amount_sats,
        "fiat_amount": order.fiat_amount,
        "premium": order.premium,
        "payment_methods": order.payment_methods,
        "network": order.network,
        "layer": order.layer,
        "expires_at": order.expires_at,
        "platform": order.platform,
        "is_valid": order.is_valid,
        "validation_errors": order.validation_errors,
    });

    println!("{}", serde_json::to_string(&json).unwrap_or_default());
}

/// Print aggregated orderbook view
pub fn print_aggregated_view(state: &OrderbookState, relays_connected: usize) {
    // Clear screen (ANSI escape)
    print!("\x1B[2J\x1B[1;1H");
    let _ = io::stdout().flush();

    let stats = state.stats();

    println!("=== NIP-69 Orderbook Viewer ===");
    println!(
        "Connected: {} relay(s) | Events: {} | Orders: {} active",
        relays_connected, stats.total_events_processed, stats.active_orders
    );
    println!();

    // Get all markets
    let markets = state.get_markets();

    if markets.is_empty() {
        println!("(no orders yet)");
        return;
    }

    for market in markets {
        print_market_depth(state, &market);
    }
}

/// Print depth for a single market
fn print_market_depth(state: &OrderbookState, market: &MarketKey) {
    let orders = state.get_orders_by_market(market);

    // Separate into bids and asks
    let mut bids: Vec<_> = orders
        .iter()
        .filter(|o| o.is_buy() && o.is_active())
        .collect();
    let mut asks: Vec<_> = orders
        .iter()
        .filter(|o| o.is_sell() && o.is_active())
        .collect();

    // Sort bids by premium descending (best first)
    bids.sort_by(|a, b| {
        b.premium
            .unwrap_or(f64::NEG_INFINITY)
            .partial_cmp(&a.premium.unwrap_or(f64::NEG_INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sort asks by premium ascending (best first)
    asks.sort_by(|a, b| {
        a.premium
            .unwrap_or(f64::INFINITY)
            .partial_cmp(&b.premium.unwrap_or(f64::INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("--- {} ---", market);

    if bids.is_empty() && asks.is_empty() {
        println!("  (no active orders)");
        println!();
        return;
    }

    // Header
    println!("  {:<20} {:<20}", "BIDS (BUY)", "ASKS (SELL)");

    // Print side by side (up to 5 levels each)
    let max_levels = 5;
    for i in 0..max_levels {
        let bid_str = bids
            .get(i)
            .map(|o| {
                format!(
                    "{} @ {}",
                    o.amount_sats
                        .map(|a| if a == 0 {
                            format!("${}", o.fiat_display())
                        } else {
                            format!("{} sats", a)
                        })
                        .unwrap_or_else(|| "?".to_string()),
                    o.premium_display()
                )
            })
            .unwrap_or_default();

        let ask_str = asks
            .get(i)
            .map(|o| {
                format!(
                    "{} @ {}",
                    o.amount_sats
                        .map(|a| if a == 0 {
                            format!("${}", o.fiat_display())
                        } else {
                            format!("{} sats", a)
                        })
                        .unwrap_or_else(|| "?".to_string()),
                    o.premium_display()
                )
            })
            .unwrap_or_default();

        if bid_str.is_empty() && ask_str.is_empty() {
            break;
        }

        println!("  {:<20} {:<20}", bid_str, ask_str);
    }

    // Show if there are more
    if bids.len() > max_levels || asks.len() > max_levels {
        println!(
            "  ... (+{} bids, +{} asks)",
            bids.len().saturating_sub(max_levels),
            asks.len().saturating_sub(max_levels)
        );
    }

    println!();
}

/// Print a status line (for live updates)
pub fn print_status_line(message: &str) {
    eprint!("\r{:60}", message);
    let _ = io::stderr().flush();
}

/// Print startup banner
pub fn print_banner(relays: &[String]) {
    println!("NIP-69 Orderbook Viewer");
    println!("=======================");
    println!();
    println!("Connecting to {} relay(s):", relays.len());
    for relay in relays {
        println!("  - {}", relay);
    }
    println!();
    println!("Subscribing to kind 38383 (P2P orders)...");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::OrderCoord;

    fn make_test_order(side: &str, premium: f64, status: &str) -> ParsedOrder {
        ParsedOrder {
            event_id: "test123".to_string(),
            coord: OrderCoord::new(38383, "pubkey".to_string(), "order-1".to_string()),
            created_at: 1735400000,
            relay_url: "wss://test.relay".to_string(),
            side: Some(side.to_string()),
            currency: Some("USD".to_string()),
            status: Some(status.to_string()),
            amount_sats: Some(10000),
            fiat_amount: vec![100],
            premium: Some(premium),
            payment_methods: vec!["cashu".to_string()],
            network: Some("mainnet".to_string()),
            layer: Some("lightning".to_string()),
            expires_at: Some(1735403600),
            expiration: None,
            platform: Some("mostro".to_string()),
            source: None,
            name: None,
            geohash: None,
            bond: None,
            validation_errors: vec![],
            is_valid: true,
        }
    }

    #[test]
    fn test_format_timestamp() {
        let ts = format_timestamp(1735400000);
        // Just check it doesn't panic and returns something
        assert!(!ts.is_empty());
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world!", 8), "hello...");
        // UTF-8 safe: emoji and unicode chars count as single chars
        assert_eq!(truncate("🏦🇵🇾", 10), "🏦🇵🇾");
        assert_eq!(truncate("🏦🇵🇾hello", 5), "🏦🇵...");
        // Edge cases
        assert_eq!(truncate("ab", 3), "ab");
        assert_eq!(truncate("abcd", 3), "...");
    }

    #[test]
    fn test_print_order_json() {
        let order = make_test_order("sell", 2.5, "pending");
        // Just verify it doesn't panic
        print_order_json(&order);
    }
}
