use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, AutopilotMessageStatus, AutopilotRole, ChatPaneInputs, PaneKind,
    RenderState,
};
use crate::pane_renderer::{paint_action_button, split_text_for_display};
use crate::pane_system::{
    chat_composer_input_bounds, chat_cycle_model_button_bounds, chat_interrupt_button_bounds,
    chat_refresh_threads_button_bounds, chat_send_button_bounds,
    chat_thread_action_archive_button_bounds, chat_thread_action_compact_button_bounds,
    chat_thread_action_fork_button_bounds, chat_thread_action_rename_button_bounds,
    chat_thread_action_rollback_button_bounds, chat_thread_action_unarchive_button_bounds,
    chat_thread_action_unsubscribe_button_bounds, chat_thread_filter_archived_button_bounds,
    chat_thread_filter_provider_button_bounds, chat_thread_filter_sort_button_bounds,
    chat_thread_filter_source_button_bounds, chat_thread_rail_bounds, chat_thread_row_bounds,
    chat_transcript_bounds, chat_visible_thread_row_count, pane_content_bounds,
};

pub fn paint(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let rail_bounds = chat_thread_rail_bounds(content_bounds);
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let composer_bounds = chat_composer_input_bounds(content_bounds);
    let send_bounds = chat_send_button_bounds(content_bounds);
    let refresh_bounds = chat_refresh_threads_button_bounds(content_bounds);
    let model_bounds = chat_cycle_model_button_bounds(content_bounds);
    let interrupt_bounds = chat_interrupt_button_bounds(content_bounds);
    let archived_filter_bounds = chat_thread_filter_archived_button_bounds(content_bounds);
    let sort_filter_bounds = chat_thread_filter_sort_button_bounds(content_bounds);
    let source_filter_bounds = chat_thread_filter_source_button_bounds(content_bounds);
    let provider_filter_bounds = chat_thread_filter_provider_button_bounds(content_bounds);
    let fork_bounds = chat_thread_action_fork_button_bounds(content_bounds);
    let archive_bounds = chat_thread_action_archive_button_bounds(content_bounds);
    let unarchive_bounds = chat_thread_action_unarchive_button_bounds(content_bounds);
    let rename_bounds = chat_thread_action_rename_button_bounds(content_bounds);
    let rollback_bounds = chat_thread_action_rollback_button_bounds(content_bounds);
    let compact_bounds = chat_thread_action_compact_button_bounds(content_bounds);
    let unsubscribe_bounds = chat_thread_action_unsubscribe_button_bounds(content_bounds);

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
    paint.scene.draw_text(paint.text.layout(
        "Codex",
        Point::new(
            transcript_bounds.origin.x + 10.0,
            transcript_bounds.origin.y + 14.0,
        ),
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "status={} model={} effort={}",
            autopilot_chat.connection_status,
            autopilot_chat.current_model(),
            autopilot_chat
                .reasoning_effort
                .as_deref()
                .unwrap_or("default")
        ),
        Point::new(
            transcript_bounds.origin.x + 10.0,
            transcript_bounds.origin.y + 42.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    paint_action_button(refresh_bounds, "Refresh", paint);
    let archived_filter_label = match autopilot_chat.thread_filter_archived {
        Some(false) => "Live",
        Some(true) => "Archived",
        None => "Any",
    };
    paint_action_button(archived_filter_bounds, archived_filter_label, paint);
    let sort_filter_label = match autopilot_chat.thread_filter_sort_key {
        codex_client::ThreadSortKey::CreatedAt => "Sort:Created",
        codex_client::ThreadSortKey::UpdatedAt => "Sort:Updated",
    };
    paint_action_button(sort_filter_bounds, sort_filter_label, paint);
    let source_filter_label = match autopilot_chat.thread_filter_source_kind {
        None => "Source:Any",
        Some(codex_client::ThreadSourceKind::AppServer) => "Source:App",
        Some(codex_client::ThreadSourceKind::Cli) => "Source:CLI",
        Some(codex_client::ThreadSourceKind::Exec) => "Source:Exec",
        Some(_) => "Source:Other",
    };
    paint_action_button(source_filter_bounds, source_filter_label, paint);
    let provider_filter_label = match autopilot_chat.thread_filter_model_provider.as_deref() {
        None => "Provider:*",
        Some("openai") => "Provider:OA",
        Some("azure-openai") => "Provider:AZ",
        Some(_) => "Provider:Other",
    };
    paint_action_button(provider_filter_bounds, provider_filter_label, paint);
    paint_action_button(fork_bounds, "Fork", paint);
    paint_action_button(archive_bounds, "Archive", paint);
    paint_action_button(unarchive_bounds, "Unarchive", paint);
    paint_action_button(rename_bounds, "Rename", paint);
    paint_action_button(rollback_bounds, "Rollback", paint);
    paint_action_button(compact_bounds, "Compact", paint);
    paint_action_button(unsubscribe_bounds, "Unsub", paint);
    paint_action_button(model_bounds, "Cycle Model", paint);
    paint_action_button(interrupt_bounds, "Interrupt", paint);

    let visible_threads = chat_visible_thread_row_count(autopilot_chat.threads.len());
    for row_index in 0..visible_threads {
        let row_bounds = chat_thread_row_bounds(content_bounds, row_index);
        let Some(thread_id) = autopilot_chat.threads.get(row_index) else {
            continue;
        };
        let is_active = autopilot_chat
            .active_thread_id
            .as_deref()
            .is_some_and(|active| active == thread_id);

        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if is_active {
                    theme::bg::ELEVATED.with_alpha(0.9)
                } else {
                    theme::bg::APP.with_alpha(0.6)
                })
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );
        let status = autopilot_chat
            .thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.status.as_deref())
            .unwrap_or("unknown");
        let loaded = autopilot_chat
            .thread_metadata
            .get(thread_id)
            .is_some_and(|metadata| metadata.loaded);
        let row_label = format!(
            "{} [{}|{}]",
            autopilot_chat.thread_label(thread_id),
            if loaded { "loaded" } else { "cold" },
            status
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &row_label,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 15.0),
            10.0,
            if is_active {
                theme::accent::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
    }

    let turn_status = autopilot_chat
        .last_turn_status
        .as_deref()
        .unwrap_or("idle")
        .to_string();
    let active_thread_status = autopilot_chat.active_thread_status().unwrap_or("n/a");
    let active_thread_loaded = autopilot_chat
        .active_thread_loaded()
        .map(|loaded| if loaded { "loaded" } else { "cold" })
        .unwrap_or("n/a");
    let token_summary = autopilot_chat
        .token_usage
        .as_ref()
        .map(|usage| {
            format!(
                "tokens(in={}, cache={}, out={})",
                usage.input_tokens, usage.cached_input_tokens, usage.output_tokens
            )
        })
        .unwrap_or_else(|| "tokens(n/a)".to_string());
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "thread={}({}) turn={} {}",
            active_thread_status, active_thread_loaded, turn_status, token_summary
        ),
        Point::new(
            transcript_bounds.origin.x + 10.0,
            transcript_bounds.origin.y + 56.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = transcript_bounds.origin.y + 84.0;
    if !autopilot_chat.turn_plan.is_empty() {
        let plan_compact = autopilot_chat
            .turn_plan
            .iter()
            .take(3)
            .map(|step| format!("{}:{}", step.status, step.step))
            .collect::<Vec<_>>()
            .join(" | ");
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("plan {}", plan_compact),
            Point::new(transcript_bounds.origin.x + 10.0, y),
            10.0,
            theme::accent::PRIMARY,
        ));
        y += 14.0;
    }
    if let Some(diff) = autopilot_chat.turn_diff.as_deref() {
        let diff_preview = diff.lines().next().unwrap_or_default();
        if !diff_preview.is_empty() {
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("diff {}", diff_preview),
                Point::new(transcript_bounds.origin.x + 10.0, y),
                10.0,
                theme::text::MUTED,
            ));
            y += 14.0;
        }
    }
    for event in autopilot_chat.turn_timeline.iter().rev().take(2).rev() {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("event {}", event),
            Point::new(transcript_bounds.origin.x + 10.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 14.0;
    }
    for message in autopilot_chat.messages.iter().rev().take(14).rev() {
        let status = match message.status {
            AutopilotMessageStatus::Queued => "queued",
            AutopilotMessageStatus::Running => "running",
            AutopilotMessageStatus::Done => "done",
            AutopilotMessageStatus::Error => "error",
        };
        let role = match message.role {
            AutopilotRole::User => "you",
            AutopilotRole::Codex => "codex",
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

        let content = if message.content.trim().is_empty()
            && matches!(message.status, AutopilotMessageStatus::Queued)
        {
            "Waiting for Codex response...".to_string()
        } else {
            message.content.clone()
        };

        for line in split_text_for_display(&content, 78) {
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
