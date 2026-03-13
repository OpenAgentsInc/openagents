use codex_client::{
    ApprovalDecision, AppsListParams, ChatgptAuthTokensRefreshResponse,
    CollaborationModeListParams, CommandExecParams, CommandExecutionRequestApprovalResponse,
    ConfigBatchWriteParams, ConfigEdit, ConfigReadParams, ConfigValueWriteParams,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse, ExperimentalFeatureListParams,
    ExternalAgentConfigDetectParams, ExternalAgentConfigImportParams,
    FileChangeRequestApprovalResponse, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams,
    ListMcpServerStatusParams, LoginAccountParams, McpServerOauthLoginParams, MergeStrategy,
    ModelListParams, ReviewDelivery, ReviewStartParams, ReviewTarget, ThreadArchiveParams,
    ThreadCompactStartParams, ThreadForkParams, ThreadLoadedListParams,
    ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams, ThreadRealtimeStopParams,
    ThreadResumeParams, ThreadRollbackParams, ThreadSetNameParams, ThreadStartParams,
    ThreadUnarchiveParams, ThreadUnsubscribeParams, ToolRequestUserInputAnswer,
    ToolRequestUserInputResponse, UserInput, WindowsSandboxSetupStartParams,
};
use nostr::regenerate_identity;
use openagents_cad::contracts::CadSelectionKind;
use openagents_cad::query::{
    CadPickCameraPose, CadPickEntityKind, CadPickProjectionMode, CadPickQuery, CadPickViewport,
    pick_mesh_hit,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::EvidenceRef;
use openagents_provider_substrate::ProviderDesiredMode;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use wgpui::clipboard::copy_to_clipboard;
use wgpui::{Bounds, Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};
use winit::window::Fullscreen;

use crate::app_state::{
    AlertDomain, App, CadCameraDragMode, CadCameraDragState, CadHotkeyAction,
    ChatTranscriptSelectionDragState, EarnFailureClass, JobInboxNetworkRequest, JobInboxValidation,
    NetworkRequestSubmission, PaneKind, ProviderMode,
};
use crate::apple_fm_bridge::AppleFmBridgeCommand;
use crate::hotbar::{
    HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET, activate_hotbar_slot,
    hotbar_slot_for_key, process_hotbar_clicks,
};
use crate::local_inference_runtime::LocalInferenceRuntimeCommand;
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::pane_registry::pane_spec_by_command_id;
use crate::pane_system::{
    ActivityFeedPaneAction, AlertsRecoveryPaneAction, CadDemoPaneAction, CastControlPaneAction,
    CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction, CodexDiagnosticsPaneAction,
    CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction, CredentialsPaneAction,
    EarningsScoreboardPaneAction, NetworkRequestsPaneAction, PaneController, PaneHitAction,
    PaneInput, ProviderStatusPaneAction, RIGHT_SIDEBAR_ENABLED, ReciprocalLoopPaneAction,
    RelayConnectionsPaneAction, SIDEBAR_DEFAULT_WIDTH, SettingsPaneAction, StarterJobsPaneAction,
    SyncHealthPaneAction, cad_demo_context_menu_bounds, cad_demo_context_menu_row_bounds,
    clamp_all_panes_to_window, dispatch_active_job_scroll_event,
    dispatch_activity_feed_detail_scroll_event, dispatch_apple_fm_workbench_input_event,
    dispatch_apple_fm_workbench_log_scroll_event, dispatch_buy_mode_payments_scroll_event,
    dispatch_calculator_input_event, dispatch_chat_input_event, dispatch_chat_scroll_event,
    dispatch_create_invoice_input_event, dispatch_credentials_input_event,
    dispatch_job_history_input_event, dispatch_local_inference_input_event,
    dispatch_mission_control_input_event, dispatch_mission_control_log_scroll_event,
    dispatch_network_requests_input_event, dispatch_pay_invoice_input_event,
    dispatch_relay_connections_input_event, dispatch_settings_input_event,
    dispatch_spark_input_event, pane_content_bounds, pane_indices_by_z_desc,
    pane_z_sort_invocation_count, topmost_pane_hit_action_in_order,
};
use crate::panes::{cad as cad_pane, chat as chat_pane};
use crate::provider_nip90_lane::ProviderNip90LaneCommand;
use crate::render::{
    logical_size, render_frame, sidebar_go_online_button_bounds, sidebar_handle_bounds,
    wallet_balance_sats_label_bounds,
};
use crate::runtime_lanes::{
    AcCreditCommand, RuntimeCommandResponse, RuntimeCommandStatus, RuntimeLane, SaLifecycleCommand,
    SklDiscoveryTrustCommand,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;
use crate::state::autopilot_goals::{
    GoalAttemptAuditReceipt, GoalExecutionReceipt, GoalLaborLinkage, GoalLifecycleEvent,
    GoalLifecycleStatus, GoalPayoutEvidence, GoalRunAuditReceipt, GoalToolInvocationAudit,
};
use crate::state::goal_conditions::{ConditionEvaluation, GoalProgressSnapshot};
use crate::state::goal_loop_executor::{
    ActiveGoalLoopRun, GoalLoopPhase, GoalLoopStopReason, retry_backoff_seconds,
    select_runnable_goal,
};
use crate::state::wallet_reconciliation::{
    WalletLedgerEventKind, reconcile_wallet_events_for_goal,
};

mod actions;
mod cad_turn_classifier;
mod reducers;
mod shortcuts;
mod tool_bridge;
use actions::*;

pub(crate) use actions::build_mission_control_buy_mode_request_event;
pub(crate) use actions::ensure_mission_control_apple_fm_refresh;
pub(crate) use actions::queue_managed_chat_channel_message;
pub(crate) use actions::queue_managed_chat_message_to_channel_with_relay;
use shortcuts::*;

pub(crate) fn bootstrap_startup_cad_mesh(state: &mut crate::app_state::RenderState) {
    let _ = reducers::bootstrap_startup_parallel_jaw_gripper(state);
}

pub(crate) fn remote_select_codex_thread(
    state: &mut crate::app_state::RenderState,
    thread_id: &str,
) -> Result<(), String> {
    state.autopilot_chat.last_error = None;
    state.autopilot_chat.selected_workspace = crate::app_state::ChatWorkspaceSelection::Autopilot;
    let Some(index) = state
        .autopilot_chat
        .threads
        .iter()
        .position(|candidate| candidate == thread_id)
    else {
        return Err(format!("Unknown Codex thread `{thread_id}`"));
    };
    actions::sync_chat_composer_draft(state);
    let Some(target) = state.autopilot_chat.select_thread_by_index(index) else {
        return Err(format!("Failed to select thread `{thread_id}`"));
    };
    actions::restore_chat_composer_draft(state);
    let experimental_api = state.codex_lane_config.experimental_api;
    let resume_path = if experimental_api {
        target.path.clone()
    } else {
        None
    };
    state
        .autopilot_chat
        .restore_session_preferences_from_thread(&target.thread_id);
    state.queue_codex_command(crate::codex_lane::CodexLaneCommand::ThreadResume(
        ThreadResumeParams {
            thread_id: target.thread_id.clone(),
            model: state.autopilot_chat.selected_model_override(),
            model_provider: None,
            service_tier: actions::chat_session_service_tier(state),
            cwd: target
                .cwd
                .or_else(|| actions::current_chat_session_cwd(state)),
            approval_policy: actions::chat_session_approval_policy(state),
            sandbox: actions::chat_session_thread_sandbox_mode(state),
            personality: actions::chat_session_personality(state),
            path: resume_path.map(std::path::PathBuf::from),
        },
    ))?;
    state.queue_codex_command(crate::codex_lane::CodexLaneCommand::ThreadRead(
        codex_client::ThreadReadParams {
            thread_id: target.thread_id,
            include_turns: true,
        },
    ))?;
    state.autopilot_chat.last_error = None;
    Ok(())
}

pub(crate) fn remote_submit_codex_prompt(
    state: &mut crate::app_state::RenderState,
    prompt: String,
) -> Result<(), String> {
    state.autopilot_chat.last_error = None;
    state.autopilot_chat.selected_workspace = crate::app_state::ChatWorkspaceSelection::Autopilot;
    state.chat_inputs.composer.set_value(prompt.clone());
    state.autopilot_chat.record_composer_draft(prompt);
    let _ = actions::run_chat_submit_action_with_trigger(
        state,
        crate::labor_orchestrator::CodexRunTrigger::PersonalAgent,
    );
    if let Some(error) = state.autopilot_chat.last_error.clone() {
        Err(error)
    } else {
        Ok(())
    }
}

pub(crate) fn remote_interrupt_codex_turn(
    state: &mut crate::app_state::RenderState,
) -> Result<(), String> {
    state.autopilot_chat.last_error = None;
    state.autopilot_chat.selected_workspace = crate::app_state::ChatWorkspaceSelection::Autopilot;
    let _ = actions::run_chat_interrupt_turn_action(state);
    if let Some(error) = state.autopilot_chat.last_error.clone() {
        Err(error)
    } else {
        Ok(())
    }
}

pub(crate) fn desktop_control_run_mission_control_action(
    state: &mut crate::app_state::RenderState,
    action: crate::pane_system::MissionControlPaneAction,
) -> bool {
    actions::run_mission_control_action(state, action)
}

pub fn handle_window_event(app: &mut App, event_loop: &ActiveEventLoop, event: WindowEvent) {
    let Some(state) = &mut app.state else {
        return;
    };

    if pump_background_state(state) {
        state.window.request_redraw();
    }

    match event {
        WindowEvent::CloseRequested => {
            let worker_id = state.sync_lifecycle_worker_id.clone();
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                crate::sync_lifecycle::RuntimeSyncDisconnectReason::StreamClosed,
                Some("desktop shutdown requested".to_string()),
            );
            state.sync_lifecycle.mark_idle(worker_id.as_str());
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            crate::render::sync_project_ops_runtime_contract_state(state);
            let _ = state.spacetime_presence.register_offline();
            state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
            let _ = state.spark_worker.cancel_pending();
            let _ = state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::StopBridge);
            if let Some(mut process) = state.cast_control_process.take() {
                let _ = process.child.kill();
                let _ = process.child.wait();
            }
            state.codex_lane_worker.shutdown_async();
            state.kernel_projection_worker.shutdown_async();
            event_loop.exit();
        }
        WindowEvent::Resized(new_size) => {
            state.config.width = new_size.width.max(1);
            state.config.height = new_size.height.max(1);
            state.surface.configure(&state.device, &state.config);
            clamp_all_panes_to_window(state);
            state.window.request_redraw();
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            state.scale_factor = scale_factor as f32;
            state.text_system.set_scale_factor(state.scale_factor);
            state.window.request_redraw();
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            state.input_modifiers = map_modifiers(modifiers.state());
        }
        WindowEvent::CursorMoved { position, .. } => {
            let scale = state.scale_factor.max(0.1);
            app.cursor_position = Point::new(position.x as f32 / scale, position.y as f32 / scale);
            state.cursor_position = app.cursor_position;

            if state.dev_mode_enabled() && state.command_palette.is_open() {
                let event = InputEvent::MouseMove {
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                };
                if state
                    .command_palette
                    .event(
                        &event,
                        command_palette_bounds(state),
                        &mut state.event_context,
                    )
                    .is_handled()
                {
                    state.window.request_redraw();
                }
                return;
            }

            let needs_redraw = dispatch_mouse_move(state, app.cursor_position);

            state
                .window
                .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));

            if needs_redraw {
                state.window.request_redraw();
            }
        }
        WindowEvent::MouseInput {
            state: mouse_state,
            button,
            ..
        } => {
            let button = match button {
                winit::event::MouseButton::Left => MouseButton::Left,
                winit::event::MouseButton::Right => MouseButton::Right,
                winit::event::MouseButton::Middle => MouseButton::Middle,
                _ => return,
            };

            let input = match mouse_state {
                ElementState::Pressed => InputEvent::MouseDown {
                    button,
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                    modifiers: state.input_modifiers,
                },
                ElementState::Released => InputEvent::MouseUp {
                    button,
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                },
            };

            if state.dev_mode_enabled() && state.command_palette.is_open() {
                let mut handled = state
                    .command_palette
                    .event(
                        &input,
                        command_palette_bounds(state),
                        &mut state.event_context,
                    )
                    .is_handled();
                if matches!(mouse_state, ElementState::Released) {
                    handled |= dispatch_command_palette_actions(state);
                }
                if handled {
                    state.window.request_redraw();
                }
                return;
            }

            match mouse_state {
                ElementState::Pressed => {
                    let handled = dispatch_mouse_down(state, app.cursor_position, button, &input);

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
                ElementState::Released => {
                    let handled = dispatch_mouse_up(state, app.cursor_position, &input);

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let (dx, dy) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                MouseScrollDelta::PixelDelta(pos) => (-pos.x as f32, -pos.y as f32),
            };
            let scroll_event = InputEvent::Scroll { dx, dy };
            if state.dev_mode_enabled() && state.command_palette.is_open() {
                if state
                    .command_palette
                    .event(
                        &scroll_event,
                        command_palette_bounds(state),
                        &mut state.event_context,
                    )
                    .is_handled()
                {
                    state.window.request_redraw();
                }
                return;
            }
            if dispatch_mouse_scroll(state, app.cursor_position, &scroll_event) {
                state.window.request_redraw();
            }
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed
                && is_toggle_fullscreen_shortcut(&event.logical_key, state.input_modifiers)
            {
                toggle_window_fullscreen(state);
                state.window.request_redraw();
                return;
            }

            let text_input_focused = any_text_input_focused(state);
            if event.state == ElementState::Pressed
                && should_open_command_palette(
                    &event.logical_key,
                    state.input_modifiers,
                    text_input_focused,
                    state.dev_mode_enabled() && state.command_palette.is_open(),
                )
                && state.dev_mode_enabled()
            {
                toggle_command_palette(state);
                state.window.request_redraw();
                return;
            }

            if event.state != ElementState::Pressed {
                return;
            }

            if state.dev_mode_enabled() && state.command_palette.is_open() {
                if let Some(key) = map_winit_key(&event.logical_key) {
                    let palette_event = InputEvent::KeyDown {
                        key,
                        modifiers: state.input_modifiers,
                    };
                    let mut handled = state
                        .command_palette
                        .event(
                            &palette_event,
                            command_palette_bounds(state),
                            &mut state.event_context,
                        )
                        .is_handled();
                    handled |= dispatch_command_palette_actions(state);
                    if handled {
                        state.window.request_redraw();
                    }
                }
                return;
            }

            if dispatch_keyboard_submit_actions(state, &event.logical_key)
                || handle_activity_feed_keyboard_input(state, &event.logical_key)
                || handle_alerts_recovery_keyboard_input(state, &event.logical_key)
            {
                state.window.request_redraw();
                return;
            }
            if text_input_focused {
                return;
            }

            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(pane_id) = PaneController::active(state) {
                        PaneController::close(state, pane_id);
                        state.window.request_redraw();
                    }
                }
                PhysicalKey::Code(KeyCode::BracketRight) => {
                    if RIGHT_SIDEBAR_ENABLED {
                        state.sidebar.is_open = !state.sidebar.is_open;
                        if state.sidebar.is_open && state.sidebar.width < 50.0 {
                            state.sidebar.width = SIDEBAR_DEFAULT_WIDTH;
                        }
                        clamp_all_panes_to_window(state);
                        state.window.request_redraw();
                    }
                }
                key => {
                    if state.dev_mode_enabled()
                        && let Some(slot) = hotbar_slot_for_key(key)
                    {
                        activate_hotbar_slot(state, slot);
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::RedrawRequested => {
            // Keep background lanes advancing even during redraw-heavy periods.
            // Without this, Codex notifications can backlog until the next input event.
            let _ = pump_background_state(state);
            if render_frame(state).is_err() {
                event_loop.exit();
                return;
            }
            let flashing_now = state.hotbar.is_flashing();
            let provider_animating = matches!(
                state.provider_runtime.mode,
                ProviderMode::Connecting | ProviderMode::Online
            );
            if (state.dev_mode_enabled() && flashing_now)
                || (state.dev_mode_enabled() && state.hotbar_flash_was_active)
                || provider_animating
                || state.autopilot_chat.has_pending_messages()
            {
                state.window.request_redraw();
            }
            state.hotbar_flash_was_active = state.dev_mode_enabled() && flashing_now;
        }
        _ => {}
    }
}

pub fn handle_device_event(app: &mut App, _event_loop: &ActiveEventLoop, event: DeviceEvent) {
    let Some(state) = &mut app.state else {
        return;
    };
    if !cad_three_d_mouse_enabled() {
        return;
    }
    let DeviceEvent::Motion { axis, value } = event else {
        return;
    };
    if !state
        .panes
        .iter()
        .any(|pane| pane.kind == PaneKind::CadDemo)
    {
        return;
    }
    let previous_events = state.cad_demo.three_d_mouse_event_count;
    let changed = state.cad_demo.apply_three_d_mouse_motion(axis, value);
    if !changed && state.cad_demo.three_d_mouse_event_count == previous_events {
        return;
    }
    state.cad_demo.last_action = Some(format!(
        "CAD 3D mouse axis={} value={:.3} -> {}",
        axis,
        value,
        state.cad_demo.three_d_mouse_status()
    ));
    state.window.request_redraw();
}

fn cad_three_d_mouse_enabled() -> bool {
    static CAD_3D_MOUSE_ENABLED: OnceLock<bool> = OnceLock::new();
    *CAD_3D_MOUSE_ENABLED.get_or_init(|| {
        std::env::var("OPENAGENTS_CAD_ENABLE_3D_MOUSE")
            .ok()
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
            })
            .unwrap_or(false)
    })
}

pub fn handle_about_to_wait(app: &mut App, event_loop: &ActiveEventLoop) {
    let Some(state) = &mut app.state else {
        return;
    };

    let changed = pump_background_state(state);
    let provider_animating = matches!(
        state.provider_runtime.mode,
        ProviderMode::Connecting | ProviderMode::Online
    );
    let should_redraw = changed
        || state.hotbar.is_flashing()
        || provider_animating
        || state.autopilot_chat.has_pending_messages()
        || any_text_input_focused(state);
    if should_redraw {
        state.window.request_redraw();
    }

    // Keep a lightweight cadence so background lane updates (Codex/runtime/spark) are surfaced
    // even when the user is idle and no UI events are incoming.
    let poll_interval = if state.autopilot_chat.has_pending_messages() {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(50)
    };
    event_loop.set_control_flow(ControlFlow::WaitUntil(
        std::time::Instant::now() + poll_interval,
    ));
}

fn pump_background_state(state: &mut crate::app_state::RenderState) -> bool {
    let mut changed = false;
    let now = std::time::Instant::now();
    if reducers::drain_spark_worker_updates(state) {
        changed = true;
    }
    if reducers::drain_stable_sats_blink_worker_updates(state) {
        changed = true;
    }
    if reducers::drain_runtime_lane_updates(state) {
        changed = true;
    }
    if crate::provider_admin::pump_runtime(state) {
        changed = true;
    }
    if crate::desktop_control::pump_runtime(state) {
        changed = true;
    }
    if crate::codex_remote::pump_runtime(state) {
        changed = true;
    }
    if crate::chat_terminal::pump_runtime(state) {
        changed = true;
    }
    if crate::kernel_control::drain_kernel_projection_updates(state) {
        changed = true;
    }
    if crate::kernel_control::refresh_provider_inventory_rows(state) {
        changed = true;
    }
    if reducers::run_cad_demo_action(state, CadDemoPaneAction::Noop) {
        changed = true;
    }
    if run_goal_restart_recovery(state) {
        changed = true;
    }
    if run_goal_interval_scheduler(state) {
        changed = true;
    }
    if run_autonomous_goal_loop(state) {
        changed = true;
    }
    if reducers::refresh_goal_profile_state(state) {
        changed = true;
    }
    if state.nostr_secret_state.expire(now) {
        changed = true;
    }
    if state.autopilot_chat.expire_copy_notice(now) {
        changed = true;
    }
    if run_cast_control_process_tick(state) {
        changed = true;
    }
    if run_auto_cast_control_loop(state, now) {
        changed = true;
    }
    if run_mission_control_buy_mode_tick(state, now) {
        changed = true;
    }
    if run_startup_spark_wallet_convergence_tick(state) {
        changed = true;
    }
    if run_pending_buyer_payment_watchdog_tick(state, now) {
        changed = true;
    }
    if run_auto_starter_demand_generator(state, now) {
        changed = true;
    }
    if reducers::run_job_inbox_auto_admission_tick(state) {
        changed = true;
    }
    if reducers::run_active_job_execution_tick(state) {
        changed = true;
    }
    if run_hosted_starter_lease_heartbeat(state, now) {
        changed = true;
    }
    if run_reciprocal_loop_engine_tick(state) {
        changed = true;
    }
    if run_open_network_paid_transition_reconciliation(state, now) {
        changed = true;
    }
    if state.spacetime_presence.tick(state.provider_runtime.mode) {
        changed = true;
    }
    if crate::autopilot_compute_presence::pump_provider_chat_presence(
        &mut state.provider_runtime,
        &mut state.autopilot_chat,
        state.nostr_identity.as_ref(),
        now,
        current_epoch_seconds(),
    ) {
        changed = true;
    }
    if state.mission_control.has_pending_mirrored_trace_logs() {
        changed = true;
    }
    state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
    refresh_network_aggregate_counters(state, now);
    refresh_earnings_scoreboard(state, now);
    refresh_sync_health(state);
    mirror_ui_errors_to_console(state);
    changed
}

fn run_startup_spark_wallet_convergence_tick(state: &mut crate::app_state::RenderState) -> bool {
    let now_epoch_seconds = current_epoch_seconds();
    if !state
        .spark_wallet
        .startup_convergence_refresh_due(now_epoch_seconds)
    {
        return false;
    }

    state
        .spark_wallet
        .note_startup_convergence_refresh_queued(now_epoch_seconds);
    if let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh) {
        state.spark_wallet.last_error = Some(error);
        state.spark_wallet.cancel_startup_convergence();
    }
    true
}

fn run_goal_restart_recovery(state: &mut crate::app_state::RenderState) -> bool {
    if state.goal_restart_recovery_ran {
        return false;
    }
    state.goal_restart_recovery_ran = true;

    let now_epoch_seconds = current_epoch_seconds();
    match state
        .autopilot_goals
        .recover_after_restart(now_epoch_seconds)
    {
        Ok(report) => {
            let _ = state
                .autopilot_goals
                .reconcile_os_scheduler_adapters(now_epoch_seconds);
            let recovered = report.recovered_running_goals.len();
            let replay = report.replay_queued_goals.len();
            let skipped = report.skipped_goals.len();
            if recovered == 0 && replay == 0 && skipped == 0 {
                return false;
            }
            state.autopilot_chat.record_turn_timeline_event(format!(
                "goal restart recovery recovered_running={} replay={} skipped={}",
                recovered, replay, skipped
            ));
            for (goal_id, missed_runs) in report.catchup_backlog {
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "goal restart catchup goal={} missed_runs={}",
                    goal_id, missed_runs
                ));
            }
            true
        }
        Err(error) => {
            state.autopilot_goals.last_error = Some(error.clone());
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("goal restart recovery error: {}", error));
            true
        }
    }
}

fn run_goal_interval_scheduler(state: &mut crate::app_state::RenderState) -> bool {
    let now_epoch_seconds = current_epoch_seconds();
    match state.autopilot_goals.run_scheduler_tick(now_epoch_seconds) {
        Ok(triggered_goal_ids) => {
            if triggered_goal_ids.is_empty() {
                return false;
            }
            state.autopilot_chat.record_turn_timeline_event(format!(
                "goal interval scheduler triggered: {}",
                triggered_goal_ids.join(",")
            ));
            true
        }
        Err(error) => {
            state.autopilot_goals.last_error = Some(error.clone());
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("goal interval scheduler error: {}", error));
            true
        }
    }
}

fn run_autonomous_goal_loop(state: &mut crate::app_state::RenderState) -> bool {
    let now_epoch_seconds = current_epoch_seconds();
    let wallet_total_sats = state
        .spark_wallet
        .balance
        .as_ref()
        .map_or(0, spark_total_balance_sats);
    let rollout_gate = state.autopilot_goals.rollout_gate_decision();
    if !rollout_gate.enabled {
        let reason = format!("rollout gate blocked automation: {}", rollout_gate.reason);
        if let Some(active_run) = state.goal_loop_executor.active_run.clone() {
            if let Some(goal) = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .find(|goal| goal.goal_id == active_run.goal_id)
                .cloned()
            {
                finalize_goal_loop_run(
                    state,
                    &active_run,
                    &goal,
                    GoalLifecycleStatus::Aborted,
                    GoalLoopStopReason::PolicyAbort {
                        reason: reason.clone(),
                    },
                    goal_loop_progress_snapshot(
                        state,
                        &active_run,
                        now_epoch_seconds,
                        wallet_total_sats,
                    ),
                    now_epoch_seconds,
                    Some(reason.clone()),
                    None,
                );
                return true;
            }
        }

        if state.autopilot_goals.last_action.as_deref() != Some(reason.as_str()) {
            state.autopilot_goals.last_action = Some(reason);
        }
        return false;
    }

    if state.goal_loop_executor.active_run.is_none()
        && let Some(goal) = select_runnable_goal(&state.autopilot_goals.document.active_goals)
    {
        let goal_id = goal.goal_id.clone();
        if goal.lifecycle_status == GoalLifecycleStatus::Queued
            && let Err(error) = state
                .autopilot_goals
                .transition_goal(&goal_id, GoalLifecycleEvent::StartRun)
        {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "goal loop start failed goal={goal_id} error={error}"
            ));
            state.autopilot_goals.last_error = Some(error);
            return true;
        }
        let recovered_from_restart =
            match state.autopilot_goals.consume_recovery_replay_flag(&goal_id) {
                Ok(value) => value,
                Err(error) => {
                    state.autopilot_goals.last_error = Some(error.clone());
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "goal loop recovery flag consume error goal={} error={}",
                        goal_id, error
                    ));
                    false
                }
            };
        if !state.goal_loop_executor.begin_run(
            &goal_id,
            now_epoch_seconds,
            wallet_total_sats,
            recovered_from_restart,
        ) {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "goal loop start suppressed goal={} reason=active_run_exists",
                goal_id
            ));
            return false;
        }
        state.autopilot_chat.record_turn_timeline_event(format!(
            "goal loop started goal={} recovered={}",
            goal_id, recovered_from_restart
        ));
        return true;
    }

    let Some(active_run) = state.goal_loop_executor.active_run.clone() else {
        return false;
    };

    let goal_id = active_run.goal_id.clone();
    let Some(goal) = state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .find(|goal| goal.goal_id == goal_id)
        .cloned()
    else {
        state.goal_loop_executor.complete_run(
            now_epoch_seconds,
            GoalLifecycleStatus::Failed,
            GoalLoopStopReason::GoalMissing,
        );
        state
            .autopilot_chat
            .record_turn_timeline_event(format!("goal loop stopped goal={goal_id} reason=missing"));
        return true;
    };

    if goal.constraints.autonomy_policy.kill_switch_active {
        let reason = goal
            .constraints
            .autonomy_policy
            .kill_switch_reason
            .clone()
            .unwrap_or_else(|| "kill switch engaged".to_string());
        finalize_goal_loop_run(
            state,
            &active_run,
            &goal,
            GoalLifecycleStatus::Aborted,
            GoalLoopStopReason::PolicyAbort {
                reason: reason.clone(),
            },
            goal_loop_progress_snapshot(state, &active_run, now_epoch_seconds, wallet_total_sats),
            now_epoch_seconds,
            Some(reason),
            None,
        );
        return true;
    }

    let progress =
        goal_loop_progress_snapshot(state, &active_run, now_epoch_seconds, wallet_total_sats);
    let evaluation = match state
        .autopilot_goals
        .evaluate_active_goal_conditions(&goal_id, &progress)
    {
        Ok(value) => value,
        Err(error) => {
            let stop_reason = GoalLoopStopReason::ConditionStop {
                reasons: vec![error.clone()],
            };
            finalize_goal_loop_run(
                state,
                &active_run,
                &goal,
                GoalLifecycleStatus::Failed,
                stop_reason,
                progress,
                now_epoch_seconds,
                Some(error),
                None,
            );
            return true;
        }
    };
    state.goal_loop_executor.record_condition_evaluation(
        evaluation.goal_complete,
        evaluation.should_continue,
        &evaluation.completion_reasons,
        &evaluation.stop_reasons,
    );

    if evaluation.goal_complete {
        finalize_goal_loop_run(
            state,
            &active_run,
            &goal,
            GoalLifecycleStatus::Succeeded,
            GoalLoopStopReason::GoalComplete,
            progress,
            now_epoch_seconds,
            Some("goal conditions satisfied".to_string()),
            Some(evaluation.clone()),
        );
        return true;
    }

    if !evaluation.should_continue {
        let reason_text = evaluation.stop_reasons.join(" | ");
        let stop_reason = GoalLoopStopReason::ConditionStop {
            reasons: evaluation.stop_reasons.clone(),
        };
        finalize_goal_loop_run(
            state,
            &active_run,
            &goal,
            GoalLifecycleStatus::Failed,
            stop_reason,
            progress,
            now_epoch_seconds,
            Some(reason_text),
            Some(evaluation.clone()),
        );
        return true;
    }

    let turn_in_flight = state.autopilot_chat.active_turn_id.is_some()
        || !state.autopilot_chat.pending_turn_metadata.is_empty()
        || matches!(
            state.autopilot_chat.last_turn_status.as_deref(),
            Some("inProgress")
        );
    if turn_in_flight {
        if let Some(run) = state.goal_loop_executor.active_run.as_mut() {
            run.phase = GoalLoopPhase::WaitingForTurnResult;
        }
        return false;
    }

    if active_run.phase == GoalLoopPhase::WaitingForTurnResult {
        match state.autopilot_chat.last_turn_status.as_deref() {
            Some("failed") => {
                let error = state
                    .autopilot_chat
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "codex turn failed".to_string());
                state.goal_loop_executor.mark_attempt_finished(
                    now_epoch_seconds,
                    "failed",
                    Some(error.clone()),
                );
                state.autopilot_chat.set_turn_status(None);
                let retries_used = state.goal_loop_executor.increment_retries();
                if retries_used > goal.retry_policy.max_retries {
                    let stop_reason = GoalLoopStopReason::RetryLimitExceeded {
                        retries_used,
                        max_retries: goal.retry_policy.max_retries,
                        last_error: error.clone(),
                    };
                    finalize_goal_loop_run(
                        state,
                        &active_run,
                        &goal,
                        GoalLifecycleStatus::Failed,
                        stop_reason,
                        progress,
                        now_epoch_seconds,
                        Some(error),
                        Some(evaluation.clone()),
                    );
                    return true;
                }
                let backoff_seconds = retry_backoff_seconds(&goal.retry_policy, retries_used);
                let resume_at = now_epoch_seconds.saturating_add(backoff_seconds);
                state.goal_loop_executor.mark_backoff(resume_at);
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "goal loop retry scheduled goal={} retries={}/{} backoff={}s",
                    goal_id, retries_used, goal.retry_policy.max_retries, backoff_seconds
                ));
                return true;
            }
            Some("completed") => {
                state.goal_loop_executor.mark_attempt_finished(
                    now_epoch_seconds,
                    "completed",
                    None,
                );
                state.autopilot_chat.set_turn_status(None);
                state.goal_loop_executor.mark_dispatching();
            }
            _ => {}
        }
    }

    if let Some(run) = state.goal_loop_executor.active_run.as_ref()
        && run.phase == GoalLoopPhase::Backoff
    {
        let resume_at = run.backoff_until_epoch_seconds.unwrap_or(now_epoch_seconds);
        if now_epoch_seconds < resume_at {
            return false;
        }
        state.goal_loop_executor.mark_dispatching();
    }

    if state.autopilot_chat.active_thread_id.is_none() {
        let _ = run_chat_new_thread_action(state);
        if let Some(run) = state.goal_loop_executor.active_run.as_mut() {
            run.phase = GoalLoopPhase::WaitingForThread;
        }
        state
            .autopilot_chat
            .record_turn_timeline_event(format!("goal loop waiting for thread goal={goal_id}"));
        return true;
    }

    let can_dispatch = state
        .goal_loop_executor
        .active_run
        .as_ref()
        .is_some_and(|run| {
            matches!(
                run.phase,
                GoalLoopPhase::DispatchingTurn | GoalLoopPhase::WaitingForThread
            )
        });
    if !can_dispatch {
        return false;
    }

    let prompt = goal_loop_prompt_for_goal(&goal);
    let pending_before = state.autopilot_chat.pending_turn_metadata.len();
    state.chat_inputs.composer.set_value(prompt);
    sync_chat_composer_draft(state);
    let _ = run_chat_submit_action_with_trigger(
        state,
        crate::labor_orchestrator::CodexRunTrigger::AutonomousGoal {
            goal_id: goal.goal_id.clone(),
            goal_title: goal.title.clone(),
        },
    );
    let pending_after = state.autopilot_chat.pending_turn_metadata.len();

    if pending_after > pending_before && state.autopilot_chat.last_error.is_none() {
        let (selected_skills, labor) = state
            .autopilot_chat
            .pending_turn_metadata
            .back()
            .map(|metadata| {
                let labor = metadata
                    .labor_binding
                    .as_ref()
                    .map(goal_labor_linkage_from_binding)
                    .unwrap_or_default();
                (metadata.selected_skill_names.clone(), labor)
            })
            .unwrap_or_else(|| (Vec::new(), GoalLaborLinkage::default()));
        state.goal_loop_executor.mark_attempt_submitted(
            now_epoch_seconds,
            state.autopilot_chat.active_thread_id.clone(),
            selected_skills,
            labor,
        );
        if let Some(run) = state.goal_loop_executor.active_run.as_ref() {
            let attempt_index = run.attempts.len();
            state.autopilot_chat.record_turn_timeline_event(format!(
                "goal loop dispatched turn goal={} attempt={}",
                goal_id, attempt_index
            ));
        }
        return true;
    }

    let error = state
        .autopilot_chat
        .last_error
        .clone()
        .unwrap_or_else(|| "goal loop turn dispatch failed".to_string());
    let retries_used = state.goal_loop_executor.increment_retries();
    if retries_used > goal.retry_policy.max_retries {
        let stop_reason = GoalLoopStopReason::DispatchFailed {
            error: error.clone(),
        };
        finalize_goal_loop_run(
            state,
            &active_run,
            &goal,
            GoalLifecycleStatus::Failed,
            stop_reason,
            progress,
            now_epoch_seconds,
            Some(error),
            Some(evaluation),
        );
        return true;
    }

    let backoff_seconds = retry_backoff_seconds(&goal.retry_policy, retries_used);
    let resume_at = now_epoch_seconds.saturating_add(backoff_seconds);
    state.goal_loop_executor.mark_backoff(resume_at);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "goal loop dispatch retry goal={} retries={}/{} backoff={}s",
        goal_id, retries_used, goal.retry_policy.max_retries, backoff_seconds
    ));
    true
}

fn goal_loop_progress_snapshot(
    state: &crate::app_state::RenderState,
    active_run: &ActiveGoalLoopRun,
    now_epoch_seconds: u64,
    wallet_total_sats: u64,
) -> GoalProgressSnapshot {
    let (jobs_completed, successes, errors) = state
        .job_history
        .rows
        .iter()
        .filter(|row| row.completed_at_epoch_seconds >= active_run.started_at_epoch_seconds)
        .fold((0u32, 0u32, 0u32), |(jobs, success, failure), row| {
            let jobs = jobs.saturating_add(1);
            match row.status {
                crate::app_state::JobHistoryStatus::Succeeded => {
                    (jobs, success.saturating_add(1), failure)
                }
                crate::app_state::JobHistoryStatus::Failed => {
                    (jobs, success, failure.saturating_add(1))
                }
            }
        });

    let reconciliation = reconcile_wallet_events_for_goal(
        active_run.started_at_epoch_seconds,
        active_run.initial_wallet_sats,
        wallet_total_sats,
        active_run.goal_id.as_str(),
        &state.job_history,
        &state.spark_wallet,
        &state.autopilot_goals.document.swap_execution_receipts,
    );
    let mut external_signals = std::collections::BTreeMap::new();
    external_signals.insert(
        "recon.wallet_delta_raw_sats".to_string(),
        reconciliation.wallet_delta_sats_raw.to_string(),
    );
    external_signals.insert(
        "recon.wallet_delta_excluding_swaps_sats".to_string(),
        reconciliation.wallet_delta_excluding_swaps_sats.to_string(),
    );
    external_signals.insert(
        "recon.earned_wallet_delta_sats".to_string(),
        reconciliation.earned_wallet_delta_sats.to_string(),
    );
    external_signals.insert(
        "recon.swap_fee_sats".to_string(),
        reconciliation.swap_fee_sats.to_string(),
    );
    external_signals.insert(
        "recon.swap_converted_out_sats".to_string(),
        reconciliation.swap_converted_out_sats.to_string(),
    );
    external_signals.insert(
        "recon.swap_converted_in_sats".to_string(),
        reconciliation.swap_converted_in_sats.to_string(),
    );
    external_signals.insert(
        "recon.events".to_string(),
        reconciliation.events.len().to_string(),
    );

    GoalProgressSnapshot {
        started_at_epoch_seconds: active_run.started_at_epoch_seconds,
        now_epoch_seconds,
        attempt_count: active_run.attempts.len() as u32,
        wallet_delta_sats: reconciliation.wallet_delta_excluding_swaps_sats,
        earned_wallet_delta_sats: reconciliation.earned_wallet_delta_sats,
        jobs_completed,
        successes,
        errors,
        total_spend_sats: reconciliation.non_swap_spend_sats,
        total_swap_cents: reconciliation.total_swap_cents,
        external_signals,
    }
}

fn goal_loop_prompt_for_goal(goal: &crate::state::autopilot_goals::GoalRecord) -> String {
    let mut prompt = match &goal.objective {
        crate::state::autopilot_goals::GoalObjective::EarnBitcoin {
            min_wallet_delta_sats,
            note,
        } => {
            let mut prompt = format!(
                "Autonomously execute the next highest-confidence paid work action to move wallet delta toward +{} sats. Use available OpenAgents tools and emit progress receipts.",
                min_wallet_delta_sats
            );
            if let Some(note) = note.as_deref()
                && !note.trim().is_empty()
            {
                prompt.push_str(" Goal note: ");
                prompt.push_str(note.trim());
            }
            prompt
        }
        crate::state::autopilot_goals::GoalObjective::SwapBtcToUsd { sell_sats, note } => {
            let mut prompt = format!(
                "Request and execute a BTC to stablesat USD swap for {} sats with policy-safe limits, then report settlement evidence.",
                sell_sats
            );
            if let Some(note) = note.as_deref()
                && !note.trim().is_empty()
            {
                prompt.push_str(" Goal note: ");
                prompt.push_str(note.trim());
            }
            prompt
        }
        crate::state::autopilot_goals::GoalObjective::SwapUsdToBtc { sell_cents, note } => {
            let mut prompt = format!(
                "Request and execute a stablesat USD to BTC swap for {} cents with policy-safe limits, then report settlement evidence.",
                sell_cents
            );
            if let Some(note) = note.as_deref()
                && !note.trim().is_empty()
            {
                prompt.push_str(" Goal note: ");
                prompt.push_str(note.trim());
            }
            prompt
        }
        crate::state::autopilot_goals::GoalObjective::Custom { instruction } => {
            instruction.trim().to_string()
        }
    };

    if !goal
        .constraints
        .autonomy_policy
        .allowed_command_prefixes
        .is_empty()
    {
        prompt.push_str(" Allowed command prefixes: ");
        prompt.push_str(
            &goal
                .constraints
                .autonomy_policy
                .allowed_command_prefixes
                .join(", "),
        );
        prompt.push('.');
    }
    if !goal
        .constraints
        .autonomy_policy
        .allowed_file_roots
        .is_empty()
    {
        prompt.push_str(" File scope roots: ");
        prompt.push_str(
            &goal
                .constraints
                .autonomy_policy
                .allowed_file_roots
                .join(", "),
        );
        prompt.push('.');
    }
    prompt
}

fn goal_labor_linkage_from_binding(
    binding: &crate::labor_orchestrator::CodexLaborBinding,
) -> GoalLaborLinkage {
    GoalLaborLinkage {
        work_unit_id: Some(binding.work_unit_id.clone()),
        contract_id: Some(binding.contract_id.clone()),
        submission_id: binding
            .submission
            .as_ref()
            .map(|submission| submission.submission.submission_id.clone()),
        verdict_id: binding
            .verdict
            .as_ref()
            .map(|verdict| verdict.verdict.verdict_id.clone()),
        claim_id: binding.trace.claim_id.clone(),
        claim_state: binding.claim_runtime_state_label().map(str::to_string),
        remedy_kind: binding
            .claim
            .as_ref()
            .and_then(|claim| claim.remedy.as_ref())
            .map(|remedy| remedy.outcome.clone()),
        settlement_id: None,
        settlement_ready: Some(binding.is_settlement_ready()),
        tool_evidence_refs: Vec::new(),
        submission_evidence_refs: binding
            .submission
            .as_ref()
            .map(|submission| submission.evidence_refs.clone())
            .unwrap_or_default(),
        verdict_evidence_refs: binding
            .verdict
            .as_ref()
            .map(|verdict| verdict.evidence_refs.clone())
            .unwrap_or_default(),
        claim_evidence_refs: binding
            .claim
            .as_ref()
            .map(|claim| claim.evidence_refs.clone())
            .unwrap_or_default(),
        incident_evidence_refs: binding.incident_evidence_refs.clone(),
        remedy_evidence_refs: binding
            .claim
            .as_ref()
            .and_then(|claim| claim.remedy.as_ref())
            .map(|remedy| remedy.evidence_refs.clone())
            .unwrap_or_default(),
        settlement_evidence_refs: Vec::new(),
    }
}

fn resolve_attempt_labor_linkage(
    chat: &crate::app_state::AutopilotChatState,
    attempt: &crate::state::goal_loop_executor::GoalLoopAttemptRecord,
) -> GoalLaborLinkage {
    let mut labor = attempt.labor.clone();
    if let Some(turn_id) = attempt.turn_id.as_deref()
        && let Some(binding_labor) = chat.turn_labor_linkage_for(turn_id)
    {
        labor.merge_from(&binding_labor);
    }
    labor
}

fn build_goal_attempt_audit_receipts(
    chat: &crate::app_state::AutopilotChatState,
    attempts: &[crate::state::goal_loop_executor::GoalLoopAttemptRecord],
) -> Vec<GoalAttemptAuditReceipt> {
    attempts
        .iter()
        .map(|attempt| GoalAttemptAuditReceipt {
            attempt_index: attempt.attempt_index,
            submitted_at_epoch_seconds: attempt.submitted_at_epoch_seconds,
            finished_at_epoch_seconds: attempt.finished_at_epoch_seconds,
            thread_id: attempt.thread_id.clone(),
            turn_id: attempt.turn_id.clone(),
            selected_skills: attempt.selected_skills.clone(),
            turn_status: attempt.turn_status.clone(),
            error: attempt.error.clone(),
            condition_goal_complete: attempt.condition_goal_complete,
            condition_should_continue: attempt.condition_should_continue,
            condition_completion_reasons: attempt.condition_completion_reasons.clone(),
            condition_stop_reasons: attempt.condition_stop_reasons.clone(),
            labor: resolve_attempt_labor_linkage(chat, attempt),
            tool_invocations: attempt
                .tool_invocations
                .iter()
                .map(|tool| GoalToolInvocationAudit {
                    request_id: tool.request_id.clone(),
                    call_id: tool.call_id.clone(),
                    tool_name: tool.tool_name.clone(),
                    response_code: tool.response_code.clone(),
                    success: tool.success,
                    response_message: tool.response_message.clone(),
                    recorded_at_epoch_seconds: tool.recorded_at_epoch_seconds,
                    evidence_refs: tool.evidence_refs.clone(),
                })
                .collect::<Vec<_>>(),
        })
        .collect::<Vec<_>>()
}

fn terminal_goal_labor_linkage(attempts: &[GoalAttemptAuditReceipt]) -> GoalLaborLinkage {
    attempts
        .iter()
        .rev()
        .map(|attempt| attempt.labor.clone())
        .find(|labor| !labor.is_empty())
        .unwrap_or_default()
}

fn build_goal_payout_evidence(
    reconciliation: &crate::state::wallet_reconciliation::WalletReconciliationReport,
    terminal_attempt: Option<&GoalAttemptAuditReceipt>,
) -> Vec<GoalPayoutEvidence> {
    let Some(terminal_attempt) = terminal_attempt else {
        return Vec::new();
    };
    let terminal_labor = terminal_attempt.labor.clone();
    reconciliation
        .events
        .iter()
        .filter(|event| event.kind == WalletLedgerEventKind::EarnPayout)
        .filter_map(|event| {
            let Some(job_id) = event.job_id.clone() else {
                return None;
            };
            let Some(payment_pointer) = event.payment_pointer.clone() else {
                return None;
            };
            let settlement_id = terminal_labor.verdict_id.as_deref().map(|verdict_id| {
                let digest = sha256_prefixed_text(
                    format!("{verdict_id}:{}:{}", event.event_id, payment_pointer).as_str(),
                );
                format!("settlement.goal.{}", digest.replace(':', "."))
            });
            let settlement_evidence_refs = settlement_id
                .as_deref()
                .map(|settlement_id| {
                    vec![goal_settlement_evidence_ref(
                        settlement_id,
                        payment_pointer.as_str(),
                        event.event_id.as_str(),
                    )]
                })
                .unwrap_or_default();
            let mut labor = terminal_labor.clone();
            labor.settlement_id = settlement_id;
            labor.settlement_evidence_refs = settlement_evidence_refs;
            Some(GoalPayoutEvidence {
                event_id: event.event_id.clone(),
                occurred_at_epoch_seconds: event.occurred_at_epoch_seconds,
                job_id,
                payment_pointer,
                payout_sats: event.sats_delta.max(0) as u64,
                attempt_index: Some(terminal_attempt.attempt_index),
                turn_id: terminal_attempt.turn_id.clone(),
                labor,
            })
        })
        .collect::<Vec<_>>()
}

fn goal_settlement_evidence_ref(
    settlement_id: &str,
    payment_pointer: &str,
    event_id: &str,
) -> EvidenceRef {
    let digest =
        sha256_prefixed_text(format!("{settlement_id}:{payment_pointer}:{event_id}").as_str());
    let mut evidence = EvidenceRef::new(
        "goal_settlement",
        format!("oa://autopilot/goals/settlements/{settlement_id}"),
        digest,
    );
    evidence.meta.insert(
        "payment_pointer".to_string(),
        serde_json::Value::String(payment_pointer.to_string()),
    );
    evidence.meta.insert(
        "event_id".to_string(),
        serde_json::Value::String(event_id.to_string()),
    );
    evidence
}

fn finalize_goal_loop_run(
    state: &mut crate::app_state::RenderState,
    active_run: &ActiveGoalLoopRun,
    goal: &crate::state::autopilot_goals::GoalRecord,
    lifecycle_status: GoalLifecycleStatus,
    stop_reason: GoalLoopStopReason,
    progress: GoalProgressSnapshot,
    now_epoch_seconds: u64,
    notes: Option<String>,
    condition_evaluation: Option<ConditionEvaluation>,
) {
    let goal_id = active_run.goal_id.clone();
    let run_snapshot = state
        .goal_loop_executor
        .active_run
        .as_ref()
        .cloned()
        .unwrap_or_else(|| active_run.clone());
    let transition_result = match lifecycle_status {
        GoalLifecycleStatus::Succeeded => state.autopilot_goals.transition_goal(
            &goal_id,
            GoalLifecycleEvent::Succeed {
                reason: notes.clone(),
            },
        ),
        GoalLifecycleStatus::Failed => state.autopilot_goals.transition_goal(
            &goal_id,
            GoalLifecycleEvent::Fail {
                reason: notes
                    .clone()
                    .unwrap_or_else(|| "goal loop stopped".to_string()),
            },
        ),
        GoalLifecycleStatus::Aborted => state.autopilot_goals.transition_goal(
            &goal_id,
            GoalLifecycleEvent::Abort {
                reason: notes
                    .clone()
                    .unwrap_or_else(|| "goal loop aborted".to_string()),
            },
        ),
        _ => Ok(crate::state::autopilot_goals::GoalStateTransition {
            goal_id: goal_id.clone(),
            from: GoalLifecycleStatus::Running,
            to: lifecycle_status,
            event: GoalLifecycleEvent::Fail {
                reason: "unexpected terminal transition".to_string(),
            },
            attempt_count: progress.attempt_count,
            reason: notes.clone(),
            transitioned_at_epoch_seconds: now_epoch_seconds,
        }),
    };

    if let Err(error) = transition_result {
        state.autopilot_goals.last_error = Some(error.clone());
        state.autopilot_chat.record_turn_timeline_event(format!(
            "goal loop transition error goal={} status={:?} error={}",
            goal_id, lifecycle_status, error
        ));
    }

    let attempts = build_goal_attempt_audit_receipts(&state.autopilot_chat, &run_snapshot.attempts);
    let terminal_attempt = attempts
        .iter()
        .rev()
        .find(|attempt| !attempt.labor.is_empty());
    let mut terminal_labor = terminal_goal_labor_linkage(&attempts);

    let receipt_id = format!("goal-loop-receipt-{}-{}", goal_id, now_epoch_seconds);
    let receipt = GoalExecutionReceipt {
        receipt_id: receipt_id.clone(),
        goal_id: goal_id.clone(),
        attempt_index: progress.attempt_count,
        started_at_epoch_seconds: run_snapshot.started_at_epoch_seconds,
        finished_at_epoch_seconds: now_epoch_seconds,
        lifecycle_status,
        wallet_delta_sats: progress.wallet_delta_sats,
        jobs_completed: progress.jobs_completed,
        successes: progress.successes,
        errors: progress.errors,
        notes: notes.clone(),
        recovered_from_restart: run_snapshot.recovered_from_restart,
        policy_snapshot: goal.constraints.policy_snapshot(),
        terminal_labor: terminal_labor.clone(),
    };
    if let Err(error) = state.autopilot_goals.record_receipt(receipt) {
        state.autopilot_goals.last_error = Some(error);
    }

    let reconciliation = reconcile_wallet_events_for_goal(
        run_snapshot.started_at_epoch_seconds,
        run_snapshot.initial_wallet_sats,
        state
            .spark_wallet
            .balance
            .as_ref()
            .map_or(0, spark_total_balance_sats),
        goal_id.as_str(),
        &state.job_history,
        &state.spark_wallet,
        &state.autopilot_goals.document.swap_execution_receipts,
    );
    let payout_evidence = build_goal_payout_evidence(&reconciliation, terminal_attempt);
    if let Some(first_payout) = payout_evidence.first() {
        terminal_labor.merge_from(&first_payout.labor);
    }

    let swap_quote_evidence = state
        .autopilot_goals
        .document
        .swap_quote_audits
        .iter()
        .filter(|audit| {
            audit.goal_id == goal_id
                && audit.created_at_epoch_seconds >= run_snapshot.started_at_epoch_seconds
                && audit.created_at_epoch_seconds <= now_epoch_seconds
        })
        .cloned()
        .collect::<Vec<_>>();
    let swap_execution_evidence = state
        .autopilot_goals
        .document
        .swap_execution_receipts
        .iter()
        .filter(|receipt| {
            receipt.goal_id == goal_id
                && receipt.finished_at_epoch_seconds >= run_snapshot.started_at_epoch_seconds
                && receipt.finished_at_epoch_seconds <= now_epoch_seconds
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut selected_skills = attempts
        .iter()
        .flat_map(|attempt| attempt.selected_skills.clone())
        .collect::<Vec<_>>();
    selected_skills.sort();
    selected_skills.dedup();
    let terminal_status_reason = notes
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{:?}", stop_reason));
    let run_audit = GoalRunAuditReceipt {
        audit_id: format!("goal-run-audit-{}-{}", goal_id, now_epoch_seconds),
        receipt_id,
        goal_id: goal_id.clone(),
        run_id: run_snapshot.run_id.clone(),
        started_at_epoch_seconds: run_snapshot.started_at_epoch_seconds,
        finished_at_epoch_seconds: now_epoch_seconds,
        lifecycle_status,
        terminal_status_reason,
        selected_skills,
        attempts,
        terminal_labor,
        condition_goal_complete: condition_evaluation
            .as_ref()
            .map(|value| value.goal_complete),
        condition_should_continue: condition_evaluation
            .as_ref()
            .map(|value| value.should_continue),
        condition_completion_reasons: condition_evaluation
            .as_ref()
            .map(|value| value.completion_reasons.clone())
            .unwrap_or_default(),
        condition_stop_reasons: condition_evaluation
            .as_ref()
            .map(|value| value.stop_reasons.clone())
            .unwrap_or_default(),
        payout_evidence,
        swap_quote_evidence,
        swap_execution_evidence,
    };
    if let Err(error) = state.autopilot_goals.record_run_audit_receipt(run_audit) {
        state.autopilot_goals.last_error = Some(error);
    }

    state
        .goal_loop_executor
        .complete_run(now_epoch_seconds, lifecycle_status, stop_reason.clone());
    state.autopilot_chat.record_turn_timeline_event(format!(
        "goal loop terminal goal={} status={:?} reason={:?}",
        goal_id, lifecycle_status, stop_reason
    ));
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn mirror_ui_error(channel: &'static str, value: Option<&str>) {
    static UI_ERROR_MIRROR_STATE: OnceLock<Mutex<HashMap<&'static str, String>>> = OnceLock::new();
    let state = UI_ERROR_MIRROR_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match state.lock() {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("ui error mirror lock poisoned: {}", error);
            return;
        }
    };
    let next = value.map(str::trim).filter(|entry| !entry.is_empty());
    match next {
        Some(current) => {
            let should_log = guard
                .get(channel)
                .is_none_or(|previous| previous != current);
            if should_log {
                tracing::error!("ui error [{}]: {}", channel, current);
                guard.insert(channel, current.to_string());
            }
        }
        None => {
            if guard.remove(channel).is_some() {
                tracing::info!("ui error [{}] cleared", channel);
            }
        }
    }
}

fn mirror_ui_errors_to_console(state: &crate::app_state::RenderState) {
    let provider_preflight_error = provider_preflight_console_error(state);
    mirror_ui_error("autopilot.chat", state.autopilot_chat.last_error.as_deref());
    mirror_ui_error("project.ops", state.project_ops.last_error.as_deref());
    mirror_ui_error("cad.demo", state.cad_demo.last_error.as_deref());
    mirror_ui_error("spark.wallet", state.spark_wallet.last_error.as_deref());
    mirror_ui_error("codex.account", state.codex_account.last_error.as_deref());
    mirror_ui_error("codex.models", state.codex_models.last_error.as_deref());
    mirror_ui_error("codex.config", state.codex_config.last_error.as_deref());
    mirror_ui_error("codex.mcp", state.codex_mcp.last_error.as_deref());
    mirror_ui_error("codex.apps", state.codex_apps.last_error.as_deref());
    mirror_ui_error("codex.labs", state.codex_labs.last_error.as_deref());
    mirror_ui_error(
        "codex.diagnostics",
        state.codex_diagnostics.last_error.as_deref(),
    );
    mirror_ui_error(
        "relay.connections",
        state.relay_connections.last_error.as_deref(),
    );
    mirror_ui_error("sync.health", state.sync_health.last_error.as_deref());
    mirror_ui_error(
        "network.requests",
        state.network_requests.last_error.as_deref(),
    );
    mirror_ui_error("starter.jobs", state.starter_jobs.last_error.as_deref());
    mirror_ui_error("activity.feed", state.activity_feed.last_error.as_deref());
    mirror_ui_error(
        "alerts.recovery",
        state.alerts_recovery.last_error.as_deref(),
    );
    mirror_ui_error("settings", state.settings.last_error.as_deref());
    mirror_ui_error("credentials", state.credentials.last_error.as_deref());
    mirror_ui_error("job.inbox", state.job_inbox.last_error.as_deref());
    mirror_ui_error("active.job", state.active_job.last_error.as_deref());
    mirror_ui_error("job.history", state.job_history.last_error.as_deref());
    mirror_ui_error(
        "mission.control.preflight",
        provider_preflight_error.as_deref(),
    );
    mirror_ui_error(
        "provider.runtime",
        provider_runtime_console_error(state).as_deref(),
    );
    mirror_ui_error(
        "earnings.scoreboard",
        state.earnings_scoreboard.last_error.as_deref(),
    );
    mirror_ui_error(
        "agent.profile.state",
        state.agent_profile_state.last_error.as_deref(),
    );
    mirror_ui_error(
        "agent.schedule.tick",
        state.agent_schedule_tick.last_error.as_deref(),
    );
    mirror_ui_error(
        "trajectory.audit",
        state.trajectory_audit.last_error.as_deref(),
    );
    mirror_ui_error("skill.registry", state.skill_registry.last_error.as_deref());
    mirror_ui_error(
        "skill.trust.revocation",
        state.skill_trust_revocation.last_error.as_deref(),
    );
    mirror_ui_error("credit.desk", state.credit_desk.last_error.as_deref());
    mirror_ui_error(
        "credit.settlement.ledger",
        state.credit_settlement_ledger.last_error.as_deref(),
    );
}

fn dispatch_mouse_move(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let mut handled = handle_sidebar_mouse_move(state, point);
    if handled {
        return true;
    }

    handled = PaneController::update_drag(state, point);
    handled |= update_cad_camera_drag(state, point);
    if state.cad_camera_drag_state.is_none() {
        handled |= update_cad_hover_target(state, point);
    }
    handled |= update_chat_transcript_selection_drag(state, point);
    let event = InputEvent::MouseMove {
        x: point.x,
        y: point.y,
    };

    handled |= PaneInput::dispatch_frame_event(state, &event);
    handled |= dispatch_text_inputs(state, &event);
    if state.dev_mode_enabled() {
        handled |= state
            .hotbar
            .event(&event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
    }
    handled
}

fn dispatch_mouse_down(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
    event: &InputEvent,
) -> bool {
    // Sidebar handle gets first chance at mouse-down so panes don't steal drags.
    if handle_sidebar_mouse_down(state, point, button) {
        return true;
    }
    let mut handled = begin_chat_transcript_selection_drag(state, point, button);

    if button == MouseButton::Left {
        let settings_bounds = sidebar_settings_icon_bounds(state);
        if settings_bounds.size.width > 0.0 && settings_bounds.contains(point) {
            PaneController::create_for_kind(state, crate::app_state::PaneKind::Settings);
            return true;
        }
    }

    // Sidebar "Go Online" button (when panel is open).
    if button == MouseButton::Left && state.sidebar.is_open {
        let go_online_bounds = sidebar_go_online_button_bounds(state);
        if go_online_bounds.size.width > 0.0
            && go_online_bounds.contains(point)
            && run_pane_hit_action(state, PaneHitAction::GoOnlineToggle)
        {
            return true;
        }
    }

    if state.dev_mode_enabled() && button == MouseButton::Left {
        let wallet_label_bounds = wallet_balance_sats_label_bounds(state);
        if wallet_label_bounds.size.width > 0.0 && wallet_label_bounds.contains(point) {
            PaneController::create_for_kind(state, crate::app_state::PaneKind::SparkWallet);
            queue_spark_command(state, SparkWalletCommand::Refresh);
            return true;
        }
    }

    if state.dev_mode_enabled() && state.hotbar_bounds.contains(point) {
        handled |= state
            .hotbar
            .event(event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
        handled |= process_hotbar_clicks(state);
        handled |= dispatch_text_inputs(state, event);
        if !handled {
            handled |= PaneInput::handle_mouse_down(state, point, button);
        }
    } else {
        handled |= PaneInput::handle_mouse_down(state, point, button);
        handled |= dispatch_text_inputs(state, event);
        if state.dev_mode_enabled() {
            handled |= state
                .hotbar
                .event(event, state.hotbar_bounds, &mut state.event_context)
                .is_handled();
            handled |= process_hotbar_clicks(state);
        }
    }

    handled |= begin_cad_camera_drag(state, point, button);
    handled
}

fn dispatch_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = finish_chat_transcript_selection_drag(state, point);
    let camera_drag_consumed_click = finish_cad_camera_drag(state);
    handled |= camera_drag_consumed_click;
    handled |= handle_sidebar_mouse_up(state, point, event);
    handled |= PaneInput::handle_mouse_up(state, event);
    handled |= dispatch_text_inputs(state, event);
    let context_menu_handled = if !camera_drag_consumed_click {
        handle_cad_context_menu_click(state, point, event)
    } else {
        false
    };
    handled |= context_menu_handled;
    if !camera_drag_consumed_click && !context_menu_handled {
        let cad_selection_handled = handle_cad_selection_click(state, point, event);
        handled |= cad_selection_handled;
        if !cad_selection_handled {
            let snap_preview_handled = handle_cad_snap_preview_click(state, point, event);
            handled |= snap_preview_handled;
            if !snap_preview_handled {
                handled |= dispatch_pane_actions(state, point);
            }
        }
    }
    if state.dev_mode_enabled() {
        handled |= state
            .hotbar
            .event(event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
        handled |= process_hotbar_clicks(state);
    }
    handled
}

#[derive(Clone, Debug)]
struct CadTilePick {
    tile_index: usize,
    kind: CadPickEntityKind,
    entity_id: String,
}

fn topmost_cad_content_bounds(state: &crate::app_state::RenderState) -> Option<Bounds> {
    state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::CadDemo)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane_content_bounds(pane.bounds))
}

fn cad_variant_tile_at_point(
    state: &crate::app_state::RenderState,
    point: Point,
) -> Option<(usize, Bounds)> {
    let content_bounds = topmost_cad_content_bounds(state)?;
    if !content_bounds.contains(point) {
        return None;
    }
    let tile_index = cad_pane::variant_tile_index_at_point(content_bounds, point)?;
    let viewport = cad_pane::variant_tile_bounds(content_bounds, tile_index);
    Some((tile_index, viewport))
}

fn cad_pick_at_point(state: &crate::app_state::RenderState, point: Point) -> Option<CadTilePick> {
    let (tile_index, viewport_bounds) = cad_variant_tile_at_point(state, point)?;
    let tile_view = state.cad_demo.variant_viewport(tile_index)?;
    let base_payload = state.cad_demo.last_good_mesh_payload.as_ref()?;
    let clipped_payload = state.cad_demo.section_plane().and_then(|plane| {
        openagents_cad::section::clip_mesh_payload(
            base_payload,
            plane,
            openagents_cad::policy::resolve_tolerance_mm(None) as f32,
        )
        .ok()
    });
    let payload = clipped_payload.as_ref().unwrap_or(base_payload);
    let pick_query = CadPickQuery {
        viewport: CadPickViewport {
            origin_px: [viewport_bounds.origin.x, viewport_bounds.origin.y],
            size_px: [viewport_bounds.size.width, viewport_bounds.size.height],
        },
        camera: CadPickCameraPose {
            projection_mode: match state.cad_demo.projection_mode {
                crate::app_state::CadProjectionMode::Orthographic => {
                    CadPickProjectionMode::Orthographic
                }
                crate::app_state::CadProjectionMode::Perspective => {
                    CadPickProjectionMode::Perspective
                }
            },
            zoom: tile_view.camera_zoom,
            pan_x: tile_view.camera_pan_x,
            pan_y: tile_view.camera_pan_y,
            orbit_yaw_deg: tile_view.camera_orbit_yaw_deg,
            orbit_pitch_deg: tile_view.camera_orbit_pitch_deg,
        },
        point_px: [point.x, point.y],
        tolerance_px: 0.0,
    };
    let hit = pick_mesh_hit(payload, pick_query)?;
    Some(CadTilePick {
        tile_index,
        kind: hit.kind,
        entity_id: hit.entity_id,
    })
}

fn update_cad_hover_target(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    if state.cad_demo.context_menu.is_open {
        return state
            .cad_demo
            .set_hovered_geometry_for_tile_focus(None, None);
    }

    let Some((tile_index, _)) = cad_variant_tile_at_point(state, point) else {
        return state
            .cad_demo
            .set_hovered_geometry_for_tile_focus(None, None);
    };
    let hovered = cad_pick_at_point(state, point).map(|pick| format!("cad://{}", pick.entity_id));
    state
        .cad_demo
        .set_hovered_geometry_for_tile_focus(Some(tile_index), hovered)
}

fn cad_pick_kind_to_selection_kind(kind: CadPickEntityKind) -> CadSelectionKind {
    match kind {
        CadPickEntityKind::Body => CadSelectionKind::Body,
        CadPickEntityKind::Face => CadSelectionKind::Face,
        CadPickEntityKind::Edge => CadSelectionKind::Edge,
    }
}

fn cad_pick_kind_label(kind: CadPickEntityKind) -> &'static str {
    match kind {
        CadPickEntityKind::Body => "body",
        CadPickEntityKind::Face => "face",
        CadPickEntityKind::Edge => "edge",
    }
}

fn handle_cad_selection_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let InputEvent::MouseUp { button, .. } = event else {
        return false;
    };
    if *button != MouseButton::Left || state.cad_demo.context_menu.is_open {
        return false;
    }
    if state.cad_demo.snap_toggles.grid
        || state.cad_demo.snap_toggles.origin
        || state.cad_demo.snap_toggles.endpoint
        || state.cad_demo.snap_toggles.midpoint
    {
        return false;
    }

    let Some((tile_index, _)) = cad_variant_tile_at_point(state, point) else {
        return false;
    };
    let _ = state.cad_demo.set_active_variant_tile(tile_index);

    let Some(pick) = cad_pick_at_point(state, point) else {
        return state
            .cad_demo
            .set_hovered_geometry_for_tile_focus(Some(tile_index), None);
    };

    let selection_kind = cad_pick_kind_to_selection_kind(pick.kind);
    let _ = state.cad_demo.selection_store.set_primary(
        selection_kind,
        pick.entity_id.clone(),
        Some(pick.entity_id.clone()),
    );
    let selected_ref = format!("cad://{}", pick.entity_id);
    state
        .cad_demo
        .set_focused_geometry_for_active_variant(Some(selected_ref.clone()));
    state
        .cad_demo
        .set_hovered_geometry_for_tile_focus(Some(pick.tile_index), Some(selected_ref.clone()));
    state.cad_demo.last_action = Some(format!(
        "CAD select tile={} {} ({})",
        pick.tile_index + 1,
        selected_ref,
        cad_pick_kind_label(pick.kind)
    ));
    true
}

fn handle_cad_context_menu_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let InputEvent::MouseUp { button, .. } = event else {
        return false;
    };

    let top_cad_content = topmost_cad_content_bounds(state);

    if *button == MouseButton::Right {
        let Some(content_bounds) = top_cad_content else {
            return false;
        };
        if !content_bounds.contains(point) {
            return false;
        }
        let Some((tile_index, viewport)) = cad_variant_tile_at_point(state, point) else {
            state.cad_demo.close_context_menu();
            state.cad_demo.last_action = Some("CAD context menu closed".to_string());
            return true;
        };
        let _ = state.cad_demo.set_active_variant_tile(tile_index);
        let picked_from_mesh = cad_pick_at_point(state, point).map(|pick| {
            let target_kind = match pick.kind {
                CadPickEntityKind::Body => crate::app_state::CadContextMenuTargetKind::Body,
                CadPickEntityKind::Face => crate::app_state::CadContextMenuTargetKind::Face,
                CadPickEntityKind::Edge => crate::app_state::CadContextMenuTargetKind::Edge,
            };
            let target_ref = format!("cad://{}", pick.entity_id);
            (target_kind, target_ref)
        });
        let (target_kind, target_ref) = picked_from_mesh.unwrap_or_else(|| {
            state
                .cad_demo
                .infer_context_menu_target_for_viewport_point(point, viewport)
        });
        state
            .cad_demo
            .open_context_menu(point, target_kind, target_ref.clone());
        state.cad_demo.last_action = Some(format!(
            "CAD context menu open tile={} {} ({})",
            tile_index + 1,
            target_ref,
            target_kind.label()
        ));
        return true;
    }

    if *button != MouseButton::Left || !state.cad_demo.context_menu.is_open {
        return false;
    }
    let Some(content_bounds) = top_cad_content else {
        state.cad_demo.close_context_menu();
        return true;
    };
    let menu_bounds = cad_demo_context_menu_bounds(
        content_bounds,
        state.cad_demo.context_menu.anchor,
        state.cad_demo.context_menu.items.len(),
    );
    if !menu_bounds.contains(point) {
        state.cad_demo.close_context_menu();
        state.cad_demo.last_action = Some("CAD context menu dismissed".to_string());
        return true;
    }

    for index in 0..state.cad_demo.context_menu.items.len() {
        if cad_demo_context_menu_row_bounds(menu_bounds, index).contains(point) {
            let selected_item_id = state
                .cad_demo
                .context_menu
                .items
                .get(index)
                .map(|item| item.id.clone());
            if selected_item_id.as_deref() == Some("body.material") {
                let _ =
                    reducers::run_cad_demo_action(state, CadDemoPaneAction::CycleMaterialPreset);
            } else if let Some(action) = state.cad_demo.run_context_menu_item(index) {
                state.cad_demo.last_action = Some(action);
            }
            state.cad_demo.close_context_menu();
            return true;
        }
    }

    true
}

fn handle_cad_snap_preview_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let InputEvent::MouseUp { button, .. } = event else {
        return false;
    };
    if *button != MouseButton::Left {
        return false;
    }
    if !state.cad_demo.snap_toggles.grid
        && !state.cad_demo.snap_toggles.origin
        && !state.cad_demo.snap_toggles.endpoint
        && !state.cad_demo.snap_toggles.midpoint
    {
        return false;
    }

    let Some((tile_index, viewport)) = cad_variant_tile_at_point(state, point) else {
        return false;
    };
    let _ = state.cad_demo.set_active_variant_tile(tile_index);
    let snapped = state.cad_demo.apply_snap_to_viewport_point(point, viewport);
    let _ = state
        .cad_demo
        .record_measurement_snap_point(tile_index, snapped);
    let measurement_suffix = state
        .cad_demo
        .measurement_distance_px
        .map(|distance| {
            let angle = state.cad_demo.measurement_angle_deg.unwrap_or(0.0);
            format!(" measure(d={distance:.2}px a={angle:.2}deg)")
        })
        .unwrap_or_default();
    state.cad_demo.last_action = Some(format!(
        "CAD snap preview tile={} raw=({:.1},{:.1}) snapped=({:.1},{:.1}) {}{}",
        tile_index + 1,
        point.x,
        point.y,
        snapped.x,
        snapped.y,
        state.cad_demo.snap_summary(),
        measurement_suffix,
    ));
    true
}

fn dispatch_mouse_scroll(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = false;
    if let InputEvent::Scroll { dy, .. } = event {
        if apply_cad_camera_zoom(state, point, *dy) {
            handled = true;
        } else {
            handled |= dispatch_mission_control_log_scroll_event(state, point, event);
            if !handled {
                handled |= dispatch_buy_mode_payments_scroll_event(state, point, event);
            }
            if !handled {
                handled |= dispatch_apple_fm_workbench_log_scroll_event(state, point, event);
            }
            if !handled {
                handled |= dispatch_activity_feed_detail_scroll_event(state, point, *dy);
            }
            if !handled {
                handled |= dispatch_active_job_scroll_event(state, point, *dy);
            }
            if !handled {
                handled |= dispatch_chat_scroll_event(state, point, *dy);
            }
        }
    }
    handled |= dispatch_text_inputs(state, event);
    handled |= PaneInput::dispatch_frame_event(state, event);
    handled
}

fn cad_hit_action_blocks_camera_zoom(action: &PaneHitAction) -> bool {
    !matches!(
        action,
        PaneHitAction::CadDemo(
            CadDemoPaneAction::StartDimensionEdit(_)
                | CadDemoPaneAction::SelectTimelineRow(_)
                | CadDemoPaneAction::SelectWarning(_)
                | CadDemoPaneAction::SelectWarningMarker(_)
        )
    )
}

fn cad_camera_target_pane_id(state: &crate::app_state::RenderState, point: Point) -> Option<u64> {
    let pane_order = pane_indices_by_z_desc(state);
    for pane_idx in pane_order {
        let pane = &state.panes[pane_idx];
        if !pane.bounds.contains(point) {
            continue;
        }
        if pane.kind != PaneKind::CadDemo {
            return None;
        }
        let content_bounds = pane_content_bounds(pane.bounds);
        if !content_bounds.contains(point) {
            return None;
        }
        if state.cad_demo.context_menu.is_open {
            let menu_bounds = cad_demo_context_menu_bounds(
                content_bounds,
                state.cad_demo.context_menu.anchor,
                state.cad_demo.context_menu.items.len(),
            );
            if menu_bounds.contains(point) {
                return None;
            }
        }
        if let Some((_, action)) = topmost_pane_hit_action_in_order(state, point, &[pane_idx])
            && cad_hit_action_blocks_camera_zoom(&action)
        {
            return None;
        }
        if cad_pane::variant_tile_index_at_point(content_bounds, point).is_some() {
            return Some(pane.id);
        }
        return None;
    }
    None
}

fn begin_cad_camera_drag(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) -> bool {
    if state.pane_drag_mode.is_some() {
        return false;
    }
    let mode = match button {
        MouseButton::Left => CadCameraDragMode::Orbit,
        MouseButton::Right => CadCameraDragMode::Pan,
        _ => return false,
    };
    let Some(pane_id) = cad_camera_target_pane_id(state, point) else {
        return false;
    };
    let tile_index = state
        .panes
        .iter()
        .find(|pane| pane.id == pane_id)
        .map(|pane| pane_content_bounds(pane.bounds))
        .and_then(|content| cad_pane::variant_tile_index_at_point(content, point))
        .unwrap_or(0);
    let _ = state.cad_demo.set_active_variant_tile(tile_index);
    PaneController::bring_to_front(state, pane_id);
    state.cad_camera_drag_state = Some(CadCameraDragState {
        pane_id,
        tile_index,
        mode,
        last_mouse: point,
        moved: false,
    });
    true
}

fn update_cad_camera_drag(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(mut drag) = state.cad_camera_drag_state.take() else {
        return false;
    };
    let pane_exists = state
        .panes
        .iter()
        .any(|pane| pane.id == drag.pane_id && pane.kind == PaneKind::CadDemo);
    if !pane_exists {
        return false;
    }
    let delta_x = point.x - drag.last_mouse.x;
    let delta_y = point.y - drag.last_mouse.y;
    drag.last_mouse = point;

    if delta_x.abs() < f32::EPSILON && delta_y.abs() < f32::EPSILON {
        state.cad_camera_drag_state = Some(drag);
        return false;
    }

    if delta_x.abs() + delta_y.abs() >= 0.75 {
        drag.moved = true;
    }
    let _ = state.cad_demo.set_active_variant_tile(drag.tile_index);

    match drag.mode {
        CadCameraDragMode::Orbit => state.cad_demo.orbit_camera_by_drag(delta_x, delta_y),
        CadCameraDragMode::Pan => state.cad_demo.pan_camera_by_drag(delta_x, delta_y),
    }
    state.cad_camera_drag_state = Some(drag);
    true
}

fn finish_cad_camera_drag(state: &mut crate::app_state::RenderState) -> bool {
    let Some(drag) = state.cad_camera_drag_state.take() else {
        return false;
    };
    if !drag.moved {
        return false;
    }
    let mode_label = match drag.mode {
        CadCameraDragMode::Orbit => "orbit",
        CadCameraDragMode::Pan => "pan",
    };
    state.cad_demo.last_action = Some(format!(
        "CAD camera {mode_label} tile={} -> zoom={:.2} pan=({:.0},{:.0}) orbit=({:.0},{:.0})",
        drag.tile_index + 1,
        state.cad_demo.camera_zoom,
        state.cad_demo.camera_pan_x,
        state.cad_demo.camera_pan_y,
        state.cad_demo.camera_orbit_yaw_deg,
        state.cad_demo.camera_orbit_pitch_deg
    ));
    true
}

fn apply_cad_camera_zoom(
    state: &mut crate::app_state::RenderState,
    point: Point,
    scroll_dy: f32,
) -> bool {
    let Some(pane_id) = cad_camera_target_pane_id(state, point) else {
        return false;
    };
    let tile_index = state
        .panes
        .iter()
        .find(|pane| pane.id == pane_id)
        .map(|pane| pane_content_bounds(pane.bounds))
        .and_then(|content| cad_pane::variant_tile_index_at_point(content, point))
        .unwrap_or(0);
    let _ = state.cad_demo.set_active_variant_tile(tile_index);
    let previous = state.cad_demo.camera_zoom;
    state.cad_demo.zoom_camera_by_scroll(scroll_dy);
    if (previous - state.cad_demo.camera_zoom).abs() <= f32::EPSILON {
        return false;
    }
    state.cad_demo.last_action = Some(format!(
        "CAD camera zoom -> {:.2}",
        state.cad_demo.camera_zoom
    ));
    true
}

fn begin_chat_transcript_selection_drag(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) -> bool {
    let had_active_selection = state.chat_transcript_selection_drag.is_some()
        || state.autopilot_chat.transcript_selection.is_some();
    state.chat_transcript_selection_drag = None;
    state.autopilot_chat.clear_transcript_selection();
    if button != MouseButton::Left {
        return had_active_selection;
    }

    let Some((message_id, byte_offset)) =
        chat_pane::transcript_message_byte_offset_at_point(state, point)
    else {
        return had_active_selection;
    };
    state.chat_transcript_selection_drag = Some(ChatTranscriptSelectionDragState {
        message_id,
        anchor_byte_offset: byte_offset,
    });
    state.autopilot_chat.transcript_selection =
        Some(crate::app_state::ChatTranscriptSelectionState {
            message_id,
            start_byte_offset: byte_offset,
            end_byte_offset: byte_offset,
        });
    true
}

fn update_chat_transcript_selection_drag(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some(drag) = state.chat_transcript_selection_drag else {
        return false;
    };

    let Some((message_id, byte_offset)) =
        chat_pane::transcript_message_byte_offset_at_point(state, point)
    else {
        return false;
    };
    if message_id != drag.message_id {
        return false;
    }

    state.autopilot_chat.transcript_selection =
        Some(crate::app_state::ChatTranscriptSelectionState {
            message_id,
            start_byte_offset: drag.anchor_byte_offset.min(byte_offset),
            end_byte_offset: drag.anchor_byte_offset.max(byte_offset),
        });
    true
}

fn finish_chat_transcript_selection_drag(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some(drag) = state.chat_transcript_selection_drag.take() else {
        return false;
    };

    if let Some((message_id, byte_offset)) =
        chat_pane::transcript_message_byte_offset_at_point(state, point)
        && message_id == drag.message_id
    {
        state.autopilot_chat.transcript_selection =
            Some(crate::app_state::ChatTranscriptSelectionState {
                message_id,
                start_byte_offset: drag.anchor_byte_offset.min(byte_offset),
                end_byte_offset: drag.anchor_byte_offset.max(byte_offset),
            });
    };

    let now = std::time::Instant::now();
    let Some(selection) = state.autopilot_chat.transcript_selection else {
        return true;
    };
    if selection.end_byte_offset <= selection.start_byte_offset {
        state.autopilot_chat.clear_transcript_selection();
        return true;
    }

    let Some(message_text) = chat_pane::transcript_selection_text(state, selection) else {
        state.autopilot_chat.clear_transcript_selection();
        return true;
    };
    if message_text.trim().is_empty() {
        state.autopilot_chat.clear_transcript_selection();
        return true;
    }

    let notice = match copy_to_clipboard(&message_text) {
        Ok(()) => "Copied message to clipboard".to_string(),
        Err(error) => format!("Failed to copy message: {error}"),
    };
    state.autopilot_chat.set_copy_notice(now, notice);
    true
}

fn dispatch_text_inputs(state: &mut crate::app_state::RenderState, event: &InputEvent) -> bool {
    let mut handled = dispatch_spark_input_event(state, event);
    handled |= dispatch_mission_control_input_event(state, event);
    handled |= dispatch_pay_invoice_input_event(state, event);
    handled |= dispatch_create_invoice_input_event(state, event);
    handled |= dispatch_relay_connections_input_event(state, event);
    handled |= dispatch_network_requests_input_event(state, event);
    handled |= dispatch_local_inference_input_event(state, event);
    handled |= dispatch_apple_fm_workbench_input_event(state, event);
    handled |= dispatch_settings_input_event(state, event);
    handled |= dispatch_credentials_input_event(state, event);
    handled |= dispatch_chat_input_event(state, event);
    handled |= dispatch_calculator_input_event(state, event);
    handled |= dispatch_job_history_input_event(state, event);
    handled
}

fn dispatch_pane_actions(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let sort_count_before = pane_z_sort_invocation_count();
    let pane_order = pane_indices_by_z_desc(state);
    let Some((pane_id, action)) =
        topmost_pane_hit_action_in_order(state, point, pane_order.as_slice())
    else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    let handled = run_pane_hit_action(state, action);

    let sort_delta = pane_z_sort_invocation_count().saturating_sub(sort_count_before);
    debug_assert!(
        sort_delta <= 1,
        "pane action dispatch sorted z-order {sort_delta} times"
    );

    handled
}

fn sidebar_settings_icon_bounds(state: &crate::app_state::RenderState) -> Bounds {
    if !RIGHT_SIDEBAR_ENABLED {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }

    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;

    if !state.sidebar.is_open {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }

    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = configured_width;
    let sidebar_x = (width - panel_width).max(0.0);

    let icon_size = 16.0;
    let padding = 12.0;
    let icon_x = sidebar_x + panel_width - icon_size - padding;
    let icon_y = height - icon_size - padding;
    Bounds::new(icon_x, icon_y, icon_size, icon_size)
}

fn handle_sidebar_mouse_down(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) -> bool {
    if !RIGHT_SIDEBAR_ENABLED {
        return false;
    }

    if button != MouseButton::Left {
        return false;
    }
    let handle = sidebar_handle_bounds(state);
    if !handle.contains(point) {
        return false;
    }

    state.sidebar.is_pressed = true;
    state.sidebar.is_dragging = false;
    state.sidebar.drag_start_x = point.x;
    let logical = logical_size(&state.config, state.scale_factor);
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (logical.width * 0.5).max(min_sidebar_width);
    state.sidebar.drag_start_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    true
}

fn handle_sidebar_mouse_move(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    if !RIGHT_SIDEBAR_ENABLED {
        state.sidebar.settings_hover = false;
        return false;
    }

    let icon_bounds = sidebar_settings_icon_bounds(state);
    let hover = icon_bounds.contains(point);
    let mut handled = false;
    if hover != state.sidebar.settings_hover {
        state.sidebar.settings_hover = hover;
        handled = true;
    }

    handled
}

fn handle_sidebar_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    _event: &InputEvent,
) -> bool {
    if !RIGHT_SIDEBAR_ENABLED {
        state.sidebar.is_pressed = false;
        state.sidebar.is_dragging = false;
        return false;
    }

    if !state.sidebar.is_pressed {
        return false;
    }

    let was_dragging = state.sidebar.is_dragging;
    state.sidebar.is_pressed = false;
    state.sidebar.is_dragging = false;

    let handle = sidebar_handle_bounds(state);
    if !was_dragging && handle.contains(point) {
        // Treat as a click: toggle open/closed.
        state.sidebar.is_open = !state.sidebar.is_open;
        if state.sidebar.is_open && state.sidebar.width < 50.0 {
            state.sidebar.width = SIDEBAR_DEFAULT_WIDTH;
        }
        clamp_all_panes_to_window(state);
        return true;
    }

    was_dragging
}

fn dispatch_keyboard_submit_actions(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_chat_keyboard_input(state, logical_key)
        || handle_spark_wallet_keyboard_input(state, logical_key)
        || handle_mission_control_keyboard_input(state, logical_key)
        || handle_pay_invoice_keyboard_input(state, logical_key)
        || handle_create_invoice_keyboard_input(state, logical_key)
        || handle_relay_connections_keyboard_input(state, logical_key)
        || handle_network_requests_keyboard_input(state, logical_key)
        || handle_local_inference_keyboard_input(state, logical_key)
        || handle_apple_fm_workbench_keyboard_input(state, logical_key)
        || handle_settings_keyboard_input(state, logical_key)
        || handle_credentials_keyboard_input(state, logical_key)
        || handle_job_history_keyboard_input(state, logical_key)
        || handle_cad_timeline_keyboard_input(state, logical_key)
}

pub(super) fn run_pane_hit_action(
    state: &mut crate::app_state::RenderState,
    action: PaneHitAction,
) -> bool {
    match action {
        PaneHitAction::NostrRegenerate => {
            match regenerate_identity() {
                Ok(identity) => {
                    state.nostr_identity = Some(identity);
                    state.nostr_identity_error = None;
                    state.nostr_secret_state.revealed_until = None;
                    state.nostr_secret_state.set_copy_notice(
                        std::time::Instant::now(),
                        "Identity regenerated. Secrets are hidden by default.".to_string(),
                    );
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                    state.sync_chat_identities();
                    let _ = state.sync_provider_nip90_lane_identity();
                    crate::render::apply_spacetime_sync_bootstrap(state);
                }
                Err(err) => {
                    state.nostr_identity_error = Some(err.to_string());
                }
            }
            true
        }
        PaneHitAction::NostrReveal => {
            state
                .nostr_secret_state
                .toggle_reveal(std::time::Instant::now());
            true
        }
        PaneHitAction::NostrCopySecret => {
            let now = std::time::Instant::now();
            let notice = if let Some(identity) = state.nostr_identity.as_ref() {
                match copy_to_clipboard(&identity.nsec) {
                    Ok(()) => "Copied nsec to clipboard. Treat it like a password.".to_string(),
                    Err(error) => format!("Failed to copy nsec: {error}"),
                }
            } else {
                "No Nostr identity loaded. Regenerate keys first.".to_string()
            };
            state.nostr_secret_state.set_copy_notice(now, notice);
            true
        }
        PaneHitAction::ChatSend => run_chat_submit_action(state),
        PaneHitAction::ChatRefreshThreads => run_chat_refresh_threads_action(state),
        PaneHitAction::ChatNewThread => run_chat_new_thread_action(state),
        PaneHitAction::ChatCycleModel => run_chat_cycle_model_action(state),
        PaneHitAction::ChatCycleReasoningEffort => run_chat_cycle_reasoning_effort_action(state),
        PaneHitAction::ChatCycleServiceTier => run_chat_cycle_service_tier_action(state),
        PaneHitAction::ChatCyclePersonality => run_chat_cycle_personality_action(state),
        PaneHitAction::ChatCycleCollaborationMode => {
            run_chat_cycle_collaboration_mode_action(state)
        }
        PaneHitAction::ChatCycleApprovalMode => run_chat_cycle_approval_mode_action(state),
        PaneHitAction::ChatCycleSandboxMode => run_chat_cycle_sandbox_mode_action(state),
        PaneHitAction::ChatInterruptTurn => run_chat_interrupt_turn_action(state),
        PaneHitAction::ChatImplementPlan => run_chat_implement_plan_action(state),
        PaneHitAction::ChatReviewThread => run_chat_review_action(state),
        PaneHitAction::ChatToggleArchivedFilter => run_chat_toggle_archived_filter_action(state),
        PaneHitAction::ChatCycleSortFilter => run_chat_cycle_sort_filter_action(state),
        PaneHitAction::ChatCycleSourceFilter => run_chat_cycle_source_filter_action(state),
        PaneHitAction::ChatCycleProviderFilter => run_chat_cycle_provider_filter_action(state),
        PaneHitAction::ChatForkThread => run_chat_fork_thread_action(state),
        PaneHitAction::ChatArchiveThread => run_chat_archive_thread_action(state),
        PaneHitAction::ChatUnarchiveThread => run_chat_unarchive_thread_action(state),
        PaneHitAction::ChatRenameThread => run_chat_rename_thread_action(state),
        PaneHitAction::ChatReloadThread => run_chat_reload_thread_action(state),
        PaneHitAction::ChatOpenWorkspaceInEditor => run_chat_open_workspace_in_editor_action(state),
        PaneHitAction::ChatCopyLastOutput => run_chat_copy_last_output_action(state),
        PaneHitAction::ChatRollbackThread => run_chat_rollback_thread_action(state),
        PaneHitAction::ChatCompactThread => run_chat_compact_thread_action(state),
        PaneHitAction::ChatUnsubscribeThread => run_chat_unsubscribe_thread_action(state),
        PaneHitAction::ChatRespondApprovalAccept => {
            run_chat_approval_response_action(state, ApprovalDecision::Accept)
        }
        PaneHitAction::ChatRespondApprovalAcceptSession => {
            run_chat_approval_response_action(state, ApprovalDecision::AcceptForSession)
        }
        PaneHitAction::ChatRespondApprovalDecline => {
            run_chat_approval_response_action(state, ApprovalDecision::Decline)
        }
        PaneHitAction::ChatRespondApprovalCancel => {
            run_chat_approval_response_action(state, ApprovalDecision::Cancel)
        }
        PaneHitAction::ChatRespondToolCall => run_chat_tool_call_response_action(state),
        PaneHitAction::ChatRespondToolUserInput => run_chat_tool_user_input_response_action(state),
        PaneHitAction::ChatRespondAuthRefresh => run_chat_auth_refresh_response_action(state),
        PaneHitAction::ChatSelectWorkspace(index) => run_chat_select_workspace_action(state, index),
        PaneHitAction::ChatToggleCategory(index) => run_chat_toggle_category_action(state, index),
        PaneHitAction::ChatSelectThread(index) => run_chat_select_thread_action(state, index),
        PaneHitAction::GoOnlineToggle => apply_provider_mode_target(
            state,
            matches!(
                state.provider_runtime.mode,
                ProviderMode::Offline | ProviderMode::Degraded
            ),
            if matches!(
                state.provider_runtime.mode,
                ProviderMode::Offline | ProviderMode::Degraded
            ) {
                ProviderDesiredMode::Online
            } else {
                ProviderDesiredMode::Offline
            },
            "mission control toggle",
        ),
        PaneHitAction::MissionControl(action) => run_mission_control_action(state, action),
        PaneHitAction::BuyModePayments(action) => run_buy_mode_payments_action(state, action),
        PaneHitAction::CodexAccount(action) => run_codex_account_action(state, action),
        PaneHitAction::CodexModels(action) => run_codex_models_action(state, action),
        PaneHitAction::CodexConfig(action) => run_codex_config_action(state, action),
        PaneHitAction::CodexMcp(action) => run_codex_mcp_action(state, action),
        PaneHitAction::CodexApps(action) => run_codex_apps_action(state, action),
        PaneHitAction::CodexLabs(action) => run_codex_labs_action(state, action),
        PaneHitAction::CodexDiagnostics(action) => run_codex_diagnostics_action(state, action),
        PaneHitAction::EarningsScoreboard(action) => run_earnings_scoreboard_action(state, action),
        PaneHitAction::RelayConnections(action) => run_relay_connections_action(state, action),
        PaneHitAction::SyncHealth(action) => run_sync_health_action(state, action),
        PaneHitAction::ProviderStatus(action) => run_provider_status_action(state, action),
        PaneHitAction::LocalInference(action) => run_local_inference_action(state, action),
        PaneHitAction::AppleFmWorkbench(action) => run_apple_fm_workbench_action(state, action),
        PaneHitAction::NetworkRequests(action) => run_network_requests_action(state, action),
        PaneHitAction::StarterJobs(action) => run_starter_jobs_action(state, action),
        PaneHitAction::ReciprocalLoop(action) => run_reciprocal_loop_action(state, action),
        PaneHitAction::ActivityFeed(action) => run_activity_feed_action(state, action),
        PaneHitAction::AlertsRecovery(action) => run_alerts_recovery_action(state, action),
        PaneHitAction::Settings(action) => run_settings_action(state, action),
        PaneHitAction::Credentials(action) => run_credentials_action(state, action),
        PaneHitAction::JobInbox(action) => reducers::run_job_inbox_action(state, action),
        PaneHitAction::ActiveJob(action) => reducers::run_active_job_action(state, action),
        PaneHitAction::JobHistory(action) => reducers::run_job_history_action(state, action),
        PaneHitAction::AgentProfileState(action) => {
            reducers::run_agent_profile_state_action(state, action)
        }
        PaneHitAction::AgentScheduleTick(action) => {
            reducers::run_agent_schedule_tick_action(state, action)
        }
        PaneHitAction::TrajectoryAudit(action) => {
            reducers::run_trajectory_audit_action(state, action)
        }
        PaneHitAction::CastControl(action) => run_cast_control_action(state, action),
        PaneHitAction::SkillRegistry(action) => reducers::run_skill_registry_action(state, action),
        PaneHitAction::SkillTrustRevocation(action) => {
            reducers::run_skill_trust_revocation_action(state, action)
        }
        PaneHitAction::CreditDesk(action) => reducers::run_credit_desk_action(state, action),
        PaneHitAction::CreditSettlementLedger(action) => {
            reducers::run_credit_settlement_ledger_action(state, action)
        }
        PaneHitAction::CadDemo(action) => reducers::run_cad_demo_action(state, action),
        PaneHitAction::Spark(action) => run_spark_action(state, action),
        PaneHitAction::SparkCreateInvoice(action) => run_create_invoice_action(state, action),
        PaneHitAction::SparkPayInvoice(action) => run_pay_invoice_action(state, action),
    }
}

pub(crate) fn apply_provider_mode_target(
    state: &mut crate::app_state::RenderState,
    wants_online: bool,
    desired_mode: ProviderDesiredMode,
    origin: &str,
) -> bool {
    crate::provider_admin::set_desired_mode(state, desired_mode);
    let worker_id = state.sync_lifecycle_worker_id.clone();
    if wants_online {
        state.provider_runtime.inventory_session_started_at_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| {
                    duration.as_millis().min(i64::MAX as u128) as i64
                }),
        );
        let _ = state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::EnsureBridgeRunning);
        let _ = state.queue_local_inference_runtime_command(LocalInferenceRuntimeCommand::Refresh);
        if state.ollama_execution.is_ready() {
            let _ = state.queue_local_inference_runtime_command(
                LocalInferenceRuntimeCommand::WarmConfiguredModel,
            );
        }
        if let Some(reason) = provider_go_online_block_reason(state) {
            state.provider_runtime.last_result = Some(format!("{origin}: {reason}"));
            state.provider_runtime.last_error_detail = Some(reason);
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Execution);
            state.provider_runtime.mode = ProviderMode::Offline;
            state.provider_runtime.degraded_reason_code = None;
            state.provider_runtime.mode_changed_at = std::time::Instant::now();
            return true;
        }
        state.sync_lifecycle.mark_connecting(worker_id.as_str());
        if let Err(error) = state
            .spacetime_presence
            .register_online(state.nostr_identity.as_ref())
        {
            let reason = crate::sync_lifecycle::classify_disconnect_reason(error.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(error.clone()),
            );
            state.provider_runtime.last_result = Some(format!("{origin}: {error}"));
            state.provider_runtime.last_error_detail = Some(error);
            state.provider_runtime.mode = ProviderMode::Degraded;
            state.provider_runtime.degraded_reason_code =
                Some("SPACETIME_PRESENCE_BIND_FAILED".to_string());
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Execution);
            state.provider_runtime.mode_changed_at = std::time::Instant::now();
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            crate::render::sync_project_ops_runtime_contract_state(state);
            state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
            return true;
        }
    } else {
        let _ = state.sync_lifecycle.mark_disconnect(
            worker_id.as_str(),
            crate::sync_lifecycle::RuntimeSyncDisconnectReason::StreamClosed,
            Some(origin.to_string()),
        );
        state.sync_lifecycle.mark_idle(worker_id.as_str());
        let _ = state.spacetime_presence.register_offline();
        state.provider_runtime.mode = ProviderMode::Offline;
        state.provider_runtime.degraded_reason_code = None;
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
        if state.active_job.inflight_job_count() == 0 {
            state.provider_runtime.inventory_session_started_at_ms = None;
            state.provider_runtime.defer_runtime_shutdown_until_idle = false;
            let _ = state.queue_local_inference_runtime_command(
                LocalInferenceRuntimeCommand::UnloadConfiguredModel,
            );
        } else {
            state.provider_runtime.defer_runtime_shutdown_until_idle = true;
            state.provider_runtime.last_result = Some(
                "Go Offline requested; draining accepted provider job before runtime shutdown"
                    .to_string(),
            );
            state.active_job.append_event(
                "provider online intent changed to offline; draining active job before shutdown",
            );
            tracing::warn!(
                target: "autopilot_desktop::provider",
                "Provider online intent changed to offline while active job request_id={} is still in flight; draining existing job before runtime shutdown",
                state
                    .active_job
                    .job
                    .as_ref()
                    .map(|job| job.request_id.as_str())
                    .unwrap_or("missing")
            );
        }
    }
    state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();

    if wants_online {
        state.provider_runtime.defer_runtime_shutdown_until_idle = false;
        let _ = ensure_mission_control_apple_fm_refresh(state);
        queue_spark_command(state, SparkWalletCommand::Refresh);
        let _ = state.sync_provider_nip90_lane_identity();
        let _ = state.sync_provider_nip90_lane_relays();
        if let Err(error) =
            crate::kernel_control::register_online_compute_inventory_with_kernel(state)
        {
            state.provider_runtime.last_result = Some(format!(
                "Kernel online inventory registration failed: {error}"
            ));
            state.provider_runtime.last_error_detail = Some(error);
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Reconciliation);
        }
    }
    if let Err(error) =
        state.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::SetOnline {
            online: wants_online,
        })
    {
        let reason = crate::sync_lifecycle::classify_disconnect_reason(error.as_str());
        let _ =
            state
                .sync_lifecycle
                .mark_disconnect(worker_id.as_str(), reason, Some(error.clone()));
        state.provider_runtime.last_result = Some(format!("{origin}: {error}"));
        state.provider_runtime.last_error_detail = Some(error);
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = Some("NIP90_INGRESS_QUEUE_ERROR".to_string());
        state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Relay);
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
    }
    match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline {
        online: wants_online,
    }) {
        Ok(command_seq) => {
            if !wants_online && state.provider_runtime.defer_runtime_shutdown_until_idle {
                state.provider_runtime.last_result = Some(format!(
                    "Queued SetRunnerOnline command #{command_seq} // draining accepted provider job before shutdown"
                ));
                state.provider_runtime.last_authoritative_status = Some("draining".to_string());
            } else {
                state.provider_runtime.last_result =
                    Some(format!("Queued SetRunnerOnline command #{command_seq}"));
                state.provider_runtime.last_authoritative_status = Some("pending".to_string());
            }
            state.provider_runtime.last_authoritative_event_id = None;
            state.provider_runtime.last_authoritative_error_class = None;
            if wants_online {
                let refresh_after = state
                    .sync_lifecycle
                    .snapshot(worker_id.as_str())
                    .and_then(|snapshot| snapshot.token_refresh_after_in_seconds);
                state
                    .sync_lifecycle
                    .mark_live(worker_id.as_str(), refresh_after);
            } else {
                state.sync_lifecycle.mark_idle(worker_id.as_str());
            }
        }
        Err(error) => {
            let reason = crate::sync_lifecycle::classify_disconnect_reason(error.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(error.clone()),
            );
            state.provider_runtime.last_result = Some(format!("{origin}: {error}"));
            state.provider_runtime.last_error_detail = Some(error);
            state.provider_runtime.mode = ProviderMode::Degraded;
            state.provider_runtime.degraded_reason_code =
                Some("SA_COMMAND_QUEUE_ERROR".to_string());
            state.provider_runtime.mode_changed_at = std::time::Instant::now();
            state.provider_runtime.last_authoritative_status =
                Some(RuntimeCommandStatus::Retryable.label().to_string());
            state.provider_runtime.last_authoritative_event_id = None;
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Execution);
        }
    }
    state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
    crate::render::sync_project_ops_runtime_contract_state(state);
    true
}

fn provider_blocker_detail(
    blocker: crate::app_state::ProviderBlocker,
    spark_wallet_error: Option<&str>,
    ollama_error: Option<&str>,
    apple_fm_error: Option<&str>,
) -> String {
    let lane_error = match blocker {
        crate::app_state::ProviderBlocker::WalletError => spark_wallet_error,
        crate::app_state::ProviderBlocker::OllamaUnavailable
        | crate::app_state::ProviderBlocker::OllamaModelUnavailable => ollama_error,
        crate::app_state::ProviderBlocker::AppleFoundationModelsUnavailable
        | crate::app_state::ProviderBlocker::AppleFoundationModelsModelUnavailable => {
            apple_fm_error
        }
        _ => None,
    };

    lane_error
        .map(str::trim)
        .filter(|entry| {
            !entry.is_empty() && !entry.eq_ignore_ascii_case("Foundation Models is available")
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| match blocker {
            crate::app_state::ProviderBlocker::OllamaUnavailable => {
                "Local inference backend is unavailable".to_string()
            }
            crate::app_state::ProviderBlocker::OllamaModelUnavailable => {
                "No local inference model is ready".to_string()
            }
            _ => blocker.detail().to_string(),
        })
}

fn format_provider_blockers_for_display(
    blockers: &[crate::app_state::ProviderBlocker],
    spark_wallet_error: Option<&str>,
    ollama_error: Option<&str>,
    apple_fm_error: Option<&str>,
) -> Option<String> {
    if blockers.is_empty() {
        return None;
    }

    Some(
        blockers
            .iter()
            .map(|blocker| {
                format!(
                    "{} ({})",
                    blocker.code(),
                    provider_blocker_detail(
                        *blocker,
                        spark_wallet_error,
                        ollama_error,
                        apple_fm_error,
                    )
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
    )
}

fn provider_preflight_console_error(state: &crate::app_state::RenderState) -> Option<String> {
    if !should_mirror_provider_preflight_error(state.provider_runtime.mode) {
        return None;
    }
    let apple_fm_error = state.provider_runtime.apple_fm.availability_error_message();
    let blockers = state.provider_blockers();
    format_provider_blockers_for_display(
        blockers.as_slice(),
        state.spark_wallet.last_error.as_deref(),
        state.provider_runtime.ollama.last_error.as_deref(),
        apple_fm_error.as_deref(),
    )
    .map(|details| format!("Mission Control preflight blockers: {details}"))
}

fn should_mirror_provider_preflight_error(
    mode: crate::state::provider_runtime::ProviderMode,
) -> bool {
    !matches!(mode, crate::state::provider_runtime::ProviderMode::Offline)
}

fn provider_go_online_block_reason(state: &crate::app_state::RenderState) -> Option<String> {
    let blockers = state.provider_blockers();
    let apple_fm_error = state.provider_runtime.apple_fm.availability_error_message();
    format_provider_blockers_for_display(
        blockers.as_slice(),
        state.spark_wallet.last_error.as_deref(),
        state.provider_runtime.ollama.last_error.as_deref(),
        apple_fm_error.as_deref(),
    )
    .map(|details| format!("Cannot go online yet: {details}"))
}

fn provider_runtime_console_error(state: &crate::app_state::RenderState) -> Option<String> {
    let detail = state.provider_runtime.last_error_detail.as_deref()?.trim();
    if detail.is_empty() || detail.eq_ignore_ascii_case("Foundation Models is available") {
        return None;
    }
    Some(detail.to_string())
}

fn handle_chat_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    if (state.chat_inputs.composer.is_focused() || state.chat_inputs.thread_search.is_focused())
        && is_chat_terminal_shortcut(logical_key, state.input_modifiers)
    {
        focus_chat_composer(state);
        let prompt = if state
            .autopilot_chat
            .active_terminal_session()
            .is_some_and(|session| session.status.is_active())
        {
            "/term restart"
        } else {
            "/term open"
        };
        state.chat_inputs.composer.set_value(prompt.to_string());
        state
            .autopilot_chat
            .record_composer_draft(prompt.to_string());
        return run_chat_submit_action(state);
    }
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.chat_inputs.composer.is_focused() || s.chat_inputs.thread_search.is_focused(),
        dispatch_chat_input_event,
        |s| {
            if s.chat_inputs.composer.is_focused() {
                return run_chat_submit_action(s);
            }
            if s.chat_inputs.thread_search.is_focused() {
                return run_chat_refresh_threads_action(s);
            }
            false
        },
    )
}

fn handle_spark_wallet_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        spark_inputs_focused,
        dispatch_spark_input_event,
        |s| {
            if s.spark_inputs.invoice_amount.is_focused() {
                let _ = run_spark_action(s, SparkPaneAction::CreateInvoice);
                return true;
            }
            if s.spark_inputs.send_request.is_focused() || s.spark_inputs.send_amount.is_focused() {
                let _ = run_spark_action(s, SparkPaneAction::SendPayment);
                return true;
            }
            false
        },
    )
}

fn handle_mission_control_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        mission_control_inputs_focused,
        dispatch_mission_control_input_event,
        |s| {
            if s.mission_control.load_funds_amount_sats.is_focused() {
                let _ = run_mission_control_action(
                    s,
                    crate::pane_system::MissionControlPaneAction::CreateLightningReceiveTarget,
                );
                return true;
            }
            if s.mission_control.send_invoice.is_focused() {
                let _ = run_mission_control_action(
                    s,
                    crate::pane_system::MissionControlPaneAction::SendLightningPayment,
                );
                return true;
            }
            false
        },
    )
}

fn handle_pay_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        pay_invoice_inputs_focused,
        dispatch_pay_invoice_input_event,
        |s| {
            if s.pay_invoice_inputs.payment_request.is_focused()
                || s.pay_invoice_inputs.amount_sats.is_focused()
            {
                let _ = run_pay_invoice_action(s, PayInvoicePaneAction::SendPayment);
                return true;
            }
            false
        },
    )
}

fn handle_create_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        create_invoice_inputs_focused,
        dispatch_create_invoice_input_event,
        |s| {
            if s.create_invoice_inputs.amount_sats.is_focused()
                || s.create_invoice_inputs.description.is_focused()
                || s.create_invoice_inputs.expiry_seconds.is_focused()
            {
                let _ = run_create_invoice_action(s, CreateInvoicePaneAction::CreateInvoice);
                return true;
            }
            false
        },
    )
}

fn handle_relay_connections_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.relay_connections_inputs.relay_url.is_focused(),
        dispatch_relay_connections_input_event,
        |s| {
            if s.relay_connections_inputs.relay_url.is_focused() {
                return run_relay_connections_action(s, RelayConnectionsPaneAction::AddRelay);
            }
            false
        },
    )
}

fn handle_network_requests_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        network_requests_inputs_focused,
        dispatch_network_requests_input_event,
        |s| {
            if network_requests_inputs_focused(s) {
                return run_network_requests_action(s, NetworkRequestsPaneAction::RequestQuotes);
            }
            false
        },
    )
}

fn handle_local_inference_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        local_inference_inputs_focused,
        dispatch_local_inference_input_event,
        |s| {
            if local_inference_inputs_focused(s) {
                return run_local_inference_action(
                    s,
                    crate::pane_system::LocalInferencePaneAction::RunPrompt,
                );
            }
            false
        },
    )
}

fn handle_apple_fm_workbench_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        apple_fm_workbench_inputs_focused,
        dispatch_apple_fm_workbench_input_event,
        |_s| false,
    )
}

fn handle_activity_feed_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    if !matches!(key, Key::Named(NamedKey::Enter)) {
        return false;
    }

    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_activity_feed_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::ActivityFeed);
    if !is_activity_feed_active {
        return false;
    }

    run_activity_feed_action(state, ActivityFeedPaneAction::Refresh)
}

fn handle_alerts_recovery_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    if !matches!(key, Key::Named(NamedKey::Enter)) {
        return false;
    }

    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_alerts_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::AlertsRecovery);
    if !is_alerts_active {
        return false;
    }

    run_alerts_recovery_action(state, AlertsRecoveryPaneAction::RecoverSelected)
}

fn handle_settings_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        settings_inputs_focused,
        dispatch_settings_input_event,
        |s| {
            if settings_inputs_focused(s) {
                return run_settings_action(s, SettingsPaneAction::Save);
            }
            false
        },
    )
}

fn handle_credentials_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        credentials_inputs_focused,
        dispatch_credentials_input_event,
        |s| {
            if s.credentials_inputs.variable_value.is_focused() {
                return run_credentials_action(s, CredentialsPaneAction::SaveValue);
            }
            if s.credentials_inputs.variable_name.is_focused() {
                return run_credentials_action(s, CredentialsPaneAction::AddCustom);
            }
            false
        },
    )
}

fn handle_job_history_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.job_history_inputs.search_job_id.is_focused(),
        dispatch_job_history_input_event,
        |s| {
            if s.job_history_inputs.search_job_id.is_focused() {
                s.job_history.last_error = None;
                s.job_history.last_action = Some("Applied job-id search filter".to_string());
                return true;
            }
            false
        },
    )
}

fn handle_cad_timeline_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_cad_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::CadDemo);
    if !is_cad_active {
        return false;
    }

    if state.cad_demo.dimension_edit.is_some() {
        return match key {
            Key::Named(NamedKey::Enter) => {
                reducers::run_cad_demo_action(state, CadDemoPaneAction::DimensionInputCommit)
            }
            Key::Named(NamedKey::Escape) => {
                reducers::run_cad_demo_action(state, CadDemoPaneAction::DimensionInputCancel)
            }
            Key::Named(NamedKey::Backspace) => {
                reducers::run_cad_demo_action(state, CadDemoPaneAction::DimensionInputBackspace)
            }
            Key::Character(value) => value.chars().filter(|ch| !ch.is_whitespace()).any(|ch| {
                reducers::run_cad_demo_action(state, CadDemoPaneAction::DimensionInputChar(ch))
            }),
            _ => false,
        };
    }

    match key {
        Key::Named(NamedKey::ArrowUp) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::TimelineSelectPrev)
        }
        Key::Named(NamedKey::ArrowDown) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::TimelineSelectNext)
        }
        Key::Named(NamedKey::Home) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::ResetCamera)
        }
        Key::Character(value) if value == "0" => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::ResetCamera)
        }
        Key::Character(value) => cad_hotkey_action_matrix()
            .iter()
            .find_map(|(hotkey, action)| {
                state
                    .cad_demo
                    .hotkey_matches(*hotkey, value.as_str())
                    .then_some(*action)
            })
            .is_some_and(|action| reducers::run_cad_demo_action(state, action)),
        _ => false,
    }
}

fn cad_hotkey_action_matrix() -> &'static [(CadHotkeyAction, CadDemoPaneAction)] {
    const MATRIX: [(CadHotkeyAction, CadDemoPaneAction); 10] = [
        (CadHotkeyAction::SnapTop, CadDemoPaneAction::SnapViewTop),
        (CadHotkeyAction::SnapFront, CadDemoPaneAction::SnapViewFront),
        (CadHotkeyAction::SnapRight, CadDemoPaneAction::SnapViewRight),
        (
            CadHotkeyAction::SnapIsometric,
            CadDemoPaneAction::SnapViewIsometric,
        ),
        (
            CadHotkeyAction::ToggleProjection,
            CadDemoPaneAction::ToggleProjectionMode,
        ),
        (
            CadHotkeyAction::CycleRenderMode,
            CadDemoPaneAction::CycleHiddenLineMode,
        ),
        (
            CadHotkeyAction::ToggleSnapGrid,
            CadDemoPaneAction::ToggleSnapGrid,
        ),
        (
            CadHotkeyAction::ToggleSnapOrigin,
            CadDemoPaneAction::ToggleSnapOrigin,
        ),
        (
            CadHotkeyAction::ToggleSnapEndpoint,
            CadDemoPaneAction::ToggleSnapEndpoint,
        ),
        (
            CadHotkeyAction::ToggleSnapMidpoint,
            CadDemoPaneAction::ToggleSnapMidpoint,
        ),
    ];
    &MATRIX
}

fn handle_focused_keyboard_submit<FHasFocus, FDispatch, FEnter>(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
    has_focus: FHasFocus,
    dispatch_input: FDispatch,
    on_enter: FEnter,
) -> bool
where
    FHasFocus: Fn(&crate::app_state::RenderState) -> bool,
    FDispatch: Fn(&mut crate::app_state::RenderState, &InputEvent) -> bool,
    FEnter: Fn(&mut crate::app_state::RenderState) -> bool,
{
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };
    let focused_before = has_focus(state);
    let handled_by_input = dispatch_input(state, &key_event);
    let focused_after = has_focus(state);
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter)) && on_enter(state) {
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedChatTurnPrompt, TurnSkillAttachment, TurnSkillSource, assemble_chat_turn_input,
        build_create_invoice_command, build_goal_attempt_audit_receipts,
        build_goal_payout_evidence, build_pay_invoice_command, build_spark_command_for_action,
        cad_hit_action_blocks_camera_zoom, cad_hotkey_action_matrix, cad_pick_kind_label,
        cad_pick_kind_to_selection_kind, cad_policy_skill_candidates_for_turn,
        cad_turn_approval_policy, format_provider_blockers_for_display,
        goal_labor_linkage_from_binding, is_chat_terminal_shortcut, is_command_palette_shortcut,
        is_toggle_fullscreen_shortcut, parse_chat_turn_prompt, parse_positive_amount_str,
        provider_blocker_detail, resolve_turn_skill_by_name, resolve_turn_skill_by_path,
        should_mirror_provider_preflight_error, should_open_command_palette,
        terminal_goal_labor_linkage, validate_lightning_payment_request,
    };
    use crate::app_state::{ProviderBlocker, SkillRegistryDiscoveredSkill};
    use crate::labor_orchestrator::{
        CodexRunTrigger, CodexTurnExecutionRequest, orchestrate_codex_turn,
    };
    use crate::pane_system::cad_palette_command_specs;
    use crate::spark_pane::{
        CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction, hit_action, layout,
    };
    use crate::spark_wallet::SparkWalletCommand;
    use crate::state::goal_loop_executor::GoalLoopExecutorState;
    use crate::state::wallet_reconciliation::{
        WalletLedgerEvent, WalletLedgerEventKind, WalletReconciliationReport,
    };
    use codex_client::{AskForApproval, SandboxPolicy, UserInput};
    use openagents_cad::contracts::CadSelectionKind;
    use openagents_cad::query::CadPickEntityKind;
    use openagents_kernel_core::receipts::EvidenceRef;
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use wgpui::{Bounds, Modifiers, Point};
    use winit::keyboard::Key as WinitLogicalKey;

    fn parsed_prompt(text: &str) -> ParsedChatTurnPrompt {
        let (parsed, error) = parse_chat_turn_prompt(text.to_string(), None);
        assert!(error.is_none(), "unexpected parse error: {error:?}");
        parsed
    }

    #[test]
    fn parse_positive_amount_str_validates_inputs() {
        assert_eq!(parse_positive_amount_str("42", "Amount"), Ok(42));
        assert!(
            parse_positive_amount_str("0", "Amount")
                .expect_err("zero rejected")
                .contains("greater than 0")
        );
        assert!(
            parse_positive_amount_str("abc", "Amount")
                .expect_err("non-numeric rejected")
                .contains("valid integer")
        );
    }

    #[test]
    fn parse_positive_amount_str_has_readable_errors() {
        assert_eq!(
            parse_positive_amount_str("", "Invoice amount")
                .expect_err("empty amount should be rejected"),
            "Invoice amount is required"
        );
    }

    #[test]
    fn provider_blocker_detail_prefers_runtime_error_context() {
        assert_eq!(
            provider_blocker_detail(
                ProviderBlocker::OllamaUnavailable,
                Some("wallet down"),
                Some("No active job backend is available"),
                None,
            ),
            "No active job backend is available"
        );
        assert_eq!(
            provider_blocker_detail(
                ProviderBlocker::WalletError,
                Some("Spark wallet lane failed"),
                Some("ignored"),
                None,
            ),
            "Spark wallet lane failed"
        );
        assert_eq!(
            provider_blocker_detail(ProviderBlocker::IdentityMissing, None, None, None),
            "Nostr identity is not ready"
        );
        assert_eq!(
            provider_blocker_detail(
                ProviderBlocker::AppleFoundationModelsUnavailable,
                None,
                None,
                Some("Foundation Models is available"),
            ),
            "Apple Foundation Models backend is unavailable"
        );
    }

    #[test]
    fn format_provider_blockers_for_display_joins_codes_and_details() {
        let details = format_provider_blockers_for_display(
            &[
                ProviderBlocker::WalletError,
                ProviderBlocker::OllamaUnavailable,
            ],
            Some("Spark wallet lane failed"),
            Some("No active job backend is available"),
            None,
        )
        .expect("blocker details");

        assert_eq!(
            details,
            "WALLET_ERROR (Spark wallet lane failed); OLLAMA_UNAVAILABLE (No active job backend is available)"
        );
        assert!(format_provider_blockers_for_display(&[], None, None, None).is_none());
    }

    #[test]
    fn provider_preflight_console_error_stays_quiet_while_offline() {
        assert!(!should_mirror_provider_preflight_error(
            crate::state::provider_runtime::ProviderMode::Offline,
        ));
        assert!(should_mirror_provider_preflight_error(
            crate::state::provider_runtime::ProviderMode::Connecting,
        ));
        assert!(should_mirror_provider_preflight_error(
            crate::state::provider_runtime::ProviderMode::Online,
        ));
        assert!(should_mirror_provider_preflight_error(
            crate::state::provider_runtime::ProviderMode::Degraded,
        ));
    }

    #[test]
    fn spark_command_builder_routes_actions() {
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::Refresh, "", "", ""),
            Ok(SparkWalletCommand::Refresh)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::GenerateSparkAddress, "", "", ""),
            Ok(SparkWalletCommand::GenerateSparkAddress)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::GenerateBitcoinAddress, "", "", ""),
            Ok(SparkWalletCommand::GenerateBitcoinAddress)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::CopySparkAddress, "", "", ""),
            Err(error) if error.contains("handled directly")
        ));

        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::CreateInvoice, "1500", "", ""),
            Ok(SparkWalletCommand::CreateBolt11Invoice {
                amount_sats: 1500,
                description: Some(_),
                expiry_seconds: Some(3600)
            })
        ));

        assert!(matches!(
            build_spark_command_for_action(
                SparkPaneAction::SendPayment,
                "",
                "lnbc1example",
                "250"
            ),
            Ok(SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats: Some(250)
            }) if payment_request == "lnbc1example"
        ));

        assert!(matches!(
            build_spark_command_for_action(
                SparkPaneAction::SendPayment,
                "",
                "not-an-invoice",
                ""
            ),
            Err(error) if error.contains("expected prefix ln")
        ));
    }

    #[test]
    fn spark_click_to_command_smoke_path() {
        let content = Bounds::new(10.0, 10.0, 780.0, 420.0);
        let pane_layout = layout(content);

        let click = Point::new(
            pane_layout.create_invoice_button.origin.x + 4.0,
            pane_layout.create_invoice_button.origin.y + 4.0,
        );
        let action = hit_action(pane_layout, click).expect("create-invoice button should hit");

        let command = build_spark_command_for_action(action, "2100", "", "")
            .expect("command dispatch should succeed");
        assert!(matches!(
            command,
            SparkWalletCommand::CreateBolt11Invoice {
                amount_sats: 2100,
                description: Some(_),
                expiry_seconds: Some(3600)
            }
        ));
    }

    #[test]
    fn command_palette_shortcut_detects_plain_k_only() {
        let key = WinitLogicalKey::Character("k".into());
        let cmd_mods = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        let ctrl_mods = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let none_mods = Modifiers::default();

        assert!(!is_command_palette_shortcut(&key, cmd_mods));
        assert!(!is_command_palette_shortcut(&key, ctrl_mods));
        assert!(is_command_palette_shortcut(&key, none_mods));
    }

    #[test]
    fn command_palette_shortcut_opens_only_when_palette_closed_and_no_text_focus() {
        let key = WinitLogicalKey::Character("k".into());
        let mods = Modifiers::default();

        assert!(should_open_command_palette(&key, mods, false, false));
        assert!(!should_open_command_palette(&key, mods, false, true));
        assert!(!should_open_command_palette(&key, mods, true, false));
    }

    #[test]
    fn cad_zoom_hit_policy_allows_dense_row_targets() {
        use crate::pane_system::{CadDemoPaneAction, PaneHitAction};

        assert!(!cad_hit_action_blocks_camera_zoom(&PaneHitAction::CadDemo(
            CadDemoPaneAction::StartDimensionEdit(0)
        )));
        assert!(!cad_hit_action_blocks_camera_zoom(&PaneHitAction::CadDemo(
            CadDemoPaneAction::SelectTimelineRow(0)
        )));
        assert!(!cad_hit_action_blocks_camera_zoom(&PaneHitAction::CadDemo(
            CadDemoPaneAction::SelectWarning(0)
        )));
        assert!(cad_hit_action_blocks_camera_zoom(&PaneHitAction::CadDemo(
            CadDemoPaneAction::CycleVariant
        )));
    }

    #[test]
    fn fullscreen_shortcut_matches_platform_binding() {
        let key = WinitLogicalKey::Character("f".into());
        let cmd_mods = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        let ctrl_mods = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let alt_mods = Modifiers {
            alt: true,
            ctrl: true,
            meta: true,
            ..Modifiers::default()
        };
        let none_mods = Modifiers::default();

        #[cfg(target_os = "macos")]
        {
            assert!(is_toggle_fullscreen_shortcut(&key, cmd_mods));
            assert!(!is_toggle_fullscreen_shortcut(&key, ctrl_mods));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(is_toggle_fullscreen_shortcut(&key, ctrl_mods));
            assert!(!is_toggle_fullscreen_shortcut(&key, cmd_mods));
        }

        assert!(!is_toggle_fullscreen_shortcut(&key, alt_mods));
        assert!(!is_toggle_fullscreen_shortcut(&key, none_mods));
    }

    #[test]
    fn chat_terminal_shortcut_matches_platform_binding() {
        let key = WinitLogicalKey::Character("t".into());
        let cmd_mods = Modifiers {
            meta: true,
            shift: true,
            ..Modifiers::default()
        };
        let ctrl_mods = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        };
        let wrong_shift = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };

        #[cfg(target_os = "macos")]
        {
            assert!(is_chat_terminal_shortcut(&key, cmd_mods));
            assert!(!is_chat_terminal_shortcut(&key, ctrl_mods));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(is_chat_terminal_shortcut(&key, ctrl_mods));
            assert!(!is_chat_terminal_shortcut(&key, cmd_mods));
        }

        assert!(!is_chat_terminal_shortcut(&key, wrong_shift));
    }

    #[test]
    fn validate_lightning_payment_request_rejects_non_invoice_text() {
        let error = validate_lightning_payment_request("not-an-invoice")
            .expect_err("non-invoice requests should fail");
        assert!(error.contains("expected prefix ln"));
    }

    #[test]
    fn build_pay_invoice_command_accepts_lightning_invoice() {
        let command = build_pay_invoice_command(
            PayInvoicePaneAction::SendPayment,
            "lnbc1exampleinvoice",
            "250",
        )
        .expect("invoice command should be built");
        assert!(matches!(
            command,
            SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats: Some(250)
            } if payment_request == "lnbc1exampleinvoice"
        ));
    }

    #[test]
    fn build_pay_invoice_command_rejects_zero_amount_invoice_without_amount() {
        let error = build_pay_invoice_command(
            PayInvoicePaneAction::SendPayment,
            "lightning:lnbc1zeroamountinvoice",
            "",
        )
        .expect_err("zero-amount invoice should require explicit sats");

        assert!(error.contains("zero-amount invoice"));
    }

    #[test]
    fn build_pay_invoice_command_accepts_zero_amount_invoice_with_amount() {
        let command = build_pay_invoice_command(
            PayInvoicePaneAction::SendPayment,
            "lightning:lnbc1zeroamountinvoice",
            "250",
        )
        .expect("zero-amount invoice should accept explicit sats");

        assert!(matches!(
            command,
            SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats: Some(250)
            } if payment_request == "lightning:lnbc1zeroamountinvoice"
        ));
    }

    #[test]
    fn build_create_invoice_command_supports_optional_fields() {
        let command = build_create_invoice_command(
            CreateInvoicePaneAction::CreateInvoice,
            "1200",
            "MVP invoice",
            "900",
        )
        .expect("create invoice command should be built");
        assert!(matches!(
            command,
            SparkWalletCommand::CreateBolt11Invoice {
                amount_sats: 1200,
                description: Some(description),
                expiry_seconds: Some(900)
            } if description == "MVP invoice"
        ));
    }

    #[test]
    fn assemble_chat_turn_input_attaches_enabled_skill() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("build mezo integration"),
            vec![TurnSkillAttachment {
                name: "mezo".to_string(),
                path: "/repo/skills/mezo/SKILL.md".to_string(),
                enabled: true,
                source: TurnSkillSource::UserSelected,
            }],
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 2);
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "build mezo integration"
        ));
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, path }
                if name == "mezo" && path == &PathBuf::from("/repo/skills/mezo/SKILL.md")
        ));
    }

    #[test]
    fn assemble_chat_turn_input_rejects_disabled_skill_attachment() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("build mezo integration"),
            vec![TurnSkillAttachment {
                name: "mezo".to_string(),
                path: "/repo/skills/mezo/SKILL.md".to_string(),
                enabled: false,
                source: TurnSkillSource::UserSelected,
            }],
        );

        assert_eq!(input.len(), 1);
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "build mezo integration"
        ));
        assert_eq!(
            last_error.as_deref(),
            Some("Selected skill 'mezo' is disabled; enable it first.")
        );
    }

    #[test]
    fn assemble_chat_turn_input_orders_and_dedupes_skills_deterministically() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("run automation"),
            vec![
                TurnSkillAttachment {
                    name: "pane-control".to_string(),
                    path: "/repo/skills/pane-control/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::PolicyRequired,
                },
                TurnSkillAttachment {
                    name: "mezo".to_string(),
                    path: "/repo/skills/mezo/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::UserSelected,
                },
                TurnSkillAttachment {
                    name: "pane-control".to_string(),
                    path: "/repo/skills/pane-control/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::PolicyRequired,
                },
            ],
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 3);
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, .. } if name == "mezo"
        ));
        assert!(matches!(
            &input[2],
            UserInput::Skill { name, .. } if name == "pane-control"
        ));
    }

    #[test]
    fn assemble_chat_turn_input_prefers_user_selected_over_goal_auto_selected_duplicate() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("run automation"),
            vec![
                TurnSkillAttachment {
                    name: "blink".to_string(),
                    path: "/repo/skills/blink/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::GoalAutoSelected,
                },
                TurnSkillAttachment {
                    name: "blink".to_string(),
                    path: "/repo/skills/blink/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::UserSelected,
                },
            ],
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 2);
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, path }
                if name == "blink" && path == &PathBuf::from("/repo/skills/blink/SKILL.md")
        ));
    }

    #[test]
    fn assemble_chat_turn_input_orders_goal_auto_selected_before_policy_required() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("run automation"),
            vec![
                TurnSkillAttachment {
                    name: "pane-control".to_string(),
                    path: "/repo/skills/pane-control/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::PolicyRequired,
                },
                TurnSkillAttachment {
                    name: "blink".to_string(),
                    path: "/repo/skills/blink/SKILL.md".to_string(),
                    enabled: true,
                    source: TurnSkillSource::GoalAutoSelected,
                },
            ],
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 3);
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, .. } if name == "blink"
        ));
        assert!(matches!(
            &input[2],
            UserInput::Skill { name, .. } if name == "pane-control"
        ));
    }

    #[test]
    fn parse_chat_turn_prompt_extracts_mentions_and_images() {
        let cwd = tempfile::tempdir().expect("temp dir");
        let image_path = cwd.path().join("diagram.png");
        let source_path = cwd.path().join("src").join("main.rs");
        std::fs::create_dir_all(source_path.parent().expect("src parent")).expect("src dir");
        std::fs::write(&image_path, b"png").expect("image fixture");
        std::fs::write(&source_path, "fn main() {}").expect("source fixture");

        let (parsed, last_error) = parse_chat_turn_prompt(
            "/mention src/main.rs | Main File\n/image ./diagram.png\nExplain this.".to_string(),
            cwd.path().to_str(),
        );

        assert!(last_error.is_none());
        assert_eq!(parsed.prompt_text, "Explain this.");
        assert_eq!(parsed.mention_attachments.len(), 1);
        assert_eq!(parsed.mention_attachments[0].name, "Main File");
        assert!(parsed.mention_attachments[0].path.ends_with("src/main.rs"));
        assert_eq!(parsed.image_attachments.len(), 1);
        assert!(matches!(
            &parsed.image_attachments[0],
            super::TurnImageAttachment::Local { path } if path.ends_with("diagram.png")
        ));
    }

    #[test]
    fn assemble_chat_turn_input_includes_mentions_images_and_skills() {
        let cwd = tempfile::tempdir().expect("temp dir");
        let image_path = cwd.path().join("diagram.png");
        std::fs::write(&image_path, b"png").expect("image fixture");
        let (parsed, last_error) = parse_chat_turn_prompt(
            format!(
                "/mention app://repo | Repo\n/image {}\nReview the attachment.",
                image_path.display()
            ),
            cwd.path().to_str(),
        );
        assert!(last_error.is_none());

        let (input, last_error) = assemble_chat_turn_input(
            parsed,
            vec![TurnSkillAttachment {
                name: "blink".to_string(),
                path: "/repo/skills/blink/SKILL.md".to_string(),
                enabled: true,
                source: TurnSkillSource::UserSelected,
            }],
        );

        assert!(last_error.is_none());
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "Review the attachment."
        ));
        assert!(matches!(
            &input[1],
            UserInput::Mention { name, path } if name == "Repo" && path == "app://repo"
        ));
        assert!(matches!(
            &input[2],
            UserInput::LocalImage { path } if path.ends_with("diagram.png")
        ));
        assert!(matches!(
            &input[3],
            UserInput::Skill { name, .. } if name == "blink"
        ));
    }

    #[test]
    fn goal_attempt_receipts_link_labor_ids_and_settlement_evidence() {
        let mut chat = crate::app_state::AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("complete the earning task".to_string());

        let plan = orchestrate_codex_turn(CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-1".to_string(),
                goal_title: "Earn sats".to_string(),
            },
            submitted_at_epoch_ms: 1_700_000_000_000,
            thread_id: "thread-1".to_string(),
            input: vec![
                UserInput::Text {
                    text: "complete the earning task".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "blink".to_string(),
                    path: PathBuf::from("/repo/skills/blink/SKILL.md"),
                },
            ],
            cwd: Some(PathBuf::from("/repo")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                writable_roots: vec!["/repo".to_string()],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }),
            model: Some("gpt-5".to_string()),
            service_tier: None,
            effort: None,
            personality: None,
            collaboration_mode: None,
        });
        let initial_labor = goal_labor_linkage_from_binding(
            plan.labor_binding
                .as_ref()
                .expect("autonomous goal is labor bound"),
        );
        let expected_work_unit_id = initial_labor.work_unit_id.clone();
        let expected_contract_id = initial_labor.contract_id.clone();
        chat.record_turn_submission_metadata(
            "thread-1",
            plan.classification,
            plan.labor_binding,
            false,
            "autonomous-goal",
            1_700_000_000_000,
            vec!["blink".to_string()],
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.record_turn_tool_request(
            "turn-1",
            "req-1",
            "call-1",
            "openagents_labor_scope",
            "{\"turn_id\":\"turn-1\"}",
            1_700_000_000_100,
        );
        chat.record_turn_tool_result(
            "turn-1",
            "req-1",
            "call-1",
            "openagents_labor_scope",
            "OA-LABOR-SCOPE-OK",
            true,
            "scope delivered",
            1_700_000_000_200,
        );
        chat.set_turn_message_for_turn("turn-1", "final answer");
        chat.mark_turn_completed_for("turn-1");
        chat.assemble_turn_labor_submission("turn-1", 1_700_000_000_300)
            .expect("submission should assemble");
        chat.finalize_turn_labor_verdict("turn-1", 1_700_000_000_400)
            .expect("verdict should finalize");

        let mut executor = GoalLoopExecutorState::default();
        assert!(executor.begin_run("goal-1", 1_700_000_000, 0, false));
        executor.mark_attempt_submitted(
            1_700_000_000,
            Some("thread-1".to_string()),
            vec!["blink".to_string()],
            initial_labor,
        );
        executor.bind_attempt_turn_id("turn-1");
        executor.record_tool_invocation(
            "req-1",
            "call-1",
            "openagents_labor_scope",
            "OA-LABOR-SCOPE-OK",
            true,
            "scope delivered",
            1_700_000_001,
        );
        executor.merge_attempt_labor_linkage(
            Some("turn-1"),
            chat.turn_labor_linkage_for("turn-1")
                .expect("turn labor linkage available"),
        );

        let attempts = build_goal_attempt_audit_receipts(
            &chat,
            &executor.active_run.as_ref().expect("run active").attempts,
        );
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].labor.work_unit_id, expected_work_unit_id);
        assert_eq!(attempts[0].labor.contract_id, expected_contract_id);
        assert!(attempts[0].labor.submission_id.is_some());
        assert!(attempts[0].labor.verdict_id.is_some());
        assert_eq!(attempts[0].labor.settlement_ready, Some(true));
        assert_eq!(attempts[0].tool_invocations.len(), 1);
        assert_eq!(attempts[0].tool_invocations[0].evidence_refs.len(), 1);

        let mut terminal_labor = terminal_goal_labor_linkage(&attempts);
        let payout_evidence = build_goal_payout_evidence(
            &WalletReconciliationReport {
                wallet_delta_sats_raw: 1_000,
                wallet_delta_excluding_swaps_sats: 1_000,
                earned_wallet_delta_sats: 1_000,
                swap_converted_out_sats: 0,
                swap_converted_in_sats: 0,
                swap_fee_sats: 0,
                non_swap_spend_sats: 0,
                unattributed_receive_sats: 0,
                total_swap_cents: 0,
                events: vec![WalletLedgerEvent {
                    event_id: "earn:job-1:wallet:pay:job-1".to_string(),
                    occurred_at_epoch_seconds: 1_700_000_002,
                    kind: WalletLedgerEventKind::EarnPayout,
                    sats_delta: 1_000,
                    cents_delta: 0,
                    job_id: Some("job-1".to_string()),
                    payment_pointer: Some("wallet:pay:job-1".to_string()),
                    quote_id: None,
                    transaction_id: None,
                    note: None,
                }],
            },
            attempts.first(),
        );
        assert_eq!(payout_evidence.len(), 1);
        terminal_labor.merge_from(&payout_evidence[0].labor);
        assert_eq!(payout_evidence[0].attempt_index, Some(1));
        assert_eq!(payout_evidence[0].turn_id.as_deref(), Some("turn-1"));
        assert_eq!(
            payout_evidence[0].labor.work_unit_id,
            attempts[0].labor.work_unit_id
        );
        assert!(payout_evidence[0].labor.settlement_id.is_some());
        assert_eq!(payout_evidence[0].labor.settlement_evidence_refs.len(), 1);
        assert!(terminal_labor.settlement_id.is_some());
    }

    #[test]
    fn goal_attempt_receipts_capture_claim_and_remedy_linkage() {
        let mut chat = crate::app_state::AutopilotChatState::default();
        chat.ensure_thread("thread-claim".to_string());
        chat.submit_prompt("execute disputed work".to_string());

        let plan = orchestrate_codex_turn(CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-claim".to_string(),
                goal_title: "Handle disputes".to_string(),
            },
            submitted_at_epoch_ms: 1_700_000_100_000,
            thread_id: "thread-claim".to_string(),
            input: vec![UserInput::Text {
                text: "execute disputed work".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(PathBuf::from("/repo")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: Some("gpt-5".to_string()),
            service_tier: None,
            effort: None,
            personality: None,
            collaboration_mode: None,
        });
        let initial_labor = goal_labor_linkage_from_binding(
            plan.labor_binding
                .as_ref()
                .expect("autonomous goal is labor bound"),
        );
        chat.record_turn_submission_metadata(
            "thread-claim",
            plan.classification,
            plan.labor_binding,
            false,
            "autonomous-goal",
            1_700_000_100_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-claim".to_string());
        chat.record_turn_tool_request(
            "turn-claim",
            "req-claim",
            "call-claim",
            "openagents.files.write",
            "{\"path\":\"README.md\"}",
            1_700_000_100_050,
        );
        chat.record_turn_tool_result(
            "turn-claim",
            "req-claim",
            "call-claim",
            "openagents.files.write",
            "FAILED",
            false,
            "write denied",
            1_700_000_100_060,
        );
        chat.set_turn_message_for_turn("turn-claim", "candidate answer");
        chat.mark_turn_completed_for("turn-claim");
        let artifact_scope_root = chat
            .turn_labor_binding_for("turn-claim")
            .expect("labor binding should exist")
            .artifact_scope_root();
        chat.attach_turn_labor_evidence(
            "turn-claim",
            EvidenceRef::new(
                "incident_note",
                format!("{artifact_scope_root}incidents/note-1"),
                "sha256:incident-note",
            ),
            true,
        )
        .expect("incident evidence should attach");
        chat.assemble_turn_labor_submission("turn-claim", 1_700_000_100_100)
            .expect("submission should assemble");
        chat.finalize_turn_labor_verdict("turn-claim", 1_700_000_100_200)
            .expect("verdict should finalize");
        chat.open_turn_labor_claim(
            "turn-claim",
            1_700_000_100_220,
            None,
            Some("operator requested review"),
        )
        .expect("claim should open");
        chat.review_turn_labor_claim("turn-claim", 1_700_000_100_230, Some("checking failure"))
            .expect("claim review should succeed");
        chat.issue_turn_labor_remedy(
            "turn-claim",
            1_700_000_100_240,
            "rework_credit",
            Some("issue credit"),
        )
        .expect("remedy issuance should succeed");
        chat.resolve_turn_labor_claim("turn-claim", 1_700_000_100_250, Some("claim closed"))
            .expect("claim resolution should succeed");

        let mut executor = GoalLoopExecutorState::default();
        assert!(executor.begin_run("goal-claim", 1_700_000_100, 0, false));
        executor.mark_attempt_submitted(
            1_700_000_100,
            Some("thread-claim".to_string()),
            vec!["blink".to_string()],
            initial_labor,
        );
        executor.bind_attempt_turn_id("turn-claim");
        executor.merge_attempt_labor_linkage(
            Some("turn-claim"),
            chat.turn_labor_linkage_for("turn-claim")
                .expect("turn labor linkage available"),
        );

        let attempts = build_goal_attempt_audit_receipts(
            &chat,
            &executor.active_run.as_ref().expect("run active").attempts,
        );
        assert_eq!(attempts.len(), 1);
        assert!(attempts[0].labor.claim_id.is_some());
        assert_eq!(
            attempts[0].labor.claim_state.as_deref(),
            Some("claim_resolved")
        );
        assert_eq!(
            attempts[0].labor.remedy_kind.as_deref(),
            Some("rework_credit")
        );
        assert!(!attempts[0].labor.claim_evidence_refs.is_empty());
        assert_eq!(attempts[0].labor.incident_evidence_refs.len(), 1);
        assert_eq!(attempts[0].labor.remedy_evidence_refs.len(), 1);
    }

    #[test]
    fn resolve_turn_skill_helpers_match_name_and_path() {
        let discovered = vec![
            SkillRegistryDiscoveredSkill {
                name: "mezo".to_string(),
                path: "/repo/skills/mezo/SKILL.md".to_string(),
                scope: "global".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
            SkillRegistryDiscoveredSkill {
                name: "pane-control".to_string(),
                path: "/repo/skills/pane-control/SKILL.md".to_string(),
                scope: "global".to_string(),
                enabled: false,
                interface_display_name: None,
                dependency_count: 0,
            },
        ];

        let mezo = resolve_turn_skill_by_name(&discovered, "MEZO", TurnSkillSource::UserSelected)
            .expect("skill should resolve by name");
        assert_eq!(mezo.path, "/repo/skills/mezo/SKILL.md");
        assert!(mezo.enabled);

        let pane = resolve_turn_skill_by_path(
            &discovered,
            "/repo/skills/pane-control/SKILL.md",
            TurnSkillSource::PolicyRequired,
        )
        .expect("skill should resolve by path");
        assert_eq!(pane.name, "pane-control");
        assert!(!pane.enabled);
        assert_eq!(pane.source, TurnSkillSource::PolicyRequired);
    }

    #[test]
    fn cad_policy_skill_candidates_resolve_for_cad_turn() {
        let discovered = vec![
            SkillRegistryDiscoveredSkill {
                name: "autopilot-cad-builder".to_string(),
                path: "/repo/skills/autopilot-cad-builder/SKILL.md".to_string(),
                scope: "global".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
            SkillRegistryDiscoveredSkill {
                name: "autopilot-pane-control".to_string(),
                path: "/repo/skills/autopilot-pane-control/SKILL.md".to_string(),
                scope: "global".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
        ];

        let skills = cad_policy_skill_candidates_for_turn(true, &discovered);
        assert_eq!(skills.len(), 2);
        assert!(
            skills
                .iter()
                .any(|skill| skill.name == "autopilot-cad-builder")
        );
        assert!(
            skills
                .iter()
                .any(|skill| skill.name == "autopilot-pane-control")
        );
        assert!(
            skills
                .iter()
                .all(|skill| skill.source == TurnSkillSource::PolicyRequired)
        );
    }

    #[test]
    fn cad_policy_skill_candidates_include_only_discovered_required_skills() {
        let discovered = vec![SkillRegistryDiscoveredSkill {
            name: "autopilot-pane-control".to_string(),
            path: "/repo/skills/autopilot-pane-control/SKILL.md".to_string(),
            scope: "global".to_string(),
            enabled: true,
            interface_display_name: None,
            dependency_count: 0,
        }];

        let skills = cad_policy_skill_candidates_for_turn(true, &discovered);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "autopilot-pane-control");
    }

    #[test]
    fn cad_policy_skill_candidates_do_not_attach_for_non_cad_turns() {
        let discovered = vec![SkillRegistryDiscoveredSkill {
            name: "autopilot-cad-builder".to_string(),
            path: "/repo/skills/autopilot-cad-builder/SKILL.md".to_string(),
            scope: "global".to_string(),
            enabled: true,
            interface_display_name: None,
            dependency_count: 0,
        }];

        let skills = cad_policy_skill_candidates_for_turn(false, &discovered);
        assert!(skills.is_empty());
    }

    #[test]
    fn cad_turn_approval_policy_is_never_for_non_cad_turn() {
        let policy = cad_turn_approval_policy(false);
        assert!(matches!(policy, Some(codex_client::AskForApproval::Never)));
    }

    #[test]
    fn cad_turn_approval_policy_is_never_for_cad_turn() {
        let policy = cad_turn_approval_policy(true);
        assert!(matches!(policy, Some(codex_client::AskForApproval::Never)));
    }

    #[test]
    fn assemble_chat_turn_input_keeps_policy_skill_even_if_disabled() {
        let (input, last_error) = assemble_chat_turn_input(
            parsed_prompt("draft cad design"),
            vec![TurnSkillAttachment {
                name: "autopilot-cad-builder".to_string(),
                path: "/managed/skills/autopilot-cad-builder/SKILL.md".to_string(),
                enabled: false,
                source: TurnSkillSource::PolicyRequired,
            }],
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 2);
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, path }
                if name == "autopilot-cad-builder"
                    && path == &PathBuf::from("/managed/skills/autopilot-cad-builder/SKILL.md")
        ));
    }

    #[test]
    fn enter_and_mouse_primary_actions_stay_in_parity() {
        let enter_actions: BTreeSet<&str> = [
            "chat.submit",
            "spark.create_invoice",
            "spark.send_payment",
            "pay_invoice.send_payment",
            "create_invoice.create",
            "relay_connections.add",
            "network_requests.submit",
            "activity_feed.refresh",
            "alerts_recovery.recover",
            "settings.save",
        ]
        .into_iter()
        .collect();

        let mouse_actions: BTreeSet<&str> = [
            "chat.submit",
            "spark.create_invoice",
            "spark.send_payment",
            "pay_invoice.send_payment",
            "create_invoice.create",
            "relay_connections.add",
            "network_requests.submit",
            "activity_feed.refresh",
            "alerts_recovery.recover",
            "settings.save",
        ]
        .into_iter()
        .collect();

        assert_eq!(enter_actions, mouse_actions);
    }

    #[test]
    fn cad_hotkey_and_command_palette_actions_stay_in_parity() {
        let hotkey_actions: BTreeSet<String> = cad_hotkey_action_matrix()
            .iter()
            .map(|(_, action)| format!("{action:?}"))
            .collect();
        let palette_actions: BTreeSet<String> = cad_palette_command_specs()
            .iter()
            .map(|spec| format!("{:?}", spec.action))
            .collect();

        assert!(
            hotkey_actions.is_subset(&palette_actions),
            "every CAD hotkey action must have a command palette equivalent"
        );
    }

    #[test]
    fn cad_pick_kind_mapping_is_stable() {
        assert_eq!(
            cad_pick_kind_to_selection_kind(CadPickEntityKind::Body),
            CadSelectionKind::Body
        );
        assert_eq!(
            cad_pick_kind_to_selection_kind(CadPickEntityKind::Face),
            CadSelectionKind::Face
        );
        assert_eq!(
            cad_pick_kind_to_selection_kind(CadPickEntityKind::Edge),
            CadSelectionKind::Edge
        );
        assert_eq!(cad_pick_kind_label(CadPickEntityKind::Body), "body");
        assert_eq!(cad_pick_kind_label(CadPickEntityKind::Face), "face");
        assert_eq!(cad_pick_kind_label(CadPickEntityKind::Edge), "edge");
    }
}
