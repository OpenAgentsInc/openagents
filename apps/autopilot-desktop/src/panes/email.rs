use crate::app_state::{EmailLaneState, PaneLoadState};
use wgpui::components::Text;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

const ROW_HEIGHT: f32 = 32.0;
const ROW_GAP: f32 = 6.0;
const MAX_ROWS: usize = 9;

pub fn paint_email_inbox_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let mut y = paint_shared_header(content_bounds, "Inbox", email_lane, paint);
    for row in email_lane.inbox_rows.iter().take(MAX_ROWS) {
        let selected = email_lane.selected_inbox_message_id.as_deref() == Some(&row.message_id);
        let label = format!(
            "{} | {} | {}",
            row.message_id, row.sender_email, row.pipeline_state
        );
        y = paint_row(content_bounds, y, label.as_str(), selected, paint);
    }
}

pub fn paint_email_draft_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let mut y = paint_shared_header(content_bounds, "Draft Queue", email_lane, paint);
    for row in email_lane.draft_rows.iter().take(MAX_ROWS) {
        let selected = email_lane.selected_draft_id.as_deref() == Some(&row.draft_id);
        let label = format!(
            "{} | {} | confidence={}m | {}",
            row.draft_id, row.recipient_email, row.confidence_milli, row.approval_status
        );
        y = paint_row(content_bounds, y, label.as_str(), selected, paint);
    }
}

pub fn paint_email_approval_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let mut y = paint_shared_header(content_bounds, "Approval Queue", email_lane, paint);
    for row in email_lane.approval_rows.iter().take(MAX_ROWS) {
        let selected =
            email_lane.selected_approval_draft_id.as_deref() == Some(row.draft_id.as_str());
        let actor = row.decision_actor.as_deref().unwrap_or("pending");
        let label = format!("{} | {} | actor={}", row.draft_id, row.status, actor);
        y = paint_row(content_bounds, y, label.as_str(), selected, paint);
    }
}

pub fn paint_email_send_log_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let mut y = paint_shared_header(content_bounds, "Send Log", email_lane, paint);
    for row in email_lane.send_rows.iter().take(MAX_ROWS) {
        let selected =
            email_lane.selected_send_idempotency_key.as_deref() == Some(&row.idempotency_key);
        let label = format!(
            "{} | {} | attempts={} | msg={}",
            row.send_id,
            row.state,
            row.attempt_count,
            row.provider_message_id.as_deref().unwrap_or("n/a")
        );
        y = paint_row(content_bounds, y, label.as_str(), selected, paint);
    }
}

pub fn paint_email_follow_up_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let mut y = paint_shared_header(content_bounds, "Follow-up Queue", email_lane, paint);
    for row in email_lane.follow_up_rows.iter().take(MAX_ROWS) {
        let selected = email_lane.selected_follow_up_job_id.as_deref() == Some(&row.job_id);
        let label = format!(
            "{} | {} | scheduled={} | {}",
            row.job_id, row.status, row.scheduled_for_unix, row.rule_id
        );
        y = paint_row(content_bounds, y, label.as_str(), selected, paint);
    }
}

fn paint_shared_header(
    content_bounds: Bounds,
    title: &str,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) -> f32 {
    Text::new(title).color(theme::text::PRIMARY).paint(
        Bounds::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 10.0,
            content_bounds.size.width - 24.0,
            20.0,
        ),
        paint,
    );

    let state_color = match email_lane.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let state_line = format!("state: {}", email_lane.load_state.label());
    paint.scene.draw_text(paint.text.layout(
        state_line.as_str(),
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 30.0,
        ),
        11.0,
        state_color,
    ));

    if let Some(action) = email_lane.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(
                content_bounds.origin.x + 12.0,
                content_bounds.origin.y + 46.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
    }
    if let Some(error) = email_lane.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                content_bounds.origin.x + 12.0,
                content_bounds.origin.y + 60.0,
            ),
            10.0,
            theme::status::ERROR,
        ));
    }

    content_bounds.origin.y + 78.0
}

fn paint_row(
    content_bounds: Bounds,
    y: f32,
    label: &str,
    selected: bool,
    paint: &mut PaintContext,
) -> f32 {
    let row_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        y,
        content_bounds.size.width - 24.0,
        ROW_HEIGHT,
    );
    let background = if selected {
        Hsla::from_hex(0x22324a)
    } else {
        Hsla::from_hex(0x1a1d24)
    };
    paint.scene.draw_quad(
        Quad::new(row_bounds)
            .with_background(background)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 20.0),
        10.0,
        if selected {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
    y + ROW_HEIGHT + ROW_GAP
}
