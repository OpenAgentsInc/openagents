#![cfg_attr(test, allow(clippy::expect_used))]

mod app_state;
mod hotbar;
mod input;
mod pane_renderer;
mod pane_system;
mod render;
mod spark_pane;
mod spark_wallet;

use anyhow::{Context, Result};
use app_state::App;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create event loop")?;
    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .context("event loop terminated with error")?;
    Ok(())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        match render::init_state(event_loop) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(_err) => {
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        input::handle_window_event(self, event_loop, event);
    }
}
