use wgpui::{
    Bounds, PaintContext, Point, Quad, RiveFitMode, RiveHandle, RiveSurface, theme,
};

use crate::app_state::{
    MissionControlLocalRuntimeLane, ProviderBlocker, ProviderControlHudRuntimeState,
    ProviderControlPaneState, ProviderMode, ProviderRuntimeState,
    mission_control_local_model_button_enabled, mission_control_local_runtime_is_ready,
    mission_control_local_runtime_lane, mission_control_local_runtime_view_model,
    mission_control_show_local_model_button,
};
use crate::bitcoin_display::format_sats_amount;
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_renderer::{
    mission_control_blocker_detail, mission_control_cyan_color, mission_control_green_color,
    mission_control_muted_color, mission_control_panel_border_color,
    mission_control_panel_header_color, mission_control_text_color, paint_disabled_button,
    paint_mission_control_command_button, paint_mission_control_go_online_button,
    paint_mission_control_section_panel, split_text_for_display,
};
use crate::pane_system::{
    provider_control_inventory_toggle_button_bounds, provider_control_local_fm_test_button_bounds,
    provider_control_local_model_button_bounds, provider_control_scroll_viewport_bounds,
    provider_control_toggle_button_bounds, provider_control_training_button_bounds,
};
use crate::provider_inventory::DesktopControlInventoryStatus;
use crate::rive_assets::simple_fui_hud_asset;
use crate::spark_wallet::SparkPaneState;
use crate::ui_style;

const PROVIDER_CONTROL_SECTION_HEADER_HEIGHT: f32 = 28.0;
const PROVIDER_CONTROL_SECTION_HEADER_MARGIN_BOTTOM: f32 = 10.0;
const PROVIDER_CONTROL_SECTION_BOTTOM_PADDING: f32 = 15.0;
const PROVIDER_CONTROL_SECTION_CONTENT_TOP: f32 =
    PROVIDER_CONTROL_SECTION_HEADER_HEIGHT + PROVIDER_CONTROL_SECTION_HEADER_MARGIN_BOTTOM;
const PROVIDER_CONTROL_SECTION_GAP: f32 = ui_style::spacing::SECTION_GAP;
const PROVIDER_CONTROL_ROW_LINE_HEIGHT: f32 = 18.0;
const PROVIDER_CONTROL_ROW_DIVIDER_TOP_GAP: f32 = 10.0;
const PROVIDER_CONTROL_ROW_DIVIDER_HEIGHT: f32 = 1.0;
const PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP: f32 = 10.0;
const PROVIDER_CONTROL_SELL_PANEL_HEIGHT: f32 = 158.0;

pub fn paint_provider_control_pane(
    content_bounds: Bounds,
    provider_control: &mut ProviderControlPaneState,
    provider_control_hud_runtime: &mut ProviderControlHudRuntimeState,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    backend_kernel_authority: bool,
    spark_wallet: &SparkPaneState,
    inventory_status: &DesktopControlInventoryStatus,
    paint: &mut PaintContext,
) {
    let runtime_view = mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let wants_online = matches!(
        provider_runtime.mode,
        ProviderMode::Offline | ProviderMode::Degraded
    );
    let go_online_enabled = !wants_online
        || mission_control_local_runtime_is_ready(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        );
    let toggle_label = if wants_online {
        "GO ONLINE"
    } else {
        "GO OFFLINE"
    };
    let actions_panel = provider_control_actions_panel_bounds(content_bounds);
    paint_mission_control_section_panel(
        actions_panel,
        "CONTROL ACTIONS",
        mission_control_cyan_color(),
        false,
        paint,
    );
    paint_mission_control_go_online_button(
        provider_control_toggle_button_bounds(content_bounds),
        if go_online_enabled {
            toggle_label
        } else {
            "GO ONLINE (BLOCKED)"
        },
        go_online_enabled,
        if wants_online {
            mission_control_green_color()
        } else {
            theme::status::WARNING
        },
        paint,
    );

    if mission_control_show_local_model_button(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    ) {
        let local_model_enabled = mission_control_local_model_button_enabled(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        );
        let label = if local_model_enabled {
            runtime_view.local_model_button_label.as_str()
        } else {
            "LOCAL RUNTIME UNAVAILABLE"
        };
        let local_model_bounds = provider_control_local_model_button_bounds(content_bounds);
        if local_model_enabled {
            paint_mission_control_command_button(
                local_model_bounds,
                label,
                mission_control_cyan_color(),
                true,
                paint,
            );
        } else {
            paint_disabled_button(local_model_bounds, label, paint);
        }
    }

    if mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime)
        == Some(MissionControlLocalRuntimeLane::AppleFoundationModels)
    {
        let test_label = if provider_control.local_fm_summary_is_pending() {
            "STREAMING LOCAL FM"
        } else if provider_runtime.apple_fm.is_ready() {
            "TEST LOCAL FM"
        } else {
            "LOCAL FM NOT READY"
        };
        let test_enabled =
            provider_control.local_fm_summary_is_pending() || provider_runtime.apple_fm.is_ready();
        let test_bounds = provider_control_local_fm_test_button_bounds(content_bounds);
        if test_enabled {
            paint_mission_control_command_button(
                test_bounds,
                test_label,
                mission_control_cyan_color(),
                true,
                paint,
            );
        } else {
            paint_disabled_button(test_bounds, test_label, paint);
        }
        paint_mission_control_command_button(
            provider_control_training_button_bounds(content_bounds),
            "OPEN TRAINING",
            theme::status::WARNING,
            true,
            paint,
        );
    }

    for (row_index, target) in crate::app_state::ProviderInventoryProductToggleTarget::all()
        .iter()
        .enumerate()
    {
        let enabled = provider_runtime.inventory_controls.is_advertised(*target);
        let label = if enabled {
            format!("Disable {}", target.display_label())
        } else {
            format!("Enable {}", target.display_label())
        };
        paint_mission_control_command_button(
            provider_control_inventory_toggle_button_bounds(content_bounds, row_index),
            &label,
            if enabled {
                theme::status::WARNING
            } else {
                mission_control_cyan_color()
            },
            true,
            paint,
        );
    }

    let viewport = provider_control_scroll_viewport_bounds(content_bounds);
    let mut detail_rows = vec![
        ("Mode".to_string(), provider_runtime.mode.label().to_string()),
        ("Model".to_string(), runtime_view.model_label.clone()),
        ("Backend".to_string(), runtime_view.backend_label.clone()),
        ("Load".to_string(), runtime_view.load_label.clone()),
        (
            "Control".to_string(),
            provider_runtime
                .control_authority_label(backend_kernel_authority)
                .to_string(),
        ),
        (
            "Preflight".to_string(),
            if provider_blockers.is_empty() {
                "clear".to_string()
            } else {
                format!("{} blocker(s)", provider_blockers.len())
            },
        ),
    ];
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        detail_rows.push((
            "Uptime".to_string(),
            format!("{}s", provider_runtime.uptime_seconds(std::time::Instant::now())),
        ));
    }

    let mut detail_notes = Vec::<(String, wgpui::Hsla)>::new();
    if let Some(blocker) = provider_blockers.first().copied() {
        detail_notes.push((
            format!(
                "Blocker: {}",
                mission_control_blocker_detail(blocker, spark_wallet, provider_runtime)
            ),
            theme::status::WARNING,
        ));
    }
    if let Some(action) = provider_control.last_action.as_deref() {
        detail_notes.push((format!("Last action: {action}"), mission_control_text_color()));
    }
    if let Some(error) = provider_control.last_error.as_deref() {
        detail_notes.push((format!("Error: {error}"), theme::status::ERROR));
    }
    if !provider_control.local_fm_summary_text.trim().is_empty() {
        detail_notes.push((
            format!("Local FM summary: {}", provider_control.local_fm_summary_text.trim()),
            mission_control_text_color(),
        ));
    }
    for line in crate::provider_inventory::inventory_detail_lines(inventory_status) {
        detail_notes.push((line, mission_control_muted_color()));
    }

    let detail_value_chunk_len = provider_control_value_chunk_len(viewport.size.width);
    let detail_note_chunk_len = provider_control_note_chunk_len(viewport.size.width);
    let detail_rows_height = detail_rows
        .iter()
        .enumerate()
        .map(|(index, (_, value))| {
            provider_control_detail_row_height(
                value,
                detail_value_chunk_len,
                index + 1 != detail_rows.len() || !detail_notes.is_empty(),
            )
        })
        .sum::<f32>();
    let detail_notes_height = if detail_notes.is_empty() {
        0.0
    } else {
        20.0
            + detail_notes
                .iter()
                .enumerate()
                .map(|(index, (text, _))| {
                    provider_control_note_block_height(
                        text,
                        detail_note_chunk_len,
                        index + 1 != detail_notes.len(),
                    )
                })
                .sum::<f32>()
    };
    let details_panel_height = (PROVIDER_CONTROL_SECTION_CONTENT_TOP
        + detail_rows_height
        + detail_notes_height
        + PROVIDER_CONTROL_SECTION_BOTTOM_PADDING)
        .max(220.0);
    let content_height =
        PROVIDER_CONTROL_SELL_PANEL_HEIGHT + PROVIDER_CONTROL_SECTION_GAP + details_panel_height;
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll = provider_control.clamp_scroll_offset_to(max_scroll);

    let sell_panel_bounds = Bounds::new(
        viewport.origin.x,
        viewport.origin.y - scroll,
        viewport.size.width.max(0.0),
        PROVIDER_CONTROL_SELL_PANEL_HEIGHT,
    );
    let details_panel_bounds = Bounds::new(
        viewport.origin.x,
        sell_panel_bounds.max_y() + PROVIDER_CONTROL_SECTION_GAP,
        viewport.size.width.max(0.0),
        details_panel_height,
    );

    paint.scene.push_clip(viewport);
    paint_mission_control_section_panel(
        sell_panel_bounds,
        "SELL COMPUTE",
        mission_control_green_color(),
        matches!(provider_runtime.mode, ProviderMode::Offline),
        paint,
    );
    ensure_provider_control_hud_loaded(provider_control_hud_runtime);
    sync_provider_control_hud_runtime(provider_control_hud_runtime);
    paint_provider_control_hud_overlay(
        sell_panel_bounds,
        provider_runtime,
        &runtime_view,
        go_online_enabled,
        provider_blockers,
        backend_kernel_authority,
        spark_wallet,
        inventory_status,
        provider_control_hud_runtime.last_error.as_deref(),
        paint,
    );

    paint_mission_control_section_panel(
        details_panel_bounds,
        "PROVIDER DETAILS",
        mission_control_cyan_color(),
        false,
        paint,
    );
    paint_provider_control_details_panel(
        details_panel_bounds,
        &detail_rows,
        &detail_notes,
        detail_value_chunk_len,
        detail_note_chunk_len,
        paint,
    );
    paint.scene.pop_clip();
}

fn provider_control_actions_panel_bounds(content_bounds: Bounds) -> Bounds {
    let toggle_count = crate::app_state::ProviderInventoryProductToggleTarget::all().len();
    let last_button = provider_control_inventory_toggle_button_bounds(
        content_bounds,
        toggle_count.saturating_sub(1),
    );
    Bounds::new(
        content_bounds.origin.x,
        content_bounds.origin.y,
        content_bounds.size.width.max(0.0),
        (last_button.max_y() - content_bounds.origin.y + ui_style::spacing::PANEL_PADDING)
            .max(0.0),
    )
}

fn provider_control_section_body_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + PROVIDER_CONTROL_SECTION_CONTENT_TOP,
        (bounds.size.width - 16.0).max(0.0),
        (bounds.size.height
            - PROVIDER_CONTROL_SECTION_CONTENT_TOP
            - PROVIDER_CONTROL_SECTION_BOTTOM_PADDING)
            .max(0.0),
    )
}

fn provider_control_value_chunk_len(panel_width: f32) -> usize {
    (((panel_width - 164.0).max(80.0)) / 7.2).floor().max(12.0) as usize
}

fn provider_control_note_chunk_len(panel_width: f32) -> usize {
    ((panel_width.max(120.0)) / 7.0).floor().max(18.0) as usize
}

fn provider_control_value_x_offset(label: &str) -> f32 {
    132.0_f32.max(label.chars().count() as f32 * 8.0 + 24.0)
}

fn provider_control_row_divider(
    paint: &mut PaintContext,
    x: f32,
    row_bottom: f32,
    row_width: f32,
) -> f32 {
    let divider_y = row_bottom + PROVIDER_CONTROL_ROW_DIVIDER_TOP_GAP;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            x,
            divider_y,
            row_width.max(0.0),
            PROVIDER_CONTROL_ROW_DIVIDER_HEIGHT,
        ))
        .with_background(mission_control_panel_border_color().with_alpha(0.38)),
    );
    divider_y + PROVIDER_CONTROL_ROW_DIVIDER_HEIGHT + PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP
}

fn paint_provider_control_detail_row(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
    row_width: f32,
    show_divider: bool,
) -> f32 {
    let label_x = x;
    let value_x = x + provider_control_value_x_offset(label);
    let label_style = ui_style::app_text_style(crate::ui_style::AppTextRole::FormLabel);
    let value_style = ui_style::app_text_style(crate::ui_style::AppTextRole::FormValue);
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{label}:"),
        Point::new(label_x, y),
        label_style.font_size,
        mission_control_muted_color(),
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(value_x, line_y),
            value_style.font_size,
            mission_control_text_color(),
        ));
        line_y += PROVIDER_CONTROL_ROW_LINE_HEIGHT;
    }
    let row_bottom = line_y.max(y + PROVIDER_CONTROL_ROW_LINE_HEIGHT);
    if show_divider {
        provider_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP
    }
}

fn provider_control_detail_row_height(
    value: &str,
    value_chunk_len: usize,
    show_divider: bool,
) -> f32 {
    let line_count = split_text_for_display(value, value_chunk_len.max(1))
        .len()
        .max(1) as f32;
    line_count * PROVIDER_CONTROL_ROW_LINE_HEIGHT
        + if show_divider {
            PROVIDER_CONTROL_ROW_DIVIDER_TOP_GAP
                + PROVIDER_CONTROL_ROW_DIVIDER_HEIGHT
                + PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP
        } else {
            PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP
        }
}

fn paint_provider_control_note_block(
    bounds: Bounds,
    text: &str,
    color: wgpui::Hsla,
    y: f32,
    chunk_len: usize,
    show_divider: bool,
    paint: &mut PaintContext,
) -> f32 {
    let helper_style = ui_style::app_text_style(crate::ui_style::AppTextRole::Helper);
    let mut line_y = y;
    for chunk in split_text_for_display(text, chunk_len.max(1)) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(bounds.origin.x, line_y),
            helper_style.font_size,
            color,
        ));
        line_y += 16.0;
    }
    let bottom = line_y.max(y + 16.0);
    if show_divider {
        provider_control_row_divider(paint, bounds.origin.x, bottom, bounds.size.width)
    } else {
        bottom + 8.0
    }
}

fn provider_control_note_block_height(text: &str, chunk_len: usize, show_divider: bool) -> f32 {
    let line_count = split_text_for_display(text, chunk_len.max(1)).len().max(1) as f32;
    line_count * 16.0
        + if show_divider {
            PROVIDER_CONTROL_ROW_DIVIDER_TOP_GAP
                + PROVIDER_CONTROL_ROW_DIVIDER_HEIGHT
                + PROVIDER_CONTROL_ROW_DIVIDER_BOTTOM_GAP
        } else {
            8.0
        }
}

fn paint_provider_control_details_panel(
    bounds: Bounds,
    rows: &[(String, String)],
    notes: &[(String, wgpui::Hsla)],
    value_chunk_len: usize,
    note_chunk_len: usize,
    paint: &mut PaintContext,
) {
    let body = provider_control_section_body_bounds(bounds);
    paint.scene.push_clip(body);
    let mut y = body.origin.y;
    for (index, (label, value)) in rows.iter().enumerate() {
        y = paint_provider_control_detail_row(
            paint,
            body.origin.x,
            y,
            label,
            value,
            value_chunk_len,
            body.size.width,
            index + 1 != rows.len() || !notes.is_empty(),
        );
    }
    if !notes.is_empty() {
        paint.scene.draw_text(paint.text.layout_mono(
            "Operational notes",
            Point::new(body.origin.x, y),
            9.0,
            mission_control_cyan_color(),
        ));
        y += 20.0;
        for (index, (text, color)) in notes.iter().enumerate() {
            y = paint_provider_control_note_block(
                body,
                text,
                *color,
                y,
                note_chunk_len,
                index + 1 != notes.len(),
                paint,
            );
        }
    }
    paint.scene.pop_clip();
}

fn ensure_provider_control_hud_loaded(runtime: &mut ProviderControlHudRuntimeState) {
    if runtime.surface.is_some() || runtime.last_error.is_some() {
        return;
    }

    let asset = simple_fui_hud_asset();
    match RiveSurface::from_bytes_with_handles(
        asset.bytes,
        RiveHandle::Default,
        RiveHandle::Default,
        None,
    ) {
        Ok(mut surface) => {
            surface.controller_mut().set_fit_mode(RiveFitMode::Contain);
            surface.controller_mut().pause();
            runtime.surface = Some(surface);
            runtime.last_error = None;
        }
        Err(error) => {
            runtime.last_error = Some(error.to_string());
        }
    }
}

fn sync_provider_control_hud_runtime(runtime: &mut ProviderControlHudRuntimeState) {
    let Some(surface) = runtime.surface.as_mut() else {
        return;
    };
    surface.controller_mut().set_fit_mode(RiveFitMode::Contain);
    // The production pane keeps the HUD settled on a truthful hero frame rather
    // than looping forever and forcing steady-state redraw churn.
    surface.controller_mut().pause();
}

#[expect(
    clippy::too_many_arguments,
    reason = "Provider HUD overlay binds app truth into the packaged asset shell."
)]
fn paint_provider_control_hud_overlay(
    bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
    runtime_view: &crate::app_state::MissionControlLocalRuntimeViewModel,
    go_online_enabled: bool,
    provider_blockers: &[ProviderBlocker],
    backend_kernel_authority: bool,
    spark_wallet: &SparkPaneState,
    inventory_status: &DesktopControlInventoryStatus,
    hud_error: Option<&str>,
    paint: &mut PaintContext,
) {
    let body = provider_control_section_body_bounds(bounds);
    paint.scene.push_clip(body);

    let wallet_label = spark_wallet
        .total_balance_sats()
        .map(format_sats_amount)
        .map(|balance| format!("wallet {balance}"))
        .unwrap_or_else(|| {
            if spark_wallet.balance_reconciling() {
                "wallet reconciling".to_string()
            } else {
                format!("wallet {}", spark_wallet.network_status_label())
            }
        });
    let inventory_label = format!(
        "inventory {} active",
        inventory_status.projection.compute_products_active
    );
    let control_label = provider_runtime.control_authority_label(backend_kernel_authority);
    let chip_specs = [
        (
            format!("mode {}", provider_runtime.mode.label()),
            provider_mode_color(provider_runtime.mode),
        ),
        (
            format!("backend {}", runtime_view.backend_label),
            theme::accent::PRIMARY,
        ),
        (
            wallet_label,
            if spark_wallet.balance_known() {
                theme::status::SUCCESS
            } else {
                theme::status::WARNING
            },
        ),
        (inventory_label, theme::text::MUTED),
    ];
    let mut chip_x = body.origin.x;
    let mut chip_y = body.origin.y;
    let chip_right = body.max_x();
    for (label, color) in chip_specs {
        let width = (label.len() as f32 * 6.5) + 18.0;
        let chip_width = width.min(body.size.width.max(48.0));
        if chip_x + chip_width > chip_right && chip_x > body.origin.x {
            chip_x = body.origin.x;
            chip_y += 28.0;
        }
        let chip_bounds = Bounds::new(chip_x, chip_y, chip_width, 22.0);
        paint_provider_control_hud_chip(chip_bounds, label.as_str(), color, paint);
        chip_x = chip_bounds.max_x() + 8.0;
    }

    let summary_bounds = Bounds::new(
        body.origin.x,
        chip_y + 32.0,
        body.size.width.max(0.0),
        64.0,
    );
    paint.scene.draw_quad(
        Quad::new(summary_bounds)
            .with_background(mission_control_panel_header_color().with_alpha(0.42))
            .with_border(
                provider_mode_color(provider_runtime.mode).with_alpha(0.28),
                1.0,
            )
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        runtime_view.model_label.as_str(),
        Point::new(summary_bounds.origin.x + 12.0, summary_bounds.origin.y + 12.0),
        16.0,
        mission_control_text_color(),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        runtime_view.backend_label.as_str(),
        Point::new(summary_bounds.origin.x + 12.0, summary_bounds.origin.y + 32.0),
        10.0,
        mission_control_muted_color(),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        format!("control {control_label}").as_str(),
        Point::new(summary_bounds.origin.x + 12.0, summary_bounds.origin.y + 48.0),
        9.0,
        mission_control_cyan_color(),
    ));

    let footer_label = provider_control_hud_footer(
        provider_runtime,
        runtime_view,
        go_online_enabled,
        provider_blockers,
        control_label,
        spark_wallet,
        hud_error,
    );
    let footer_color = if footer_label.starts_with("HUD asset:")
        || footer_label.starts_with("Blocker:")
        || footer_label.starts_with("Runtime:")
    {
        theme::status::WARNING
    } else {
        mission_control_text_color()
    };
    paint.scene.draw_text(paint.text.layout(
        footer_label.as_str(),
        Point::new(body.origin.x, summary_bounds.max_y() + 14.0),
        10.0,
        footer_color,
    ));
    paint.scene.pop_clip();
}

fn paint_provider_control_hud_chip(
    bounds: Bounds,
    label: &str,
    accent: wgpui::Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.76))
            .with_border(accent.with_alpha(0.48), 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 7.0),
        9.0,
        accent,
    ));
}

fn provider_control_hud_footer(
    provider_runtime: &ProviderRuntimeState,
    runtime_view: &crate::app_state::MissionControlLocalRuntimeViewModel,
    go_online_enabled: bool,
    provider_blockers: &[ProviderBlocker],
    control_label: &str,
    spark_wallet: &SparkPaneState,
    hud_error: Option<&str>,
) -> String {
    if let Some(error) = hud_error {
        return format!("HUD asset: {error}");
    }
    if let Some(blocker) = provider_blockers.first().copied() {
        return format!(
            "Blocker: {}",
            mission_control_blocker_detail(blocker, spark_wallet, provider_runtime)
        );
    }
    if !go_online_enabled {
        return format!("Runtime: {}", runtime_view.go_online_hint);
    }
    match provider_runtime.mode {
        ProviderMode::Online => format!("Online. Control authority {control_label}."),
        ProviderMode::Connecting => {
            format!("Connecting. Control authority {control_label}.")
        }
        ProviderMode::Degraded => {
            format!("Degraded. Review wallet, relay, or runtime health under {control_label}.")
        }
        ProviderMode::Offline => format!("Ready. Flip GO ONLINE when {control_label} is clear."),
    }
}

fn provider_mode_color(mode: ProviderMode) -> wgpui::Hsla {
    match mode {
        ProviderMode::Offline => theme::status::WARNING,
        ProviderMode::Connecting => theme::accent::PRIMARY,
        ProviderMode::Online => theme::status::SUCCESS,
        ProviderMode::Degraded => theme::status::ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::paint_provider_control_pane;
    use crate::app_state::{
        ProviderControlHudRuntimeState, ProviderControlPaneState, ProviderRuntimeState,
    };
    use crate::desktop_shell::DesktopShellMode;
    use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
    use crate::provider_inventory::DesktopControlInventoryStatus;
    use crate::spark_wallet::SparkPaneState;
    use wgpui::{Bounds, PaintContext, RiveFitMode, Scene, TextSystem};

    #[test]
    fn provider_control_paint_loads_packaged_hud_runtime_without_animation_churn() {
        let mut pane_state = ProviderControlPaneState::default();
        let mut hud_runtime = ProviderControlHudRuntimeState::default();
        let provider_runtime = ProviderRuntimeState::default();
        let local_runtime = LocalInferenceExecutionSnapshot::default();
        let spark_wallet = SparkPaneState::default();
        let inventory = DesktopControlInventoryStatus::default();
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint_provider_control_pane(
            Bounds::new(0.0, 0.0, 760.0, 520.0),
            &mut pane_state,
            &mut hud_runtime,
            DesktopShellMode::Production,
            &provider_runtime,
            &local_runtime,
            &[],
            false,
            &spark_wallet,
            &inventory,
            &mut paint_context,
        );

        let surface = hud_runtime.surface.as_ref().expect("provider control HUD");
        assert_eq!(surface.controller().fit_mode(), RiveFitMode::Contain);
        assert!(
            !surface.is_animating(),
            "production HUD should settle instead of forcing continuous redraws",
        );
        assert!(hud_runtime.last_error.is_none());
    }
}
