use super::*;

pub(super) fn spark_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.spark_inputs.invoice_amount.is_focused()
        || state.spark_inputs.send_request.is_focused()
        || state.spark_inputs.send_amount.is_focused()
}

pub(super) fn pay_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.pay_invoice_inputs.payment_request.is_focused()
        || state.pay_invoice_inputs.amount_sats.is_focused()
}

pub(super) fn create_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.create_invoice_inputs.amount_sats.is_focused()
        || state.create_invoice_inputs.description.is_focused()
        || state.create_invoice_inputs.expiry_seconds.is_focused()
}

pub(super) fn network_requests_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.network_requests_inputs.compute_family.is_focused()
        || state.network_requests_inputs.preferred_backend.is_focused()
        || state
            .network_requests_inputs
            .capability_constraints
            .is_focused()
        || state.network_requests_inputs.quantity.is_focused()
        || state
            .network_requests_inputs
            .delivery_start_minutes
            .is_focused()
        || state.network_requests_inputs.window_minutes.is_focused()
        || state.network_requests_inputs.max_price_sats.is_focused()
}

pub(super) fn settings_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.settings_inputs.relay_url.is_focused()
        || state.settings_inputs.wallet_default_send_sats.is_focused()
        || state.settings_inputs.provider_max_queue_depth.is_focused()
}

pub(super) fn local_inference_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.local_inference_inputs.prompt.is_focused()
        || state.local_inference_inputs.requested_model.is_focused()
        || state.local_inference_inputs.max_tokens.is_focused()
        || state.local_inference_inputs.temperature.is_focused()
        || state.local_inference_inputs.top_k.is_focused()
        || state.local_inference_inputs.top_p.is_focused()
}

pub(super) fn credentials_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.credentials_inputs.variable_name.is_focused()
        || state.credentials_inputs.variable_value.is_focused()
}

pub(super) fn any_text_input_focused(state: &crate::app_state::RenderState) -> bool {
    state.chat_inputs.composer.is_focused()
        || state.chat_inputs.thread_search.is_focused()
        || state.calculator_inputs.expression.is_focused()
        || spark_inputs_focused(state)
        || pay_invoice_inputs_focused(state)
        || create_invoice_inputs_focused(state)
        || network_requests_inputs_focused(state)
        || local_inference_inputs_focused(state)
        || settings_inputs_focused(state)
        || credentials_inputs_focused(state)
        || state.relay_connections_inputs.relay_url.is_focused()
        || state.job_history_inputs.search_job_id.is_focused()
}

pub(super) fn blur_non_chat_text_inputs(state: &mut crate::app_state::RenderState) {
    state.calculator_inputs.expression.blur();
    state.spark_inputs.invoice_amount.blur();
    state.spark_inputs.send_request.blur();
    state.spark_inputs.send_amount.blur();
    state.pay_invoice_inputs.payment_request.blur();
    state.pay_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.description.blur();
    state.create_invoice_inputs.expiry_seconds.blur();
    state.relay_connections_inputs.relay_url.blur();
    state.network_requests_inputs.compute_family.blur();
    state.network_requests_inputs.preferred_backend.blur();
    state.network_requests_inputs.capability_constraints.blur();
    state.network_requests_inputs.quantity.blur();
    state.network_requests_inputs.delivery_start_minutes.blur();
    state.network_requests_inputs.window_minutes.blur();
    state.network_requests_inputs.max_price_sats.blur();
    state.local_inference_inputs.prompt.blur();
    state.local_inference_inputs.requested_model.blur();
    state.local_inference_inputs.max_tokens.blur();
    state.local_inference_inputs.temperature.blur();
    state.local_inference_inputs.top_k.blur();
    state.local_inference_inputs.top_p.blur();
    state.settings_inputs.relay_url.blur();
    state.settings_inputs.wallet_default_send_sats.blur();
    state.settings_inputs.provider_max_queue_depth.blur();
    state.credentials_inputs.variable_name.blur();
    state.credentials_inputs.variable_value.blur();
    state.job_history_inputs.search_job_id.blur();
    state.chat_inputs.thread_search.blur();
}

pub(super) fn focus_chat_composer(state: &mut crate::app_state::RenderState) {
    blur_non_chat_text_inputs(state);
    state.chat_inputs.thread_search.blur();
    state.chat_inputs.composer.focus();
}

pub(super) fn map_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

pub(super) fn map_winit_key(logical_key: &WinitLogicalKey) -> Option<Key> {
    match logical_key {
        WinitLogicalKey::Character(text) => Some(Key::Character(text.to_string())),
        WinitLogicalKey::Named(named) => Some(Key::Named(map_named_key(*named))),
        _ => None,
    }
}

pub(super) fn map_named_key(named: WinitNamedKey) -> NamedKey {
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

pub(super) fn toggle_command_palette(state: &mut crate::app_state::RenderState) {
    if state.command_palette.is_open() {
        state.command_palette.close();
    } else {
        state.command_palette.open();
    }
}

pub(super) fn toggle_window_fullscreen(state: &mut crate::app_state::RenderState) {
    let next_state = if state.window.fullscreen().is_some() {
        None
    } else {
        Some(Fullscreen::Borderless(None))
    };
    state.window.set_fullscreen(next_state);
}

pub(super) fn command_palette_bounds(state: &crate::app_state::RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    Bounds::new(0.0, 0.0, logical.width, logical.height)
}

pub(super) fn is_command_palette_shortcut(
    logical_key: &WinitLogicalKey,
    modifiers: Modifiers,
) -> bool {
    let is_k = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("k"),
        _ => false,
    };

    is_k && !modifiers.meta && !modifiers.ctrl && !modifiers.alt
}

pub(super) fn should_open_command_palette(
    logical_key: &WinitLogicalKey,
    modifiers: Modifiers,
    text_input_focused: bool,
    command_palette_open: bool,
) -> bool {
    !command_palette_open
        && !text_input_focused
        && is_command_palette_shortcut(logical_key, modifiers)
}

pub(super) fn is_toggle_fullscreen_shortcut(
    logical_key: &WinitLogicalKey,
    modifiers: Modifiers,
) -> bool {
    let is_f = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("f"),
        _ => false,
    };
    if !is_f || modifiers.alt {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        modifiers.meta && !modifiers.ctrl
    }

    #[cfg(not(target_os = "macos"))]
    {
        modifiers.ctrl && !modifiers.meta
    }
}

pub(super) fn is_chat_terminal_shortcut(
    logical_key: &WinitLogicalKey,
    modifiers: Modifiers,
) -> bool {
    let is_t = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("t"),
        _ => false,
    };
    if !is_t || modifiers.alt || !modifiers.shift {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        modifiers.meta && !modifiers.ctrl
    }

    #[cfg(not(target_os = "macos"))]
    {
        modifiers.ctrl && !modifiers.meta
    }
}

pub(super) fn dispatch_command_palette_actions(state: &mut crate::app_state::RenderState) -> bool {
    let action_ids: Vec<String> = {
        let mut queue = state.command_palette_actions.borrow_mut();
        queue.drain(..).collect()
    };
    if action_ids.is_empty() {
        return false;
    }

    let mut changed = false;
    for action in action_ids {
        if let Some(cad_action) = crate::pane_system::cad_palette_action_for_command_id(&action) {
            let _ = PaneController::create_for_kind(state, PaneKind::CadDemo);
            changed |= reducers::run_cad_demo_action(state, cad_action);
            continue;
        }

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
                    let _ = state.activity_feed.reload_projection();
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
