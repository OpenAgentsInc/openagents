//! GUI application entry point

use anyhow::Result;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use super::server::start_server;

/// Run the unified OpenAgents GUI
pub fn run() -> Result<()> {
    tracing::info!("Starting OpenAgents Desktop...");

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

            // Keep runtime alive
            tokio::signal::ctrl_c().await.ok();
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

    // Run event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
