//! GUI application entry point

use anyhow::Result;
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use bip39::Mnemonic;

use super::server::start_server;
use crate::core::identity::UnifiedIdentity;
use crate::storage::identities::{current_identity, DEFAULT_IDENTITY_NAME};
use crate::storage::keychain::{SecureKeychain, WALLET_PASSWORD_ENV};

/// Run the wallet GUI application
pub fn run_gui() -> Result<()> {
    tracing::info!("Starting Wallet GUI...");

    // Load identity from keychain
    let identity_name = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
    let identity = if SecureKeychain::has_mnemonic_for(&identity_name) {
        let mnemonic_result = if SecureKeychain::is_password_protected_for(&identity_name) {
            match std::env::var(WALLET_PASSWORD_ENV) {
                Ok(password) => {
                    SecureKeychain::retrieve_mnemonic_with_password_for(&identity_name, &password)
                }
                Err(_) => {
                    tracing::warn!(
                        "Wallet is password protected. Set {} to unlock.",
                        WALLET_PASSWORD_ENV
                    );
                    Err(anyhow::anyhow!("Wallet is password protected"))
                }
            }
        } else {
            SecureKeychain::retrieve_mnemonic_for(&identity_name)
        };

        match mnemonic_result {
            Ok(mnemonic_str) => {
                match Mnemonic::parse(&mnemonic_str) {
                    Ok(mnemonic) => {
                        match UnifiedIdentity::from_mnemonic(mnemonic) {
                            Ok(id) => Some(Arc::new(id)),
                            Err(e) => {
                                tracing::error!("Failed to create identity: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse mnemonic: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to retrieve mnemonic: {}", e);
                None
            }
        }
    } else {
        tracing::warn!(
            "No wallet found for identity '{}' - please run 'openagents wallet init' first",
            identity_name
        );
        None
    };

    // Start tokio runtime + actix server in background thread
    let identity_clone = identity.clone();
    let (port_tx, port_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let port = start_server(identity_clone)
                .await
                .expect("start server");
            port_tx.send(port).expect("send port");

            // Keep runtime alive
            tokio::signal::ctrl_c().await.ok();
        });
    });

    // Wait for server to start
    let port = port_rx
        .recv()
        .expect("receive port from server thread");
    tracing::info!("Server started on port {}", port);

    // Create event loop and window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("OpenAgents Wallet")
        .with_inner_size(tao::dpi::LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    // Create webview
    let url = format!("http://127.0.0.1:{}", port);
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)?;

    // Run event loop
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
