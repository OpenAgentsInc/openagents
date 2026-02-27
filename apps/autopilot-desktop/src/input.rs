use nostr::regenerate_identity;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use wgpui::clipboard::copy_to_clipboard;
use wgpui::{Bounds, Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};

use crate::app_state::{ActivityEventDomain, ActivityEventRow, AlertDomain, App, ProviderMode};
use crate::hotbar::{
    HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET, activate_hotbar_slot,
    hotbar_slot_for_key, process_hotbar_clicks,
};
use crate::pane_registry::pane_spec_by_command_id;
use crate::pane_system::{
    ActiveJobPaneAction, ActivityFeedPaneAction, AgentProfileStatePaneAction,
    AgentScheduleTickPaneAction, AlertsRecoveryPaneAction, CreditDeskPaneAction,
    CreditSettlementLedgerPaneAction, EarningsScoreboardPaneAction, JobInboxPaneAction,
    NetworkRequestsPaneAction, PaneController, PaneHitAction, PaneInput,
    RelayConnectionsPaneAction, SettingsPaneAction, SkillRegistryPaneAction,
    SkillTrustRevocationPaneAction, StarterJobsPaneAction, SyncHealthPaneAction,
    TrajectoryAuditPaneAction, dispatch_chat_input_event, dispatch_create_invoice_input_event,
    dispatch_job_history_input_event, dispatch_network_requests_input_event,
    dispatch_pay_invoice_input_event, dispatch_relay_connections_input_event,
    dispatch_settings_input_event, dispatch_spark_input_event, pane_indices_by_z_desc,
    pane_z_sort_invocation_count, topmost_pane_hit_action_in_order,
};
use crate::render::{logical_size, render_frame};
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneUpdate, RuntimeCommandErrorClass, RuntimeCommandResponse,
    RuntimeCommandStatus, RuntimeLane, SaLaneUpdate, SaLifecycleCommand, SaRunnerMode,
    SklDiscoveryTrustCommand, SklLaneUpdate,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;

pub fn handle_window_event(app: &mut App, event_loop: &ActiveEventLoop, event: WindowEvent) {
    let Some(state) = &mut app.state else {
        return;
    };

    if drain_spark_worker_updates(state) {
        state.window.request_redraw();
    }
    if drain_runtime_lane_updates(state) {
        state.window.request_redraw();
    }
    if state.nostr_secret_state.expire(std::time::Instant::now()) {
        state.window.request_redraw();
    }
    let now = std::time::Instant::now();
    if state.autopilot_chat.tick(now) {
        state.window.request_redraw();
    }
    refresh_earnings_scoreboard(state, now);
    refresh_sync_health(state);

    match event {
        WindowEvent::CloseRequested => {
            let _ = state.spark_worker.cancel_pending();
            event_loop.exit();
        }
        WindowEvent::Resized(new_size) => {
            state.config.width = new_size.width.max(1);
            state.config.height = new_size.height.max(1);
            state.surface.configure(&state.device, &state.config);
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

            if state.command_palette.is_open() {
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

            if state.command_palette.is_open() {
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
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed
                && is_command_palette_shortcut(&event.logical_key, state.input_modifiers)
            {
                toggle_command_palette(state);
                state.window.request_redraw();
                return;
            }

            if event.state != ElementState::Pressed {
                return;
            }

            if state.command_palette.is_open() {
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

            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(pane_id) = PaneController::active(state) {
                        PaneController::close(state, pane_id);
                        state.window.request_redraw();
                    }
                }
                key => {
                    if let Some(slot) = hotbar_slot_for_key(key) {
                        activate_hotbar_slot(state, slot);
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::RedrawRequested => {
            if render_frame(state).is_err() {
                event_loop.exit();
                return;
            }
            let flashing_now = state.hotbar.is_flashing();
            let provider_animating = matches!(
                state.provider_runtime.mode,
                ProviderMode::Connecting | ProviderMode::Online
            );
            if flashing_now
                || state.hotbar_flash_was_active
                || provider_animating
                || state.autopilot_chat.has_pending_messages()
            {
                state.window.request_redraw();
            }
            state.hotbar_flash_was_active = flashing_now;
        }
        _ => {}
    }
}

fn dispatch_mouse_move(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let mut handled = PaneController::update_drag(state, point);
    let event = InputEvent::MouseMove {
        x: point.x,
        y: point.y,
    };

    handled |= PaneInput::dispatch_frame_event(state, &event);
    handled |= dispatch_text_inputs(state, &event);
    handled |= state
        .hotbar
        .event(&event, state.hotbar_bounds, &mut state.event_context)
        .is_handled();
    handled
}

fn dispatch_mouse_down(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
    event: &InputEvent,
) -> bool {
    let mut handled = false;
    if state.hotbar_bounds.contains(point) {
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
        handled |= state
            .hotbar
            .event(event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
        handled |= process_hotbar_clicks(state);
    }

    handled
}

fn dispatch_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = PaneInput::handle_mouse_up(state, event);
    handled |= dispatch_text_inputs(state, event);
    handled |= dispatch_pane_actions(state, point);
    handled |= state
        .hotbar
        .event(event, state.hotbar_bounds, &mut state.event_context)
        .is_handled();
    handled |= process_hotbar_clicks(state);
    handled
}

fn dispatch_text_inputs(state: &mut crate::app_state::RenderState, event: &InputEvent) -> bool {
    let mut handled = dispatch_spark_input_event(state, event);
    handled |= dispatch_pay_invoice_input_event(state, event);
    handled |= dispatch_create_invoice_input_event(state, event);
    handled |= dispatch_relay_connections_input_event(state, event);
    handled |= dispatch_network_requests_input_event(state, event);
    handled |= dispatch_settings_input_event(state, event);
    handled |= dispatch_chat_input_event(state, event);
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

fn dispatch_keyboard_submit_actions(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_chat_keyboard_input(state, logical_key)
        || handle_spark_wallet_keyboard_input(state, logical_key)
        || handle_pay_invoice_keyboard_input(state, logical_key)
        || handle_create_invoice_keyboard_input(state, logical_key)
        || handle_relay_connections_keyboard_input(state, logical_key)
        || handle_network_requests_keyboard_input(state, logical_key)
        || handle_settings_keyboard_input(state, logical_key)
        || handle_job_history_keyboard_input(state, logical_key)
}

fn run_pane_hit_action(state: &mut crate::app_state::RenderState, action: PaneHitAction) -> bool {
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
        PaneHitAction::GoOnlineToggle => {
            let wants_online = matches!(
                state.provider_runtime.mode,
                ProviderMode::Offline | ProviderMode::Degraded
            );
            if wants_online {
                queue_spark_command(state, SparkWalletCommand::Refresh);
            }
            match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline {
                online: wants_online,
            }) {
                Ok(command_seq) => {
                    state.provider_runtime.last_result =
                        Some(format!("Queued SetRunnerOnline command #{command_seq}"));
                    state.provider_runtime.last_authoritative_status = Some("pending".to_string());
                    state.provider_runtime.last_authoritative_event_id = None;
                    state.provider_runtime.last_authoritative_error_class = None;
                }
                Err(error) => {
                    state.provider_runtime.last_result = Some(error.clone());
                    state.provider_runtime.last_error_detail = Some(error);
                    state.provider_runtime.mode = ProviderMode::Degraded;
                    state.provider_runtime.degraded_reason_code =
                        Some("SA_COMMAND_QUEUE_ERROR".to_string());
                    state.provider_runtime.mode_changed_at = std::time::Instant::now();
                    state.provider_runtime.last_authoritative_status =
                        Some(RuntimeCommandStatus::Retryable.label().to_string());
                    state.provider_runtime.last_authoritative_event_id = None;
                    state.provider_runtime.last_authoritative_error_class =
                        Some(RuntimeCommandErrorClass::Transport.label().to_string());
                }
            }
            true
        }
        PaneHitAction::EarningsScoreboard(action) => run_earnings_scoreboard_action(state, action),
        PaneHitAction::RelayConnections(action) => run_relay_connections_action(state, action),
        PaneHitAction::SyncHealth(action) => run_sync_health_action(state, action),
        PaneHitAction::NetworkRequests(action) => run_network_requests_action(state, action),
        PaneHitAction::StarterJobs(action) => run_starter_jobs_action(state, action),
        PaneHitAction::ActivityFeed(action) => run_activity_feed_action(state, action),
        PaneHitAction::AlertsRecovery(action) => run_alerts_recovery_action(state, action),
        PaneHitAction::Settings(action) => run_settings_action(state, action),
        PaneHitAction::JobInbox(action) => run_job_inbox_action(state, action),
        PaneHitAction::ActiveJob(action) => run_active_job_action(state, action),
        PaneHitAction::JobHistory(action) => run_job_history_action(state, action),
        PaneHitAction::AgentProfileState(action) => run_agent_profile_state_action(state, action),
        PaneHitAction::AgentScheduleTick(action) => run_agent_schedule_tick_action(state, action),
        PaneHitAction::TrajectoryAudit(action) => run_trajectory_audit_action(state, action),
        PaneHitAction::SkillRegistry(action) => run_skill_registry_action(state, action),
        PaneHitAction::SkillTrustRevocation(action) => {
            run_skill_trust_revocation_action(state, action)
        }
        PaneHitAction::CreditDesk(action) => run_credit_desk_action(state, action),
        PaneHitAction::CreditSettlementLedger(action) => {
            run_credit_settlement_ledger_action(state, action)
        }
        PaneHitAction::Spark(action) => run_spark_action(state, action),
        PaneHitAction::SparkCreateInvoice(action) => run_create_invoice_action(state, action),
        PaneHitAction::SparkPayInvoice(action) => run_pay_invoice_action(state, action),
    }
}

fn handle_chat_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.chat_inputs.composer.is_focused(),
        dispatch_chat_input_event,
        |s| {
            if s.chat_inputs.composer.is_focused() {
                return run_chat_submit_action(s);
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
                return run_network_requests_action(s, NetworkRequestsPaneAction::SubmitRequest);
            }
            false
        },
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

fn run_chat_submit_action(state: &mut crate::app_state::RenderState) -> bool {
    let prompt = state.chat_inputs.composer.get_value().trim().to_string();
    if prompt.is_empty() {
        state.autopilot_chat.last_error = Some("Prompt cannot be empty".to_string());
        return true;
    }

    state.chat_inputs.composer.set_value(String::new());
    state
        .autopilot_chat
        .submit_prompt(std::time::Instant::now(), prompt);
    true
}

fn run_earnings_scoreboard_action(
    state: &mut crate::app_state::RenderState,
    action: EarningsScoreboardPaneAction,
) -> bool {
    match action {
        EarningsScoreboardPaneAction::Refresh => {
            refresh_earnings_scoreboard(state, std::time::Instant::now());
            true
        }
    }
}

fn run_job_inbox_action(
    state: &mut crate::app_state::RenderState,
    action: JobInboxPaneAction,
) -> bool {
    match action {
        JobInboxPaneAction::SelectRow(index) => {
            if !state.job_inbox.select_by_index(index) {
                state.job_inbox.last_error = Some("Request row out of range".to_string());
                state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        JobInboxPaneAction::AcceptSelected => {
            match state
                .job_inbox
                .decide_selected(true, "validated + queued for runtime")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_add(1);
                    state.provider_runtime.last_result =
                        Some(format!("runtime accepted request {request_id}"));
                    if let Some(request) = state
                        .job_inbox
                        .requests
                        .iter_mut()
                        .find(|request| request.request_id == request_id)
                    {
                        request.skill_scope_id = request.skill_scope_id.clone().or_else(|| {
                            state
                                .network_requests
                                .submitted
                                .first()
                                .and_then(|submitted| submitted.skill_scope_id.clone())
                        });
                        request.skl_manifest_a = request
                            .skl_manifest_a
                            .clone()
                            .or_else(|| state.skl_lane.manifest_a.clone());
                        request.skl_manifest_event_id = request
                            .skl_manifest_event_id
                            .clone()
                            .or_else(|| state.skl_lane.manifest_event_id.clone());
                        request.sa_tick_request_event_id = request
                            .sa_tick_request_event_id
                            .clone()
                            .or_else(|| state.sa_lane.last_tick_request_event_id.clone());
                        request.sa_tick_result_event_id = request
                            .sa_tick_result_event_id
                            .clone()
                            .or_else(|| state.sa_lane.last_tick_result_event_id.clone());
                        request.ac_envelope_event_id = request
                            .ac_envelope_event_id
                            .clone()
                            .or_else(|| state.ac_lane.envelope_event_id.clone());
                    }
                    let selected_request = state
                        .job_inbox
                        .requests
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .cloned();
                    if let Some(request) = selected_request.as_ref() {
                        state.active_job.start_from_request(request);
                        let _ = PaneController::create_for_kind(
                            state,
                            crate::app_state::PaneKind::ActiveJob,
                        );
                    }
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        JobInboxPaneAction::RejectSelected => {
            match state
                .job_inbox
                .decide_selected(false, "failed policy preflight")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
                    state.provider_runtime.last_result =
                        Some(format!("runtime rejected request {request_id}"));
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_active_job_action(
    state: &mut crate::app_state::RenderState,
    action: ActiveJobPaneAction,
) -> bool {
    let now = std::time::Instant::now();
    match action {
        ActiveJobPaneAction::AdvanceStage => {
            if let Ok(stage) = state.active_job.advance_stage() {
                state.provider_runtime.last_result =
                    Some(format!("active job advanced to {}", stage.label()));
                if stage == crate::app_state::JobLifecycleStage::Paid {
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_sub(1);
                    state.provider_runtime.last_completed_job_at = Some(now);
                    if let Some(job) = state.active_job.job.as_ref() {
                        state.job_history.record_from_active_job(
                            job,
                            crate::app_state::JobHistoryStatus::Succeeded,
                        );
                    }
                }
            }
            refresh_earnings_scoreboard(state, now);
            true
        }
        ActiveJobPaneAction::AbortJob => {
            if state
                .active_job
                .abort_job("operator requested abort")
                .is_ok()
            {
                state.provider_runtime.last_result = Some("active job aborted".to_string());
                state.provider_runtime.queue_depth =
                    state.provider_runtime.queue_depth.saturating_sub(1);
                state.provider_runtime.last_completed_job_at = Some(now);
                if let Some(job) = state.active_job.job.as_ref() {
                    state
                        .job_history
                        .record_from_active_job(job, crate::app_state::JobHistoryStatus::Failed);
                }
            }
            refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

fn run_job_history_action(
    state: &mut crate::app_state::RenderState,
    action: crate::pane_system::JobHistoryPaneAction,
) -> bool {
    let now = std::time::Instant::now();
    match action {
        crate::pane_system::JobHistoryPaneAction::CycleStatusFilter => {
            state.job_history.cycle_status_filter();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::CycleTimeRange => {
            state.job_history.cycle_time_range();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::PreviousPage => {
            state.job_history.previous_page();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::NextPage => {
            state.job_history.next_page();
            refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

fn run_agent_profile_state_action(
    state: &mut crate::app_state::RenderState,
    action: AgentProfileStatePaneAction,
) -> bool {
    match action {
        AgentProfileStatePaneAction::PublishProfile => {
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentProfile {
                display_name: state.agent_profile_state.profile_name.clone(),
                about: state.agent_profile_state.profile_about.clone(),
                version: "mvp".to_string(),
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Ready;
                    state.agent_profile_state.last_action =
                        Some(format!("Queued profile publish command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        AgentProfileStatePaneAction::PublishState => {
            let encrypted_state_ref = format!(
                "nip44:state:{}:{}",
                state.agent_profile_state.profile_name.to_lowercase(),
                state.agent_profile_state.profile_about.len()
            );
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
                encrypted_state_ref,
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Ready;
                    state.agent_profile_state.last_action =
                        Some(format!("Queued state publish command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        AgentProfileStatePaneAction::UpdateGoals => {
            let encrypted_state_ref = format!(
                "nip44:goals:{}",
                state.agent_profile_state.goals_summary.len()
            );
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
                encrypted_state_ref,
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Ready;
                    state.agent_profile_state.goals_event_id =
                        Some(format!("sa:goals:pending:{command_seq}"));
                    state.agent_profile_state.last_action =
                        Some(format!("Queued goals update command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_agent_schedule_tick_action(
    state: &mut crate::app_state::RenderState,
    action: AgentScheduleTickPaneAction,
) -> bool {
    match action {
        AgentScheduleTickPaneAction::ApplySchedule => {
            match state.queue_sa_command(SaLifecycleCommand::ConfigureAgentSchedule {
                heartbeat_seconds: state.agent_schedule_tick.heartbeat_seconds.max(1),
            }) {
                Ok(command_seq) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action =
                        Some(format!("Queued schedule command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        AgentScheduleTickPaneAction::PublishManualTick => {
            match state.queue_sa_command(SaLifecycleCommand::PublishTickRequest {
                reason: state.agent_schedule_tick.next_tick_reason.clone(),
            }) {
                Ok(command_seq) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action =
                        Some(format!("Queued manual tick request #{command_seq}"));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        AgentScheduleTickPaneAction::InspectLastResult => {
            state.agent_schedule_tick.last_tick_outcome = state
                .sa_lane
                .last_result
                .clone()
                .unwrap_or_else(|| "No SA tick result yet".to_string());
            state.agent_schedule_tick.last_error = None;
            state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Ready;
            state.agent_schedule_tick.last_action =
                Some("Refreshed last tick outcome from SA lane".to_string());
            true
        }
    }
}

fn run_trajectory_audit_action(
    state: &mut crate::app_state::RenderState,
    action: TrajectoryAuditPaneAction,
) -> bool {
    match action {
        TrajectoryAuditPaneAction::OpenSession => {
            let session = state
                .sa_lane
                .last_tick_request_event_id
                .as_deref()
                .map(|event| format!("traj:{event}"))
                .unwrap_or_else(|| format!("traj:manual:{}", state.sa_lane.tick_count + 1));
            state.trajectory_audit.active_session_id = Some(session.clone());
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Ready;
            state.trajectory_audit.last_action =
                Some(format!("Opened trajectory session {session}"));
            true
        }
        TrajectoryAuditPaneAction::CycleStepFilter => {
            state.trajectory_audit.step_filter =
                next_trajectory_step_filter(&state.trajectory_audit.step_filter);
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Ready;
            state.trajectory_audit.last_action = Some(format!(
                "Set trajectory filter to {}",
                state.trajectory_audit.step_filter
            ));
            true
        }
        TrajectoryAuditPaneAction::VerifyTrajectoryHash => {
            let Some(session) = state.trajectory_audit.active_session_id.as_deref() else {
                state.trajectory_audit.last_error =
                    Some("Open a trajectory session before verification".to_string());
                state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            state.trajectory_audit.verified_hash = Some(trajectory_verification_hash(
                session,
                state
                    .sa_lane
                    .last_tick_result_event_id
                    .as_deref()
                    .unwrap_or("none"),
                state.sa_lane.tick_count,
            ));
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Ready;
            state.trajectory_audit.last_action =
                Some("Verified trajectory hash from SA tick context".to_string());
            true
        }
    }
}

fn run_skill_registry_action(
    state: &mut crate::app_state::RenderState,
    action: SkillRegistryPaneAction,
) -> bool {
    match action {
        SkillRegistryPaneAction::DiscoverSkills => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch {
                query: state.skill_registry.search_query.clone(),
                limit: 8,
            }) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Ready;
                    state.skill_registry.last_action =
                        Some(format!("Queued skill discovery command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        SkillRegistryPaneAction::InspectManifest => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillManifest {
                skill_slug: state.skill_registry.manifest_slug.clone(),
                version: state.skill_registry.manifest_version.clone(),
            }) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Ready;
                    state.skill_registry.last_action =
                        Some(format!("Queued manifest inspect command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        SkillRegistryPaneAction::InstallSelectedSkill => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillVersionLog {
                skill_slug: state.skill_registry.manifest_slug.clone(),
                version: state.skill_registry.manifest_version.clone(),
                summary: "installed from skill registry pane".to_string(),
            }) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Ready;
                    state.skill_registry.manifest_a = Some(format!(
                        "33400:npub1agent:{}:{}",
                        state.skill_registry.manifest_slug, state.skill_registry.manifest_version
                    ));
                    state.skill_registry.last_action =
                        Some(format!("Queued install command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_skill_trust_revocation_action(
    state: &mut crate::app_state::RenderState,
    action: SkillTrustRevocationPaneAction,
) -> bool {
    match action {
        SkillTrustRevocationPaneAction::RefreshTrust => {
            let query = state
                .skill_trust_revocation
                .manifest_a
                .clone()
                .unwrap_or_else(|| "skill:trust.refresh".to_string());
            match state
                .queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch { query, limit: 8 })
            {
                Ok(command_seq) => {
                    state.skill_trust_revocation.last_error = None;
                    state.skill_trust_revocation.load_state =
                        crate::app_state::PaneLoadState::Ready;
                    state.skill_trust_revocation.last_action =
                        Some(format!("Queued trust refresh command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_trust_revocation.last_error = Some(error);
                    state.skill_trust_revocation.load_state =
                        crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        SkillTrustRevocationPaneAction::InspectAttestations => {
            let trust_count = match state.skl_lane.trust_tier {
                crate::runtime_lanes::SkillTrustTier::Unknown => 0,
                crate::runtime_lanes::SkillTrustTier::Provisional => 1,
                crate::runtime_lanes::SkillTrustTier::Trusted => 3,
            };
            state.skill_trust_revocation.attestation_count = trust_count;
            state.skill_trust_revocation.last_error = None;
            state.skill_trust_revocation.load_state = crate::app_state::PaneLoadState::Ready;
            state.skill_trust_revocation.last_action =
                Some(format!("Loaded {trust_count} trust attestations"));
            true
        }
        SkillTrustRevocationPaneAction::ToggleKillSwitch => {
            state.skill_trust_revocation.kill_switch_active =
                !state.skill_trust_revocation.kill_switch_active;
            state.skill_trust_revocation.trust_tier =
                if state.skill_trust_revocation.kill_switch_active {
                    "revoked".to_string()
                } else {
                    "trusted".to_string()
                };
            state.skill_trust_revocation.last_error = None;
            state.skill_trust_revocation.load_state = crate::app_state::PaneLoadState::Ready;
            state.skill_trust_revocation.last_action = Some(format!(
                "Kill-switch {}",
                if state.skill_trust_revocation.kill_switch_active {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
            true
        }
        SkillTrustRevocationPaneAction::RevokeSkill => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch {
                query: "skill:revocation".to_string(),
                limit: 1,
            }) {
                Ok(command_seq) => {
                    state.skill_trust_revocation.kill_switch_active = true;
                    state.skill_trust_revocation.trust_tier = "revoked".to_string();
                    state.skill_trust_revocation.revocation_event_id =
                        Some(format!("skl:revocation:pending:{command_seq}"));
                    state.skill_trust_revocation.last_error = None;
                    state.skill_trust_revocation.load_state =
                        crate::app_state::PaneLoadState::Ready;
                    state.skill_trust_revocation.last_action =
                        Some(format!("Queued skill revocation command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_trust_revocation.last_error = Some(error);
                    state.skill_trust_revocation.load_state =
                        crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_credit_desk_action(
    state: &mut crate::app_state::RenderState,
    action: CreditDeskPaneAction,
) -> bool {
    match action {
        CreditDeskPaneAction::PublishIntent => {
            let scope = state.credit_desk.scope.trim().to_string();
            let skill_scope_id = skill_scope_from_scope(&scope);
            match state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                scope,
                request_type: "credit.intent".to_string(),
                payload: "{\"source\":\"credit_desk\"}".to_string(),
                skill_scope_id,
                credit_envelope_ref: state.credit_desk.envelope_event_id.clone(),
                requested_sats: state.credit_desk.requested_sats.max(1),
                timeout_seconds: 60,
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued credit intent command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::PublishOffer => {
            let Some(intent_event_id) = state
                .credit_desk
                .intent_event_id
                .clone()
                .or_else(|| state.ac_lane.intent_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish intent before creating an offer".to_string());
                state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditOffer {
                intent_event_id,
                offered_sats: state.credit_desk.offered_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued credit offer command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::PublishEnvelope => {
            let Some(offer_event_id) = state
                .credit_desk
                .offer_event_id
                .clone()
                .or_else(|| state.ac_lane.offer_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish offer before creating an envelope".to_string());
                state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditEnvelope {
                offer_event_id,
                cap_sats: state.credit_desk.envelope_cap_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued envelope command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::AuthorizeSpend => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish envelope before authorizing spend".to_string());
                state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditSpendAuth {
                envelope_event_id,
                job_id: state.credit_desk.spend_job_id.clone(),
                spend_sats: state.credit_desk.spend_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued spend authorization #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_credit_settlement_ledger_action(
    state: &mut crate::app_state::RenderState,
    action: CreditSettlementLedgerPaneAction,
) -> bool {
    match action {
        CreditSettlementLedgerPaneAction::VerifySettlement => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_settlement_ledger.last_error =
                    Some("No credit envelope available for settlement".to_string());
                state.credit_settlement_ledger.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditSettlement {
                envelope_event_id,
                result_event_id: state.credit_settlement_ledger.result_event_id.clone(),
                payment_pointer: state.credit_settlement_ledger.payment_pointer.clone(),
            }) {
                Ok(command_seq) => {
                    state.credit_settlement_ledger.last_error = None;
                    state.credit_settlement_ledger.load_state =
                        crate::app_state::PaneLoadState::Ready;
                    state.credit_settlement_ledger.last_action =
                        Some(format!("Queued settlement verification #{command_seq}"));
                }
                Err(error) => {
                    state.credit_settlement_ledger.last_error = Some(error);
                    state.credit_settlement_ledger.load_state =
                        crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        CreditSettlementLedgerPaneAction::EmitDefaultNotice => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_settlement_ledger.last_error =
                    Some("No credit envelope available for default notice".to_string());
                state.credit_settlement_ledger.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditDefault {
                envelope_event_id,
                reason: state.credit_settlement_ledger.default_reason.clone(),
            }) {
                Ok(command_seq) => {
                    state.credit_settlement_ledger.last_error = None;
                    state.credit_settlement_ledger.load_state =
                        crate::app_state::PaneLoadState::Ready;
                    state.credit_settlement_ledger.last_action =
                        Some(format!("Queued default notice #{command_seq}"));
                }
                Err(error) => {
                    state.credit_settlement_ledger.last_error = Some(error);
                    state.credit_settlement_ledger.load_state =
                        crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        CreditSettlementLedgerPaneAction::EmitReputationLabel => {
            let label = if state.credit_settlement_ledger.settlement_event_id.is_some() {
                "reputation:positive:settled"
            } else if state.credit_settlement_ledger.default_event_id.is_some() {
                "reputation:negative:default"
            } else {
                "reputation:neutral:pending"
            };
            state.credit_settlement_ledger.last_error = None;
            state.credit_settlement_ledger.load_state = crate::app_state::PaneLoadState::Ready;
            state.credit_settlement_ledger.last_action =
                Some(format!("Emitted NIP-32 label {label}"));
            true
        }
    }
}

fn run_relay_connections_action(
    state: &mut crate::app_state::RenderState,
    action: RelayConnectionsPaneAction,
) -> bool {
    match action {
        RelayConnectionsPaneAction::SelectRow(index) => {
            if !state.relay_connections.select_by_index(index) {
                state.relay_connections.last_error = Some("Relay row out of range".to_string());
                state.relay_connections.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.relay_connections.load_state = crate::app_state::PaneLoadState::Ready;
            }
        }
        RelayConnectionsPaneAction::AddRelay => {
            let relay_url = state.relay_connections_inputs.relay_url.get_value();
            match state.relay_connections.add_relay(relay_url) {
                Ok(()) => {
                    state.provider_runtime.last_result =
                        state.relay_connections.last_action.clone();
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
        RelayConnectionsPaneAction::RemoveSelected => {
            match state.relay_connections.remove_selected() {
                Ok(url) => {
                    state.provider_runtime.last_result = Some(format!("removed relay {url}"));
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
        RelayConnectionsPaneAction::RetrySelected => {
            match state.relay_connections.retry_selected() {
                Ok(url) => {
                    state.provider_runtime.last_result = Some(format!("retried relay {url}"));
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
    }

    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    refresh_sync_health(state);
    true
}

fn run_sync_health_action(
    state: &mut crate::app_state::RenderState,
    action: SyncHealthPaneAction,
) -> bool {
    match action {
        SyncHealthPaneAction::Rebootstrap => {
            state.sync_health.rebootstrap();
            state.provider_runtime.last_result = state.sync_health.last_action.clone();
            refresh_sync_health(state);
            true
        }
    }
}

fn run_network_requests_action(
    state: &mut crate::app_state::RenderState,
    action: NetworkRequestsPaneAction,
) -> bool {
    match action {
        NetworkRequestsPaneAction::SubmitRequest => {
            let request_type = state
                .network_requests_inputs
                .request_type
                .get_value()
                .trim()
                .to_string();
            let payload = state
                .network_requests_inputs
                .payload
                .get_value()
                .trim()
                .to_string();
            let skill_scope_id =
                normalize_optional_text(state.network_requests_inputs.skill_scope_id.get_value());
            let credit_envelope_ref = normalize_optional_text(
                state
                    .network_requests_inputs
                    .credit_envelope_ref
                    .get_value(),
            );
            let budget_sats = match parse_positive_amount_str(
                state.network_requests_inputs.budget_sats.get_value(),
                "Budget sats",
            ) {
                Ok(value) => value,
                Err(error) => {
                    state.network_requests.last_error = Some(error);
                    state.network_requests.load_state = crate::app_state::PaneLoadState::Error;
                    return true;
                }
            };
            let timeout_seconds = match parse_positive_amount_str(
                state.network_requests_inputs.timeout_seconds.get_value(),
                "Timeout seconds",
            ) {
                Ok(value) => value,
                Err(error) => {
                    state.network_requests.last_error = Some(error);
                    state.network_requests.load_state = crate::app_state::PaneLoadState::Error;
                    return true;
                }
            };

            let scope = if let Some(skill_scope) = skill_scope_id.as_deref() {
                format!("skill:{skill_scope}:constraints")
            } else {
                format!("network:{request_type}:{budget_sats}")
            };
            let queue_result = state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                scope,
                request_type: request_type.clone(),
                payload: payload.clone(),
                skill_scope_id: skill_scope_id.clone(),
                credit_envelope_ref: credit_envelope_ref.clone(),
                requested_sats: budget_sats,
                timeout_seconds,
            });
            match queue_result {
                Ok(command_seq) => {
                    match state.network_requests.queue_request_submission(
                        &request_type,
                        &payload,
                        skill_scope_id,
                        credit_envelope_ref,
                        budget_sats,
                        timeout_seconds,
                        command_seq,
                    ) {
                        Ok(request_id) => {
                            state.provider_runtime.last_result = Some(format!(
                                "Queued network request {request_id} -> AC cmd#{command_seq}"
                            ));
                            state.sync_health.last_applied_event_seq =
                                state.sync_health.last_applied_event_seq.saturating_add(1);
                            state.sync_health.cursor_last_advanced_seconds_ago = 0;
                            refresh_sync_health(state);
                        }
                        Err(error) => {
                            state.network_requests.last_error = Some(error);
                        }
                    }
                }
                Err(error) => {
                    state.network_requests.last_error = Some(error.clone());
                    state.network_requests.mark_authority_enqueue_failure(
                        state.next_runtime_command_seq.saturating_sub(1),
                        RuntimeCommandErrorClass::Transport.label(),
                        &error,
                    );
                }
            }
            true
        }
    }
}

fn run_starter_jobs_action(
    state: &mut crate::app_state::RenderState,
    action: StarterJobsPaneAction,
) -> bool {
    match action {
        StarterJobsPaneAction::SelectRow(index) => {
            if !state.starter_jobs.select_by_index(index) {
                state.starter_jobs.last_error = Some("Starter job row out of range".to_string());
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        StarterJobsPaneAction::CompleteSelected => {
            match state.starter_jobs.complete_selected() {
                Ok((job_id, payout_sats, payout_pointer)) => {
                    state.spark_wallet.last_payment_id = Some(payout_pointer.clone());
                    state.spark_wallet.last_action = Some(format!(
                        "Starter payout settled for {job_id} ({payout_sats} sats)"
                    ));
                    state.provider_runtime.last_result =
                        Some(format!("completed starter job {job_id}"));
                    state
                        .job_history
                        .upsert_row(crate::app_state::JobHistoryReceiptRow {
                            job_id,
                            status: crate::app_state::JobHistoryStatus::Succeeded,
                            completed_at_epoch_seconds: state
                                .job_history
                                .reference_epoch_seconds
                                .saturating_add(state.job_history.rows.len() as u64 * 19),
                            skill_scope_id: state
                                .network_requests
                                .submitted
                                .first()
                                .and_then(|request| request.skill_scope_id.clone()),
                            skl_manifest_a: state.skl_lane.manifest_a.clone(),
                            skl_manifest_event_id: state.skl_lane.manifest_event_id.clone(),
                            sa_tick_result_event_id: state
                                .sa_lane
                                .last_tick_result_event_id
                                .clone(),
                            sa_trajectory_session_id: Some("traj:starter-job".to_string()),
                            ac_envelope_event_id: state.ac_lane.envelope_event_id.clone(),
                            ac_settlement_event_id: state.ac_lane.settlement_event_id.clone(),
                            ac_default_event_id: None,
                            payout_sats,
                            result_hash: "sha256:starter-job".to_string(),
                            payment_pointer: payout_pointer,
                            failure_reason: None,
                        });
                    refresh_earnings_scoreboard(state, std::time::Instant::now());
                }
                Err(error) => {
                    state.starter_jobs.last_error = Some(error);
                }
            }
            true
        }
    }
}

fn run_activity_feed_action(
    state: &mut crate::app_state::RenderState,
    action: ActivityFeedPaneAction,
) -> bool {
    match action {
        ActivityFeedPaneAction::Refresh => {
            let rows = build_activity_feed_snapshot_events(state);
            state.activity_feed.record_refresh(rows);
            true
        }
        ActivityFeedPaneAction::SetFilter(filter) => {
            state.activity_feed.set_filter(filter);
            true
        }
        ActivityFeedPaneAction::SelectRow(index) => {
            if !state.activity_feed.select_visible_row(index) {
                state.activity_feed.last_error = Some("Activity row out of range".to_string());
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
    }
}

fn run_alerts_recovery_action(
    state: &mut crate::app_state::RenderState,
    action: AlertsRecoveryPaneAction,
) -> bool {
    match action {
        AlertsRecoveryPaneAction::SelectRow(index) => {
            if !state.alerts_recovery.select_by_index(index) {
                state.alerts_recovery.last_error = Some("Alert row out of range".to_string());
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        AlertsRecoveryPaneAction::AcknowledgeSelected => {
            match state.alerts_recovery.acknowledge_selected() {
                Ok(alert_id) => {
                    state.provider_runtime.last_result = Some(format!("acknowledged {alert_id}"));
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                }
            }
            true
        }
        AlertsRecoveryPaneAction::ResolveSelected => {
            match state.alerts_recovery.resolve_selected() {
                Ok(alert_id) => {
                    state.provider_runtime.last_result = Some(format!("resolved {alert_id}"));
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                }
            }
            true
        }
        AlertsRecoveryPaneAction::RecoverSelected => {
            let Some(domain) = state.alerts_recovery.selected_domain() else {
                state.alerts_recovery.last_error = Some("Select an alert first".to_string());
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };

            let recovery = match domain {
                AlertDomain::Identity => match regenerate_identity() {
                    Ok(identity) => {
                        state.nostr_identity = Some(identity);
                        state.nostr_identity_error = None;
                        state.nostr_secret_state.revealed_until = None;
                        state.nostr_secret_state.set_copy_notice(
                            std::time::Instant::now(),
                            "Identity regenerated. Secrets are hidden by default.".to_string(),
                        );
                        queue_spark_command(state, SparkWalletCommand::Refresh);
                        Ok("Identity lane recovered".to_string())
                    }
                    Err(error) => Err(format!("Identity recovery failed: {error}")),
                },
                AlertDomain::Wallet => {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                    Ok("Wallet refresh queued".to_string())
                }
                AlertDomain::Relays => {
                    if state.relay_connections.selected_url.is_none() {
                        state.relay_connections.selected_url = state
                            .relay_connections
                            .relays
                            .first()
                            .map(|row| row.url.clone());
                    }
                    match state.relay_connections.retry_selected() {
                        Ok(url) => Ok(format!("Relay reconnect attempted for {url}")),
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::ProviderRuntime => {
                    let wants_online = matches!(
                        state.provider_runtime.mode,
                        ProviderMode::Offline | ProviderMode::Degraded
                    );
                    match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline {
                        online: wants_online,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued SA runner recovery command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::Sync => {
                    state.sync_health.rebootstrap();
                    Ok("Sync rebootstrap started".to_string())
                }
                AlertDomain::SkillTrust => {
                    match state.queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch {
                        query: "trust.recovery".to_string(),
                        limit: 8,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued SKL trust refresh command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::Credit => {
                    match state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                        scope: "credit:recovery".to_string(),
                        request_type: "credit.recovery".to_string(),
                        payload: "{\"recovery\":true}".to_string(),
                        skill_scope_id: None,
                        credit_envelope_ref: state.ac_lane.envelope_event_id.clone(),
                        requested_sats: 1200,
                        timeout_seconds: 60,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued AC credit refresh command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
            };

            match recovery {
                Ok(result) => {
                    if let Err(error) = state.alerts_recovery.resolve_selected() {
                        state.alerts_recovery.last_error = Some(error);
                        state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                    } else {
                        state.alerts_recovery.last_action = Some(result.clone());
                        state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
                        state.provider_runtime.last_result = Some(result);
                    }
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                }
            }

            refresh_sync_health(state);
            true
        }
    }
}

fn run_settings_action(
    state: &mut crate::app_state::RenderState,
    action: SettingsPaneAction,
) -> bool {
    match action {
        SettingsPaneAction::Save => {
            let relay_url = state.settings_inputs.relay_url.get_value().to_string();
            let wallet_default_send_sats = state
                .settings_inputs
                .wallet_default_send_sats
                .get_value()
                .to_string();
            let provider_max_queue_depth = state
                .settings_inputs
                .provider_max_queue_depth
                .get_value()
                .to_string();
            match state.settings.apply_updates(
                &relay_url,
                &wallet_default_send_sats,
                &provider_max_queue_depth,
            ) {
                Ok(()) => {
                    state.settings_inputs.sync_from_state(&state.settings);
                    state
                        .relay_connections_inputs
                        .relay_url
                        .set_value(state.settings.document.relay_url.clone());
                    state
                        .spark_inputs
                        .send_amount
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state
                        .pay_invoice_inputs
                        .amount_sats
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state.provider_runtime.last_result = state.settings.last_action.clone();
                    if state.settings.document.reconnect_required {
                        state.sync_health.subscription_state = "resubscribing".to_string();
                        state.sync_health.last_action = Some(
                            "Settings changed connectivity lanes; reconnect required".to_string(),
                        );
                    }
                }
                Err(error) => {
                    state.settings.last_error = Some(error);
                }
            }
            true
        }
        SettingsPaneAction::ResetDefaults => {
            match state.settings.reset_defaults() {
                Ok(()) => {
                    state.settings_inputs.sync_from_state(&state.settings);
                    state
                        .relay_connections_inputs
                        .relay_url
                        .set_value(state.settings.document.relay_url.clone());
                    state
                        .spark_inputs
                        .send_amount
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state
                        .pay_invoice_inputs
                        .amount_sats
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state.provider_runtime.last_result = state.settings.last_action.clone();
                }
                Err(error) => {
                    state.settings.last_error = Some(error);
                }
            }
            true
        }
    }
}

fn build_activity_feed_snapshot_events(
    state: &crate::app_state::RenderState,
) -> Vec<ActivityEventRow> {
    let now_epoch = state
        .job_history
        .reference_epoch_seconds
        .saturating_add(state.job_history.rows.len() as u64 * 23);
    let mut rows = Vec::new();

    for message in state.autopilot_chat.messages.iter().rev().take(6) {
        let role = match message.role {
            crate::app_state::AutopilotRole::User => "user",
            crate::app_state::AutopilotRole::Autopilot => "autopilot",
        };
        let status = match message.status {
            crate::app_state::AutopilotMessageStatus::Queued => "queued",
            crate::app_state::AutopilotMessageStatus::Running => "running",
            crate::app_state::AutopilotMessageStatus::Done => "done",
            crate::app_state::AutopilotMessageStatus::Error => "error",
        };
        rows.push(ActivityEventRow {
            event_id: format!("chat:msg:{}", message.id),
            domain: ActivityEventDomain::Chat,
            source_tag: ActivityEventDomain::Chat.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(message.id),
            summary: format!("{role} message {status}"),
            detail: message.content.clone(),
        });
    }

    for receipt in state.job_history.rows.iter().take(6) {
        rows.push(ActivityEventRow {
            event_id: format!("job:receipt:{}", receipt.job_id),
            domain: ActivityEventDomain::Job,
            source_tag: ActivityEventDomain::Job.source_tag().to_string(),
            occurred_at_epoch_seconds: receipt.completed_at_epoch_seconds,
            summary: format!(
                "{} {} sats {}",
                receipt.job_id,
                receipt.payout_sats,
                receipt.status.label()
            ),
            detail: receipt.payment_pointer.clone(),
        });
    }

    if let Some(last_payment_id) = state.spark_wallet.last_payment_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("wallet:payment:{last_payment_id}"),
            domain: ActivityEventDomain::Wallet,
            source_tag: ActivityEventDomain::Wallet.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch,
            summary: "Spark payment pointer updated".to_string(),
            detail: last_payment_id.to_string(),
        });
    }
    if let Some(wallet_action) = state.spark_wallet.last_action.as_deref() {
        rows.push(ActivityEventRow {
            event_id: "wallet:last_action".to_string(),
            domain: ActivityEventDomain::Wallet,
            source_tag: ActivityEventDomain::Wallet.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(1),
            summary: "Wallet activity".to_string(),
            detail: wallet_action.to_string(),
        });
    }

    for (idx, request) in state.network_requests.submitted.iter().take(6).enumerate() {
        rows.push(ActivityEventRow {
            event_id: format!("network:request:{}", request.request_id),
            domain: ActivityEventDomain::Network,
            source_tag: ActivityEventDomain::Network.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(20 + idx as u64 * 2),
            summary: format!("{} {}", request.request_id, request.status.label()),
            detail: format!("{} -> {}", request.request_type, request.response_stream_id),
        });
    }

    if let Some(profile_event_id) = state.sa_lane.profile_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("sa:profile:{profile_event_id}"),
            domain: ActivityEventDomain::Sa,
            source_tag: ActivityEventDomain::Sa.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(3),
            summary: format!("SA profile {}", state.sa_lane.mode.label()),
            detail: profile_event_id.to_string(),
        });
    }
    if let Some(tick_event_id) = state.sa_lane.last_tick_result_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("sa:tick:{tick_event_id}"),
            domain: ActivityEventDomain::Sa,
            source_tag: ActivityEventDomain::Sa.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(4),
            summary: format!("SA tick {}", state.sa_lane.tick_count),
            detail: tick_event_id.to_string(),
        });
    }

    if let Some(manifest) = state.skl_lane.manifest_a.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("skl:manifest:{manifest}"),
            domain: ActivityEventDomain::Skl,
            source_tag: ActivityEventDomain::Skl.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(5),
            summary: format!("SKL trust {}", state.skl_lane.trust_tier.label()),
            detail: manifest.to_string(),
        });
    }

    if let Some(intent_event_id) = state.ac_lane.intent_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("ac:intent:{intent_event_id}"),
            domain: ActivityEventDomain::Ac,
            source_tag: ActivityEventDomain::Ac.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(6),
            summary: if state.ac_lane.credit_available {
                "AC credit available".to_string()
            } else {
                "AC credit unavailable".to_string()
            },
            detail: intent_event_id.to_string(),
        });
    }

    for response in state.runtime_command_responses.iter().rev().take(12) {
        let domain = match response.lane {
            RuntimeLane::SaLifecycle => ActivityEventDomain::Sa,
            RuntimeLane::SklDiscoveryTrust => ActivityEventDomain::Skl,
            RuntimeLane::AcCredit => ActivityEventDomain::Ac,
        };
        rows.push(ActivityEventRow {
            event_id: format!("runtime:cmd:{}", response.command_seq),
            domain,
            source_tag: domain.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch
                .saturating_sub(7_u64.saturating_add(response.command_seq)),
            summary: format!("{} {}", response.command.label(), response.status.label()),
            detail: response
                .event_id
                .clone()
                .unwrap_or_else(|| "event:n/a".to_string()),
        });
    }

    rows.push(ActivityEventRow {
        event_id: format!("sync:cursor:{}", state.sync_health.last_applied_event_seq),
        domain: ActivityEventDomain::Sync,
        source_tag: ActivityEventDomain::Sync.source_tag().to_string(),
        occurred_at_epoch_seconds: now_epoch.saturating_sub(2),
        summary: format!(
            "cursor={} phase={}",
            state.sync_health.last_applied_event_seq,
            state.sync_health.recovery_phase.label()
        ),
        detail: format!(
            "stale_age={}s duplicate_drops={}",
            state.sync_health.cursor_last_advanced_seconds_ago,
            state.sync_health.duplicate_drop_count
        ),
    });

    rows
}

fn refresh_earnings_scoreboard(state: &mut crate::app_state::RenderState, now: std::time::Instant) {
    state.earnings_scoreboard.refresh_from_sources(
        now,
        &state.provider_runtime,
        &state.job_history,
        &state.spark_wallet,
    );
}

fn refresh_sync_health(state: &mut crate::app_state::RenderState) {
    state.sync_health.refresh_from_runtime(
        std::time::Instant::now(),
        &state.provider_runtime,
        &state.relay_connections,
    );
}

fn drain_runtime_lane_updates(state: &mut crate::app_state::RenderState) -> bool {
    let mut changed = false;

    for update in state.sa_lane_worker.drain_updates() {
        changed = true;
        match update {
            SaLaneUpdate::Snapshot(snapshot) => apply_sa_lane_snapshot(state, snapshot),
            SaLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.skl_lane_worker.drain_updates() {
        changed = true;
        match update {
            SklLaneUpdate::Snapshot(snapshot) => {
                apply_skl_lane_snapshot(state, snapshot);
            }
            SklLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.ac_lane_worker.drain_updates() {
        changed = true;
        match update {
            AcLaneUpdate::Snapshot(snapshot) => {
                apply_ac_lane_snapshot(state, snapshot);
            }
            AcLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    changed
}

fn apply_sa_lane_snapshot(
    state: &mut crate::app_state::RenderState,
    snapshot: crate::runtime_lanes::SaLaneSnapshot,
) {
    state.provider_runtime.mode = match snapshot.mode {
        SaRunnerMode::Offline => ProviderMode::Offline,
        SaRunnerMode::Connecting => ProviderMode::Connecting,
        SaRunnerMode::Online => ProviderMode::Online,
    };
    state.provider_runtime.mode_changed_at = snapshot.mode_changed_at;
    state.provider_runtime.connecting_until = snapshot.connect_until;
    state.provider_runtime.online_since = snapshot.online_since;
    state.provider_runtime.last_heartbeat_at = snapshot.last_heartbeat_at;
    state.provider_runtime.heartbeat_interval =
        std::time::Duration::from_secs(snapshot.heartbeat_seconds.max(1));
    state.provider_runtime.queue_depth = snapshot.queue_depth;
    state.provider_runtime.last_result = snapshot.last_result.clone();
    state.provider_runtime.degraded_reason_code = snapshot.degraded_reason_code.clone();
    state.provider_runtime.last_error_detail = snapshot.last_error_detail.clone();
    state.sa_lane = snapshot;
    sync_agent_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn apply_skl_lane_snapshot(
    state: &mut crate::app_state::RenderState,
    snapshot: crate::runtime_lanes::SklLaneSnapshot,
) {
    state.skl_lane = snapshot;
    sync_skill_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn apply_ac_lane_snapshot(
    state: &mut crate::app_state::RenderState,
    snapshot: crate::runtime_lanes::AcLaneSnapshot,
) {
    state.ac_lane = snapshot;
    sync_credit_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn sync_agent_pane_snapshots(state: &mut crate::app_state::RenderState) {
    state.agent_profile_state.profile_event_id = state.sa_lane.profile_event_id.clone();
    state.agent_profile_state.state_event_id = state.sa_lane.state_event_id.clone();
    state.agent_profile_state.goals_event_id = state.sa_lane.state_event_id.clone();
    if state.agent_profile_state.profile_event_id.is_some()
        || state.agent_profile_state.state_event_id.is_some()
    {
        state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Ready;
    }

    state.agent_schedule_tick.heartbeat_seconds = state.sa_lane.heartbeat_seconds;
    state.agent_schedule_tick.schedule_event_id = state.sa_lane.schedule_event_id.clone();
    state.agent_schedule_tick.tick_request_event_id =
        state.sa_lane.last_tick_request_event_id.clone();
    state.agent_schedule_tick.tick_result_event_id =
        state.sa_lane.last_tick_result_event_id.clone();
    if let Some(outcome) = state.sa_lane.last_result.as_deref() {
        state.agent_schedule_tick.last_tick_outcome = outcome.to_string();
    }
    if state.agent_schedule_tick.schedule_event_id.is_some() {
        state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Ready;
    }

    state.trajectory_audit.active_session_id = state
        .sa_lane
        .last_tick_request_event_id
        .as_deref()
        .map(|event| format!("traj:{event}"));
    if state.trajectory_audit.active_session_id.is_some() {
        state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Ready;
    }
}

fn sync_skill_pane_snapshots(state: &mut crate::app_state::RenderState) {
    state.skill_registry.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_registry.manifest_event_id = state.skl_lane.manifest_event_id.clone();
    state.skill_registry.version_event_id = state.skl_lane.version_log_event_id.clone();
    state.skill_registry.search_result_event_id = state.skl_lane.search_result_event_id.clone();
    if state.skill_registry.manifest_event_id.is_some()
        || state.skill_registry.search_result_event_id.is_some()
    {
        state.skill_registry.load_state = crate::app_state::PaneLoadState::Ready;
    }

    state.skill_trust_revocation.trust_tier = state.skl_lane.trust_tier.label().to_string();
    state.skill_trust_revocation.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_trust_revocation.kill_switch_active = state.skl_lane.kill_switch_active;
    state.skill_trust_revocation.revocation_event_id = state.skl_lane.revocation_event_id.clone();
    state.skill_trust_revocation.attestation_count = match state.skl_lane.trust_tier {
        crate::runtime_lanes::SkillTrustTier::Unknown => 0,
        crate::runtime_lanes::SkillTrustTier::Provisional => 1,
        crate::runtime_lanes::SkillTrustTier::Trusted => 3,
    };
    if state.skill_trust_revocation.manifest_a.is_some() {
        state.skill_trust_revocation.load_state = crate::app_state::PaneLoadState::Ready;
    }
}

fn sync_credit_pane_snapshots(state: &mut crate::app_state::RenderState) {
    state.credit_desk.intent_event_id = state.ac_lane.intent_event_id.clone();
    state.credit_desk.offer_event_id = state.ac_lane.offer_event_id.clone();
    state.credit_desk.envelope_event_id = state.ac_lane.envelope_event_id.clone();
    state.credit_desk.spend_event_id = state.ac_lane.spend_auth_event_id.clone();
    if state.credit_desk.intent_event_id.is_some() {
        state.credit_desk.load_state = crate::app_state::PaneLoadState::Ready;
    }

    state.credit_settlement_ledger.settlement_event_id = state.ac_lane.settlement_event_id.clone();
    state.credit_settlement_ledger.default_event_id = state.ac_lane.default_event_id.clone();
    if state.credit_settlement_ledger.settlement_event_id.is_some()
        || state.credit_settlement_ledger.default_event_id.is_some()
    {
        state.credit_settlement_ledger.load_state = crate::app_state::PaneLoadState::Ready;
    }
}

fn apply_runtime_command_response(
    state: &mut crate::app_state::RenderState,
    response: RuntimeCommandResponse,
) {
    let summary = command_response_summary(&response);
    match response.lane {
        RuntimeLane::SaLifecycle => {
            state.provider_runtime.last_result = Some(summary);
            state.provider_runtime.last_authoritative_status =
                Some(response.status.label().to_string());
            state.provider_runtime.last_authoritative_event_id = response.event_id.clone();
            state.provider_runtime.last_authoritative_error_class = response
                .error
                .as_ref()
                .map(|error| error.class.label().to_string());
            if response.status != RuntimeCommandStatus::Accepted {
                state.provider_runtime.last_error_detail = response
                    .error
                    .as_ref()
                    .map(|error| error.message.clone())
                    .or_else(|| Some("SA lane command rejected".to_string()));
                state.provider_runtime.mode = ProviderMode::Degraded;
                state.provider_runtime.degraded_reason_code = response
                    .error
                    .as_ref()
                    .map(|error| format!("SA_{}", error.class.label().to_ascii_uppercase()))
                    .or_else(|| Some("SA_COMMAND_REJECTED".to_string()));
                state.provider_runtime.mode_changed_at = std::time::Instant::now();
                let error = response.error.as_ref().map_or_else(
                    || "SA lane command rejected".to_string(),
                    |err| err.message.clone(),
                );
                state.agent_profile_state.last_error = Some(error.clone());
                state.agent_profile_state.load_state = crate::app_state::PaneLoadState::Error;
                state.agent_schedule_tick.last_error = Some(error.clone());
                state.agent_schedule_tick.load_state = crate::app_state::PaneLoadState::Error;
                state.trajectory_audit.last_error = Some(error);
                state.trajectory_audit.load_state = crate::app_state::PaneLoadState::Error;
            }
        }
        RuntimeLane::SklDiscoveryTrust => {
            state.sync_health.last_action = Some(summary);
            if response.status != RuntimeCommandStatus::Accepted {
                state.sync_health.last_error = response
                    .error
                    .as_ref()
                    .map(|error| format!("SKL {}: {}", error.class.label(), error.message));
                let error = response.error.as_ref().map_or_else(
                    || "SKL lane command rejected".to_string(),
                    |err| err.message.clone(),
                );
                state.skill_registry.last_error = Some(error.clone());
                state.skill_registry.load_state = crate::app_state::PaneLoadState::Error;
                state.skill_trust_revocation.last_error = Some(error);
                state.skill_trust_revocation.load_state = crate::app_state::PaneLoadState::Error;
            }
        }
        RuntimeLane::AcCredit => {
            state.network_requests.apply_authority_response(&response);
            state.provider_runtime.last_result = Some(summary);
            if response.status != RuntimeCommandStatus::Accepted {
                let error = response.error.as_ref().map_or_else(
                    || "AC lane command rejected".to_string(),
                    |err| err.message.clone(),
                );
                state.credit_desk.last_error = Some(error.clone());
                state.credit_desk.load_state = crate::app_state::PaneLoadState::Error;
                state.credit_settlement_ledger.last_error = Some(error);
                state.credit_settlement_ledger.load_state = crate::app_state::PaneLoadState::Error;
            }
        }
    }

    if response.status != RuntimeCommandStatus::Accepted {
        upsert_runtime_incident_alert(state, &response);
    }

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    state.record_runtime_command_response(response);
}

fn command_response_summary(response: &RuntimeCommandResponse) -> String {
    let mut parts = vec![format!(
        "{} {} {}",
        response.lane.label(),
        response.command.label(),
        response.status.label()
    )];
    if let Some(event_id) = response.event_id.as_deref() {
        parts.push(format!("event:{event_id}"));
    }
    if let Some(error) = response.error.as_ref() {
        parts.push(format!("{}:{}", error.class.label(), error.message));
    }
    parts.join(" | ")
}

fn upsert_runtime_incident_alert(
    state: &mut crate::app_state::RenderState,
    response: &RuntimeCommandResponse,
) {
    let domain = match response.lane {
        RuntimeLane::SaLifecycle => AlertDomain::ProviderRuntime,
        RuntimeLane::SklDiscoveryTrust => AlertDomain::SkillTrust,
        RuntimeLane::AcCredit => AlertDomain::Credit,
    };
    let severity = match response.status {
        RuntimeCommandStatus::Accepted => crate::app_state::AlertSeverity::Info,
        RuntimeCommandStatus::Retryable => crate::app_state::AlertSeverity::Warning,
        RuntimeCommandStatus::Rejected => crate::app_state::AlertSeverity::Critical,
    };
    let alert_id = format!(
        "alert:{}:{}",
        response.lane.label(),
        response.command.label()
    );
    let summary = if let Some(error) = response.error.as_ref() {
        format!(
            "{} {} ({})",
            response.command.label(),
            response.status.label(),
            error.class.label()
        )
    } else {
        format!("{} {}", response.command.label(), response.status.label())
    };
    let remediation = if let Some(error) = response.error.as_ref() {
        format!(
            "Investigate {} lane command failure: {}",
            response.lane.label(),
            error.message
        )
    } else {
        format!("Review {} lane runtime status.", response.lane.label())
    };

    if let Some(existing) = state
        .alerts_recovery
        .alerts
        .iter_mut()
        .find(|alert| alert.alert_id == alert_id)
    {
        existing.domain = domain;
        existing.severity = severity;
        existing.lifecycle = crate::app_state::AlertLifecycle::Active;
        existing.summary = summary;
        existing.remediation = remediation;
        existing.last_transition_epoch_seconds =
            existing.last_transition_epoch_seconds.saturating_add(1);
    } else {
        state
            .alerts_recovery
            .alerts
            .push(crate::app_state::RecoveryAlertRow {
                alert_id,
                domain,
                severity,
                lifecycle: crate::app_state::AlertLifecycle::Active,
                summary,
                remediation,
                last_transition_epoch_seconds: state
                    .job_history
                    .reference_epoch_seconds
                    .saturating_add(state.alerts_recovery.alerts.len() as u64 * 29),
            });
    }
    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
    state.alerts_recovery.last_error = None;
    state.alerts_recovery.last_action = Some("Updated runtime incident queue".to_string());
}

fn run_spark_action(state: &mut crate::app_state::RenderState, action: SparkPaneAction) -> bool {
    if action == SparkPaneAction::CopySparkAddress {
        state.spark_wallet.last_error = None;
        let notice = match state.spark_wallet.spark_address.as_deref() {
            Some(address) if !address.trim().is_empty() => match copy_to_clipboard(address) {
                Ok(()) => "Copied Spark address to clipboard".to_string(),
                Err(error) => format!("Failed to copy Spark address: {error}"),
            },
            _ => "No Spark address available. Generate Spark receive first.".to_string(),
        };

        if notice.starts_with("Failed") || notice.starts_with("No Spark address") {
            state.spark_wallet.last_error = Some(notice);
        } else {
            state.spark_wallet.last_action = Some(notice);
        }
        return true;
    }

    let command = match build_spark_command_for_action(
        action,
        state.spark_inputs.invoice_amount.get_value(),
        state.spark_inputs.send_request.get_value(),
        state.spark_inputs.send_amount.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

fn run_pay_invoice_action(
    state: &mut crate::app_state::RenderState,
    action: PayInvoicePaneAction,
) -> bool {
    let command = match build_pay_invoice_command(
        action,
        state.pay_invoice_inputs.payment_request.get_value(),
        state.pay_invoice_inputs.amount_sats.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

fn run_create_invoice_action(
    state: &mut crate::app_state::RenderState,
    action: CreateInvoicePaneAction,
) -> bool {
    if action == CreateInvoicePaneAction::CopyInvoice {
        state.spark_wallet.last_error = None;
        let notice = match state.spark_wallet.last_invoice.as_deref() {
            Some(invoice) if !invoice.trim().is_empty() => match copy_to_clipboard(invoice) {
                Ok(()) => "Copied invoice to clipboard".to_string(),
                Err(error) => format!("Failed to copy invoice: {error}"),
            },
            _ => "No invoice generated yet. Create one first.".to_string(),
        };

        if notice.starts_with("Failed") || notice.starts_with("No invoice generated") {
            state.spark_wallet.last_error = Some(notice);
        } else {
            state.spark_wallet.last_action = Some(notice);
        }
        return true;
    }

    let command = match build_create_invoice_command(
        action,
        state.create_invoice_inputs.amount_sats.get_value(),
        state.create_invoice_inputs.description.get_value(),
        state.create_invoice_inputs.expiry_seconds.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

fn build_spark_command_for_action(
    action: SparkPaneAction,
    invoice_amount: &str,
    send_request: &str,
    send_amount: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        SparkPaneAction::Refresh => Ok(SparkWalletCommand::Refresh),
        SparkPaneAction::GenerateSparkAddress => Ok(SparkWalletCommand::GenerateSparkAddress),
        SparkPaneAction::GenerateBitcoinAddress => Ok(SparkWalletCommand::GenerateBitcoinAddress),
        SparkPaneAction::CopySparkAddress => {
            Err("Spark copy action is handled directly in UI".to_string())
        }
        SparkPaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateInvoice {
            amount_sats: parse_positive_amount_str(invoice_amount, "Invoice amount")?,
            description: Some("OpenAgents Spark receive".to_string()),
            expiry_seconds: Some(3600),
        }),
        SparkPaneAction::SendPayment => {
            let request = validate_lightning_payment_request(send_request)?;

            let amount = if send_amount.trim().is_empty() {
                None
            } else {
                Some(parse_positive_amount_str(send_amount, "Send amount")?)
            };

            Ok(SparkWalletCommand::SendPayment {
                payment_request: request,
                amount_sats: amount,
            })
        }
    }
}

fn build_pay_invoice_command(
    action: PayInvoicePaneAction,
    payment_request: &str,
    amount_sats: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        PayInvoicePaneAction::SendPayment => {
            let request = validate_lightning_payment_request(payment_request)?;

            let amount = if amount_sats.trim().is_empty() {
                None
            } else {
                Some(parse_positive_amount_str(amount_sats, "Send amount")?)
            };

            Ok(SparkWalletCommand::SendPayment {
                payment_request: request,
                amount_sats: amount,
            })
        }
    }
}

fn build_create_invoice_command(
    action: CreateInvoicePaneAction,
    amount_sats: &str,
    description: &str,
    expiry_seconds: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        CreateInvoicePaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateInvoice {
            amount_sats: parse_positive_amount_str(amount_sats, "Invoice amount")?,
            description: normalize_optional_text(description),
            expiry_seconds: parse_optional_positive_amount_str(expiry_seconds, "Expiry seconds")?,
        }),
        CreateInvoicePaneAction::CopyInvoice => {
            Err("Copy invoice action is handled directly in UI".to_string())
        }
    }
}

fn validate_lightning_payment_request(raw: &str) -> Result<String, String> {
    let request = raw.trim();
    if request.is_empty() {
        return Err("Payment request cannot be empty".to_string());
    }

    let normalized = request.to_ascii_lowercase();
    let is_invoice = normalized.starts_with("ln")
        || normalized.starts_with("lightning:ln")
        || normalized.starts_with("lightning://ln");
    if !is_invoice {
        return Err(
            "Payment request must be a Lightning invoice (expected prefix ln...)".to_string(),
        );
    }

    Ok(request.to_string())
}

fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn next_trajectory_step_filter(current: &str) -> String {
    match current {
        "all" => "tick".to_string(),
        "tick" => "delivery".to_string(),
        "delivery" => "settlement".to_string(),
        _ => "all".to_string(),
    }
}

fn trajectory_verification_hash(session_id: &str, tick_event: &str, tick_count: u64) -> String {
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    tick_event.hash(&mut hasher);
    tick_count.hash(&mut hasher);
    format!("trajhash:{:016x}", hasher.finish())
}

fn skill_scope_from_scope(scope: &str) -> Option<String> {
    let trimmed = scope.trim();
    if !trimmed.starts_with("skill:") {
        return None;
    }
    let scope_value = trimmed.trim_start_matches("skill:");
    match scope_value.rsplit_once(':') {
        Some((skill_scope_id, _constraints_hash)) if !skill_scope_id.trim().is_empty() => {
            Some(skill_scope_id.to_string())
        }
        _ if !scope_value.is_empty() => Some(scope_value.to_string()),
        _ => None,
    }
}

fn parse_optional_positive_amount_str(raw: &str, label: &str) -> Result<Option<u64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_positive_amount_str(trimmed, label).map(Some)
}

fn queue_spark_command(state: &mut crate::app_state::RenderState, command: SparkWalletCommand) {
    state.spark_wallet.last_error = None;
    if let Err(error) = state.spark_worker.enqueue(command) {
        state.spark_wallet.last_error = Some(error);
    }
}

fn drain_spark_worker_updates(state: &mut crate::app_state::RenderState) -> bool {
    let previous_invoice = state.spark_wallet.last_invoice.clone();
    if !state.spark_worker.drain_updates(&mut state.spark_wallet) {
        return false;
    }

    if state.spark_wallet.last_invoice != previous_invoice
        && state
            .spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created invoice"))
        && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
    {
        let invoice = invoice.to_string();
        state.spark_inputs.send_request.set_value(invoice.clone());
        state.pay_invoice_inputs.payment_request.set_value(invoice);
    }

    refresh_earnings_scoreboard(state, std::time::Instant::now());
    true
}

fn parse_positive_amount_str(raw: &str, label: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }

    match trimmed.parse::<u64>() {
        Ok(value) if value > 0 => Ok(value),
        Ok(_) => Err(format!("{label} must be greater than 0")),
        Err(error) => Err(format!("{label} must be a valid integer: {error}")),
    }
}

fn spark_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.spark_inputs.invoice_amount.is_focused()
        || state.spark_inputs.send_request.is_focused()
        || state.spark_inputs.send_amount.is_focused()
}

fn pay_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.pay_invoice_inputs.payment_request.is_focused()
        || state.pay_invoice_inputs.amount_sats.is_focused()
}

fn create_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.create_invoice_inputs.amount_sats.is_focused()
        || state.create_invoice_inputs.description.is_focused()
        || state.create_invoice_inputs.expiry_seconds.is_focused()
}

fn network_requests_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.network_requests_inputs.request_type.is_focused()
        || state.network_requests_inputs.payload.is_focused()
        || state.network_requests_inputs.skill_scope_id.is_focused()
        || state
            .network_requests_inputs
            .credit_envelope_ref
            .is_focused()
        || state.network_requests_inputs.budget_sats.is_focused()
        || state.network_requests_inputs.timeout_seconds.is_focused()
}

fn settings_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.settings_inputs.relay_url.is_focused()
        || state.settings_inputs.wallet_default_send_sats.is_focused()
        || state.settings_inputs.provider_max_queue_depth.is_focused()
}

fn map_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

fn map_winit_key(logical_key: &WinitLogicalKey) -> Option<Key> {
    match logical_key {
        WinitLogicalKey::Character(text) => Some(Key::Character(text.to_string())),
        WinitLogicalKey::Named(named) => Some(Key::Named(map_named_key(*named))),
        _ => None,
    }
}

fn map_named_key(named: WinitNamedKey) -> NamedKey {
    match named {
        WinitNamedKey::Enter => NamedKey::Enter,
        WinitNamedKey::Escape => NamedKey::Escape,
        WinitNamedKey::Backspace => NamedKey::Backspace,
        WinitNamedKey::Delete => NamedKey::Delete,
        WinitNamedKey::Tab => NamedKey::Tab,
        WinitNamedKey::Space => NamedKey::Space,
        WinitNamedKey::Home => NamedKey::Home,
        WinitNamedKey::End => NamedKey::End,
        WinitNamedKey::PageUp => NamedKey::PageUp,
        WinitNamedKey::PageDown => NamedKey::PageDown,
        WinitNamedKey::ArrowUp => NamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => NamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => NamedKey::ArrowRight,
        _ => NamedKey::Unidentified,
    }
}

fn toggle_command_palette(state: &mut crate::app_state::RenderState) {
    if state.command_palette.is_open() {
        state.command_palette.close();
    } else {
        state.command_palette.open();
    }
}

fn command_palette_bounds(state: &crate::app_state::RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    Bounds::new(0.0, 0.0, logical.width, logical.height)
}

fn is_command_palette_shortcut(logical_key: &WinitLogicalKey, modifiers: Modifiers) -> bool {
    let is_k = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("k"),
        _ => false,
    };

    is_k && !modifiers.meta && !modifiers.ctrl && !modifiers.alt
}

fn dispatch_command_palette_actions(state: &mut crate::app_state::RenderState) -> bool {
    let action_ids: Vec<String> = {
        let mut queue = state.command_palette_actions.borrow_mut();
        queue.drain(..).collect()
    };
    if action_ids.is_empty() {
        return false;
    }

    let mut changed = false;
    for action in action_ids {
        let Some(spec) = pane_spec_by_command_id(&action) else {
            continue;
        };

        match spec.kind {
            crate::app_state::PaneKind::EarningsScoreboard => {
                let _ = PaneController::create_for_kind(state, spec.kind);
                refresh_earnings_scoreboard(state, std::time::Instant::now());
                changed = true;
            }
            crate::app_state::PaneKind::SyncHealth => {
                let _ = PaneController::create_for_kind(state, spec.kind);
                refresh_sync_health(state);
                changed = true;
            }
            crate::app_state::PaneKind::ActivityFeed => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::ActivityFeed);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    state
                        .activity_feed
                        .record_refresh(build_activity_feed_snapshot_events(state));
                }
                changed = true;
            }
            crate::app_state::PaneKind::AlertsRecovery => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::AlertsRecovery);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    state.alerts_recovery.last_error = None;
                    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
                    state.alerts_recovery.last_action =
                        Some("Alerts lane opened for active incident triage".to_string());
                }
                changed = true;
            }
            crate::app_state::PaneKind::NostrIdentity => {
                activate_hotbar_slot(state, HOTBAR_SLOT_NOSTR_IDENTITY);
                changed = true;
            }
            crate::app_state::PaneKind::SparkWallet => {
                activate_hotbar_slot(state, HOTBAR_SLOT_SPARK_WALLET);
                changed = true;
            }
            crate::app_state::PaneKind::SparkPayInvoice => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkPayInvoice);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            crate::app_state::PaneKind::SparkCreateInvoice => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkCreateInvoice);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            kind => {
                let _ = PaneController::create_for_kind(state, kind);
                changed = true;
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_invoice_command, build_pay_invoice_command, build_spark_command_for_action,
        is_command_palette_shortcut, parse_positive_amount_str, validate_lightning_payment_request,
    };
    use crate::spark_pane::{
        CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction, hit_action, layout,
    };
    use crate::spark_wallet::SparkWalletCommand;
    use std::collections::BTreeSet;
    use wgpui::{Bounds, Modifiers, Point};
    use winit::keyboard::Key as WinitLogicalKey;

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
            Ok(SparkWalletCommand::CreateInvoice {
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
            SparkWalletCommand::CreateInvoice {
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
            SparkWalletCommand::CreateInvoice {
                amount_sats: 1200,
                description: Some(description),
                expiry_seconds: Some(900)
            } if description == "MVP invoice"
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
}
