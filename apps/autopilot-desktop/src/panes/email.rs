use crate::app_state::{EmailLaneState, PaneLoadState};
use crate::pane_system::{
    email_approval_approve_button_bounds, email_approval_kill_switch_button_bounds,
    email_approval_pause_button_bounds, email_approval_reject_button_bounds,
    email_approval_request_edits_button_bounds, email_approval_row_bounds,
    email_approval_visible_row_count, email_draft_row_bounds, email_draft_visible_row_count,
    email_follow_up_row_bounds, email_follow_up_run_button_bounds,
    email_follow_up_visible_row_count, email_inbox_generate_draft_button_bounds,
    email_inbox_refresh_button_bounds, email_inbox_row_bounds, email_inbox_visible_row_count,
    email_send_row_bounds, email_send_send_button_bounds, email_send_visible_row_count,
};
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

pub fn paint_email_inbox_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    paint_shared_header(content_bounds, email_lane, paint);

    paint_action_button(
        email_inbox_refresh_button_bounds(content_bounds),
        "Refresh inbox",
        paint,
    );
    paint_action_button(
        email_inbox_generate_draft_button_bounds(content_bounds),
        "Generate draft",
        paint,
    );

    let visible_rows = email_inbox_visible_row_count(email_lane.inbox_rows.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No inbox messages loaded.",
            Point::new(
                content_bounds.origin.x + 12.0,
                email_inbox_row_bounds(content_bounds, 0).origin.y + 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &email_lane.inbox_rows[row_index];
        let selected = email_lane.selected_inbox_message_id.as_deref() == Some(&row.message_id);
        let label = format!(
            "{} | {} | {}",
            row.message_id, row.sender_email, row.pipeline_state
        );
        paint_row(
            email_inbox_row_bounds(content_bounds, row_index),
            label.as_str(),
            selected,
            paint,
        );
    }
}

pub fn paint_email_draft_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    paint_shared_header(content_bounds, email_lane, paint);

    let visible_rows = email_draft_visible_row_count(email_lane.draft_rows.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No drafts generated yet.",
            Point::new(
                content_bounds.origin.x + 12.0,
                email_draft_row_bounds(content_bounds, 0).origin.y + 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &email_lane.draft_rows[row_index];
        let selected = email_lane.selected_draft_id.as_deref() == Some(&row.draft_id);
        let label = format!(
            "{} | {} | confidence={}m | {}",
            row.draft_id, row.recipient_email, row.confidence_milli, row.approval_status
        );
        paint_row(
            email_draft_row_bounds(content_bounds, row_index),
            label.as_str(),
            selected,
            paint,
        );
    }
}

pub fn paint_email_approval_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    paint_shared_header(content_bounds, email_lane, paint);

    paint_action_button(
        email_approval_approve_button_bounds(content_bounds),
        "Approve",
        paint,
    );
    paint_action_button(
        email_approval_reject_button_bounds(content_bounds),
        "Reject",
        paint,
    );
    paint_action_button(
        email_approval_request_edits_button_bounds(content_bounds),
        "Request edits",
        paint,
    );
    paint_action_button(
        email_approval_pause_button_bounds(content_bounds),
        if email_lane.approval_workflow.queue_paused {
            "Resume queue"
        } else {
            "Pause queue"
        },
        paint,
    );
    paint_action_button(
        email_approval_kill_switch_button_bounds(content_bounds),
        if email_lane.approval_workflow.kill_switch_engaged {
            "Disengage kill"
        } else {
            "Engage kill"
        },
        paint,
    );

    let visible_rows = email_approval_visible_row_count(email_lane.approval_rows.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No drafts in approval queue.",
            Point::new(
                content_bounds.origin.x + 12.0,
                email_approval_row_bounds(content_bounds, 0).origin.y + 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &email_lane.approval_rows[row_index];
        let selected =
            email_lane.selected_approval_draft_id.as_deref() == Some(row.draft_id.as_str());
        let actor = row.decision_actor.as_deref().unwrap_or("pending");
        let label = format!("{} | {} | actor={}", row.draft_id, row.status, actor);
        paint_row(
            email_approval_row_bounds(content_bounds, row_index),
            label.as_str(),
            selected,
            paint,
        );
    }
}

pub fn paint_email_send_log_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    paint_shared_header(content_bounds, email_lane, paint);

    paint_action_button(
        email_send_send_button_bounds(content_bounds),
        "Send selected approved draft",
        paint,
    );

    let visible_rows = email_send_visible_row_count(email_lane.send_rows.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No send attempts yet.",
            Point::new(
                content_bounds.origin.x + 12.0,
                email_send_row_bounds(content_bounds, 0).origin.y + 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &email_lane.send_rows[row_index];
        let selected =
            email_lane.selected_send_idempotency_key.as_deref() == Some(&row.idempotency_key);
        let label = format!(
            "{} | {} | attempts={} | msg={}",
            row.send_id,
            row.state,
            row.attempt_count,
            row.provider_message_id.as_deref().unwrap_or("n/a")
        );
        paint_row(
            email_send_row_bounds(content_bounds, row_index),
            label.as_str(),
            selected,
            paint,
        );
    }
}

pub fn paint_email_follow_up_queue_pane(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    paint_shared_header(content_bounds, email_lane, paint);

    paint_action_button(
        email_follow_up_run_button_bounds(content_bounds),
        "Run scheduler tick",
        paint,
    );

    let visible_rows = email_follow_up_visible_row_count(email_lane.follow_up_rows.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No follow-up jobs queued.",
            Point::new(
                content_bounds.origin.x + 12.0,
                email_follow_up_row_bounds(content_bounds, 0).origin.y + 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &email_lane.follow_up_rows[row_index];
        let selected = email_lane.selected_follow_up_job_id.as_deref() == Some(&row.job_id);
        let label = format!(
            "{} | {} | scheduled={} | {}",
            row.job_id, row.status, row.scheduled_for_unix, row.rule_id
        );
        paint_row(
            email_follow_up_row_bounds(content_bounds, row_index),
            label.as_str(),
            selected,
            paint,
        );
    }
}

fn paint_shared_header(
    content_bounds: Bounds,
    email_lane: &EmailLaneState,
    paint: &mut PaintContext,
) {
    let state_color = match email_lane.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let status_x = (content_bounds.max_x() - 320.0).max(content_bounds.origin.x + 12.0);
    let state_line = format!("state: {}", email_lane.load_state.label());
    paint.scene.draw_text(paint.text.layout(
        state_line.as_str(),
        Point::new(status_x, content_bounds.origin.y + 18.0),
        11.0,
        state_color,
    ));

    if let Some(action) = email_lane.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(status_x, content_bounds.origin.y + 32.0),
            10.0,
            theme::text::MUTED,
        ));
    }
    if let Some(error) = email_lane.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(status_x, content_bounds.origin.y + 46.0),
            10.0,
            theme::status::ERROR,
        ));
    }
}

fn paint_row(row_bounds: Bounds, label: &str, selected: bool, paint: &mut PaintContext) {
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
}

fn paint_action_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x1f283a))
            .with_border(theme::border::FOCUS, 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 18.0),
        10.0,
        theme::text::PRIMARY,
    ));
}
