//! Main entry point for the Coder application.
//!
//! This file provides the native (desktop) entry point.
//! For WASM, see the `start` function in lib.rs.

use coder_app::App;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("Starting Coder application (native)");

    // Create app
    let mut app = App::new();
    app.init();

    // For now, just verify the app starts correctly
    // Full event loop integration with winit comes later
    log::info!("Application initialized successfully");
    log::info!("Current route: {:?}", app.current_route());
    log::info!("Window size: {:?}", app.size());

    // TODO: Integration with wgpui platform
    // let platform = wgpui::platform::desktop::DesktopPlatform::init().await;
    // platform.run(move |event| {
    //     app.handle_event(&event);
    //     app.update();
    //     let mut scene = Scene::new();
    //     app.paint(&mut scene, platform.text_system());
    //     platform.render(&scene);
    // });

    log::info!("Coder application ready");
}
