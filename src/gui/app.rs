//! GUI application entry point

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use gtk::glib;
use gtk::prelude::*;
use wry::WebViewBuilderExtUnix;

use super::server::start_server;

/// Run the unified OpenAgents GUI
pub fn run() -> Result<()> {
    tracing::info!("Starting OpenAgents Desktop...");

    // Workaround for webkit2gtk Wayland DMABUF issue
    // The DMABUF renderer causes "Error 71 (Protocol error) dispatching to Wayland display"
    // Disabling it falls back to WPE renderer which still has GPU acceleration
    // See: https://bugs.webkit.org/show_bug.cgi?id=262607
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // SAFETY: This is set early in startup before any other threads are spawned,
        // and these vars are only read by webkit2gtk during init.
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    // Initialize GTK (required for Wayland support)
    gtk::init().map_err(|e| anyhow::anyhow!("Failed to initialize GTK: {}", e))?;

    // Suppress Gdk clipboard "Broken pipe" warnings on Wayland
    // These occur when using navigator.clipboard.writeText() but the copy still succeeds.
    // The warning happens when the clipboard manager closes its data connection.
    // See: https://gitlab.gnome.org/GNOME/gtk/-/issues/5933
    glib::log_set_handler(
        Some("Gdk"),
        glib::LogLevels::LEVEL_WARNING,
        false,
        false,
        |_domain, level, message| {
            // Only suppress the specific clipboard broken pipe warning
            if level == glib::LogLevel::Warning
                && message.contains("Error writing selection data")
                && message.contains("Broken pipe")
            {
                // Silently ignore - the copy operation still succeeds
                return;
            }
            // Let other Gdk warnings through to the default handler
            glib::log_default_handler(_domain, level, Some(message));
        },
    );

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

    // Create GTK window (works on both X11 and Wayland)
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("OpenAgents");
    window.set_default_size(1200, 800);

    // Use a Box container for the webview (handles sizing better on Wayland)
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.set_hexpand(true);
    vbox.set_vexpand(true);
    window.add(&vbox);

    // Show the window first so GTK can realize it
    window.show_all();

    // Create webview using the Box container (Wayland-compatible)
    let url = format!("http://127.0.0.1:{}", port);
    let _webview = wry::WebViewBuilder::new()
        .with_url(&url)
        .build_gtk(&vbox)
        .map_err(|e| anyhow::anyhow!("Failed to create webview: {}", e))?;

    // Handle window close
    let shutdown_for_close = shutdown.clone();
    window.connect_delete_event(move |_, _| {
        shutdown_for_close.store(true, Ordering::SeqCst);
        gtk::main_quit();
        glib::Propagation::Stop
    });

    // Run GTK main loop
    gtk::main();

    Ok(())
}
