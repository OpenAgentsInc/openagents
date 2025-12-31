//! NIP-69 Orderbook GUI - Bloomberg Terminal Style
//!
//! A graphical orderbook viewer using wgpui.

use orderbook::OrderbookState;
use orderbook::gui::OrderbookApp;
use std::sync::Arc;
use tokio::sync::RwLock;
use winit::event_loop::EventLoop;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("orderbook=info".parse().unwrap())
                .add_directive("nostr_client=warn".parse().unwrap())
                .add_directive("wgpu=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    // Create shared orderbook state
    let orderbook_state = Arc::new(RwLock::new(OrderbookState::new()));

    // Default relays with NIP-69 activity
    let relays = vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.mostro.network".to_string(),
    ];

    // Create event loop and app
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = OrderbookApp::new(orderbook_state, relays);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
