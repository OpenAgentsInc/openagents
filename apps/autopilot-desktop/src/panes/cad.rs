use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::CadDemoPaneState;
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{cad_demo_cycle_variant_button_bounds, cad_demo_reset_button_bounds};

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
    paint_action_button(cycle_bounds, "Cycle Variant", paint);
    paint_action_button(reset_bounds, "Reset Session", paint);

    let body_top = (cycle_bounds.max_y().max(reset_bounds.max_y()) + 8.0).min(content_bounds.max_y());
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
            let latest_label = Point::new(viewport_label.x, viewport_label.y + RECEIPT_LINE_HEIGHT + 2.0);
            if latest_label.y + 9.0 <= layout.viewport_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &format!(
                        "latest rebuild {}ms hash={} cache(h={},m={},e={})",
                        receipt.duration_ms,
                        receipt.rebuild_hash,
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
            let mesh_line = Point::new(viewport_label.x, viewport_label.y + (RECEIPT_LINE_HEIGHT * 5.0));
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
            let pending_line = Point::new(viewport_label.x, viewport_label.y + (RECEIPT_LINE_HEIGHT * 6.0));
            if pending_line.y + 8.0 <= layout.viewport_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &format!("pending rebuild request: #{request_id}"),
                    pending_line,
                    9.0,
                    theme::text::SECONDARY,
                ));
            }
        }
    }

    if layout.footer_origin.y + 8.0 <= content_bounds.max_y() {
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "session={} active={}",
                pane_state.session_id, pane_state.active_variant_id
            ),
            layout.footer_origin,
            9.0,
            theme::text::MUTED,
        ));
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
