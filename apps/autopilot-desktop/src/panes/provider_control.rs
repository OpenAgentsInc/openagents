use wgpui::{Bounds, PaintContext, Point, theme};

use crate::app_state::{
    MissionControlLocalRuntimeLane, ProviderBlocker, ProviderControlPaneState,
    ProviderRuntimeState, mission_control_local_model_button_enabled,
    mission_control_local_runtime_is_ready, mission_control_local_runtime_lane,
    mission_control_local_runtime_view_model, mission_control_show_local_model_button,
};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_renderer::{
    mission_control_blocker_detail, paint_action_button, paint_source_badge, split_text_for_display,
};
use crate::pane_system::{
    provider_control_inventory_toggle_button_bounds, provider_control_local_fm_test_button_bounds,
    provider_control_local_model_button_bounds, provider_control_scroll_viewport_bounds,
    provider_control_toggle_button_bounds,
};
use crate::provider_inventory::DesktopControlInventoryStatus;
use crate::spark_wallet::SparkPaneState;

pub fn paint_provider_control_pane(
    content_bounds: Bounds,
    provider_control: &mut ProviderControlPaneState,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    backend_kernel_authority: bool,
    spark_wallet: &SparkPaneState,
    inventory_status: &DesktopControlInventoryStatus,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let runtime_view = mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let wants_online = matches!(
        provider_runtime.mode,
        crate::app_state::ProviderMode::Offline | crate::app_state::ProviderMode::Degraded
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
    paint_action_button(
        provider_control_toggle_button_bounds(content_bounds),
        if go_online_enabled {
            toggle_label
        } else {
            "GO ONLINE (BLOCKED)"
        },
        paint,
    );

    if mission_control_show_local_model_button(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    ) {
        let label = if mission_control_local_model_button_enabled(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        ) {
            runtime_view.local_model_button_label.as_str()
        } else {
            "LOCAL RUNTIME UNAVAILABLE"
        };
        paint_action_button(
            provider_control_local_model_button_bounds(content_bounds),
            label,
            paint,
        );
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
        paint_action_button(
            provider_control_local_fm_test_button_bounds(content_bounds),
            test_label,
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
        paint_action_button(
            provider_control_inventory_toggle_button_bounds(content_bounds, row_index),
            &label,
            paint,
        );
    }

    let viewport = provider_control_scroll_viewport_bounds(content_bounds);
    let mut detail_lines = vec![
        format!("Mode: {}", provider_runtime.mode.label()),
        format!("Model: {}", runtime_view.model_label),
        format!("Backend: {}", runtime_view.backend_label),
        format!("Load: {}", runtime_view.load_label),
        format!(
            "Control: {}",
            provider_runtime.control_authority_label(backend_kernel_authority)
        ),
        format!(
            "Preflight: {}",
            if provider_blockers.is_empty() {
                "clear".to_string()
            } else {
                format!("{} blocker(s)", provider_blockers.len())
            }
        ),
    ];
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        detail_lines.push(format!(
            "Uptime: {}s",
            provider_runtime.uptime_seconds(std::time::Instant::now())
        ));
    }
    if let Some(blocker) = provider_blockers.first().copied() {
        detail_lines.extend(
            split_text_for_display(
                &format!(
                    "Blocker: {}",
                    mission_control_blocker_detail(blocker, spark_wallet, provider_runtime)
                ),
                96,
            )
            .into_iter(),
        );
    }
    if let Some(action) = provider_control.last_action.as_deref() {
        detail_lines
            .extend(split_text_for_display(&format!("Last action: {action}"), 96).into_iter());
    }
    if let Some(error) = provider_control.last_error.as_deref() {
        detail_lines.extend(split_text_for_display(&format!("Error: {error}"), 96).into_iter());
    }
    if !provider_control.local_fm_summary_text.trim().is_empty() {
        detail_lines.extend(
            split_text_for_display(
                &format!(
                    "Local FM summary: {}",
                    provider_control.local_fm_summary_text.trim()
                ),
                96,
            )
            .into_iter(),
        );
    }
    for line in crate::provider_inventory::inventory_detail_lines(inventory_status) {
        detail_lines.extend(split_text_for_display(&line, 96).into_iter());
    }

    let content_height = (detail_lines.len() as f32 * 16.0) + 16.0;
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll = provider_control.clamp_scroll_offset_to(max_scroll);

    paint.scene.push_clip(viewport);
    let mut y = viewport.origin.y - scroll;
    paint.scene.draw_text(paint.text.layout(
        "Provider details",
        Point::new(viewport.origin.x + 4.0, y),
        11.0,
        theme::text::MUTED,
    ));
    y += 18.0;
    for line in detail_lines {
        paint.scene.draw_text(paint.text.layout_mono(
            &line,
            Point::new(viewport.origin.x + 4.0, y),
            10.0,
            if line.starts_with("Error:") {
                theme::status::ERROR
            } else if line.starts_with("Blocker:") {
                theme::status::WARNING
            } else {
                theme::text::PRIMARY
            },
        ));
        y += 16.0;
    }
    paint.scene.pop_clip();

    let footer_y = viewport.max_y() - 18.0;
    if footer_y > viewport.origin.y {
        paint.scene.draw_text(paint.text.layout(
            "Scroll for runtime detail and advertised inventory",
            Point::new(content_bounds.origin.x + 12.0, footer_y),
            10.0,
            theme::text::MUTED,
        ));
    }
}
