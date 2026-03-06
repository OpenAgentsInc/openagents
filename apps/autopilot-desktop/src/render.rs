use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpui::components::Text;
use wgpui::components::hud::{Command, CommandPalette, DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, SvgQuad, TextSystem,
    theme,
};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app_state::{
    PaneKind, ProviderMode, RenderState, SidebarState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH,
};
use crate::codex_lane::{CodexLaneConfig, CodexLaneSnapshot, CodexLaneWorker};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::input::bootstrap_startup_cad_mesh;
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::pane_registry::{
    pane_enabled_in_runtime, pane_specs, simulation_panes_enabled_from_env, startup_pane_kinds,
};
use crate::pane_renderer::PaneRenderer;
use crate::pane_system::{
    PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PaneController, cad_palette_command_specs,
};
use crate::provider_nip90_lane::{ProviderNip90LaneSnapshot, ProviderNip90LaneWorker};
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, SaLaneSnapshot, SaLaneWorker,
    SaLifecycleCommand, SklLaneSnapshot, SklLaneWorker,
};
use crate::spark_wallet::SparkWalletCommand;

const GRID_DOT_DISTANCE: f32 = 32.0;
const WALLET_BALANCE_CHIP_MARGIN: f32 = 12.0;
const WALLET_BALANCE_CHIP_HEIGHT: f32 = 28.0;
const WALLET_BALANCE_CHIP_MIN_WIDTH: f32 = 140.0;
const WALLET_BALANCE_CHIP_MAX_WIDTH: f32 = 220.0;

const SETTINGS_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M249.9 176.3C243.4 179.5 237.1 183.1 231.1 187.2C222.9 192.7 212.6 194.1 203.1 191L131.4 167.2L93.5 232.8L150 283.1C157.4 289.7 161.3 299.3 160.7 309.2C160.2 316.4 160.2 323.8 160.7 331C161.4 340.9 157.4 350.5 150 357.1L93.5 407.3L131.4 473L203.1 449.2C212.5 446.1 222.8 447.5 231.1 453C237.1 457 243.4 460.7 249.9 463.9C258.8 468.3 265.1 476.5 267.1 486.2L282.3 560.2L358.1 560.2L373.3 486.2C375.3 476.5 381.7 468.3 390.5 463.9C397 460.7 403.3 457.1 409.3 453C417.5 447.5 427.8 446.1 437.3 449.2L509 473L546.9 407.3L490.4 357.1C483 350.5 479.1 340.9 479.7 331C479.9 327.4 480.1 323.8 480.1 320.1C480.1 316.4 480 312.8 479.7 309.2C479 299.3 483 289.7 490.4 283.1L546.9 232.9L509 167.2L437.3 191C427.9 194.1 417.6 192.7 409.3 187.2C403.3 183.2 397 179.5 390.5 176.3C381.6 171.9 375.3 163.7 373.3 154L358.1 80L282.3 80L267.1 154C265.1 163.7 258.7 171.9 249.9 176.3zM358.2 48C373.4 48 386.5 58.7 389.5 73.5L404.7 147.5C412.5 151.3 420.1 155.7 427.3 160.6L499 136.8C513.4 132 529.2 138 536.8 151.2L574.7 216.9C582.3 230.1 579.6 246.7 568.2 256.8L511.9 307C512.5 315.6 512.5 324.5 511.9 333L568.4 383.2C579.8 393.3 582.4 410 574.9 423.1L537 488.8C529.4 502 513.6 508 499.2 503.2L427.5 479.4C420.3 484.2 412.8 488.6 404.9 492.5L389.7 566.5C386.6 581.4 373.5 592 358.4 592L282.6 592C267.4 592 254.3 581.3 251.3 566.5L236.1 492.5C228.3 488.7 220.7 484.3 213.5 479.4L141.5 503.2C127.1 508 111.3 502 103.7 488.8L65.8 423.2C58.2 410.1 60.9 393.4 72.3 383.3L128.7 333C128.1 324.4 128.1 315.5 128.7 307L72.2 256.8C60.8 246.7 58.2 230 65.7 216.9L103.7 151.2C111.3 138 127.1 132 141.5 136.8L213.2 160.6C220.4 155.8 227.9 151.4 235.8 147.5L251 73.5C254.1 58.7 267.2 48 282.4 48L358.2 48zM264.3 320C264.3 350.8 289.2 375.7 320 375.7C350.8 375.7 375.7 350.8 375.7 320C375.7 289.2 350.8 264.3 320 264.3C289.2 264.3 264.3 289.2 264.3 320zM319.7 408C271.1 407.8 231.8 368.3 232 319.7C232.2 271.1 271.7 231.8 320.3 232C368.9 232.2 408.2 271.7 408 320.3C407.8 368.9 368.3 408.2 319.7 408z"/></svg>"##;

fn read_system_clipboard() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        read_clipboard_with_command("pbpaste", &[])
            .or_else(|| read_clipboard_with_command("/usr/bin/pbpaste", &[]))
    }

    #[cfg(target_os = "linux")]
    {
        read_clipboard_with_command("wl-paste", &["-n"])
            .or_else(|| read_clipboard_with_command("xclip", &["-selection", "clipboard", "-o"]))
            .or_else(|| read_clipboard_with_command("xsel", &["--clipboard", "--output"]))
    }

    #[cfg(target_os = "windows")]
    {
        read_clipboard_with_command("powershell", &["-NoProfile", "-Command", "Get-Clipboard"])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn read_clipboard_with_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8(output.stdout) {
        Ok(text) => Some(text),
        Err(error) => Some(String::from_utf8_lossy(error.as_bytes()).to_string()),
    }
}

fn configure_event_context_clipboard(event_context: &mut wgpui::EventContext) {
    event_context.set_clipboard(read_system_clipboard, |text| {
        let _ = wgpui::clipboard::copy_to_clipboard(text);
    });
}

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
        let spacetime_presence =
            crate::spacetime_presence::SpacetimePresenceRuntime::new(nostr_identity.as_ref());
        let spacetime_presence_snapshot = spacetime_presence.snapshot();

        let spark_wallet = crate::spark_wallet::SparkPaneState::default();
        let spark_worker = crate::spark_wallet::SparkWalletWorker::spawn(spark_wallet.network);
        let stable_sats_blink_worker =
            crate::stablesats_blink_worker::StableSatsBlinkWorker::spawn();
        let settings = crate::app_state::SettingsState::load_from_disk();
        let settings_inputs = crate::app_state::SettingsPaneInputs::from_state(&settings);
        let initial_relay_urls = settings.document.configured_relay_urls();
        let credentials = crate::app_state::CredentialsState::load_from_disk();
        let credentials_inputs = crate::app_state::CredentialsPaneInputs::from_state(&credentials);
        let autopilot_goals = crate::state::autopilot_goals::AutopilotGoalsState::load_from_disk();
        let codex_lane_config = CodexLaneConfig::default();
        let codex_lane_worker = CodexLaneWorker::spawn(codex_lane_config.clone());
        let sa_lane_worker = SaLaneWorker::spawn();
        let skl_lane_worker = SklLaneWorker::spawn();
        let ac_lane_worker = AcLaneWorker::spawn();
        let provider_nip90_lane_worker = ProviderNip90LaneWorker::spawn(initial_relay_urls.clone());
        let simulation_panes_enabled = simulation_panes_enabled_from_env();
        let sync_apply_engine = match crate::sync_apply::SyncApplyEngine::load_or_new_default() {
            Ok(engine) => engine,
            Err(error) => {
                tracing::warn!("sync apply checkpoint load failed: {}", error);
                crate::sync_apply::SyncApplyEngine::load_or_new(
                    std::env::temp_dir().join("openagents-sync-checkpoints-fallback.json"),
                    crate::sync_apply::SyncApplyPolicy::default(),
                )
                .map_err(|fallback_error| {
                    anyhow::anyhow!(
                        "failed to initialize fallback sync apply checkpoint engine: {}",
                        fallback_error
                    )
                })?
            }
        };
        let mut sync_health = crate::app_state::SyncHealthState::default();
        sync_health.last_applied_event_seq = sync_apply_engine.max_checkpoint_seq();
        sync_health.cursor_position = sync_health.last_applied_event_seq;
        sync_health.cursor_target_position = sync_health.last_applied_event_seq;
        let command_palette_actions = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut command_palette = CommandPalette::new()
            .mono(true)
            .commands(command_registry(simulation_panes_enabled));
        {
            let action_queue = Rc::clone(&command_palette_actions);
            command_palette = command_palette.on_select(move |command| {
                action_queue.borrow_mut().push(command.id.clone());
            });
        }

        let mut event_context = wgpui::EventContext::new();
        configure_event_context_clipboard(&mut event_context);

        let mut relay_connections = crate::app_state::RelayConnectionsState::default();
        relay_connections.replace_configured_relays(initial_relay_urls.as_slice());

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
            event_context,
            input_modifiers: wgpui::Modifiers::default(),
            panes: Vec::new(),
            nostr_identity,
            nostr_identity_error,
            nostr_secret_state: crate::app_state::NostrSecretState::default(),
            spark_wallet,
            spark_worker,
            stable_sats_blink_worker,
            spark_inputs: crate::app_state::SparkPaneInputs::default(),
            pay_invoice_inputs: crate::app_state::PayInvoicePaneInputs::default(),
            create_invoice_inputs: crate::app_state::CreateInvoicePaneInputs::default(),
            relay_connections_inputs: crate::app_state::RelayConnectionsPaneInputs::default(),
            network_requests_inputs: crate::app_state::NetworkRequestsPaneInputs::default(),
            settings_inputs,
            credentials_inputs,
            job_history_inputs: crate::app_state::JobHistoryPaneInputs::default(),
            chat_inputs: crate::app_state::ChatPaneInputs::default(),
            calculator_inputs: crate::app_state::CalculatorPaneInputs::default(),
            autopilot_chat: crate::app_state::AutopilotChatState::default(),
            chat_transcript_selection_drag: None,
            codex_account: crate::app_state::CodexAccountPaneState::default(),
            codex_models: crate::app_state::CodexModelsPaneState::default(),
            codex_config: crate::app_state::CodexConfigPaneState::default(),
            codex_mcp: crate::app_state::CodexMcpPaneState::default(),
            codex_apps: crate::app_state::CodexAppsPaneState::default(),
            codex_labs: crate::app_state::CodexLabsPaneState::default(),
            codex_diagnostics: crate::app_state::CodexDiagnosticsPaneState::default(),
            codex_lane: CodexLaneSnapshot::default(),
            codex_lane_config,
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
            provider_nip90_lane: ProviderNip90LaneSnapshot::with_relays(initial_relay_urls),
            provider_nip90_lane_worker,
            runtime_command_responses: Vec::new(),
            next_runtime_command_seq: 1,
            provider_runtime: crate::app_state::ProviderRuntimeState::default(),
            earnings_scoreboard: crate::app_state::EarningsScoreboardState::default(),
            network_aggregate_counters: crate::app_state::NetworkAggregateCountersState::default(),
            relay_connections,
            sync_health,
            sync_bootstrap_note: None,
            sync_bootstrap_error: None,
            sync_apply_engine,
            sync_lifecycle_worker_id: "desktopw:sync".to_string(),
            sync_lifecycle: crate::sync_lifecycle::RuntimeSyncLifecycleManager::default(),
            sync_lifecycle_snapshot: None,
            spacetime_presence,
            spacetime_presence_snapshot,
            network_requests: crate::app_state::NetworkRequestsState::default(),
            starter_jobs: crate::app_state::StarterJobsState::default(),
            reciprocal_loop: crate::app_state::ReciprocalLoopState::default(),
            activity_feed: crate::app_state::ActivityFeedState::default(),
            alerts_recovery: crate::app_state::AlertsRecoveryState::default(),
            settings,
            credentials,
            job_inbox: crate::app_state::JobInboxState::default(),
            active_job: crate::app_state::ActiveJobState::default(),
            job_history: crate::app_state::JobHistoryState::default(),
            earn_job_lifecycle_projection:
                crate::app_state::EarnJobLifecycleProjectionState::default(),
            earn_kernel_receipts:
                crate::state::earn_kernel_receipts::EarnKernelReceiptState::default(),
            economy_snapshot: crate::state::economy_snapshot::EconomySnapshotState::default(),
            agent_profile_state: crate::app_state::AgentProfileStatePaneState::default(),
            agent_schedule_tick: crate::app_state::AgentScheduleTickPaneState::default(),
            trajectory_audit: crate::app_state::TrajectoryAuditPaneState::default(),
            cast_control: crate::app_state::CastControlPaneState::default(),
            cast_control_process: None,
            skill_registry: crate::app_state::SkillRegistryPaneState::default(),
            skill_trust_revocation: crate::app_state::SkillTrustRevocationPaneState::default(),
            credit_desk: crate::app_state::CreditDeskPaneState::default(),
            credit_settlement_ledger: crate::app_state::CreditSettlementLedgerPaneState::default(),
            cad_demo: crate::app_state::CadDemoPaneState::default(),
            agent_network_simulation: crate::app_state::AgentNetworkSimulationPaneState::default(),
            treasury_exchange_simulation:
                crate::app_state::TreasuryExchangeSimulationPaneState::default(),
            relay_security_simulation: crate::app_state::RelaySecuritySimulationPaneState::default(
            ),
            stable_sats_simulation: crate::app_state::StableSatsSimulationPaneState::default(),
            simulation_panes_enabled,
            autopilot_goals,
            goal_loop_executor: crate::state::goal_loop_executor::GoalLoopExecutorState::default(),
            goal_restart_recovery_ran: false,
            sidebar: SidebarState::default(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            cad_camera_drag_state: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
            command_palette,
            command_palette_actions,
        };
        apply_spacetime_sync_bootstrap(&mut state);
        bootstrap_runtime_lanes(&mut state);
        let _ = state.sync_provider_nip90_lane_relays();
        open_startup_panes(&mut state);
        Ok(state)
    })
}

fn apply_spacetime_sync_bootstrap(state: &mut RenderState) {
    state.sync_bootstrap_note = None;
    state.sync_bootstrap_error = None;
    let worker_id = state.sync_lifecycle_worker_id.clone();
    state.sync_lifecycle.mark_idle(worker_id.as_str());
    state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());

    let client = match reqwest::blocking::Client::builder().build() {
        Ok(value) => value,
        Err(error) => {
            let message = format!("Sync token client initialization failed: {error}");
            state.sync_bootstrap_error = Some(message.clone());
            let reason = crate::sync_lifecycle::classify_disconnect_reason(message.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(message.clone()),
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.sync_health.last_action = Some("Spacetime bootstrap failed".to_string());
            state.sync_health.last_error = Some(message);
            return;
        }
    };

    match crate::sync_bootstrap::bootstrap_sync_session_from_env(&client) {
        Ok(Some(result)) => {
            let note = format!(
                "Minted sync token via {} and prepared subscribe target {}",
                result.control_token_endpoint, result.target.subscribe_url
            );
            state.sync_bootstrap_note = Some(note.clone());
            state.sync_lifecycle.mark_connecting(worker_id.as_str());
            let replay_cursor = state.sync_apply_engine.max_checkpoint_seq();
            state.sync_lifecycle.mark_replay_bootstrap(
                worker_id.as_str(),
                replay_cursor,
                Some(replay_cursor),
            );
            state.sync_lifecycle.mark_live(
                worker_id.as_str(),
                result.token_lease.refresh_after_in_seconds,
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.sync_health.last_error = None;
            state.sync_health.last_action = Some(note);
        }
        Ok(None) => {
            let note =
                "Spacetime bootstrap disabled (set OPENAGENTS_ENABLE_SPACETIME_SYNC=1)".to_string();
            state.sync_bootstrap_note = Some(note.clone());
            state.sync_lifecycle.mark_idle(worker_id.as_str());
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.sync_health.last_error = None;
            state.sync_health.last_action = Some(note);
        }
        Err(error) => {
            state.sync_bootstrap_error = Some(error.clone());
            let reason = crate::sync_lifecycle::classify_disconnect_reason(error.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(error.clone()),
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.sync_health.last_action = Some("Spacetime bootstrap failed".to_string());
            state.sync_health.last_error = Some(error);
        }
    }
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
            PaneKind::GoOnline => {
                let _ = PaneController::create_for_kind(state, pane_kind);
                if let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh) {
                    state.spark_wallet.last_error = Some(error);
                }
            }
            PaneKind::CadDemo => {
                let _ = PaneController::create_for_kind(state, pane_kind);
                bootstrap_startup_cad_mesh(state);
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

    // Sidebar: when open, reserve a right-hand panel; when closed, only a slim handle remains.
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = if state.sidebar.is_open {
        configured_width
    } else {
        0.0
    };
    let sidebar_x = (width - panel_width).max(0.0);

    if panel_width > 0.0 {
        let sidebar_color = Hsla::from_hex(0x171718).with_alpha(0.9);
        scene.draw_quad(
            Quad::new(Bounds::new(sidebar_x, 0.0, panel_width, height))
                .with_background(sidebar_color),
        );

        // Settings icon in the bottom-right corner of the sidebar.
        let icon_size = 16.0;
        let padding = 12.0;
        let icon_x = sidebar_x + panel_width - icon_size - padding;
        let icon_y = height - icon_size - padding;
        let icon_bounds = Bounds::new(icon_x, icon_y, icon_size, icon_size);
        let icon_tint = Hsla::from_hex(0x525256);
        let svg = SvgQuad::new(
            icon_bounds,
            std::sync::Arc::<[u8]>::from(SETTINGS_SVG_RAW.as_bytes()),
        )
        .with_tint(icon_tint);
        scene.draw_svg(svg);
    }

    // Minimal vertical handle for resize/collapse/expand affordance.
    let handle_height = 40.0;
    let handle_width = 4.0;
    let handle_offset_from_panel = 8.0;
    let handle_x = if state.sidebar.is_open {
        (sidebar_x + handle_offset_from_panel).min(width.max(0.0))
    } else {
        (width - handle_offset_from_panel - handle_width).max(0.0)
    };
    let handle_y = (height - handle_height) * 0.5;
    let handle_bounds = Bounds::new(handle_x, handle_y, handle_width, handle_height);
    let handle_color = theme::border::DEFAULT.with_alpha(0.6);
    // Draw three small bars to suggest a draggable handle.
    let bar_gap = 6.0;
    for i in 0..3 {
        let y =
            handle_bounds.origin.y + i as f32 * (handle_bounds.size.height - 2.0 * bar_gap) / 2.0;
        scene.draw_quad(
            Quad::new(Bounds::new(
                handle_bounds.origin.x,
                y,
                handle_bounds.size.width,
                4.0,
            ))
            .with_background(handle_color),
        );
    }

    // Animate tooltip: quick fade-in, immediate disappear on mouse-out.
    if state.sidebar.settings_hover {
        state.sidebar.settings_tooltip_t = (state.sidebar.settings_tooltip_t + 0.25).min(1.0);
    } else {
        state.sidebar.settings_tooltip_t = 0.0;
    }

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
        // Use fixed full-window bounds for the grid so dot positions don't shift when the sidebar is dragged.
        // Clip to the main canvas so we don't draw under the sidebar.
        let main_canvas_width = (width - panel_width).max(0.0);
        paint
            .scene
            .push_clip(Bounds::new(0.0, 0.0, main_canvas_width, height));
        dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut paint);
        paint.scene.pop_clip();

        if panel_width > 0.0 {
            let left = sidebar_x + 12.0;
            let right = sidebar_x + panel_width - 12.0;
            let mut y = 16.0;

            let providers_online = state.network_aggregate_counters.providers_online;
            let jobs_completed = state.network_aggregate_counters.jobs_completed;
            let sats_paid_network = state.network_aggregate_counters.sats_paid;
            let global_earnings_today_sats =
                state.network_aggregate_counters.global_earnings_today_sats;

            paint.scene.draw_text(paint.text.layout(
                "Autopilot - Mission Control",
                Point::new(left, y),
                13.0,
                theme::text::PRIMARY,
            ));
            y += 18.0;

            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Global Network Earnings Today: {} BTC",
                    format_btc_from_sats(global_earnings_today_sats)
                ),
                Point::new(left, y),
                10.0,
                if state.network_aggregate_counters.load_state
                    == crate::app_state::PaneLoadState::Ready
                {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));
            y += 18.0;

            let status_label = match state.provider_runtime.mode {
                ProviderMode::Offline => "OFFLINE",
                ProviderMode::Connecting => "CONNECTING",
                ProviderMode::Online => "ONLINE",
                ProviderMode::Degraded => "DEGRADED",
            };
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Status: {status_label}"),
                Point::new(left, y),
                11.0,
                if state.provider_runtime.mode == ProviderMode::Online {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));

            let go_online_bounds = Bounds::new(left, 72.0, (panel_width - 24.0).max(120.0), 34.0);
            let is_online = state.provider_runtime.mode != ProviderMode::Offline;
            let action_label = if is_online { "GO OFFLINE" } else { "GO ONLINE" };
            paint.scene.draw_quad(
                Quad::new(go_online_bounds)
                    .with_background(if is_online {
                        theme::status::ERROR.with_alpha(0.25)
                    } else {
                        theme::status::SUCCESS.with_alpha(0.28)
                    })
                    .with_border(
                        if is_online {
                            theme::status::ERROR.with_alpha(0.75)
                        } else {
                            theme::status::SUCCESS.with_alpha(0.75)
                        },
                        1.0,
                    )
                    .with_corner_radius(8.0),
            );
            let action_width = paint.text.measure(action_label, 11.0);
            paint.scene.draw_text(paint.text.layout_mono(
                action_label,
                Point::new(
                    go_online_bounds.origin.x + (go_online_bounds.size.width - action_width) * 0.5,
                    go_online_bounds.origin.y + 22.0,
                ),
                11.0,
                theme::text::PRIMARY,
            ));

            y = go_online_bounds.max_y() + 18.0;
            paint.scene.draw_text(paint.text.layout(
                "Network Stats",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Providers Online: {providers_online}"),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Providers Source: {}",
                    state.network_aggregate_counters.providers_online_source_tag
                ),
                Point::new(left, y),
                9.0,
                theme::text::MUTED,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Jobs Completed: {jobs_completed}"),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Sats Paid: {} BTC", format_btc_from_sats(sats_paid_network)),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            if let Some(snapshot) = state.economy_snapshot.latest_snapshot.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!("sv: {:.2}% | N: {}", snapshot.sv * 100.0, snapshot.n),
                    Point::new(left, y),
                    10.0,
                    theme::text::PRIMARY,
                ));
                y += 14.0;
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!(
                        "Snapshot: {}",
                        snapshot
                            .snapshot_id
                            .strip_prefix("snapshot.economy:")
                            .unwrap_or(snapshot.snapshot_id.as_str())
                    ),
                    Point::new(left, y),
                    9.0,
                    theme::text::MUTED,
                ));
            } else {
                paint.scene.draw_text(paint.text.layout_mono(
                    "Snapshot: pending",
                    Point::new(left, y),
                    9.0,
                    theme::text::MUTED,
                ));
            }

            y += 20.0;
            paint.scene.draw_text(paint.text.layout(
                "Your Earnings",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Today: {} sats", state.earnings_scoreboard.sats_today),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Total: {} sats", state.earnings_scoreboard.lifetime_sats),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));

            y += 20.0;
            paint.scene.draw_text(paint.text.layout(
                "Recent Payouts",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;

            if let Some(active) = state.active_job.job.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!(
                        "{} | {} | {} sats | {}",
                        active.job_id,
                        active.capability,
                        active.quoted_price_sats,
                        active.stage.label()
                    ),
                    Point::new(left, y),
                    10.0,
                    theme::text::PRIMARY,
                ));
                y += 14.0;
            }

            let recent_rows = state
                .job_history
                .wallet_reconciled_payout_rows(&state.spark_wallet);

            if recent_rows.is_empty() && state.active_job.job.is_none() {
                let waiting = if state.provider_runtime.mode == ProviderMode::Online {
                    "Waiting for first job..."
                } else {
                    "(empty)"
                };
                paint.scene.draw_text(paint.text.layout(
                    waiting,
                    Point::new(left, y),
                    10.0,
                    theme::text::MUTED,
                ));
                y += 14.0;
            } else {
                for row in recent_rows.into_iter().take(2) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &format!("{} | settled | {} sats", row.job_id, row.payout_sats),
                        Point::new(left, y),
                        10.0,
                        theme::text::PRIMARY,
                    ));
                    y += 14.0;
                }
            }

            y += 10.0;
            let wallet_state = if state.spark_wallet.last_error.is_some() {
                "degraded"
            } else {
                state.spark_wallet.network_status_label()
            };
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Wallet: {wallet_state}"),
                Point::new(left, y),
                10.0,
                if wallet_state == "connected" {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Lane: {}", state.sync_health.subscription_state),
                Point::new(right - 118.0, y),
                10.0,
                theme::text::MUTED,
            ));
        }

        let hotbar_layer = PaneRenderer::paint(
            &mut state.panes,
            active_pane,
            state.nostr_identity.as_ref(),
            state.nostr_identity_error.as_deref(),
            &state.nostr_secret_state,
            &state.autopilot_chat,
            &state.codex_account,
            &state.codex_models,
            &state.codex_config,
            &state.codex_mcp,
            &state.codex_apps,
            &state.codex_labs,
            &state.codex_diagnostics,
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
            &state.reciprocal_loop,
            &state.activity_feed,
            &state.alerts_recovery,
            &state.settings,
            &state.credentials,
            &state.job_inbox,
            &state.active_job,
            &state.job_history,
            &state.earn_job_lifecycle_projection,
            &state.agent_profile_state,
            &state.agent_schedule_tick,
            &state.trajectory_audit,
            &state.cast_control,
            &state.skill_registry,
            &state.skill_trust_revocation,
            &state.credit_desk,
            &state.credit_settlement_ledger,
            &state.cad_demo,
            &state.agent_network_simulation,
            &state.treasury_exchange_simulation,
            &state.relay_security_simulation,
            &state.stable_sats_simulation,
            &state.spark_wallet,
            &mut state.spark_inputs,
            &mut state.pay_invoice_inputs,
            &mut state.create_invoice_inputs,
            &mut state.relay_connections_inputs,
            &mut state.network_requests_inputs,
            &mut state.settings_inputs,
            &mut state.credentials_inputs,
            &mut state.job_history_inputs,
            &mut state.chat_inputs,
            &mut state.calculator_inputs,
            &mut paint,
        );
        paint.scene.set_layer(hotbar_layer);

        let wallet_chip_bounds = wallet_balance_chip_bounds_for_logical(logical);
        let total_sats = state
            .spark_wallet
            .balance
            .as_ref()
            .map_or(0, spark_total_balance_sats);
        let wallet_chip_label = format!("{total_sats} sats");
        paint.scene.draw_quad(
            Quad::new(wallet_chip_bounds)
                .with_background(theme::bg::SURFACE.with_alpha(0.88))
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(6.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &wallet_chip_label,
            Point::new(
                wallet_chip_bounds.origin.x + 10.0,
                wallet_chip_bounds.origin.y + 17.0,
            ),
            11.0,
            theme::text::PRIMARY,
        ));

        let bar_bounds = hotbar_bounds(logical);
        state.hotbar_bounds = bar_bounds;
        configure_hotbar(&mut state.hotbar);
        state.hotbar.paint(bar_bounds, &mut paint);

        state
            .command_palette
            .paint(Bounds::new(0.0, 0.0, width, height), &mut paint);

        // Sidebar tooltip for the settings icon.
        if state.sidebar.settings_tooltip_t > 0.01 && panel_width > 0.0 {
            let _tooltip_alpha = state.sidebar.settings_tooltip_t;
            let icon_size = 16.0;
            let padding = 12.0;
            let icon_x = sidebar_x + panel_width - icon_size - padding;
            let icon_y = height - icon_size - padding;

            let tooltip_text = "Settings";
            let tooltip_font_size = theme::font_size::XS - 1.0;
            let measured_w = paint.text.measure(tooltip_text, tooltip_font_size);
            let tooltip_h_pad = 10.0;
            let tooltip_width = measured_w + tooltip_h_pad * 2.0 + 10.0;
            let tooltip_height = 24.0;
            let caret_size = 6.0;
            let tooltip_margin = 8.0 + caret_size;
            let tooltip_radius = 6.0;
            let icon_center_x = icon_x + icon_size * 0.5;
            let mut tooltip_x = icon_center_x - tooltip_width * 0.5;
            let tooltip_y = icon_y - tooltip_height - tooltip_margin;

            // Clamp so the tooltip stays inside the sidebar panel.
            let sidebar_left = sidebar_x;
            let sidebar_right = sidebar_x + panel_width;
            if tooltip_x < sidebar_left + 4.0 {
                tooltip_x = sidebar_left + 4.0;
            }
            if tooltip_x + tooltip_width > sidebar_right - 4.0 {
                tooltip_x = sidebar_right - tooltip_width - 4.0;
            }
            let tooltip_y = tooltip_y.max(4.0);
            let tooltip_bounds = Bounds::new(tooltip_x, tooltip_y, tooltip_width, tooltip_height);

            let tooltip_bg = theme::bg::MUTED.with_alpha(_tooltip_alpha);
            paint.scene.draw_quad(
                Quad::new(tooltip_bounds)
                    .with_background(tooltip_bg)
                    .with_corner_radius(tooltip_radius),
            );

            // Downward caret at bottom-right of tooltip
            let caret_svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{h}" viewBox="0 0 {s} {h}"><polygon points="0,0 {s},0 {mid},{h}" fill="#1A1A1A"/></svg>"##,
                s = (caret_size * 2.0) as i32,
                h = caret_size as i32,
                mid = caret_size as i32,
            );
            let caret_w = caret_size * 2.0;
            let caret_h = caret_size;
            let caret_x = icon_center_x - caret_w * 0.5;
            let caret_y = tooltip_bounds.origin.y + tooltip_bounds.size.height;
            let caret_bounds = Bounds::new(caret_x, caret_y, caret_w, caret_h);
            paint.scene.draw_svg(SvgQuad {
                bounds: caret_bounds,
                svg_data: std::sync::Arc::from(caret_svg.as_bytes()),
                tint: Some(theme::bg::MUTED.with_alpha(_tooltip_alpha)),
            });

            // Horizontally centered text — use full tooltip width to avoid clipping
            let tooltip_text_color = theme::text::PRIMARY.with_alpha(_tooltip_alpha);
            let font_size = tooltip_font_size;
            let text_x = tooltip_bounds.origin.x + 5.0;
            let text_y =
                tooltip_bounds.origin.y + (tooltip_bounds.size.height - font_size) * 0.5 - 13.0;
            let text_bounds = Bounds::new(text_x, text_y, tooltip_bounds.size.width, font_size);
            let mut label = Text::new(tooltip_text)
                .font_size(font_size)
                .color(tooltip_text_color);
            label.paint(text_bounds, &mut paint);
        }
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

pub fn wallet_balance_chip_bounds_for_logical(logical: Size) -> Bounds {
    let available_width = (logical.width - WALLET_BALANCE_CHIP_MARGIN * 2.0).max(0.0);
    let width = available_width
        .min(WALLET_BALANCE_CHIP_MAX_WIDTH)
        .max(WALLET_BALANCE_CHIP_MIN_WIDTH.min(available_width));
    let y = (logical.height - WALLET_BALANCE_CHIP_MARGIN - WALLET_BALANCE_CHIP_HEIGHT).max(0.0);
    Bounds::new(
        WALLET_BALANCE_CHIP_MARGIN,
        y,
        width,
        WALLET_BALANCE_CHIP_HEIGHT,
    )
}

pub fn wallet_balance_chip_bounds(state: &RenderState) -> Bounds {
    wallet_balance_chip_bounds_for_logical(logical_size(&state.config, state.scale_factor))
}

/// Bounds of the sidebar resize handle in logical coordinates. Used for hit-testing and cursor.
pub fn sidebar_handle_bounds(state: &RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = if state.sidebar.is_open {
        configured_width
    } else {
        0.0
    };
    let sidebar_x = (width - panel_width).max(0.0);
    let handle_height = 40.0;
    let handle_width = 4.0;
    let handle_offset_from_panel = 8.0;
    let handle_x = if state.sidebar.is_open {
        (sidebar_x + handle_offset_from_panel).min(width.max(0.0))
    } else {
        (width - handle_offset_from_panel - handle_width).max(0.0)
    };
    let handle_y = (height - handle_height) * 0.5;
    Bounds::new(handle_x, handle_y, handle_width, handle_height)
}

/// Bounds of the "Go Online" mission-control button in the sidebar (when panel is open).
pub fn sidebar_go_online_button_bounds(state: &RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = if state.sidebar.is_open {
        configured_width
    } else {
        0.0
    };
    let sidebar_x = (width - panel_width).max(0.0);
    if panel_width < 1.0 {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }
    let width = (panel_width - 24.0).max(120.0);
    Bounds::new(sidebar_x + 12.0, 72.0, width, 34.0)
}

fn format_btc_from_sats(sats: u64) -> String {
    format!("{:.8}", sats as f64 / 100_000_000.0)
}

fn command_registry(simulation_panes_enabled: bool) -> Vec<Command> {
    let mut commands: Vec<Command> = pane_specs()
        .iter()
        .filter(|spec| pane_enabled_in_runtime(spec.kind, simulation_panes_enabled))
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
        .collect();

    commands.extend(cad_palette_command_specs().iter().map(|spec| {
        let mut command = Command::new(spec.id, spec.label)
            .description(spec.description)
            .category("CAD");
        if let Some(keys) = spec.keybinding {
            command = command.keybinding(keys);
        }
        command
    }));

    commands
}

#[cfg(test)]
mod tests {
    use super::{command_registry, wallet_balance_chip_bounds_for_logical};
    use crate::app_state::PaneKind;
    use crate::pane_registry::{
        pane_enabled_in_runtime, pane_spec_by_command_id, pane_specs, startup_pane_kinds,
    };
    use crate::pane_system::cad_palette_command_specs;
    use std::collections::BTreeSet;
    use wgpui::Size;

    #[test]
    fn command_registry_matches_pane_specs() {
        let commands = command_registry(true);
        let command_ids: BTreeSet<&str> =
            commands.iter().map(|command| command.id.as_str()).collect();

        let pane_command_ids: BTreeSet<&str> = pane_specs()
            .iter()
            .filter_map(|spec| spec.command.map(|command| command.id))
            .collect();
        let cad_command_ids: BTreeSet<&str> = cad_palette_command_specs()
            .iter()
            .map(|spec| spec.id)
            .collect();
        let expected_ids: BTreeSet<&str> =
            pane_command_ids.union(&cad_command_ids).copied().collect();
        assert_eq!(command_ids, expected_ids);

        for command in &commands {
            if let Some(spec) = pane_spec_by_command_id(&command.id) {
                let pane_command = spec.command.expect("resolved pane must define a command");
                assert_eq!(command.label, pane_command.label);
                continue;
            }
            let cad_spec = cad_palette_command_specs()
                .iter()
                .find(|spec| spec.id == command.id)
                .expect("command id from registry should resolve to pane or cad command");
            assert_eq!(command.label, cad_spec.label);
        }
    }

    #[test]
    fn command_registry_includes_job_inbox_command() {
        let commands = command_registry(false);
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
            command.id == "pane.reciprocal_loop" && command.label == "Reciprocal Loop"
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
            commands.iter().any(|command| {
                command.id == "pane.credentials" && command.label == "Credentials"
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
            command.id == "pane.cast_control" && command.label == "CAST Control"
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
    }

    #[test]
    fn command_registry_hides_simulation_commands_when_disabled() {
        let commands = command_registry(false);
        assert!(!commands.iter().any(|command| {
            command.id == "pane.agent_network_simulation"
                || command.id == "pane.treasury_exchange_simulation"
                || command.id == "pane.relay_security_simulation"
                || command.id == "pane.stablesats_simulation"
        }));
    }

    #[test]
    fn command_registry_includes_simulation_commands_when_enabled() {
        let commands = command_registry(true);
        let command_ids: BTreeSet<&str> =
            commands.iter().map(|command| command.id.as_str()).collect();

        let expected_simulation_ids: BTreeSet<&str> = pane_specs()
            .iter()
            .filter(|spec| {
                pane_enabled_in_runtime(spec.kind, true)
                    && !pane_enabled_in_runtime(spec.kind, false)
            })
            .filter_map(|spec| spec.command.map(|command| command.id))
            .collect();
        assert_eq!(
            expected_simulation_ids,
            BTreeSet::from([
                "pane.agent_network_simulation",
                "pane.treasury_exchange_simulation",
                "pane.relay_security_simulation",
                "pane.stablesats_simulation",
            ])
        );
        for command_id in expected_simulation_ids {
            assert!(command_ids.contains(command_id));
        }
    }

    #[test]
    fn startup_pane_set_matches_mvp_core_surfaces() {
        let startup = startup_pane_kinds();
        assert_eq!(startup, vec![PaneKind::GoOnline]);
        assert!(!startup.contains(&PaneKind::AutopilotChat));
        assert!(!startup.contains(&PaneKind::CadDemo));
        assert!(!startup.contains(&PaneKind::SparkWallet));
        assert!(!startup.contains(&PaneKind::Empty));
    }

    #[test]
    fn wallet_balance_chip_is_anchored_bottom_left() {
        let bounds = wallet_balance_chip_bounds_for_logical(Size::new(1280.0, 800.0));
        assert!(bounds.origin.x <= 16.0);
        assert!(bounds.max_y() >= 784.0);
        assert!(bounds.size.width >= 140.0);
        assert!(bounds.size.height >= 24.0);
    }
}
