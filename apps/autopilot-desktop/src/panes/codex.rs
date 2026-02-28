use wgpui::PaintContext;

use crate::app_state::{
    CodexAccountPaneState, CodexAppsPaneState, CodexConfigPaneState, CodexDiagnosticsPaneState,
    CodexLabsPaneState, CodexMcpPaneState, CodexModelsPaneState, CodexRemoteSkillsPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    codex_account_cancel_login_button_bounds, codex_account_login_button_bounds,
    codex_account_logout_button_bounds, codex_account_rate_limits_button_bounds,
    codex_account_refresh_button_bounds, codex_apps_refresh_button_bounds, codex_apps_row_bounds,
    codex_apps_visible_row_count, codex_config_batch_write_button_bounds,
    codex_config_detect_external_button_bounds, codex_config_import_external_button_bounds,
    codex_config_read_button_bounds, codex_config_requirements_button_bounds,
    codex_config_write_button_bounds, codex_diagnostics_clear_events_button_bounds,
    codex_diagnostics_disable_wire_log_button_bounds,
    codex_diagnostics_enable_wire_log_button_bounds, codex_labs_collaboration_modes_button_bounds,
    codex_labs_command_exec_button_bounds, codex_labs_experimental_features_button_bounds,
    codex_labs_fuzzy_start_button_bounds, codex_labs_fuzzy_stop_button_bounds,
    codex_labs_fuzzy_update_button_bounds, codex_labs_realtime_append_text_button_bounds,
    codex_labs_realtime_start_button_bounds, codex_labs_realtime_stop_button_bounds,
    codex_labs_review_detached_button_bounds, codex_labs_review_inline_button_bounds,
    codex_labs_toggle_experimental_button_bounds, codex_labs_windows_sandbox_setup_button_bounds,
    codex_mcp_login_button_bounds, codex_mcp_refresh_button_bounds, codex_mcp_reload_button_bounds,
    codex_mcp_row_bounds, codex_mcp_visible_row_count, codex_models_refresh_button_bounds,
    codex_models_toggle_hidden_button_bounds, codex_remote_skills_export_button_bounds,
    codex_remote_skills_refresh_button_bounds, codex_remote_skills_row_bounds,
    codex_remote_skills_visible_row_count,
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

pub fn paint_mcp_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexMcpPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let refresh = codex_mcp_refresh_button_bounds(content_bounds);
    let login = codex_mcp_login_button_bounds(content_bounds);
    let reload = codex_mcp_reload_button_bounds(content_bounds);
    paint_action_button(refresh, "Refresh MCP", paint);
    paint_action_button(login, "OAuth Selected", paint);
    paint_action_button(reload, "Reload MCP Config", paint);

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
        "Servers",
        &pane_state.servers.len().to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Selected server",
        pane_state
            .selected_server_index
            .and_then(|idx| pane_state.servers.get(idx))
            .map(|entry| entry.name.as_str())
            .unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OAuth URL",
        pane_state.last_oauth_url.as_deref().unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OAuth result",
        pane_state.last_oauth_result.as_deref().unwrap_or("n/a"),
    );

    let visible_rows = codex_mcp_visible_row_count(pane_state.servers.len());
    for row_index in 0..visible_rows {
        let row = &pane_state.servers[row_index];
        let row_bounds = codex_mcp_row_bounds(content_bounds, row_index);
        let selected = pane_state.selected_server_index == Some(row_index);
        paint.scene.draw_quad(
            wgpui::Quad::new(row_bounds)
                .with_background(if selected {
                    wgpui::theme::bg::APP.with_alpha(0.88)
                } else {
                    wgpui::theme::bg::APP.with_alpha(0.72)
                })
                .with_border(wgpui::theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} auth={} tools={} resources={} templates={}",
                row.name, row.auth_status, row.tool_count, row.resource_count, row.template_count
            ),
            wgpui::Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                wgpui::theme::text::PRIMARY
            } else {
                wgpui::theme::text::MUTED
            },
        ));
    }

    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No MCP servers returned yet.",
            wgpui::Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            wgpui::theme::text::MUTED,
        ));
    }
}

pub fn paint_apps_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexAppsPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let refresh = codex_apps_refresh_button_bounds(content_bounds);
    paint_action_button(refresh, "Refresh Apps", paint);

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
        "Apps",
        &pane_state.apps.len().to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Updates seen",
        &pane_state.update_count.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Next cursor",
        pane_state.next_cursor.as_deref().unwrap_or("n/a"),
    );

    let visible_rows = codex_apps_visible_row_count(pane_state.apps.len());
    for row_index in 0..visible_rows {
        let app = &pane_state.apps[row_index];
        let row_bounds = codex_apps_row_bounds(content_bounds, row_index);
        let selected = pane_state.selected_app_index == Some(row_index);
        paint.scene.draw_quad(
            wgpui::Quad::new(row_bounds)
                .with_background(if selected {
                    wgpui::theme::bg::APP.with_alpha(0.88)
                } else {
                    wgpui::theme::bg::APP.with_alpha(0.72)
                })
                .with_border(wgpui::theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );
        let description = app.description.as_deref().unwrap_or("n/a");
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} ({}) accessible={} enabled={} desc={}",
                app.name, app.id, app.is_accessible, app.is_enabled, description
            ),
            wgpui::Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                wgpui::theme::text::PRIMARY
            } else {
                wgpui::theme::text::MUTED
            },
        ));
    }

    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No apps returned yet.",
            wgpui::Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            wgpui::theme::text::MUTED,
        ));
    }
}

pub fn paint_remote_skills_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexRemoteSkillsPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let refresh = codex_remote_skills_refresh_button_bounds(content_bounds);
    let export = codex_remote_skills_export_button_bounds(content_bounds);
    paint_action_button(refresh, "Refresh Remote Skills", paint);
    paint_action_button(export, "Export Selected Skill", paint);

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
        "Remote skills",
        &pane_state.skills.len().to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last exported path",
        pane_state.last_exported_path.as_deref().unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Local repo skills",
        "Primary skills registry remains `skills/` in this repo; exported remote skills are additive.",
    );

    let visible_rows = codex_remote_skills_visible_row_count(pane_state.skills.len());
    for row_index in 0..visible_rows {
        let skill = &pane_state.skills[row_index];
        let row_bounds = codex_remote_skills_row_bounds(content_bounds, row_index);
        let selected = pane_state.selected_skill_index == Some(row_index);
        paint.scene.draw_quad(
            wgpui::Quad::new(row_bounds)
                .with_background(if selected {
                    wgpui::theme::bg::APP.with_alpha(0.88)
                } else {
                    wgpui::theme::bg::APP.with_alpha(0.72)
                })
                .with_border(wgpui::theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{} ({}) {}", skill.name, skill.id, skill.description),
            wgpui::Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                wgpui::theme::text::PRIMARY
            } else {
                wgpui::theme::text::MUTED
            },
        ));
    }

    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No remote skills returned yet.",
            wgpui::Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            wgpui::theme::text::MUTED,
        ));
    }
}

pub fn paint_labs_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexLabsPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let review_inline = codex_labs_review_inline_button_bounds(content_bounds);
    let review_detached = codex_labs_review_detached_button_bounds(content_bounds);
    let command_exec = codex_labs_command_exec_button_bounds(content_bounds);
    let collaboration_modes = codex_labs_collaboration_modes_button_bounds(content_bounds);
    let experimental_features = codex_labs_experimental_features_button_bounds(content_bounds);
    let toggle_experimental = codex_labs_toggle_experimental_button_bounds(content_bounds);
    let realtime_start = codex_labs_realtime_start_button_bounds(content_bounds);
    let realtime_append_text = codex_labs_realtime_append_text_button_bounds(content_bounds);
    let realtime_stop = codex_labs_realtime_stop_button_bounds(content_bounds);
    let windows_setup = codex_labs_windows_sandbox_setup_button_bounds(content_bounds);
    let fuzzy_start = codex_labs_fuzzy_start_button_bounds(content_bounds);
    let fuzzy_update = codex_labs_fuzzy_update_button_bounds(content_bounds);
    let fuzzy_stop = codex_labs_fuzzy_stop_button_bounds(content_bounds);

    paint_action_button(review_inline, "Review Inline", paint);
    paint_action_button(review_detached, "Review Detached", paint);
    paint_action_button(command_exec, "Command Exec", paint);
    paint_action_button(collaboration_modes, "Collab Modes", paint);
    paint_action_button(experimental_features, "Experimental List", paint);
    paint_action_button(
        toggle_experimental,
        if pane_state.experimental_enabled {
            "Experimental: ON"
        } else {
            "Experimental: OFF"
        },
        paint,
    );
    paint_action_button(realtime_start, "Realtime Start", paint);
    paint_action_button(realtime_append_text, "Realtime Append", paint);
    paint_action_button(realtime_stop, "Realtime Stop", paint);
    paint_action_button(windows_setup, "Windows Sandbox Setup", paint);
    paint_action_button(fuzzy_start, "Fuzzy Start", paint);
    paint_action_button(fuzzy_update, "Fuzzy Update", paint);
    paint_action_button(fuzzy_stop, "Fuzzy Stop", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        fuzzy_stop.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Review turn",
        pane_state.review_last_turn_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Review thread",
        pane_state.review_last_thread_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Command exit",
        pane_state
            .command_last_exit_code
            .map(|value| value.to_string())
            .as_deref()
            .unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Command stdout",
        &pane_state.command_last_stdout,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Command stderr",
        &pane_state.command_last_stderr,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Collaboration modes",
        &pane_state.collaboration_modes_json,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Experimental features",
        &pane_state.experimental_features_json,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Realtime active",
        if pane_state.realtime_started {
            "true"
        } else {
            "false"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Fuzzy session id",
        &pane_state.fuzzy_session_id,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Fuzzy status",
        &pane_state.fuzzy_last_status,
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Windows setup",
        pane_state.windows_last_status.as_deref().unwrap_or("n/a"),
    );
}

pub fn paint_diagnostics_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CodexDiagnosticsPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "codex", paint);

    let enable_wire_log = codex_diagnostics_enable_wire_log_button_bounds(content_bounds);
    let disable_wire_log = codex_diagnostics_disable_wire_log_button_bounds(content_bounds);
    let clear_events = codex_diagnostics_clear_events_button_bounds(content_bounds);

    paint_action_button(enable_wire_log, "Enable Wire Log", paint);
    paint_action_button(disable_wire_log, "Disable Wire Log", paint);
    paint_action_button(clear_events, "Clear Events", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        clear_events.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Wire log enabled",
        if pane_state.wire_log_enabled {
            "true"
        } else {
            "false"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Wire log path",
        &pane_state.wire_log_path,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last command failure",
        pane_state.last_command_failure.as_deref().unwrap_or("n/a"),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last snapshot error",
        pane_state.last_snapshot_error.as_deref().unwrap_or("n/a"),
    );

    let notification_summary = pane_state
        .notification_counts
        .iter()
        .take(8)
        .map(|entry| format!("{}={}", entry.method, entry.count))
        .collect::<Vec<_>>()
        .join(" | ");
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Notification counts",
        if notification_summary.is_empty() {
            "n/a"
        } else {
            notification_summary.as_str()
        },
    );

    let request_summary = pane_state
        .server_request_counts
        .iter()
        .take(8)
        .map(|entry| format!("{}={}", entry.method, entry.count))
        .collect::<Vec<_>>()
        .join(" | ");
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Server request counts",
        if request_summary.is_empty() {
            "n/a"
        } else {
            request_summary.as_str()
        },
    );

    for event in pane_state.raw_events.iter().rev().take(10) {
        paint.scene.draw_text(paint.text.layout_mono(
            event,
            wgpui::Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            wgpui::theme::text::MUTED,
        ));
        y += 14.0;
    }
}
