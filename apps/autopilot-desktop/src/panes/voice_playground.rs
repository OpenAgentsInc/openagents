use wgpui::{Bounds, InputEvent, PaintContext, Point, theme};

use crate::app_state::{PaneKind, RenderState, VoicePlaygroundPaneInputs, VoicePlaygroundPaneState};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    pane_content_bounds, voice_playground_cancel_button_bounds,
    voice_playground_refresh_button_bounds, voice_playground_start_button_bounds,
    voice_playground_stop_button_bounds,
};

pub fn paint(
    content_bounds: Bounds,
    pane_state: &VoicePlaygroundPaneState,
    _inputs: &mut VoicePlaygroundPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "voice", paint);

    let refresh = voice_playground_refresh_button_bounds(content_bounds);
    let start = voice_playground_start_button_bounds(content_bounds);
    let stop = voice_playground_stop_button_bounds(content_bounds);
    let cancel = voice_playground_cancel_button_bounds(content_bounds);

    paint_action_button(refresh, "Refresh backend", paint);
    paint_action_button(start, "Start recording", paint);
    paint_action_button(stop, "Stop + transcribe", paint);
    paint_action_button(cancel, "Cancel", paint);

    paint.scene.draw_text(paint.text.layout(
        "Google Cloud Speech-to-Text",
        Point::new(cancel.max_x() + 18.0, content_bounds.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Short-clip push-to-talk workbench",
        Point::new(cancel.max_x() + 18.0, content_bounds.origin.y + 34.0),
        11.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "Recording={} | Transcription={}",
        pane_state.recording_state.label(),
        pane_state.transcription_state.label()
    );
    let _ = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        refresh.max_y() + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    let mut line_y = refresh.max_y() + 86.0;
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Backend",
        pane_state.backend_label.as_str(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Project",
        pane_state.project_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Account",
        pane_state.active_account.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Model",
        pane_state.stt_model.as_str(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Region",
        pane_state.stt_location.as_str(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Language",
        pane_state.stt_language_code.as_str(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Input device",
        pane_state.input_device_name.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Clip",
        pane_state
            .clip_duration_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Pending",
        pane_state.pending_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Last request",
        pane_state.last_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Latency",
        pane_state
            .last_transcription_latency_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );

    let transcript = if pane_state.transcript_preview.trim().is_empty() {
        "No transcript yet. Record a short clip, then stop to send it to Google STT.".to_string()
    } else if pane_state.transcript_chars > pane_state.transcript_preview.chars().count() {
        format!(
            "{}\n\n[truncated {} chars]",
            pane_state.transcript_preview,
            pane_state
                .transcript_chars
                .saturating_sub(pane_state.transcript_preview.chars().count())
        )
    } else {
        pane_state.transcript_preview.clone()
    };
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y + 8.0,
        "Transcript",
        transcript.as_str(),
    );
}

pub fn dispatch_input_event(state: &mut RenderState, _event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::VoicePlayground)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let _content_bounds = pane_content_bounds(bounds);
    false
}
