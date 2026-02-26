use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpui::components::hud::{Command, CommandPalette, DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, Easing, Hsla, PaintContext, Quad, Scene, Size, TextSystem, theme};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app_state::{RenderState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::pane_renderer::PaneRenderer;
use crate::pane_system::{PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PaneController};

const GRID_DOT_DISTANCE: f32 = 32.0;

pub fn init_state(event_loop: &ActiveEventLoop) -> Result<RenderState> {
    let window_attrs = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );

    pollster::block_on(async move {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .context("failed to create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find compatible adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .context("failed to create device")?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .or_else(|| surface_caps.formats.first().copied())
            .context("surface formats empty")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, surface_format);
        let scale_factor = window.scale_factor() as f32;
        let text_system = TextSystem::new(scale_factor);

        let hotbar = new_hotbar();
        let initial_hotbar_bounds = hotbar_bounds(logical_size(&config, scale_factor));

        let (nostr_identity, nostr_identity_error) = match load_or_create_identity() {
            Ok(identity) => (Some(identity), None),
            Err(err) => (None, Some(err.to_string())),
        };

        let spark_wallet = crate::spark_wallet::SparkPaneState::default();
        let spark_worker = crate::spark_wallet::SparkWalletWorker::spawn(spark_wallet.network);
        let command_palette_actions = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut command_palette = CommandPalette::new()
            .mono(true)
            .commands(command_registry());
        {
            let action_queue = Rc::clone(&command_palette_actions);
            command_palette = command_palette.on_select(move |command| {
                action_queue.borrow_mut().push(command.id.clone());
            });
        }

        let mut state = RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            hotbar,
            hotbar_bounds: initial_hotbar_bounds,
            event_context: wgpui::EventContext::new(),
            input_modifiers: wgpui::Modifiers::default(),
            panes: Vec::new(),
            nostr_identity,
            nostr_identity_error,
            nostr_secret_state: crate::app_state::NostrSecretState::default(),
            spark_wallet,
            spark_worker,
            spark_inputs: crate::app_state::SparkPaneInputs::default(),
            pay_invoice_inputs: crate::app_state::PayInvoicePaneInputs::default(),
            create_invoice_inputs: crate::app_state::CreateInvoicePaneInputs::default(),
            relay_connections_inputs: crate::app_state::RelayConnectionsPaneInputs::default(),
            job_history_inputs: crate::app_state::JobHistoryPaneInputs::default(),
            chat_inputs: crate::app_state::ChatPaneInputs::default(),
            autopilot_chat: crate::app_state::AutopilotChatState::default(),
            provider_runtime: crate::app_state::ProviderRuntimeState::default(),
            earnings_scoreboard: crate::app_state::EarningsScoreboardState::default(),
            relay_connections: crate::app_state::RelayConnectionsState::default(),
            sync_health: crate::app_state::SyncHealthState::default(),
            job_inbox: crate::app_state::JobInboxState::default(),
            active_job: crate::app_state::ActiveJobState::default(),
            job_history: crate::app_state::JobHistoryState::default(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
            command_palette,
            command_palette_actions,
        };
        PaneController::create_empty(&mut state);
        Ok(state)
    })
}

pub fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let active_pane = PaneController::active(state);

    let mut scene = Scene::new();
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let provider_blockers = state.provider_blockers();
    {
        let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);

        let mut dots_grid = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.30, 0.26))
            .shape(DotShape::Cross)
            .distance(GRID_DOT_DISTANCE)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut);
        dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut paint);
        let hotbar_layer = PaneRenderer::paint(
            &mut state.panes,
            active_pane,
            state.nostr_identity.as_ref(),
            state.nostr_identity_error.as_deref(),
            &state.nostr_secret_state,
            &state.autopilot_chat,
            &state.provider_runtime,
            provider_blockers.as_slice(),
            &state.earnings_scoreboard,
            &state.relay_connections,
            &state.sync_health,
            &state.job_inbox,
            &state.active_job,
            &state.job_history,
            &state.spark_wallet,
            &mut state.spark_inputs,
            &mut state.pay_invoice_inputs,
            &mut state.create_invoice_inputs,
            &mut state.relay_connections_inputs,
            &mut state.job_history_inputs,
            &mut state.chat_inputs,
            &mut paint,
        );
        paint.scene.set_layer(hotbar_layer);

        let bar_bounds = hotbar_bounds(logical);
        state.hotbar_bounds = bar_bounds;
        configure_hotbar(&mut state.hotbar);
        state.hotbar.paint(bar_bounds, &mut paint);

        state
            .command_palette
            .paint(Bounds::new(0.0, 0.0, width, height), &mut paint);
    }

    state
        .renderer
        .resize(&state.queue, logical, state.scale_factor.max(0.1));

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    let output = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return Ok(());
        }
        Err(err) => return Err(anyhow::anyhow!("surface error: {err:?}")),
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Autopilot Render Encoder"),
        });

    state.renderer.prepare(
        &state.device,
        &state.queue,
        &scene,
        state.scale_factor.max(0.1),
    );
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

pub fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn command_registry() -> Vec<Command> {
    vec![
        Command::new("pane.autopilot_chat", "Autopilot Chat")
            .description("Open chat thread and composer for Autopilot")
            .category("Panes"),
        Command::new("pane.go_online", "Go Online")
            .description("Open provider mode toggle and lifecycle controls")
            .category("Panes"),
        Command::new("pane.provider_status", "Provider Status")
            .description("Open runtime health and heartbeat visibility pane")
            .category("Panes"),
        Command::new("pane.earnings_scoreboard", "Earnings Scoreboard")
            .description("Open sats/day, lifetime, jobs/day and last-result metrics pane")
            .category("Panes"),
        Command::new("pane.relay_connections", "Relay Connections")
            .description("Open relay connectivity and retry controls")
            .category("Panes"),
        Command::new("pane.sync_health", "Sync Health")
            .description("Open spacetime subscription and stale-cursor diagnostics pane")
            .category("Panes"),
        Command::new("pane.job_inbox", "Job Inbox")
            .description("Open incoming NIP-90 request intake pane")
            .category("Panes"),
        Command::new("pane.active_job", "Active Job")
            .description("Open in-flight job lifecycle timeline pane")
            .category("Panes"),
        Command::new("pane.job_history", "Job History")
            .description("Open deterministic completed/failed job receipts pane")
            .category("Panes"),
        Command::new("pane.identity_keys", "Identity Keys")
            .description("Open Nostr keys (NIP-06) pane")
            .category("Panes")
            .keybinding("2"),
        Command::new("pane.wallet", "Spark Wallet")
            .description("Show Spark wallet controls")
            .category("Panes")
            .keybinding("3"),
        Command::new("pane.pay_invoice", "Pay Lightning Invoice")
            .description("Open dedicated pane for paying Lightning invoices")
            .category("Panes"),
        Command::new("pane.create_invoice", "Create Lightning Invoice")
            .description("Open dedicated pane for creating Lightning invoices")
            .category("Panes"),
    ]
}

#[cfg(test)]
mod tests {
    use super::command_registry;

    #[test]
    fn command_registry_includes_job_inbox_command() {
        let commands = command_registry();
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.job_inbox" && command.label == "Job Inbox" })
        );
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.active_job" && command.label == "Active Job"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.earnings_scoreboard" && command.label == "Earnings Scoreboard"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.relay_connections" && command.label == "Relay Connections"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.sync_health" && command.label == "Sync Health"
            })
        );
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.wallet" && command.label == "Spark Wallet" })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.pay_invoice" && command.label == "Pay Lightning Invoice"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.create_invoice" && command.label == "Create Lightning Invoice"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.job_history" && command.label == "Job History"
            })
        );
    }
}
