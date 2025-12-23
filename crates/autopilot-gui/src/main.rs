//! Autopilot GUI entry point
//!
//! Configure port with AUTOPILOT_GUI_PORT environment variable:
//! ```bash
//! AUTOPILOT_GUI_PORT=8080 cargo run -p autopilot-gui
//! ```

use autopilot_gui::{Server, Window};
use std::thread;
use tracing::info;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    // Read port from environment variable or use default
    let port: u16 = std::env::var("AUTOPILOT_GUI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3847);

    info!("Starting Autopilot GUI on port {}", port);

    // Start web server in background thread
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let server = Server::new(port);
        if let Err(e) = rt.block_on(server.start()) {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    thread::sleep(std::time::Duration::from_millis(500));

    // Launch window (blocks until window closes)
    let window = Window::new(port);
    window.launch()?;

    info!("Autopilot GUI shutdown complete");
    Ok(())
}
