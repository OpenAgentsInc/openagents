#![allow(dead_code)]
#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use anyhow::{Context, Result};
use app_state::App;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

mod app_state;
mod apple_adapter_eval_contract;
mod apple_adapter_training_control;
pub mod apple_architecture_explainer_reference_run;
mod apple_fm_bridge;
mod apple_repo_lookup_tools;
mod attnres_lab_control;
mod autopilot_compute_presence;
mod autopilot_peer_roster;
mod bitcoin_display;
mod cad_rebuild_worker;
pub(crate) mod chat_message_classifier;
mod chat_spacetime;
mod chat_terminal;
mod codex_lane;
mod codex_remote;
pub mod compute_mcp;
mod credentials;
mod data_buyer_control;
mod data_market_control;
mod data_seller_control;
pub mod desktop_control;
mod desktop_shell;
mod economy_kernel_receipts;
pub mod headless_compute;
mod hotbar;
mod input;
mod kernel_control;
mod labor_orchestrator;
mod local_inference_runtime;
mod local_runtime_capabilities;
pub mod logging;
mod nip28_chat_lane;
mod nip90_compute_domain_events;
mod nip90_compute_flow;
mod nip90_compute_semantics;
mod nip_sa_wallet_bridge;
#[cfg(test)]
mod ollama_execution;
mod onboarding;
mod openagents_dynamic_tools;
mod pane_registry;
mod pane_renderer;
mod pane_system;
mod panes;
mod project_ops;
mod provider_admin;
mod provider_inventory;
mod provider_nip90_lane;
mod render;
mod research_control;
pub mod rive_assets;
mod runtime_lanes;
mod runtime_log;
mod skill_autoload;
mod skills_registry;
mod snapshot_domains;
mod spacetime_presence;
mod spark_pane;
mod spark_wallet;
mod stablesats_blink_worker;
mod starter_demand_client;
mod state;
mod sync_apply;
mod sync_bootstrap;
mod sync_lifecycle;
mod tassadar_lab_control;
pub mod throughput_bench;
mod voice_playground;

#[cfg(test)]
mod chat_regression_tests;

pub use local_inference_runtime::{
    LocalRuntimeCacheInvalidation, LocalRuntimeCacheInvalidationReason, LocalRuntimeCompileFailure,
    LocalRuntimeDiagnostics, LocalRuntimeExecutionPosture, compile_path_temperature_label,
    local_runtime_cache_invalidation_reason_label, local_runtime_device_inventory_label,
    local_runtime_execution_posture_label, local_runtime_scheduler_posture_label,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopAppOptions {
    pub window_visible: bool,
    pub disable_codex: bool,
}

impl Default for DesktopAppOptions {
    fn default() -> Self {
        Self {
            window_visible: true,
            disable_codex: false,
        }
    }
}

pub fn run_desktop_app() -> Result<()> {
    run_desktop_app_with_options(DesktopAppOptions::default())
}

pub fn run_desktop_app_with_options(options: DesktopAppOptions) -> Result<()> {
    logging::init();
    let event_loop = EventLoop::new().context("failed to create event loop")?;
    let mut app = AppShell {
        inner: App::default(),
        options,
    };
    event_loop
        .run_app(&mut app)
        .context("event loop terminated with error")?;
    Ok(())
}

struct AppShell {
    inner: App,
    options: DesktopAppOptions,
}

impl ApplicationHandler for AppShell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.inner.state.is_some() {
            return;
        }

        match render::init_state(
            event_loop,
            self.options.window_visible,
            self.options.disable_codex,
        ) {
            Ok(state) => {
                state.window.request_redraw();
                self.inner.state = Some(state);
            }
            Err(_err) => {
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        input::handle_window_event(&mut self.inner, event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        input::handle_device_event(&mut self.inner, event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        input::handle_about_to_wait(&mut self.inner, event_loop);
    }
}
