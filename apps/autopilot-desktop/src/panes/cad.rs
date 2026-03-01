use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{CadDemoPaneState, CadDemoWarningState};
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{
    cad_demo_cycle_variant_button_bounds, cad_demo_reset_button_bounds,
    cad_demo_timeline_panel_bounds, cad_demo_timeline_row_bounds,
    cad_demo_warning_filter_code_button_bounds, cad_demo_warning_filter_severity_button_bounds,
    cad_demo_warning_marker_bounds, cad_demo_warning_panel_bounds, cad_demo_warning_row_bounds,
};

const PAD: f32 = 12.0;
const HEADER_LINE_HEIGHT: f32 = 14.0;
const SUBHEADER_GAP: f32 = 4.0;
const VIEWPORT_TOP_GAP: f32 = 10.0;
const FOOTER_RESERVED: f32 = 18.0;
const RECEIPT_LINE_HEIGHT: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadDemoPlaceholderLayout {
    pub header_origin: Point,
    pub subheader_origin: Point,
    pub viewport_bounds: Bounds,
    pub footer_origin: Point,
}

/// Compute a bounded placeholder layout that never exceeds pane content bounds.
pub fn placeholder_layout(content_bounds: Bounds) -> CadDemoPlaceholderLayout {
    let max_x = content_bounds.max_x();
    let max_y = content_bounds.max_y();
    let inner_width = (content_bounds.size.width - PAD * 2.0).max(0.0);

    let header_y = (content_bounds.origin.y + PAD).min(max_y);
    let subheader_y = (header_y + HEADER_LINE_HEIGHT + SUBHEADER_GAP).min(max_y);
    let viewport_top = (subheader_y + HEADER_LINE_HEIGHT + VIEWPORT_TOP_GAP).min(max_y);
    let viewport_bottom = (max_y - FOOTER_RESERVED).max(viewport_top);
    let viewport_height = (viewport_bottom - viewport_top).max(0.0);
    let viewport_bounds = Bounds::new(
        (content_bounds.origin.x + PAD).min(max_x),
        viewport_top,
        inner_width,
        viewport_height,
    );

    let footer_y = (max_y - 12.0).max(content_bounds.origin.y);
    CadDemoPlaceholderLayout {
        header_origin: Point::new(content_bounds.origin.x + PAD, header_y),
        subheader_origin: Point::new(content_bounds.origin.x + PAD, subheader_y),
        viewport_bounds,
        footer_origin: Point::new(content_bounds.origin.x + PAD, footer_y),
    }
}

pub fn paint_cad_demo_placeholder_pane(
    content_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) {
    let cycle_bounds = cad_demo_cycle_variant_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let warning_panel = cad_demo_warning_panel_bounds(content_bounds);
    let timeline_panel = cad_demo_timeline_panel_bounds(content_bounds);
    let severity_filter_bounds = cad_demo_warning_filter_severity_button_bounds(content_bounds);
    let code_filter_bounds = cad_demo_warning_filter_code_button_bounds(content_bounds);
    paint_action_button(cycle_bounds, "Cycle Variant", paint);
    paint_action_button(reset_bounds, "Reset Session", paint);
    paint_action_button(
        severity_filter_bounds,
        &format!("Severity: {}", pane_state.warning_filter_severity),
        paint,
    );
    paint_action_button(
        code_filter_bounds,
        &format!("Code: {}", pane_state.warning_filter_code),
        paint,
    );

    let body_top =
        (cycle_bounds.max_y().max(reset_bounds.max_y()) + 8.0).min(content_bounds.max_y());
    let body_bounds = Bounds::new(
        content_bounds.origin.x,
        body_top,
        content_bounds.size.width,
        (content_bounds.max_y() - body_top).max(0.0),
    );
    let layout = placeholder_layout(body_bounds);

    if layout.header_origin.y + 10.0 <= content_bounds.max_y() {
        paint.scene.draw_text(paint.text.layout(
            "CAD demo placeholder",
            layout.header_origin,
            12.0,
            theme::text::PRIMARY,
        ));
    }

    if layout.subheader_origin.y + 10.0 <= content_bounds.max_y() {
        let variant_count = pane_state.variant_ids.len();
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "doc={} rev={} variants={variant_count}",
                pane_state.document_id, pane_state.document_revision
            ),
            layout.subheader_origin,
            10.0,
            theme::text::MUTED,
        ));
    }

    if layout.viewport_bounds.size.width > 1.0 && layout.viewport_bounds.size.height > 1.0 {
        paint.scene.draw_quad(
            Quad::new(layout.viewport_bounds)
                .with_background(theme::bg::ELEVATED)
                .with_corner_radius(4.0),
        );

        let viewport_label = Point::new(
            layout.viewport_bounds.origin.x + 8.0,
            (layout.viewport_bounds.origin.y + 10.0).min(layout.viewport_bounds.max_y()),
        );
        if viewport_label.y + 9.0 <= content_bounds.max_y() {
            paint.scene.draw_text(paint.text.layout(
                "Viewport reserved for CAD geometry",
                viewport_label,
                10.0,
                theme::text::MUTED,
            ));
        }

        if let Some(receipt) = pane_state.last_rebuild_receipt.as_ref() {
            let latest_label = Point::new(
                viewport_label.x,
                viewport_label.y + RECEIPT_LINE_HEIGHT + 2.0,
            );
            if latest_label.y + 9.0 <= layout.viewport_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &format!(
                        "latest rebuild {}ms hash={} mesh={} tris={} cache(h={},m={},e={})",
                        receipt.duration_ms,
                        receipt.rebuild_hash,
                        receipt.mesh_hash,
                        receipt.triangle_count,
                        receipt.cache_hits,
                        receipt.cache_misses,
                        receipt.cache_evictions
                    ),
                    latest_label,
                    9.0,
                    theme::text::SECONDARY,
                ));
            }
        }

        let mut timeline_y = viewport_label.y + (RECEIPT_LINE_HEIGHT * 2.0) + 4.0;
        for receipt in pane_state.rebuild_receipts.iter().rev().take(3) {
            if timeline_y + 8.0 > layout.viewport_bounds.max_y() {
                break;
            }
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "#{} rev={} {}ms {}",
                    receipt.event_id,
                    receipt.document_revision,
                    receipt.duration_ms,
                    receipt.variant_id
                ),
                Point::new(viewport_label.x, timeline_y),
                9.0,
                theme::text::MUTED,
            ));
            timeline_y += RECEIPT_LINE_HEIGHT;
        }

        if let Some(mesh_id) = pane_state.last_good_mesh_id.as_ref() {
            let mesh_line = Point::new(
                viewport_label.x,
                viewport_label.y + (RECEIPT_LINE_HEIGHT * 5.0),
            );
            if mesh_line.y + 8.0 <= layout.viewport_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &format!("last-good mesh: {mesh_id}"),
                    mesh_line,
                    9.0,
                    theme::text::MUTED,
                ));
            }
        }

        if let Some(request_id) = pane_state.pending_rebuild_request_id {
            let pending_line = Point::new(
                viewport_label.x,
                viewport_label.y + (RECEIPT_LINE_HEIGHT * 6.0),
            );
            if pending_line.y + 8.0 <= layout.viewport_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &format!("pending rebuild request: #{request_id}"),
                    pending_line,
                    9.0,
                    theme::text::SECONDARY,
                ));
            }
        }

        let visible_warning_indices = visible_warning_indices(pane_state);
        for (marker_index, warning_index) in visible_warning_indices.iter().take(8).enumerate() {
            let marker_bounds = cad_demo_warning_marker_bounds(content_bounds, marker_index);
            let warning = &pane_state.warnings[*warning_index];
            let mut marker = Quad::new(marker_bounds).with_corner_radius(2.0);
            marker = marker.with_background(warning_color(warning));
            if pane_state.focused_warning_index == Some(*warning_index)
                || pane_state.warning_hover_index == Some(*warning_index)
            {
                marker = marker.with_border(theme::text::PRIMARY, 1.0);
            }
            paint.scene.draw_quad(marker);
        }
    }

    if warning_panel.size.width > 2.0 && warning_panel.size.height > 2.0 {
        paint.scene.draw_quad(
            Quad::new(warning_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        let visible_warning_indices = visible_warning_indices(pane_state);
        for (row_index, warning_index) in visible_warning_indices.iter().take(8).enumerate() {
            let row_bounds = cad_demo_warning_row_bounds(content_bounds, row_index);
            if row_bounds.max_y() > warning_panel.max_y() {
                break;
            }
            let warning = &pane_state.warnings[*warning_index];
            if pane_state.focused_warning_index == Some(*warning_index) {
                paint.scene.draw_quad(
                    Quad::new(row_bounds)
                        .with_background(theme::bg::ELEVATED)
                        .with_corner_radius(3.0),
                );
            }
            let row_text = format!("[{}] {}", warning.severity, warning.code);
            paint.scene.draw_text(paint.text.layout(
                &row_text,
                Point::new(row_bounds.origin.x + 4.0, row_bounds.origin.y + 9.0),
                9.0,
                warning_color(warning),
            ));
            let detail_y = row_bounds.origin.y + 18.0;
            if detail_y + 8.0 <= warning_panel.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &warning.message,
                    Point::new(row_bounds.origin.x + 4.0, detail_y),
                    8.0,
                    theme::text::MUTED,
                ));
            }
        }
    }

    if timeline_panel.size.width > 2.0 && timeline_panel.size.height > 2.0 {
        paint.scene.draw_quad(
            Quad::new(timeline_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            "Feature Timeline",
            Point::new(
                timeline_panel.origin.x + 6.0,
                timeline_panel.origin.y + 10.0,
            ),
            10.0,
            theme::text::PRIMARY,
        ));

        for visible_index in 0..10 {
            let actual_index = pane_state.timeline_scroll_offset + visible_index;
            if actual_index >= pane_state.timeline_rows.len() {
                break;
            }
            let row = &pane_state.timeline_rows[actual_index];
            let row_bounds = cad_demo_timeline_row_bounds(content_bounds, visible_index);
            if row_bounds.max_y() > timeline_panel.max_y() {
                break;
            }
            if pane_state.timeline_selected_index == Some(actual_index) {
                paint.scene.draw_quad(
                    Quad::new(row_bounds)
                        .with_background(theme::bg::ELEVATED)
                        .with_corner_radius(3.0),
                );
            }
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "{} [{}] {} ({})",
                    row.feature_name, row.status_badge, row.op_type, row.provenance
                ),
                Point::new(row_bounds.origin.x + 4.0, row_bounds.origin.y + 9.0),
                9.0,
                timeline_row_color(row),
            ));
        }

        if pane_state.timeline_selected_index.is_some() {
            let inspector_origin = Point::new(
                timeline_panel.origin.x + 6.0,
                (timeline_panel.max_y() - 34.0).max(timeline_panel.origin.y + 14.0),
            );
            paint.scene.draw_text(paint.text.layout(
                "Params:",
                inspector_origin,
                9.0,
                theme::text::SECONDARY,
            ));
            for (offset, (name, value)) in pane_state
                .selected_feature_params
                .iter()
                .take(2)
                .enumerate()
            {
                let y = inspector_origin.y + 10.0 + offset as f32 * 9.0;
                if y + 8.0 > timeline_panel.max_y() {
                    break;
                }
                paint.scene.draw_text(paint.text.layout(
                    &format!("{name}={value}"),
                    Point::new(inspector_origin.x, y),
                    8.0,
                    theme::text::MUTED,
                ));
            }
        }
    }

    if layout.footer_origin.y + 8.0 <= content_bounds.max_y() {
        let focus_suffix = pane_state
            .focused_geometry_ref
            .as_ref()
            .map(|value| format!(" focus={value}"))
            .unwrap_or_default();
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "session={} active={} warnings={}{}",
                pane_state.session_id,
                pane_state.active_variant_id,
                pane_state.warnings.len(),
                focus_suffix
            ),
            layout.footer_origin,
            9.0,
            theme::text::MUTED,
        ));
    }
}

fn visible_warning_indices(state: &CadDemoPaneState) -> Vec<usize> {
    state
        .warnings
        .iter()
        .enumerate()
        .filter(|(_, warning)| warning_passes_filter(state, warning))
        .map(|(index, _)| index)
        .collect()
}

fn warning_passes_filter(state: &CadDemoPaneState, warning: &CadDemoWarningState) -> bool {
    let severity_ok = state.warning_filter_severity == "all"
        || warning
            .severity
            .eq_ignore_ascii_case(&state.warning_filter_severity);
    let code_ok = state.warning_filter_code == "all"
        || warning
            .code
            .eq_ignore_ascii_case(&state.warning_filter_code);
    severity_ok && code_ok
}

fn warning_color(warning: &CadDemoWarningState) -> wgpui::Hsla {
    if warning.severity.eq_ignore_ascii_case("critical") {
        return theme::status::ERROR;
    }
    if warning.severity.eq_ignore_ascii_case("warning") {
        return theme::status::WARNING;
    }
    theme::text::MUTED
}

fn timeline_row_color(row: &crate::app_state::CadTimelineRowState) -> wgpui::Hsla {
    match row.status_badge.as_str() {
        "fail" => theme::status::ERROR,
        "warn" => theme::status::WARNING,
        _ => theme::text::SECONDARY,
    }
}

#[cfg(test)]
mod tests {
    use super::placeholder_layout;
    use wgpui::Bounds;

    #[test]
    fn placeholder_layout_stays_within_large_bounds() {
        let content = Bounds::new(20.0, 30.0, 640.0, 420.0);
        let layout = placeholder_layout(content);
        assert!(layout.header_origin.x >= content.origin.x);
        assert!(layout.header_origin.y >= content.origin.y);
        assert!(layout.subheader_origin.y >= content.origin.y);
        assert!(layout.viewport_bounds.origin.x >= content.origin.x);
        assert!(layout.viewport_bounds.origin.y >= content.origin.y);
        assert!(layout.viewport_bounds.max_x() <= content.max_x() + 0.001);
        assert!(layout.viewport_bounds.max_y() <= content.max_y() + 0.001);
        assert!(layout.footer_origin.y <= content.max_y() + 0.001);
    }

    #[test]
    fn placeholder_layout_stays_within_tight_bounds() {
        let content = Bounds::new(4.0, 6.0, 160.0, 92.0);
        let layout = placeholder_layout(content);
        assert!(layout.viewport_bounds.size.width >= 0.0);
        assert!(layout.viewport_bounds.size.height >= 0.0);
        assert!(layout.viewport_bounds.max_x() <= content.max_x() + 0.001);
        assert!(layout.viewport_bounds.max_y() <= content.max_y() + 0.001);
        assert!(layout.footer_origin.y >= content.origin.y);
        assert!(layout.footer_origin.y <= content.max_y() + 0.001);
    }
}
