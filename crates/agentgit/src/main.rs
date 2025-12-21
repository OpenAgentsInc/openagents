//! AgentGit Desktop - Nostr-native GitHub Alternative
//!
//! Decentralized git collaboration powered by NIP-34 (Git Stuff) and NIP-SA (Sovereign Agents).
//! Enables agents as first-class contributors with trajectory proof and bounty payments.

mod server;
mod views;
mod ws;

use anyhow::Result;
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use server::start_server;
use ws::WsBroadcaster;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting AgentGit...");

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
            let port = start_server(broadcaster_clone).await.expect("start server");
            port_tx.send(port).expect("send port");
            // Keep runtime alive
            std::future::pending::<()>().await;
        });
    });

    let port = port_rx.recv().expect("receive port");
    println!("AGENTGIT_PORT={}", port);
    tracing::info!("AgentGit server running on http://127.0.0.1:{}", port);

    // tao event loop (must be on main thread for macOS)
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("AgentGit - Nostr GitHub Alternative")
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
