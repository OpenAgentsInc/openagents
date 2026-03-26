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
    mission_control_blocker_detail, paint_action_button, split_text_for_display,
};
use crate::pane_system::{
    provider_control_inventory_toggle_button_bounds, provider_control_local_fm_test_button_bounds,
    provider_control_local_model_button_bounds, provider_control_scroll_viewport_bounds,
    provider_control_toggle_button_bounds, provider_control_training_button_bounds,
};
use crate::provider_inventory::DesktopControlInventoryStatus;
use crate::rive_assets::simple_fui_hud_asset;
use crate::spark_wallet::SparkPaneState;

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
        paint_action_button(
            provider_control_training_button_bounds(content_bounds),
            "OPEN TRAINING",
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
    let hud_height = 126.0;
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

    let content_height = hud_height + 18.0 + (detail_lines.len() as f32 * 16.0) + 16.0;
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll = provider_control.clamp_scroll_offset_to(max_scroll);

    let hud_bounds = Bounds::new(
        viewport.origin.x + 4.0,
        viewport.origin.y - scroll,
        (viewport.size.width - 8.0).max(0.0),
        hud_height,
    );

    paint.scene.push_clip(viewport);
    paint_provider_control_hud_shell(hud_bounds, paint);
    ensure_provider_control_hud_loaded(provider_control_hud_runtime);
    sync_provider_control_hud_runtime(provider_control_hud_runtime);
    paint_provider_control_hud_overlay(
        hud_bounds,
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

    let mut y = hud_bounds.max_y() + 12.0;
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

}

fn paint_provider_control_hud_shell(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.94))
            .with_border(theme::accent::PRIMARY.with_alpha(0.24), 1.0)
            .with_corner_radius(12.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 6.0,
            bounds.origin.y + 6.0,
            (bounds.size.width - 12.0).max(0.0),
            (bounds.size.height - 12.0).max(0.0),
        ))
        .with_background(theme::bg::SURFACE.with_alpha(0.24))
        .with_corner_radius(10.0),
    );
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
    let top_panel = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 12.0,
        (bounds.size.width - 24.0).max(0.0),
        46.0,
    );
    let bottom_panel = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.max_y() - 42.0,
        (bounds.size.width - 24.0).max(0.0),
        30.0,
    );
    paint.scene.draw_quad(
        Quad::new(top_panel)
            .with_background(theme::bg::APP.with_alpha(0.74))
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(bottom_panel)
            .with_background(theme::bg::APP.with_alpha(0.8))
            .with_corner_radius(8.0),
    );

    paint.scene.draw_text(paint.text.layout_mono(
        "PACKAGED HUD // WGPUI RIVE",
        Point::new(top_panel.origin.x + 10.0, top_panel.origin.y + 9.0),
        9.0,
        theme::accent::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Sell compute",
        Point::new(top_panel.origin.x + 10.0, top_panel.origin.y + 24.0),
        17.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        runtime_view.model_label.as_str(),
        Point::new(top_panel.origin.x + 168.0, top_panel.origin.y + 26.0),
        11.0,
        theme::text::MUTED,
    ));

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
    let mut chip_x = bounds.origin.x + 12.0;
    let chip_y = top_panel.max_y() + 12.0;
    for (label, color) in chip_specs {
        let width = (label.len() as f32 * 6.5) + 18.0;
        let chip_bounds = Bounds::new(chip_x, chip_y, width.min(bounds.size.width - 24.0), 22.0);
        paint_provider_control_hud_chip(chip_bounds, label.as_str(), color, paint);
        chip_x = chip_bounds.max_x() + 8.0;
    }

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
        theme::text::PRIMARY
    };
    paint.scene.draw_text(paint.text.layout(
        footer_label.as_str(),
        Point::new(bottom_panel.origin.x + 10.0, bottom_panel.origin.y + 10.0),
        10.0,
        footer_color,
    ));
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
