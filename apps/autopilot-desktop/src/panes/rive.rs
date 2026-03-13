use std::time::Instant;

use wgpui::components::hud::{DotShape, DotsGrid, Scanlines};
use wgpui::{
    Bounds, Component, InputEvent, PaintContext, Point, Quad, RiveHandle, RiveSurface, theme,
};

use crate::app_state::{PaneKind, RenderState, RivePreviewPaneState, RivePreviewRuntimeState};
use crate::pane_renderer::{
    paint_action_button, paint_secondary_button, paint_source_badge, paint_state_summary,
    paint_tertiary_button,
};
use crate::pane_system::{
    pane_content_bounds, rive_preview_canvas_bounds, rive_preview_fit_button_bounds,
    rive_preview_metrics_bounds, rive_preview_play_button_bounds,
    rive_preview_reload_button_bounds, rive_preview_restart_button_bounds,
};
use crate::rive_assets::simple_fui_hud_asset;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut RivePreviewPaneState,
    runtime: &mut RivePreviewRuntimeState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "packaged.rive", paint);

    let reload_bounds = rive_preview_reload_button_bounds(content_bounds);
    let play_bounds = rive_preview_play_button_bounds(content_bounds);
    let restart_bounds = rive_preview_restart_button_bounds(content_bounds);
    paint_action_button(reload_bounds, "Reload asset", paint);
    paint_action_button(
        play_bounds,
        if pane_state.playing { "Pause" } else { "Play" },
        paint,
    );
    paint_action_button(restart_bounds, "Restart", paint);

    paint_fit_button(
        rive_preview_fit_button_bounds(content_bounds, 0),
        "Contain",
        pane_state.fit_mode == wgpui::RiveFitMode::Contain,
        paint,
    );
    paint_fit_button(
        rive_preview_fit_button_bounds(content_bounds, 1),
        "Cover",
        pane_state.fit_mode == wgpui::RiveFitMode::Cover,
        paint,
    );
    paint_fit_button(
        rive_preview_fit_button_bounds(content_bounds, 2),
        "Fill",
        pane_state.fit_mode == wgpui::RiveFitMode::Fill,
        paint,
    );

    paint.scene.draw_text(paint.text.layout(
        "Rive Preview",
        Point::new(
            restart_bounds.max_x() + 18.0,
            content_bounds.origin.y + 16.0,
        ),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Workbench pane for the packaged simple FUI HUD asset using the shared native RiveSurface.",
        Point::new(
            restart_bounds.max_x() + 18.0,
            content_bounds.origin.y + 34.0,
        ),
        11.0,
        theme::text::MUTED,
    ));

    ensure_runtime_loaded(pane_state, runtime);
    sync_runtime_state(pane_state, runtime);

    let summary = if pane_state.load_state == crate::app_state::PaneLoadState::Error {
        "Packaged HUD asset failed to load".to_string()
    } else if pane_state.playing {
        "Packaged HUD asset is rendering".to_string()
    } else {
        "Packaged HUD asset is paused".to_string()
    };
    let _ = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        rive_preview_fit_button_bounds(content_bounds, 0).max_y() + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    let canvas_bounds = rive_preview_canvas_bounds(content_bounds);
    let metrics_bounds = rive_preview_metrics_bounds(content_bounds);
    paint_canvas_shell(canvas_bounds, paint);
    paint_metrics_shell(metrics_bounds, paint);

    if let Some(surface) = runtime.surface.as_mut() {
        let start = Instant::now();
        surface.paint(canvas_bounds, paint);
        let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
        let metrics = surface.controller().metrics().clone();
        pane_state.load_state = crate::app_state::PaneLoadState::Ready;
        pane_state.last_error = None;
        pane_state.frame_build_ms = Some(elapsed_ms);
        pane_state.draw_call_count = metrics.command_count.min(u32::MAX as usize) as u32;
        pane_state.image_count = metrics.image_count.min(u32::MAX as usize) as u32;
        pane_state.scene_name = Some(metrics.scene_name.clone());
        paint_canvas_overlay(canvas_bounds, paint);
        paint_metrics_panel(metrics_bounds, pane_state, &metrics, paint);
    } else {
        paint.scene.draw_text(paint.text.layout(
            "No Rive runtime is loaded.",
            Point::new(canvas_bounds.origin.x + 16.0, canvas_bounds.origin.y + 20.0),
            12.0,
            theme::status::ERROR,
        ));
        paint_metrics_panel(
            metrics_bounds,
            pane_state,
            &wgpui::RiveMetrics::default(),
            paint,
        );
    }
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::RivePreview)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };
    let content_bounds = pane_content_bounds(bounds);
    let canvas_bounds = rive_preview_canvas_bounds(content_bounds);

    if let InputEvent::MouseMove { x, y }
    | InputEvent::MouseDown { x, y, .. }
    | InputEvent::MouseUp { x, y, .. } = event
    {
        state.rive_preview.last_pointer = Some(Point::new(*x, *y));
    }

    let Some(surface) = state.rive_preview_runtime.surface.as_mut() else {
        return false;
    };
    surface
        .event(event, canvas_bounds, &mut state.event_context)
        .is_handled()
}

fn ensure_runtime_loaded(
    pane_state: &mut RivePreviewPaneState,
    runtime: &mut RivePreviewRuntimeState,
) {
    if runtime.surface.is_some() {
        return;
    }

    let asset = simple_fui_hud_asset();
    let artboard_handle = handle_from_state(pane_state.artboard_name.as_deref());
    let scene_handle = handle_from_state(pane_state.state_machine_name.as_deref());
    match RiveSurface::from_bytes_with_handles(asset.bytes, artboard_handle, scene_handle, None) {
        Ok(mut surface) => {
            if !pane_state.autoplay || !pane_state.playing {
                surface.controller_mut().pause();
            }
            surface.controller_mut().set_fit_mode(pane_state.fit_mode);
            pane_state.asset_name = asset.file_name.to_string();
            pane_state.load_state = crate::app_state::PaneLoadState::Ready;
            pane_state.last_error = None;
            pane_state.last_action = Some(format!(
                "Loaded packaged HUD asset from {}",
                asset.runtime_path
            ));
            runtime.surface = Some(surface);
        }
        Err(error) => {
            pane_state.load_state = crate::app_state::PaneLoadState::Error;
            pane_state.last_error = Some(error.to_string());
            pane_state.last_action = Some("Packaged HUD asset load failed".to_string());
            runtime.surface = None;
        }
    }
}

fn sync_runtime_state(pane_state: &RivePreviewPaneState, runtime: &mut RivePreviewRuntimeState) {
    let Some(surface) = runtime.surface.as_mut() else {
        return;
    };
    surface.controller_mut().set_fit_mode(pane_state.fit_mode);
    if pane_state.playing {
        surface.controller_mut().play();
    } else {
        surface.controller_mut().pause();
    }
}

fn handle_from_state(value: Option<&str>) -> RiveHandle {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("default") => RiveHandle::Default,
        Some(value) => RiveHandle::Name(value.to_string()),
    }
}

fn paint_fit_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if active {
        paint_secondary_button(bounds, label, paint);
    } else {
        paint_tertiary_button(bounds, label, paint);
    }
}

fn paint_canvas_shell(bounds: Bounds, paint: &mut PaintContext) {
    let shell_bounds = Bounds::new(
        bounds.origin.x - 10.0,
        bounds.origin.y - 10.0,
        bounds.size.width + 20.0,
        bounds.size.height + 20.0,
    );
    paint.scene.draw_quad(
        Quad::new(shell_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::accent::PRIMARY.with_alpha(0.32), 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.92))
            .with_corner_radius(8.0),
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(1.0)
        .color(theme::accent::PRIMARY.with_alpha(0.16))
        .animation_progress(1.0);
    dots.paint(bounds, paint);
    let mut scanlines = Scanlines::new()
        .spacing(14.0)
        .line_color(theme::accent::PRIMARY.with_alpha(0.04))
        .scan_color(theme::accent::PRIMARY.with_alpha(0.12))
        .scan_width(18.0)
        .scan_progress(0.38)
        .opacity(0.76);
    scanlines.paint(bounds, paint);
}

fn paint_canvas_overlay(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "PACKAGED HUD CANVAS",
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 10.0),
        9.0,
        theme::text::MUTED,
    ));
}

fn paint_metrics_shell(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(10.0),
    );
}

fn paint_metrics_panel(
    bounds: Bounds,
    pane_state: &RivePreviewPaneState,
    metrics: &wgpui::RiveMetrics,
    paint: &mut PaintContext,
) {
    let lines = [
        format!("asset         {}", pane_state.asset_name),
        format!(
            "artboard      {}",
            pane_state.artboard_name.as_deref().unwrap_or("default")
        ),
        format!(
            "scene         {}",
            pane_state
                .state_machine_name
                .as_deref()
                .unwrap_or("default")
        ),
        format!("fit           {}", fit_label(pane_state.fit_mode)),
        format!(
            "playback      {}",
            if pane_state.playing {
                "playing"
            } else {
                "paused"
            }
        ),
        format!("scene label   {}", metrics.scene_name),
        format!(
            "frame build   {}",
            pane_state
                .frame_build_ms
                .map(|value| format!("{value:.2} ms"))
                .unwrap_or_else(|| "-".to_string())
        ),
        format!("commands      {}", pane_state.draw_call_count),
        format!("images        {}", pane_state.image_count),
        format!(
            "canvas size   {:.0} x {:.0}",
            metrics.artboard_size.width, metrics.artboard_size.height
        ),
        format!(
            "pointer       {}",
            pane_state
                .last_pointer
                .map(|point| format!("{:.0},{:.0}", point.x, point.y))
                .unwrap_or_else(|| "-".to_string())
        ),
    ];

    paint.scene.draw_text(paint.text.layout_mono(
        "RIVE METRICS",
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 16.0),
        11.0,
        theme::accent::PRIMARY,
    ));
    for (index, line) in lines.iter().enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(
                bounds.origin.x + 14.0,
                bounds.origin.y + 42.0 + index as f32 * 18.0,
            ),
            10.0,
            theme::text::PRIMARY,
        ));
    }
}

fn fit_label(fit_mode: wgpui::RiveFitMode) -> &'static str {
    match fit_mode {
        wgpui::RiveFitMode::Contain => "contain",
        wgpui::RiveFitMode::Cover => "cover",
        wgpui::RiveFitMode::Fill => "fill",
    }
}

#[cfg(test)]
mod tests {
    use super::paint;
    use crate::app_state::{PaneLoadState, RivePreviewPaneState, RivePreviewRuntimeState};
    use wgpui::{Bounds, PaintContext, Scene, TextSystem};

    #[test]
    fn packaged_rive_preview_paint_loads_runtime_and_updates_metrics() {
        let mut pane_state = RivePreviewPaneState::default();
        let mut runtime = RivePreviewRuntimeState::default();
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint(
            Bounds::new(0.0, 0.0, 1080.0, 700.0),
            &mut pane_state,
            &mut runtime,
            &mut paint_context,
        );

        assert!(
            runtime.surface.is_some(),
            "pane paint should load the packaged HUD asset"
        );
        assert_eq!(pane_state.load_state, PaneLoadState::Ready);
        assert!(
            pane_state.draw_call_count > 0 || pane_state.image_count > 0,
            "pane paint should produce drawable content",
        );
        assert!(pane_state.frame_build_ms.is_some());
    }
}
