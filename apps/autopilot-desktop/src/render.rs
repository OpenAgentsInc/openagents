use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpui::components::hud::{Command, CommandPalette, DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, Easing, Hsla, PaintContext, Quad, Scene, Size, TextSystem, theme};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app_state::{PaneKind, RenderState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use crate::codex_lane::{CodexLaneConfig, CodexLaneSnapshot, CodexLaneWorker};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::pane_registry::{pane_specs, startup_pane_kinds};
use crate::pane_renderer::PaneRenderer;
use crate::pane_system::{PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PaneController};
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, SaLaneSnapshot, SaLaneWorker,
    SaLifecycleCommand, SklDiscoveryTrustCommand, SklLaneSnapshot, SklLaneWorker,
};
use crate::spark_wallet::SparkWalletCommand;

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
        let settings = crate::app_state::SettingsState::load_from_disk();
        let settings_inputs = crate::app_state::SettingsPaneInputs::from_state(&settings);
        let codex_lane_worker = CodexLaneWorker::spawn(CodexLaneConfig::default());
        let sa_lane_worker = SaLaneWorker::spawn();
        let skl_lane_worker = SklLaneWorker::spawn();
        let ac_lane_worker = AcLaneWorker::spawn();
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
            network_requests_inputs: crate::app_state::NetworkRequestsPaneInputs::default(),
            settings_inputs,
            job_history_inputs: crate::app_state::JobHistoryPaneInputs::default(),
            chat_inputs: crate::app_state::ChatPaneInputs::default(),
            autopilot_chat: crate::app_state::AutopilotChatState::default(),
            codex_lane: CodexLaneSnapshot::default(),
            codex_lane_worker,
            codex_command_responses: Vec::new(),
            codex_notifications: Vec::new(),
            next_codex_command_seq: 1,
            sa_lane: SaLaneSnapshot::default(),
            skl_lane: SklLaneSnapshot::default(),
            ac_lane: AcLaneSnapshot::default(),
            sa_lane_worker,
            skl_lane_worker,
            ac_lane_worker,
            runtime_command_responses: Vec::new(),
            next_runtime_command_seq: 1,
            provider_runtime: crate::app_state::ProviderRuntimeState::default(),
            earnings_scoreboard: crate::app_state::EarningsScoreboardState::default(),
            relay_connections: crate::app_state::RelayConnectionsState::default(),
            sync_health: crate::app_state::SyncHealthState::default(),
            network_requests: crate::app_state::NetworkRequestsState::default(),
            starter_jobs: crate::app_state::StarterJobsState::default(),
            activity_feed: crate::app_state::ActivityFeedState::default(),
            alerts_recovery: crate::app_state::AlertsRecoveryState::default(),
            settings,
            job_inbox: crate::app_state::JobInboxState::default(),
            active_job: crate::app_state::ActiveJobState::default(),
            job_history: crate::app_state::JobHistoryState::default(),
            agent_profile_state: crate::app_state::AgentProfileStatePaneState::default(),
            agent_schedule_tick: crate::app_state::AgentScheduleTickPaneState::default(),
            trajectory_audit: crate::app_state::TrajectoryAuditPaneState::default(),
            skill_registry: crate::app_state::SkillRegistryPaneState::default(),
            skill_trust_revocation: crate::app_state::SkillTrustRevocationPaneState::default(),
            credit_desk: crate::app_state::CreditDeskPaneState::default(),
            credit_settlement_ledger: crate::app_state::CreditSettlementLedgerPaneState::default(),
            agent_network_simulation: crate::app_state::AgentNetworkSimulationPaneState::default(),
            treasury_exchange_simulation:
                crate::app_state::TreasuryExchangeSimulationPaneState::default(),
            relay_security_simulation: crate::app_state::RelaySecuritySimulationPaneState::default(
            ),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
            command_palette,
            command_palette_actions,
        };
        bootstrap_runtime_lanes(&mut state);
        open_startup_panes(&mut state);
        Ok(state)
    })
}

fn bootstrap_runtime_lanes(state: &mut RenderState) {
    let _ = state.queue_sa_command(SaLifecycleCommand::PublishAgentProfile {
        display_name: "Autopilot".to_string(),
        about: "Desktop sovereign agent runtime".to_string(),
        version: "mvp".to_string(),
    });
    let _ = state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
        encrypted_state_ref: "nip44:ciphertext:bootstrap".to_string(),
    });
    let _ = state.queue_sa_command(SaLifecycleCommand::ConfigureAgentSchedule {
        heartbeat_seconds: 30,
    });

    let _ = state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillManifest {
        skill_slug: "summarize-text".to_string(),
        version: "0.1.0".to_string(),
    });
    let _ = state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillVersionLog {
        skill_slug: "summarize-text".to_string(),
        version: "0.1.0".to_string(),
        summary: "bootstrap manifest".to_string(),
    });

    let _ = state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
        scope: "bootstrap:credit".to_string(),
        request_type: "bootstrap.credit".to_string(),
        payload: "{\"bootstrap\":true}".to_string(),
        skill_scope_id: Some("33400:npub1agent:summarize-text:0.1.0".to_string()),
        credit_envelope_ref: None,
        requested_sats: 1500,
        timeout_seconds: 60,
    });
}

fn open_startup_panes(state: &mut RenderState) {
    for pane_kind in startup_pane_kinds() {
        match pane_kind {
            PaneKind::AutopilotChat | PaneKind::GoOnline => {
                let _ = PaneController::create_for_kind(state, pane_kind);
            }
            PaneKind::SparkWallet => {
                let _ = PaneController::create_for_kind(state, pane_kind);
                if let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh) {
                    state.spark_wallet.last_error = Some(error);
                }
            }
            _ => {}
        }
    }
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
            &state.sa_lane,
            &state.skl_lane,
            &state.ac_lane,
            &state.provider_runtime,
            provider_blockers.as_slice(),
            &state.earnings_scoreboard,
            &state.relay_connections,
            &state.sync_health,
            &state.network_requests,
            &state.starter_jobs,
            &state.activity_feed,
            &state.alerts_recovery,
            &state.settings,
            &state.job_inbox,
            &state.active_job,
            &state.job_history,
            &state.agent_profile_state,
            &state.agent_schedule_tick,
            &state.trajectory_audit,
            &state.skill_registry,
            &state.skill_trust_revocation,
            &state.credit_desk,
            &state.credit_settlement_ledger,
            &state.agent_network_simulation,
            &state.treasury_exchange_simulation,
            &state.relay_security_simulation,
            &state.spark_wallet,
            &mut state.spark_inputs,
            &mut state.pay_invoice_inputs,
            &mut state.create_invoice_inputs,
            &mut state.relay_connections_inputs,
            &mut state.network_requests_inputs,
            &mut state.settings_inputs,
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
    pane_specs()
        .iter()
        .filter_map(|spec| {
            let command = spec.command?;
            let mut entry = Command::new(command.id, command.label)
                .description(command.description)
                .category("Panes");
            if let Some(keybinding) = command.keybinding {
                entry = entry.keybinding(keybinding);
            }
            Some(entry)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::command_registry;
    use crate::app_state::PaneKind;
    use crate::pane_registry::{pane_spec_by_command_id, pane_specs, startup_pane_kinds};
    use std::collections::BTreeSet;

    #[test]
    fn command_registry_matches_pane_specs() {
        let commands = command_registry();
        let command_ids: BTreeSet<&str> =
            commands.iter().map(|command| command.id.as_str()).collect();

        let expected_ids: BTreeSet<&str> = pane_specs()
            .iter()
            .filter_map(|spec| spec.command.map(|command| command.id))
            .collect();
        assert_eq!(command_ids, expected_ids);

        for command in &commands {
            let spec = pane_spec_by_command_id(&command.id)
                .expect("command id from registry should resolve to pane spec");
            let pane_command = spec.command.expect("resolved pane must define a command");
            assert_eq!(command.label, pane_command.label);
        }
    }

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
        assert!(commands.iter().any(|command| {
            command.id == "pane.network_requests" && command.label == "Network Requests"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.starter_jobs" && command.label == "Starter Jobs"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.activity_feed" && command.label == "Activity Feed"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.alerts_recovery" && command.label == "Alerts and Recovery"
        }));
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.settings" && command.label == "Settings" })
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
        assert!(commands.iter().any(|command| {
            command.id == "pane.agent_profile_state" && command.label == "Agent Profile and State"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.agent_schedule_tick" && command.label == "Agent Schedule and Tick"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.trajectory_audit" && command.label == "Trajectory Audit"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.skill_registry" && command.label == "Agent Skill Registry"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.skill_trust_revocation"
                && command.label == "Skill Trust and Revocation"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.credit_desk" && command.label == "Credit Desk"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.credit_settlement_ledger"
                && command.label == "Credit Settlement Ledger"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.agent_network_simulation"
                && command.label == "Sovereign Agent Simulation"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.treasury_exchange_simulation"
                && command.label == "Treasury Exchange Simulation"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.relay_security_simulation"
                && command.label == "Relay Security Simulation"
        }));
    }

    #[test]
    fn startup_pane_set_matches_mvp_core_surfaces() {
        let startup = startup_pane_kinds();
        assert!(startup.contains(&PaneKind::AutopilotChat));
        assert!(startup.contains(&PaneKind::GoOnline));
        assert!(startup.contains(&PaneKind::SparkWallet));
        assert!(!startup.contains(&PaneKind::Empty));
    }
}
