//! NIP-69 Orderbook Viewer CLI
//!
//! Watch live P2P order flows from Nostr relays.

use clap::Parser;
use nostr::Event;
use nostr_client::RelayConnection;
use orderbook::parser::{P2P_ORDER_KIND, parse_order_lenient};
use orderbook::state::OrderbookState;
use orderbook::viewer::{
    DisplayMode, print_aggregated_view, print_banner, print_order_json, print_order_raw,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};

/// NIP-69 orderbook viewer - watch live P2P order flows from Nostr
#[derive(Parser, Debug)]
#[command(name = "orderbook")]
#[command(version, about, long_about = None)]
struct Args {
    /// Relay URLs to connect to (defaults to relays with known NIP-69 activity)
    #[arg(long, default_values_t = vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.mostro.network".to_string(),
    ])]
    relays: Vec<String>,

    /// Filter by fiat currency (ISO 4217, e.g., USD, EUR)
    #[arg(long)]
    currency: Option<String>,

    /// Filter by network (mainnet, testnet, signet)
    #[arg(long)]
    network: Option<String>,

    /// Filter by layer (onchain, lightning, liquid)
    #[arg(long)]
    layer: Option<String>,

    /// Show canceled/expired orders
    #[arg(long)]
    show_inactive: bool,

    /// Only show orders created after this Unix timestamp
    #[arg(long)]
    since: Option<u64>,

    /// Output as JSON (one event per line)
    #[arg(long)]
    json: bool,

    /// Show aggregated orderbook view (refreshes periodically)
    #[arg(long)]
    aggregate: bool,

    /// Refresh interval for aggregated view (milliseconds)
    #[arg(long, default_value = "2000")]
    refresh_ms: u64,

    /// Maximum number of events to show in feed before stopping
    #[arg(long)]
    limit: Option<usize>,
}

/// Build subscription filter based on CLI args
fn build_filter(args: &Args) -> serde_json::Value {
    let mut filter = json!({
        "kinds": [P2P_ORDER_KIND]
    });

    if let Some(since) = args.since {
        filter["since"] = json!(since);
    }

    filter
}

/// Check if an order passes the CLI filters
fn passes_filter(order: &orderbook::parser::ParsedOrder, args: &Args) -> bool {
    // Currency filter
    if let Some(ref currency) = args.currency {
        if order.currency.as_ref().map(|c| c.to_uppercase()) != Some(currency.to_uppercase()) {
            return false;
        }
    }

    // Network filter
    if let Some(ref network) = args.network {
        if order.network.as_ref().map(|n| n.to_lowercase()) != Some(network.to_lowercase()) {
            return false;
        }
    }

    // Layer filter
    if let Some(ref layer) = args.layer {
        if order.layer.as_ref().map(|l| l.to_lowercase()) != Some(layer.to_lowercase()) {
            return false;
        }
    }

    // Inactive filter
    if !args.show_inactive {
        if order.status.as_deref() != Some("pending") {
            return false;
        }
    }

    true
}

/// Process a received event
fn process_event(
    event: Event,
    relay_url: &str,
    state: &mut OrderbookState,
    args: &Args,
    display_mode: DisplayMode,
) {
    let order = parse_order_lenient(&event, relay_url);

    // Always add to state (for aggregated view)
    state.process_order(order.clone());

    // Check if passes display filters
    if !passes_filter(&order, args) {
        return;
    }

    // Display based on mode
    match display_mode {
        DisplayMode::Json => print_order_json(&order),
        DisplayMode::RawFeed => print_order_raw(&order),
        DisplayMode::Aggregated => {
            // Don't print individual events in aggregated mode
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("orderbook=info".parse()?)
                .add_directive("nostr_client=warn".parse()?),
        )
        .with_target(false)
        .init();

    let args = Args::parse();

    // Determine display mode
    let display_mode = if args.json {
        DisplayMode::Json
    } else if args.aggregate {
        DisplayMode::Aggregated
    } else {
        DisplayMode::RawFeed
    };

    // Print banner (unless JSON mode)
    if display_mode != DisplayMode::Json {
        print_banner(&args.relays);
    }

    // Create shared state
    let state = Arc::new(RwLock::new(OrderbookState::new()));

    // Track connected relay count
    let connected_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Event counter for limit
    let event_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Build subscription filter
    let filter = build_filter(&args);

    // Connect to each relay and start event processing
    let mut handles = Vec::new();

    for relay_url in &args.relays {
        let relay_url = relay_url.clone();
        let state = Arc::clone(&state);
        let args_clone = Args::parse(); // Clone args for each task
        let filter = filter.clone();
        let connected_count = Arc::clone(&connected_count);
        let event_count = Arc::clone(&event_count);
        let limit = args.limit;

        let handle = tokio::spawn(async move {
            // Connect to relay
            let relay = match RelayConnection::new(&relay_url) {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to create relay connection to {}: {}", relay_url, e);
                    return;
                }
            };

            if let Err(e) = relay.connect().await {
                error!("Failed to connect to {}: {}", relay_url, e);
                return;
            }

            info!("Connected to {}", relay_url);
            connected_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Subscribe to orders
            let rx = match relay
                .subscribe_with_channel("nip69-orders", &[filter])
                .await
            {
                Ok(rx) => rx,
                Err(e) => {
                    error!("Failed to subscribe on {}: {}", relay_url, e);
                    return;
                }
            };

            info!("Subscribed to kind {} on {}", P2P_ORDER_KIND, relay_url);

            // Process events
            let mut rx = rx;
            while let Some(event) = rx.recv().await {
                // Check limit
                if let Some(max) = limit {
                    let count = event_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count >= max {
                        info!("Reached event limit ({})", max);
                        break;
                    }
                }

                let mut state_guard = state.write().await;
                process_event(
                    event,
                    &relay_url,
                    &mut state_guard,
                    &args_clone,
                    display_mode,
                );
            }

            warn!("Event stream ended for {}", relay_url);
        });

        handles.push(handle);
    }

    // If aggregated mode, start refresh loop
    if display_mode == DisplayMode::Aggregated {
        let state = Arc::clone(&state);
        let connected_count = Arc::clone(&connected_count);
        let refresh_ms = args.refresh_ms;

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(refresh_ms));
            loop {
                ticker.tick().await;
                let state_guard = state.read().await;
                let count = connected_count.load(std::sync::atomic::Ordering::Relaxed);
                print_aggregated_view(&state_guard, count);
            }
        });
    }

    // Wait for all relay tasks (or Ctrl+C)
    tokio::select! {
        _ = futures::future::join_all(handles) => {
            info!("All relay connections ended");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Shutting down...");
        }
    }

    Ok(())
}
