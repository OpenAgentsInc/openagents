//! GUI application entry point

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use super::server::start_server;

/// Run the unified OpenAgents GUI
pub fn run() -> Result<()> {
    tracing::info!("Starting OpenAgents Desktop...");

    // Shared shutdown signal
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    // Start tokio runtime + actix server in background thread
    let (port_tx, port_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let port = start_server().await.expect("start server");
            port_tx.send(port).expect("send port");

            // Wait for shutdown signal or Ctrl+C
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Received Ctrl+C, shutting down...");
                        shutdown_clone.store(true, Ordering::SeqCst);
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        if shutdown_clone.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });
    });

    // Wait for server to start
    let port = port_rx.recv().expect("receive port from server thread");
    println!("OPENAGENTS_PORT={}", port);
    tracing::info!("Server running on http://127.0.0.1:{}", port);

    // Create event loop and window (must be on main thread for macOS)
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("OpenAgents")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 800.0))
        .build(&event_loop)?;

    // Create webview
    let url = format!("http://127.0.0.1:{}", port);
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)?;

    // Run event loop with shutdown check
    event_loop.run(move |event, _, control_flow| {
        // Check if shutdown was requested
        if shutdown.load(Ordering::SeqCst) {
            *control_flow = ControlFlow::Exit;
            return;
        }

        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } = event {
            shutdown.store(true, Ordering::SeqCst);
            *control_flow = ControlFlow::Exit;
        }
    });
}
