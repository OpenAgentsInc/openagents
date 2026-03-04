use wgpui::components::Text;
use wgpui::{Bounds, Component, Hsla, PaintContext, Quad, theme};

const ROW_HEIGHT: f32 = 28.0;
const ROW_GAP: f32 = 6.0;

pub fn paint_email_inbox_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    paint_section_title(content_bounds, "Inbox (imported)", paint);
    let rows = [
        "msg-2026-001 | sender@example.com | received -> normalized",
        "msg-2026-002 | ops@example.com | received -> parse_failed(reason=invalid_mime)",
        "msg-2026-003 | support@example.com | received -> normalized -> draft_pending",
    ];
    paint_rows(content_bounds, &rows, paint);
}

pub fn paint_email_draft_queue_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    paint_section_title(content_bounds, "Draft Queue", paint);
    let rows = [
        "draft:m1:profile-1 | generated -> awaiting_approval | confidence=810m",
        "draft:m2:profile-1 | generated -> revision_requested(reason=tone_mismatch) | confidence=620m",
        "draft:m3:profile-2 | generated -> approved(policy=manual) | confidence=910m",
    ];
    paint_rows(content_bounds, &rows, paint);
}

pub fn paint_email_approval_queue_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    paint_section_title(content_bounds, "Approval Queue", paint);
    let rows = [
        "decision-001 | draft:m1 | pending -> approve/reject/edit",
        "decision-002 | draft:m2 | rejected(actor=operator, reason=missing_refs)",
        "decision-003 | draft:m3 | approved(actor=policy:auto-safe-mode)",
    ];
    paint_rows(content_bounds, &rows, paint);
}

pub fn paint_email_send_log_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    paint_section_title(content_bounds, "Send Log", paint);
    let rows = [
        "send-0001 | queued -> sent | provider_msg_id=gmail:17f...",
        "send-0002 | queued -> retrying(attempt=2, reason=rate_limit)",
        "send-0003 | queued -> failed(reason=invalid_recipient)",
    ];
    paint_rows(content_bounds, &rows, paint);
}

pub fn paint_email_follow_up_queue_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    paint_section_title(content_bounds, "Follow-up Queue", paint);
    let rows = [
        "followup-1001 | scheduled(2026-03-05T14:00Z) | rule=no_reply_48h",
        "followup-1002 | deferred(reason=quiet_hours, retry=2026-03-05T15:30Z)",
        "followup-1003 | failed(reason=recipient_limit_exceeded)",
    ];
    paint_rows(content_bounds, &rows, paint);
}

fn paint_section_title(content_bounds: Bounds, title: &str, paint: &mut PaintContext) {
    Text::new(title).color(theme::text::PRIMARY).paint(
        Bounds::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 10.0,
            content_bounds.size.width - 24.0,
            24.0,
        ),
        paint,
    );
}

fn paint_rows(content_bounds: Bounds, rows: &[&str], paint: &mut PaintContext) {
    let mut y = content_bounds.origin.y + 44.0;
    for row in rows {
        let row_bounds = Bounds::new(
            content_bounds.origin.x + 12.0,
            y,
            content_bounds.size.width - 24.0,
            ROW_HEIGHT,
        );
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(Hsla::from_hex(0x1a1d24))
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(6.0),
        );
        Text::new(*row).color(theme::text::MUTED).paint(
            Bounds::new(
                row_bounds.origin.x + 10.0,
                row_bounds.origin.y + 7.0,
                row_bounds.size.width - 20.0,
                18.0,
            ),
            paint,
        );
        y += ROW_HEIGHT + ROW_GAP;
    }
}
