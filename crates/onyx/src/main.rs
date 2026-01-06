//! Onyx - Markdown Editor
//!
//! A local-first markdown note editor with live inline formatting.

mod app;
mod config;
mod vault;

use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = app::OnyxApp::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
