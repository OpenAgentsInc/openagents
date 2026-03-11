use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AppleFmWorkbenchPaneInputs, AppleFmWorkbenchPaneState, PaneKind, RenderState,
};
use crate::apple_fm_bridge::AppleFmBridgeSnapshot;
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_secondary_button,
    paint_source_badge, paint_state_summary,
};
use crate::pane_system::{
    apple_fm_workbench_create_session_button_bounds,
    apple_fm_workbench_delete_session_button_bounds, apple_fm_workbench_event_log_bounds,
    apple_fm_workbench_export_transcript_button_bounds,
    apple_fm_workbench_inspect_session_button_bounds, apple_fm_workbench_instructions_input_bounds,
    apple_fm_workbench_max_tokens_input_bounds, apple_fm_workbench_model_input_bounds,
    apple_fm_workbench_output_bounds, apple_fm_workbench_probability_threshold_input_bounds,
    apple_fm_workbench_prompt_input_bounds, apple_fm_workbench_refresh_button_bounds,
    apple_fm_workbench_reset_session_button_bounds,
    apple_fm_workbench_restore_transcript_button_bounds, apple_fm_workbench_run_chat_button_bounds,
    apple_fm_workbench_run_session_button_bounds, apple_fm_workbench_run_stream_button_bounds,
    apple_fm_workbench_run_structured_button_bounds, apple_fm_workbench_run_text_button_bounds,
    apple_fm_workbench_sampling_mode_button_bounds, apple_fm_workbench_schema_input_bounds,
    apple_fm_workbench_seed_input_bounds, apple_fm_workbench_session_input_bounds,
    apple_fm_workbench_start_bridge_button_bounds, apple_fm_workbench_temperature_input_bounds,
    apple_fm_workbench_tool_profile_button_bounds, apple_fm_workbench_top_input_bounds,
    apple_fm_workbench_transcript_input_bounds, pane_content_bounds,
};

const OUTPUT_SECTION_LABEL_GAP: f32 = 14.0;
const OUTPUT_SECTION_LINE_HEIGHT: f32 = 14.0;
const OUTPUT_SECTION_MAX_LINES: usize = 7;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut AppleFmWorkbenchPaneState,
    runtime: &AppleFmBridgeSnapshot,
    inputs: &mut AppleFmWorkbenchPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "swift bridge", paint);

    let refresh_bounds = apple_fm_workbench_refresh_button_bounds(content_bounds);
    let start_bounds = apple_fm_workbench_start_bridge_button_bounds(content_bounds);
    let create_bounds = apple_fm_workbench_create_session_button_bounds(content_bounds);
    let inspect_bounds = apple_fm_workbench_inspect_session_button_bounds(content_bounds);
    let reset_bounds = apple_fm_workbench_reset_session_button_bounds(content_bounds);
    let delete_bounds = apple_fm_workbench_delete_session_button_bounds(content_bounds);
    let run_text_bounds = apple_fm_workbench_run_text_button_bounds(content_bounds);
    let run_chat_bounds = apple_fm_workbench_run_chat_button_bounds(content_bounds);
    let run_session_bounds = apple_fm_workbench_run_session_button_bounds(content_bounds);
    let run_stream_bounds = apple_fm_workbench_run_stream_button_bounds(content_bounds);
    let run_structured_bounds = apple_fm_workbench_run_structured_button_bounds(content_bounds);
    let export_bounds = apple_fm_workbench_export_transcript_button_bounds(content_bounds);
    let restore_bounds = apple_fm_workbench_restore_transcript_button_bounds(content_bounds);
    let tool_profile_bounds = apple_fm_workbench_tool_profile_button_bounds(content_bounds);
    let sampling_mode_bounds = apple_fm_workbench_sampling_mode_button_bounds(content_bounds);
    let instructions_bounds = apple_fm_workbench_instructions_input_bounds(content_bounds);
    let prompt_bounds = apple_fm_workbench_prompt_input_bounds(content_bounds);
    let schema_bounds = apple_fm_workbench_schema_input_bounds(content_bounds);
    let transcript_bounds = apple_fm_workbench_transcript_input_bounds(content_bounds);
    let model_bounds = apple_fm_workbench_model_input_bounds(content_bounds);
    let session_bounds = apple_fm_workbench_session_input_bounds(content_bounds);
    let max_tokens_bounds = apple_fm_workbench_max_tokens_input_bounds(content_bounds);
    let temperature_bounds = apple_fm_workbench_temperature_input_bounds(content_bounds);
    let top_bounds = apple_fm_workbench_top_input_bounds(content_bounds);
    let probability_threshold_bounds =
        apple_fm_workbench_probability_threshold_input_bounds(content_bounds);
    let seed_bounds = apple_fm_workbench_seed_input_bounds(content_bounds);
    let output_bounds = apple_fm_workbench_output_bounds(content_bounds);
    let event_log_bounds = apple_fm_workbench_event_log_bounds(content_bounds);

    paint_action_button(refresh_bounds, "Refresh bridge", paint);
    paint_action_button(start_bounds, "Start bridge", paint);
    paint_action_button(create_bounds, "Create session", paint);
    paint_action_button(inspect_bounds, "Inspect session", paint);
    paint_action_button(reset_bounds, "Reset session", paint);
    paint_action_button(delete_bounds, "Delete session", paint);
    paint_action_button(run_text_bounds, "Run text", paint);
    paint_action_button(run_chat_bounds, "Run chat", paint);
    paint_action_button(run_session_bounds, "Run session", paint);
    paint_action_button(run_stream_bounds, "Run stream", paint);
    paint_action_button(run_structured_bounds, "Run structured", paint);
    paint_action_button(export_bounds, "Export transcript", paint);
    paint_action_button(restore_bounds, "Restore transcript", paint);
    paint_secondary_button(tool_profile_bounds, pane_state.tool_profile.label(), paint);
    paint_secondary_button(
        sampling_mode_bounds,
        pane_state.sampling_mode.label(),
        paint,
    );

    let title_x = sampling_mode_bounds.max_x() + 16.0;
    paint.scene.draw_text(paint.text.layout(
        "Apple FM workbench",
        Point::new(title_x, content_bounds.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Swift bridge controls for sessions, streaming, tools, transcripts, and schemas.",
        Point::new(title_x, content_bounds.origin.y + 34.0),
        11.0,
        theme::text::MUTED,
    ));

    let status = if pane_state.pending_request_id.is_some() {
        "running"
    } else if runtime.is_ready() {
        "ready"
    } else if runtime.reachable {
        "reachable"
    } else if runtime.last_error.is_some() {
        "error"
    } else {
        "waiting"
    };
    let summary = format!("Bridge: {status}");
    let _ = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        restore_bounds.max_y() + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    inputs
        .instructions
        .set_max_width(instructions_bounds.size.width.max(220.0));
    inputs
        .prompt
        .set_max_width(prompt_bounds.size.width.max(220.0));
    inputs
        .schema_json
        .set_max_width(schema_bounds.size.width.max(220.0));
    inputs
        .transcript_json
        .set_max_width(transcript_bounds.size.width.max(220.0));
    inputs
        .model
        .set_max_width(model_bounds.size.width.max(120.0));
    inputs
        .session_id
        .set_max_width(session_bounds.size.width.max(120.0));
    inputs
        .max_tokens
        .set_max_width(max_tokens_bounds.size.width.max(72.0));
    inputs
        .temperature
        .set_max_width(temperature_bounds.size.width.max(72.0));
    inputs.top.set_max_width(top_bounds.size.width.max(72.0));
    inputs
        .probability_threshold
        .set_max_width(probability_threshold_bounds.size.width.max(120.0));
    inputs.seed.set_max_width(seed_bounds.size.width.max(96.0));

    inputs.instructions.paint(instructions_bounds, paint);
    inputs.prompt.paint(prompt_bounds, paint);
    inputs.schema_json.paint(schema_bounds, paint);
    inputs.transcript_json.paint(transcript_bounds, paint);
    inputs.model.paint(model_bounds, paint);
    inputs.session_id.paint(session_bounds, paint);
    inputs.max_tokens.paint(max_tokens_bounds, paint);
    inputs.temperature.paint(temperature_bounds, paint);
    inputs.top.paint(top_bounds, paint);
    inputs
        .probability_threshold
        .paint(probability_threshold_bounds, paint);
    inputs.seed.paint(seed_bounds, paint);

    paint_input_label(paint, instructions_bounds, "Instructions");
    paint_input_label(paint, prompt_bounds, "Prompt");
    paint_input_label(paint, schema_bounds, "Structured schema JSON");
    paint_input_label(paint, transcript_bounds, "Transcript JSON");
    paint_input_label(paint, model_bounds, "Requested model");
    paint_input_label(paint, session_bounds, "Session id");
    paint_input_label(paint, max_tokens_bounds, "Max tokens");
    paint_input_label(paint, temperature_bounds, "Temperature");
    paint_input_label(paint, top_bounds, "Top-k");
    paint_input_label(paint, probability_threshold_bounds, "Top-p");
    paint_input_label(paint, seed_bounds, "Seed");

    let mut line_y = model_bounds.max_y() + 20.0;
    line_y = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Bridge",
        runtime.bridge_status.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Ready model",
        runtime.ready_model.as_deref().unwrap_or("-"),
    );
    line_y = paint_multiline_phrase(
        paint,
        model_bounds.origin.x,
        line_y,
        "Available",
        join_models(runtime.available_models.as_slice()).as_str(),
    );
    line_y = paint_multiline_phrase(
        paint,
        model_bounds.origin.x,
        line_y,
        "Use cases",
        join_labels(
            runtime
                .supported_use_cases
                .iter()
                .map(|value| value.label()),
        )
        .as_str(),
    );
    line_y = paint_multiline_phrase(
        paint,
        model_bounds.origin.x,
        line_y,
        "Guardrails",
        join_labels(
            runtime
                .supported_guardrails
                .iter()
                .map(|value| value.label()),
        )
        .as_str(),
    );
    line_y = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Pending",
        pane_state.pending_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Last request",
        pane_state.last_request_id.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Operation",
        pane_state.last_operation.as_deref().unwrap_or("-"),
    );
    let _ = paint_label_line(
        paint,
        model_bounds.origin.x,
        line_y,
        "Session",
        pane_state.active_session_id.as_deref().unwrap_or("-"),
    );

    paint_output_panel(output_bounds, pane_state, paint);
    pane_state.event_log.paint(event_log_bounds, paint);
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AppleFmWorkbench)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .apple_fm_workbench_inputs
        .instructions
        .event(
            event,
            apple_fm_workbench_instructions_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .prompt
        .event(
            event,
            apple_fm_workbench_prompt_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .schema_json
        .event(
            event,
            apple_fm_workbench_schema_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .transcript_json
        .event(
            event,
            apple_fm_workbench_transcript_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .model
        .event(
            event,
            apple_fm_workbench_model_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .session_id
        .event(
            event,
            apple_fm_workbench_session_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .max_tokens
        .event(
            event,
            apple_fm_workbench_max_tokens_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .temperature
        .event(
            event,
            apple_fm_workbench_temperature_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .top
        .event(
            event,
            apple_fm_workbench_top_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .probability_threshold
        .event(
            event,
            apple_fm_workbench_probability_threshold_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .seed
        .event(
            event,
            apple_fm_workbench_seed_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled
}

fn paint_input_label(paint: &mut PaintContext, bounds: Bounds, label: &str) {
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x, bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_output_panel(
    bounds: Bounds,
    pane_state: &AppleFmWorkbenchPaneState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.72))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );

    paint.scene.draw_text(paint.text.layout(
        "Latest output",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        11.0,
        theme::text::MUTED,
    ));

    let sections = [
        (
            "Response",
            preview_or_placeholder(
                pane_state.output_preview.as_str(),
                "No Apple FM workbench response yet.",
            ),
        ),
        (
            "Structured",
            preview_or_placeholder(pane_state.structured_preview.as_str(), "-"),
        ),
        (
            "Session",
            preview_or_placeholder(pane_state.session_preview.as_str(), "-"),
        ),
        (
            "Usage",
            preview_or_placeholder(pane_state.usage_preview.as_str(), "-"),
        ),
    ];

    let mut y = bounds.origin.y + 28.0;
    for (label, value) in sections {
        if y + OUTPUT_SECTION_LABEL_GAP >= bounds.max_y() {
            break;
        }
        paint.scene.draw_text(paint.text.layout(
            label,
            Point::new(bounds.origin.x + 10.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += OUTPUT_SECTION_LABEL_GAP;
        for line in wrap_preview_lines(value.as_str(), OUTPUT_SECTION_MAX_LINES) {
            if y + OUTPUT_SECTION_LINE_HEIGHT >= bounds.max_y() {
                break;
            }
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(bounds.origin.x + 10.0, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += OUTPUT_SECTION_LINE_HEIGHT;
        }
        y += 8.0;
    }
}

fn preview_or_placeholder(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn wrap_preview_lines(value: &str, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in value.lines() {
        if lines.len() >= max_lines {
            break;
        }
        let line = if raw_line.trim().is_empty() {
            " ".to_string()
        } else {
            raw_line.to_string()
        };
        if line.chars().count() <= 62 {
            lines.push(line);
            continue;
        }

        let chars = line.chars().collect::<Vec<_>>();
        let mut start = 0usize;
        while start < chars.len() && lines.len() < max_lines {
            let end = (start + 62).min(chars.len());
            lines.push(chars[start..end].iter().collect());
            start = end;
        }
    }

    if lines.is_empty() {
        lines.push("-".to_string());
    }
    lines
}

fn join_models(models: &[String]) -> String {
    if models.is_empty() {
        "-".to_string()
    } else {
        models.join(", ")
    }
}

fn join_labels<'a>(labels: impl Iterator<Item = &'a str>) -> String {
    let values = labels.collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}
