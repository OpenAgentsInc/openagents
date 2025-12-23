//! GitAfter Desktop - Nostr-native GitHub Alternative
//!
//! Decentralized git collaboration powered by NIP-34 (Git Stuff) and NIP-SA (Sovereign Agents).
//! Enables agents as first-class contributors with trajectory proof and bounty payments.

mod git;
mod middleware;
mod nostr;
mod reputation;
mod review;
mod server;
mod trajectory;
mod views;
mod ws;

use anyhow::Result;
use bip39::Mnemonic;
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wallet::core::identity::UnifiedIdentity;
use wry::WebViewBuilder;

use nostr::NostrClient;
use server::start_server;
use ws::WsBroadcaster;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting GitAfter...");

    // Create broadcaster for WebSocket state
    let broadcaster = Arc::new(WsBroadcaster::new(64));

    // Start tokio runtime + actix server in background thread
    let broadcaster_clone = broadcaster;
    let (port_tx, port_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            // Load identity from secure storage or environment variable
            // SECURITY: Environment variables are deprecated due to visibility to other processes.
            // Prefer using secure storage instead.
            let identity = if let Ok(mnemonic_str) = std::env::var("GITAFTER_MNEMONIC") {
                tracing::warn!(
                    "Loading mnemonic from GITAFTER_MNEMONIC environment variable. \
                     WARNING: Environment variables are visible to all processes running as the same user. \
                     Consider using secure storage instead."
                );
                match Mnemonic::parse(&mnemonic_str) {
                    Ok(mnemonic) => {
                        match UnifiedIdentity::from_mnemonic(mnemonic) {
                            Ok(id) => {
                                tracing::info!("Loaded identity from GITAFTER_MNEMONIC (insecure)");
                                Some(Arc::new(id))
                            }
                            Err(e) => {
                                tracing::error!("Failed to create identity from mnemonic: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse mnemonic: {}", e);
                        None
                    }
                }
            } else {
                // TODO: Implement loading from secure storage (keychain/keyring)
                tracing::warn!("No GITAFTER_MNEMONIC environment variable set - running in read-only mode");
                tracing::info!("To enable event signing and publishing, use secure storage (not yet implemented) or set GITAFTER_MNEMONIC (insecure)");
                None
            };

            // Initialize Nostr client
            let relay_urls = vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ];

            let nostr_client = match NostrClient::new(relay_urls.clone(), broadcaster_clone.clone()) {
                Ok(client) => {
                    // Connect and subscribe to git events
                    if let Err(e) = client.connect(relay_urls.clone()).await {
                        tracing::warn!("Failed to connect to Nostr relays: {}", e);
                    } else if let Err(e) = client.subscribe_to_git_events().await {
                        tracing::warn!("Failed to subscribe to git events: {}", e);
                    }
                    Arc::new(client)
                }
                Err(e) => {
                    tracing::error!("Failed to initialize Nostr client: {}", e);
                    // Create a dummy client for now
                    Arc::new(NostrClient::new(vec![], broadcaster_clone.clone()).expect("dummy client"))
                }
            };

            // Start server with broadcaster, nostr_client, and identity
            let port = start_server(broadcaster_clone.clone(), nostr_client, identity)
                .await
                .expect("start server");
            port_tx.send(port).expect("send port");

            // Keep runtime alive
            std::future::pending::<()>().await;
        });
    });

    let port = port_rx.recv().expect("receive port");
    println!("GITAFTER_PORT={}", port);
    tracing::info!("GitAfter server running on http://127.0.0.1:{}", port);

    // tao event loop (must be on main thread for macOS)
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("GitAfter - Nostr GitHub Alternative")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 900.0))
        .build(&event_loop)
        .expect("window");

    let _webview = WebViewBuilder::new()
        .with_url(format!("http://127.0.0.1:{}/", port))
        .build(&window)
        .expect("webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
