use wgpui::{Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessageStatus, AutopilotRole, ChatPaneInputs, PaneKind,
    RenderState,
};
use crate::pane_renderer::{paint_action_button, split_text_for_display};
use crate::pane_system::{
    chat_composer_input_bounds, chat_send_button_bounds, chat_thread_rail_bounds,
    chat_transcript_bounds, pane_content_bounds,
};

pub fn paint(
    content_bounds: wgpui::Bounds,
    autopilot_chat: &AutopilotChatState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let rail_bounds = chat_thread_rail_bounds(content_bounds);
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let composer_bounds = chat_composer_input_bounds(content_bounds);
    let send_bounds = chat_send_button_bounds(content_bounds);

    paint.scene.draw_quad(
        Quad::new(rail_bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.72))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_quad(
        Quad::new(transcript_bounds)
            .with_background(theme::bg::APP.with_alpha(0.82))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );

    paint.scene.draw_text(paint.text.layout(
        "Threads",
        Point::new(rail_bounds.origin.x + 10.0, rail_bounds.origin.y + 14.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut thread_y = rail_bounds.origin.y + 30.0;
    for (idx, thread) in autopilot_chat.threads.iter().enumerate() {
        let color = if idx == autopilot_chat.active_thread {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };
        paint.scene.draw_text(paint.text.layout(
            thread,
            Point::new(rail_bounds.origin.x + 10.0, thread_y),
            11.0,
            color,
        ));
        thread_y += 16.0;
    }

    let mut y = transcript_bounds.origin.y + 10.0;
    for message in autopilot_chat.messages.iter().rev().take(12).rev() {
        let status = match message.status {
            AutopilotMessageStatus::Queued => "queued",
            AutopilotMessageStatus::Running => "running",
            AutopilotMessageStatus::Done => "done",
            AutopilotMessageStatus::Error => "error",
        };
        let role = match message.role {
            AutopilotRole::User => "you",
            AutopilotRole::Autopilot => "autopilot",
        };
        let status_color = match message.status {
            AutopilotMessageStatus::Queued => theme::text::MUTED,
            AutopilotMessageStatus::Running => theme::accent::PRIMARY,
            AutopilotMessageStatus::Done => theme::status::SUCCESS,
            AutopilotMessageStatus::Error => theme::status::ERROR,
        };

        paint.scene.draw_text(paint.text.layout_mono(
            &format!("[#{:04}] [{role}] [{status}]", message.id),
            Point::new(transcript_bounds.origin.x + 10.0, y),
            10.0,
            status_color,
        ));
        y += 14.0;
        for line in split_text_for_display(&message.content, 78) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(transcript_bounds.origin.x + 10.0, y),
                11.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
        y += 8.0;
    }

    if let Some(error) = autopilot_chat.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                transcript_bounds.origin.x + 10.0,
                transcript_bounds.max_y() - 14.0,
            ),
            11.0,
            theme::status::ERROR,
        ));
    }

    chat_inputs
        .composer
        .set_max_width(composer_bounds.size.width);
    chat_inputs.composer.paint(composer_bounds, paint);
    paint_action_button(send_bounds, "Send", paint);
}

pub fn topmost_send_hit_in_order(
    state: &RenderState,
    point: Point,
    pane_order: &[usize],
) -> Option<u64> {
    for pane_idx in pane_order {
        let pane_idx = *pane_idx;
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::AutopilotChat {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if chat_send_button_bounds(content_bounds).contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_chat = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AutopilotChat)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_chat else {
        return false;
    };

    let composer_bounds = chat_composer_input_bounds(pane_content_bounds(bounds));
    state
        .chat_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled()
}
