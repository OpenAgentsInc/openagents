//! Native window using wry/tao

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;
use tracing::info;

/// Native window for autopilot GUI
pub struct Window {
    url: String,
}

impl Window {
    /// Create a new window
    pub fn new(port: u16) -> Self {
        Self {
            url: format!("http://localhost:{}", port),
        }
    }

    /// Launch the window and event loop
    pub fn launch(self) -> anyhow::Result<()> {
        info!("Launching autopilot GUI window at {}", self.url);

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Autopilot GUI")
            .with_inner_size(tao::dpi::LogicalSize::new(1200, 800))
            .build(&event_loop)?;

        let _webview = WebViewBuilder::new()
            .with_url(&self.url)
            .build(&window)?;

        info!("Window created, starting event loop");

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    info!("Window close requested, shutting down");
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    info!("Window destroyed");
                }
                _ => {}
            }
        });
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new(3847)
    }
}
