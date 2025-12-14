//! # coder_app - Main Application Entry Point
//!
//! This crate provides the main entry point for Coder on both web and desktop.
//! It initializes the platform, sets up the application state, and runs the
//! main event loop.
//!
//! ## Architecture
//!
//! The app owns all six layers of the UI stack:
//! 1. Domain Model (coder_domain)
//! 2. UI Runtime (coder_ui_runtime)
//! 3. Layout Engine (wgpui/Taffy)
//! 4. Widgets (coder_widgets, coder_surfaces_*)
//! 5. Renderer (wgpui)
//! 6. Platform Glue (wgpui/platform)
//!
//! This architecture replaces Dioxus entirely with our own Rust-native stack.

pub mod app;
pub mod state;

#[cfg(not(target_arch = "wasm32"))]
pub mod chat_handler;

// Re-exports
pub use app::App;
pub use state::AppState;

#[cfg(not(target_arch = "wasm32"))]
pub use chat_handler::spawn_chat_handler;

// WASM entry point for web demo
#[cfg(all(feature = "web", target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Initialize logging for web
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    log::info!("Starting Coder web application...");

    // Initialize platform
    let platform = wgpui::platform::web::WebPlatform::init("coder-canvas")
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    // Hide loading indicator
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(loading) = document.get_element_by_id("loading") {
                loading
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|el| el.style().set_property("display", "none"));
            }
        }
    }

    // Set up state
    use std::cell::RefCell;
    use std::rc::Rc;
    use wgpui::platform::Platform;

    let platform = Rc::new(RefCell::new(platform));

    // Create channels (for web, we use dummy channels for now)
    let (client_tx, _client_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_server_tx, server_rx) = tokio::sync::mpsc::unbounded_channel();

    // Create app
    let mut app = App::new(client_tx, server_rx);

    // Set initial size
    {
        let p = platform.borrow();
        let size = p.logical_size();
        app.set_size(size.width, size.height);
    }

    // Initialize app
    app.init();

    let app = Rc::new(RefCell::new(app));

    log::info!("Coder web application initialized");

    // Set up resize handler
    {
        let platform_clone = platform.clone();
        let app_clone = app.clone();
        let canvas = platform.borrow().canvas().clone();
        wgpui::platform::web::setup_resize_observer(&canvas, move || {
            if let Ok(mut p) = platform_clone.try_borrow_mut() {
                p.handle_resize();
                let size = p.logical_size();
                if let Ok(mut a) = app_clone.try_borrow_mut() {
                    a.set_size(size.width, size.height);
                }
            }
        });
    }

    // Animation loop
    let platform_clone = platform.clone();
    let app_clone = app.clone();

    wgpui::platform::web::run_animation_loop(move || {
        let mut platform = platform_clone.borrow_mut();
        let mut app = app_clone.borrow_mut();

        // Update app state
        app.update();

        // Paint to scene
        let mut scene = wgpui::Scene::new();
        app.paint(&mut scene, platform.text_system());

        // Render
        if let Err(e) = platform.render(&scene) {
            log::error!("Render error: {}", e);
        }
    });

    Ok(())
}
