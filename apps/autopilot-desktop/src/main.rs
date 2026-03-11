#![allow(dead_code)]
#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

mod app_state;
mod apple_fm_bridge;
mod bitcoin_display;
mod cad_rebuild_worker;
mod chat_spacetime;
mod chat_terminal;
mod codex_lane;
mod codex_remote;
mod credentials;
mod desktop_shell;
mod economy_kernel_receipts;
mod hotbar;
mod input;
mod kernel_control;
mod labor_orchestrator;
mod local_inference_runtime;
mod logging;
mod nip_sa_wallet_bridge;
#[cfg(test)]
mod ollama_execution;
mod openagents_dynamic_tools;
mod pane_registry;
mod pane_renderer;
mod pane_system;
mod panes;
mod project_ops;
mod provider_admin;
mod provider_nip90_lane;
mod render;
mod runtime_lanes;
mod skill_autoload;
mod skills_registry;
mod spacetime_presence;
mod spark_pane;
mod spark_wallet;
mod stablesats_blink_worker;
mod starter_demand_client;
mod state;
mod sync_apply;
mod sync_bootstrap;
mod sync_lifecycle;

#[cfg(test)]
mod chat_regression_tests;

use anyhow::{Context, Result};
use app_state::App;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

fn main() -> Result<()> {
    logging::init();
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

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        input::handle_device_event(self, event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        input::handle_about_to_wait(self, event_loop);
    }
}
