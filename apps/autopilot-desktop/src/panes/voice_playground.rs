use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    PaneKind, RenderState, VoicePlaygroundPaneInputs, VoicePlaygroundPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    pane_content_bounds, voice_playground_cancel_button_bounds,
    voice_playground_refresh_button_bounds, voice_playground_replay_button_bounds,
    voice_playground_speak_button_bounds, voice_playground_start_button_bounds,
    voice_playground_stop_button_bounds, voice_playground_stop_playback_button_bounds,
    voice_playground_stt_panel_bounds, voice_playground_tts_input_bounds,
    voice_playground_tts_panel_bounds,
};

pub fn paint(
    content_bounds: Bounds,
    pane_state: &VoicePlaygroundPaneState,
    inputs: &mut VoicePlaygroundPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "voice", paint);

    let refresh = voice_playground_refresh_button_bounds(content_bounds);
    let stt_panel = voice_playground_stt_panel_bounds(content_bounds);
    let tts_panel = voice_playground_tts_panel_bounds(content_bounds);
    let start = voice_playground_start_button_bounds(content_bounds);
    let stop = voice_playground_stop_button_bounds(content_bounds);
    let cancel = voice_playground_cancel_button_bounds(content_bounds);
    let tts_input = voice_playground_tts_input_bounds(content_bounds);
    let speak = voice_playground_speak_button_bounds(content_bounds);
    let replay = voice_playground_replay_button_bounds(content_bounds);
    let stop_playback = voice_playground_stop_playback_button_bounds(content_bounds);

    paint_voice_section_panel(
        stt_panel,
        "Speech to Text",
        "Microphone capture -> Google STT",
        paint,
    );
    paint_voice_section_panel(
        tts_panel,
        "Text to Speech",
        "Text -> Google TTS -> local playback",
        paint,
    );

    paint_action_button(refresh, "Refresh backend", paint);
    paint_action_button(start, "Record", paint);
    paint_action_button(stop, "Stop + STT", paint);
    paint_action_button(cancel, "Cancel", paint);
    paint_action_button(speak, "Speak", paint);
    paint_action_button(replay, "Replay", paint);
    paint_action_button(stop_playback, "Stop", paint);

    paint.scene.draw_text(paint.text.layout(
        "Voice Playground",
        Point::new(refresh.max_x() + 18.0, content_bounds.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Desktop-only Google voice verification lane",
        Point::new(refresh.max_x() + 18.0, content_bounds.origin.y + 34.0),
        11.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "Recording={} | STT={} | TTS={} | Playback={}",
        pane_state.recording_state.label(),
        pane_state.transcription_state.label(),
        tts_lifecycle_label(pane_state),
        pane_state.playback_state.label(),
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

    let mut line_y = start.max_y() + 18.0;
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Backend",
        pane_state.backend_label.as_str(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Project",
        pane_state.project_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Account",
        pane_state.active_account.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Model",
        pane_state.stt_model.as_str(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Region",
        pane_state.stt_location.as_str(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Language",
        pane_state.stt_language_code.as_str(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Input device",
        pane_state.input_device_name.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Recording",
        pane_state.recording_state.label(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Transcription",
        pane_state.transcription_state.label(),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
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
        stt_panel.origin.x + 12.0,
        line_y,
        "Pending",
        pane_state.pending_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
        line_y,
        "Last request",
        pane_state.last_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        stt_panel.origin.x + 12.0,
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
        stt_panel.origin.x + 12.0,
        line_y + 8.0,
        "Transcript",
        transcript.as_str(),
    );

    paint.scene.draw_text(paint.text.layout(
        "Speak this text",
        Point::new(tts_input.origin.x, tts_input.origin.y - 14.0),
        11.0,
        theme::text::MUTED,
    ));
    inputs
        .tts_text
        .set_max_width(tts_input.size.width.max(120.0));
    inputs.tts_text.paint(tts_input, paint);

    let mut tts_line_y = stop_playback.max_y() + 18.0;
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "Voice",
        pane_state.tts_voice_name.as_str(),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "Language",
        pane_state.tts_language_code.as_str(),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "TTS state",
        tts_lifecycle_label(pane_state),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "Synthesis",
        pane_state.synthesis_state.label(),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "Playback",
        pane_state.playback_state.label(),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "Last request",
        pane_state
            .last_synthesis_request_id
            .as_deref()
            .unwrap_or("-"),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "TTS latency",
        pane_state
            .last_synthesis_latency_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
    tts_line_y = paint_label_line(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y,
        "TTS clip",
        pane_state
            .tts_duration_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );

    let speech_preview = if pane_state.speech_preview.trim().is_empty() {
        "No speech synthesized yet.".to_string()
    } else if pane_state.speech_chars > pane_state.speech_preview.chars().count() {
        format!(
            "{}\n\n[truncated {} chars]",
            pane_state.speech_preview,
            pane_state
                .speech_chars
                .saturating_sub(pane_state.speech_preview.chars().count())
        )
    } else {
        pane_state.speech_preview.clone()
    };
    let _ = paint_multiline_phrase(
        paint,
        tts_panel.origin.x + 12.0,
        tts_line_y + 8.0,
        "Speech",
        speech_preview.as_str(),
    );
}

fn paint_voice_section_panel(
    bounds: Bounds,
    title: &str,
    subtitle: &str,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.74))
            .with_border(theme::border::DEFAULT.with_alpha(0.9), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        title,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 16.0),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn tts_lifecycle_label(pane_state: &VoicePlaygroundPaneState) -> &'static str {
    if pane_state.synthesis_state == crate::voice_playground::VoiceSynthesisState::Running {
        "synthesizing"
    } else if pane_state.playback_state == crate::voice_playground::VoicePlaybackState::Playing {
        "playing"
    } else if pane_state.playback_state == crate::voice_playground::VoicePlaybackState::Completed {
        "completed"
    } else if pane_state.synthesis_state == crate::voice_playground::VoiceSynthesisState::Error
        || pane_state.playback_state == crate::voice_playground::VoicePlaybackState::Error
    {
        "failed"
    } else {
        "idle"
    }
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::VoicePlayground)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    state
        .voice_playground_inputs
        .tts_text
        .event(
            event,
            voice_playground_tts_input_bounds(pane_content_bounds(bounds)),
            &mut state.event_context,
        )
        .is_handled()
}
