//! Pylon Desktop - FM Bridge Visualization
//!
//! Desktop GUI for Apple Foundation Models inference with viz primitives.

mod app;
mod bridge_manager;
mod fm_runtime;
mod state;
mod ui;

use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = app::PylonApp::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
