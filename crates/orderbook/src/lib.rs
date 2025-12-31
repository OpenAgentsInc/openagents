//! NIP-69 Orderbook Viewer Library
//!
//! Provides types and utilities for tracking NIP-69 P2P order events from Nostr relays.
//!
//! # Overview
//!
//! NIP-69 defines kind 38383 addressable events for peer-to-peer trading.
//! This library provides:
//!
//! - **Lenient parsing**: Extract order data from events with validation tracking
//! - **State management**: Track orders by coordinate with deduplication
//! - **Market grouping**: Organize orders by currency/network/layer
//! - **Terminal display**: Raw feed streaming and aggregated orderbook views
//!
//! # Order Coordinates
//!
//! NIP-69 orders are addressable events. Each order is uniquely identified by:
//! - `kind`: Always 38383
//! - `pubkey`: Event author
//! - `d` tag: Order ID
//!
//! Newer events with the same coordinate supersede older ones.
//!
//! # Example
//!
//! ```rust,ignore
//! use orderbook::parser::parse_order_lenient;
//! use orderbook::state::OrderbookState;
//!
//! let mut state = OrderbookState::new();
//!
//! // Parse an incoming event
//! let order = parse_order_lenient(&event, "wss://relay.mostro.network");
//!
//! // Process into state (handles deduplication)
//! state.process_order(order);
//!
//! // Query active orders
//! for order in state.get_active_orders() {
//!     println!("{}: {} {} @ {}",
//!         order.coord,
//!         order.side.as_deref().unwrap_or("?"),
//!         order.amount_sats.unwrap_or(0),
//!         order.premium_display()
//!     );
//! }
//! ```

pub mod market;
pub mod parser;
pub mod state;
pub mod viewer;

#[cfg(feature = "gui")]
pub mod gui;

pub use market::MarketKey;
pub use parser::{P2P_ORDER_KIND, ParsedOrder, parse_order_lenient};
pub use state::{OrderCoord, OrderbookState};
