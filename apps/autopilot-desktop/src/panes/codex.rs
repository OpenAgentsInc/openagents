use wgpui::PaintContext;

use crate::app_state::{CodexAccountPaneState, CodexConfigPaneState, CodexModelsPaneState};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    codex_account_cancel_login_button_bounds, codex_account_login_button_bounds,
    codex_account_logout_button_bounds, codex_account_rate_limits_button_bounds,
    codex_account_refresh_button_bounds, codex_config_batch_write_button_bounds,
    codex_config_detect_external_button_bounds, codex_config_import_external_button_bounds,
    codex_config_read_button_bounds, codex_config_requirements_button_bounds,
    codex_config_write_button_bounds, codex_models_refresh_button_bounds,
    codex_models_toggle_hidden_button_bounds,
};

pub fn paint_account_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexAccountPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let refresh = codex_account_refresh_button_bounds(content_bounds);
    let login = codex_account_login_button_bounds(content_bounds);
    let cancel = codex_account_cancel_login_button_bounds(content_bounds);
    let logout = codex_account_logout_button_bounds(content_bounds);
    let limits = codex_account_rate_limits_button_bounds(content_bounds);

    paint_action_button(refresh, "Refresh", paint);
    paint_action_button(login, "Login ChatGPT", paint);
    paint_action_button(cancel, "Cancel Login", paint);
    paint_action_button(logout, "Logout", paint);
    paint_action_button(limits, "Rate Limits", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        refresh.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Account",
        &pane_state.account_summary,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Auth mode",
        pane_state.auth_mode.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Requires OpenAI auth",
        if pane_state.requires_openai_auth {
            "yes"
        } else {
            "no"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Pending login id",
        pane_state.pending_login_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Pending login url",
        pane_state.pending_login_url.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rate limits",
        pane_state.rate_limits_summary.as_deref().unwrap_or("n/a"),
    );
}

pub fn paint_models_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexModelsPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let refresh = codex_models_refresh_button_bounds(content_bounds);
    let toggle_hidden = codex_models_toggle_hidden_button_bounds(content_bounds);
    paint_action_button(refresh, "Refresh Catalog", paint);
    paint_action_button(
        toggle_hidden,
        if pane_state.include_hidden {
            "Hide Hidden"
        } else {
            "Show Hidden"
        },
        paint,
    );

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        refresh.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Include hidden",
        if pane_state.include_hidden {
            "true"
        } else {
            "false"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Models",
        &pane_state.entries.len().to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last reroute",
        pane_state.last_reroute.as_deref().unwrap_or("n/a"),
    );

    for entry in pane_state.entries.iter().take(6) {
        let description = entry.description.trim();
        let line = format!(
            "{} [{}] hidden={} default={} defaultEffort={} efforts={} desc={}",
            entry.model,
            entry.display_name,
            entry.hidden,
            entry.is_default,
            entry.default_reasoning_effort,
            entry.supported_reasoning_efforts.join(","),
            if description.is_empty() {
                "n/a"
            } else {
                description
            }
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &line,
            wgpui::Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            wgpui::theme::text::MUTED,
        ));
        y += 14.0;
    }
}

pub fn paint_config_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexConfigPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let read = codex_config_read_button_bounds(content_bounds);
    let requirements = codex_config_requirements_button_bounds(content_bounds);
    let write = codex_config_write_button_bounds(content_bounds);
    let batch = codex_config_batch_write_button_bounds(content_bounds);
    let detect = codex_config_detect_external_button_bounds(content_bounds);
    let import = codex_config_import_external_button_bounds(content_bounds);

    paint_action_button(read, "Read Config", paint);
    paint_action_button(requirements, "Read Requirements", paint);
    paint_action_button(write, "Write Sample", paint);
    paint_action_button(batch, "Batch Write", paint);
    paint_action_button(detect, "Detect External", paint);
    paint_action_button(import, "Import External", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        read.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Detected external configs",
        &pane_state.detected_external_configs.to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Config",
        &pane_state.config_json,
    );
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Requirements",
        &pane_state.requirements_json,
    );
}
