//! Autopilot GUI entry point

use autopilot_gui::{Server, Window};
use std::thread;
use tracing::info;

const PORT: u16 = 3847;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("Starting Autopilot GUI");

    // Start web server in background thread
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let server = Server::new(PORT);
        if let Err(e) = rt.block_on(server.start()) {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    thread::sleep(std::time::Duration::from_millis(500));

    // Launch window (blocks until window closes)
    let window = Window::new(PORT);
    window.launch()?;

    info!("Autopilot GUI shutdown complete");
    Ok(())
}
