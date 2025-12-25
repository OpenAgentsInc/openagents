//! GitAfter desktop application runner

use anyhow::Result;
use bip39::Mnemonic;
use openagents_spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wallet::core::identity::UnifiedIdentity;
use wry::WebViewBuilder;

use crate::nostr::NostrClient;
use crate::server::start_server;
use crate::ws::WsBroadcaster;

pub fn run() -> Result<()> {
    run_with_route(None)
}

pub fn run_with_route(route: Option<&str>) -> Result<()> {
    tracing_subscriber::fmt().try_init().ok();

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
            // Load identity and wallet from secure storage or environment variable
            // SECURITY: Environment variables are deprecated due to visibility to other processes.
            // Prefer using secure storage instead.
            let (identity, wallet) = if let Ok(mnemonic_str) = std::env::var("GITAFTER_MNEMONIC") {
                tracing::warn!(
                    "Loading mnemonic from GITAFTER_MNEMONIC environment variable. \
                     WARNING: Environment variables are visible to all processes running as the same user. \
                     Consider using secure storage instead."
                );
                match Mnemonic::parse(&mnemonic_str) {
                    Ok(mnemonic) => {
                        let identity_result = UnifiedIdentity::from_mnemonic(mnemonic.clone());

                        // Create Spark wallet from mnemonic
                        let wallet_result = async {
                            let signer = SparkSigner::from_mnemonic(&mnemonic.to_string(), "")?;
                            let config = WalletConfig {
                                network: Network::Testnet,
                                api_key: None,
                                storage_dir: dirs::data_local_dir()
                                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                                    .join("openagents")
                                    .join("gitafter")
                                    .join("spark"),
                            };
                            SparkWallet::new(signer, config).await
                        }
                        .await;

                        match (identity_result, wallet_result) {
                            (Ok(id), Ok(w)) => {
                                tracing::info!("Loaded identity and wallet from GITAFTER_MNEMONIC");
                                (Some(Arc::new(id)), Some(Arc::new(w)))
                            }
                            (Ok(id), Err(e)) => {
                                tracing::error!("Failed to create wallet: {}", e);
                                tracing::info!("Running without wallet - bounty payments disabled");
                                (Some(Arc::new(id)), None)
                            }
                            (Err(e), _) => {
                                tracing::error!("Failed to create identity from mnemonic: {}", e);
                                (None, None)
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse mnemonic: {}", e);
                        (None, None)
                    }
                }
            } else {
                // Try loading from secure storage (keychain/keyring)
                match crate::secure_storage::load_mnemonic() {
                    Ok(Some(mnemonic_str)) => {
                        tracing::info!("Loading mnemonic from secure storage (keychain)");
                        match Mnemonic::parse(&mnemonic_str) {
                            Ok(mnemonic) => {
                                let identity_result = UnifiedIdentity::from_mnemonic(mnemonic.clone());

                                // Create Spark wallet from mnemonic
                                let wallet_result = async {
                                    let signer = SparkSigner::from_mnemonic(&mnemonic.to_string(), "")?;
                                    let config = WalletConfig {
                                        network: Network::Testnet,
                                        api_key: None,
                                        storage_dir: dirs::data_local_dir()
                                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                                            .join("openagents")
                                            .join("gitafter")
                                            .join("spark"),
                                    };
                                    SparkWallet::new(signer, config).await
                                }
                                .await;

                                match (identity_result, wallet_result) {
                                    (Ok(id), Ok(w)) => {
                                        tracing::info!("Loaded identity and wallet from secure storage");
                                        (Some(Arc::new(id)), Some(Arc::new(w)))
                                    }
                                    (Ok(id), Err(e)) => {
                                        tracing::error!("Failed to create wallet: {}", e);
                                        tracing::info!("Running without wallet - bounty payments disabled");
                                        (Some(Arc::new(id)), None)
                                    }
                                    (Err(e), _) => {
                                        tracing::error!("Failed to create identity from stored mnemonic: {}", e);
                                        (None, None)
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse mnemonic from secure storage: {}", e);
                                (None, None)
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("No mnemonic found in secure storage - running in read-only mode");
                        tracing::info!("To enable event signing and publishing:");
                        tracing::info!("  1. Use `openagents wallet init` to create a new identity");
                        tracing::info!("  2. Or set GITAFTER_MNEMONIC environment variable (less secure)");
                        (None, None)
                    }
                    Err(e) => {
                        tracing::error!("Failed to access secure storage: {}", e);
                        tracing::warn!("Running in read-only mode");
                        (None, None)
                    }
                }
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

            // Start server with broadcaster, nostr_client, identity, and wallet
            let (port, _server_handle) = start_server(
                broadcaster_clone.clone(),
                nostr_client,
                identity,
                wallet,
            )
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

    let route = match route {
        Some(path) if path.starts_with('/') => path.to_string(),
        Some(path) => format!("/{}", path),
        None => "/".to_string(),
    };

    // tao event loop (must be on main thread for macOS)
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("GitAfter - Nostr GitHub Alternative")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 900.0))
        .build(&event_loop)
        .expect("window");

    let url = format!("http://127.0.0.1:{}{}", port, route);
    let _webview = WebViewBuilder::new()
        .with_url(&url)
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
