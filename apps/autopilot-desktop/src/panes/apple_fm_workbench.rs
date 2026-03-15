use wgpui::{Bounds, Component, Hsla, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AppleFmWorkbenchPaneInputs, AppleFmWorkbenchPaneState, PaneKind, RenderState,
};
use crate::apple_fm_bridge::AppleFmBridgeSnapshot;
use crate::local_runtime_capabilities::{
    LocalRuntimeCapabilitySurface, local_runtime_capability_summary,
};
use crate::pane_renderer::{
    paint_action_button, paint_secondary_button, paint_wrapped_label_line, split_text_for_display,
};
use crate::pane_system::{
    apple_fm_workbench_adapter_id_input_bounds, apple_fm_workbench_adapter_package_input_bounds,
    apple_fm_workbench_attach_adapter_button_bounds,
    apple_fm_workbench_create_session_button_bounds,
    apple_fm_workbench_delete_session_button_bounds,
    apple_fm_workbench_detach_adapter_button_bounds, apple_fm_workbench_event_log_bounds,
    apple_fm_workbench_export_transcript_button_bounds,
    apple_fm_workbench_inspect_session_button_bounds, apple_fm_workbench_instructions_input_bounds,
    apple_fm_workbench_layout, apple_fm_workbench_load_adapter_button_bounds,
    apple_fm_workbench_max_tokens_input_bounds, apple_fm_workbench_model_input_bounds,
    apple_fm_workbench_options_details_bounds, apple_fm_workbench_output_bounds,
    apple_fm_workbench_probability_threshold_input_bounds, apple_fm_workbench_prompt_input_bounds,
    apple_fm_workbench_refresh_button_bounds, apple_fm_workbench_reset_session_button_bounds,
    apple_fm_workbench_restore_transcript_button_bounds, apple_fm_workbench_run_chat_button_bounds,
    apple_fm_workbench_run_session_button_bounds, apple_fm_workbench_run_stream_button_bounds,
    apple_fm_workbench_run_structured_button_bounds, apple_fm_workbench_run_text_button_bounds,
    apple_fm_workbench_sampling_mode_button_bounds, apple_fm_workbench_schema_input_bounds,
    apple_fm_workbench_seed_input_bounds, apple_fm_workbench_session_input_bounds,
    apple_fm_workbench_start_bridge_button_bounds, apple_fm_workbench_temperature_input_bounds,
    apple_fm_workbench_tool_profile_button_bounds, apple_fm_workbench_top_input_bounds,
    apple_fm_workbench_transcript_input_bounds, apple_fm_workbench_unload_adapter_button_bounds,
    pane_content_bounds,
};

const OUTPUT_SECTION_LABEL_GAP: f32 = 14.0;
const OUTPUT_SECTION_LINE_HEIGHT: f32 = 14.0;
const OUTPUT_SECTION_MAX_LINES: usize = 7;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut AppleFmWorkbenchPaneState,
    capability_surface: &LocalRuntimeCapabilitySurface,
    runtime: &AppleFmBridgeSnapshot,
    inputs: &mut AppleFmWorkbenchPaneInputs,
    paint: &mut PaintContext,
) {
    let layout = apple_fm_workbench_layout(content_bounds);
    let refresh_bounds = apple_fm_workbench_refresh_button_bounds(content_bounds);
    let start_bounds = apple_fm_workbench_start_bridge_button_bounds(content_bounds);
    let create_bounds = apple_fm_workbench_create_session_button_bounds(content_bounds);
    let inspect_bounds = apple_fm_workbench_inspect_session_button_bounds(content_bounds);
    let load_adapter_bounds = apple_fm_workbench_load_adapter_button_bounds(content_bounds);
    let unload_adapter_bounds = apple_fm_workbench_unload_adapter_button_bounds(content_bounds);
    let attach_adapter_bounds = apple_fm_workbench_attach_adapter_button_bounds(content_bounds);
    let detach_adapter_bounds = apple_fm_workbench_detach_adapter_button_bounds(content_bounds);
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
    let adapter_id_bounds = apple_fm_workbench_adapter_id_input_bounds(content_bounds);
    let adapter_package_bounds = apple_fm_workbench_adapter_package_input_bounds(content_bounds);
    let max_tokens_bounds = apple_fm_workbench_max_tokens_input_bounds(content_bounds);
    let temperature_bounds = apple_fm_workbench_temperature_input_bounds(content_bounds);
    let top_bounds = apple_fm_workbench_top_input_bounds(content_bounds);
    let probability_threshold_bounds =
        apple_fm_workbench_probability_threshold_input_bounds(content_bounds);
    let seed_bounds = apple_fm_workbench_seed_input_bounds(content_bounds);
    let output_bounds = apple_fm_workbench_output_bounds(content_bounds);
    let event_log_bounds = apple_fm_workbench_event_log_bounds(content_bounds);

    let bridge_status = bridge_status_label(pane_state, runtime);
    let bridge_accent = bridge_status_accent(pane_state, runtime);
    let active_model = compact_workbench_value(
        pane_state
            .last_model
            .as_deref()
            .or(runtime.ready_model.as_deref())
            .unwrap_or("-"),
        24,
    );
    let summary_session = pane_state
        .active_session_id
        .clone()
        .or_else(|| normalize_field(inputs.session_id.get_value()))
        .unwrap_or_else(|| "-".to_string());
    let operation_summary =
        compact_workbench_value(pane_state.last_operation.as_deref().unwrap_or("idle"), 24);
    let use_cases_summary = compact_workbench_value(
        join_labels(
            runtime
                .supported_use_cases
                .iter()
                .map(|value| value.label()),
        )
        .as_str(),
        28,
    );
    let guardrails_summary = compact_workbench_value(
        join_labels(
            runtime
                .supported_guardrails
                .iter()
                .map(|value| value.label()),
        )
        .as_str(),
        28,
    );
    let adapters_summary = compact_workbench_value(
        format!(
            "{} loaded // attached {}",
            runtime.loaded_adapters.len(),
            pane_state
                .active_session_adapter
                .as_ref()
                .map(|adapter| adapter.adapter_id.as_str())
                .unwrap_or("-")
        )
        .as_str(),
        28,
    );
    let status_gap = 8.0;
    let status_cell_width = ((layout.status_row.size.width - status_gap * 3.0) / 4.0).max(0.0);
    for (index, (label, value, accent)) in [
        ("BRIDGE", bridge_status, bridge_accent),
        ("MODEL", active_model.as_str(), workbench_green_color()),
        (
            "USE CASES",
            use_cases_summary.as_str(),
            workbench_cyan_color(),
        ),
        (
            "ADAPTERS",
            adapters_summary.as_str(),
            workbench_amber_color(),
        ),
    ]
    .iter()
    .enumerate()
    {
        let x = layout.status_row.origin.x + index as f32 * (status_cell_width + status_gap);
        let width = if index == 3 {
            (layout.status_row.max_x() - x).max(0.0)
        } else {
            status_cell_width
        };
        paint_workbench_status_cell(
            Bounds::new(
                x,
                layout.status_row.origin.y,
                width,
                layout.status_row.size.height,
            ),
            label,
            value,
            *accent,
            paint,
        );
    }
    let summary_text = workbench_summary_text(
        capability_surface,
        pane_state,
        runtime,
        operation_summary.as_str(),
        summary_session.as_str(),
    );
    paint_workbench_summary_band(
        layout.summary_band,
        summary_text.as_str(),
        bridge_accent,
        paint,
    );
    paint_workbench_section_panel(
        layout.management_panel,
        "BRIDGE CONTROL",
        workbench_orange_color(),
        paint,
    );
    paint_workbench_section_panel(
        layout.execution_panel,
        "RUN WORKFLOWS",
        workbench_green_color(),
        paint,
    );
    paint_workbench_section_panel(layout.mode_panel, "TOOLING", workbench_cyan_color(), paint);
    paint_workbench_section_panel(
        layout.text_panel,
        "TEXT INPUTS",
        workbench_orange_color(),
        paint,
    );
    paint_workbench_section_panel(
        layout.payload_panel,
        "STRUCTURED PAYLOADS",
        workbench_cyan_color(),
        paint,
    );
    paint_workbench_section_panel(
        layout.options_panel,
        "REQUEST OPTIONS",
        workbench_amber_color(),
        paint,
    );
    paint_workbench_section_panel(
        layout.output_panel,
        "LATEST OUTPUT",
        workbench_green_color(),
        paint,
    );
    if layout.event_log_panel.size.height > 0.0 {
        paint_workbench_section_panel(
            layout.event_log_panel,
            "EVENT LOG",
            workbench_cyan_color(),
            paint,
        );
    }

    paint_action_button(refresh_bounds, "Refresh bridge", paint);
    paint_action_button(start_bounds, "Start bridge", paint);
    paint_action_button(create_bounds, "Create session", paint);
    paint_action_button(inspect_bounds, "Inspect session", paint);
    paint_action_button(load_adapter_bounds, "Load adapter", paint);
    paint_action_button(unload_adapter_bounds, "Unload adapter", paint);
    paint_action_button(attach_adapter_bounds, "Attach adapter", paint);
    paint_action_button(detach_adapter_bounds, "Detach adapter", paint);
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
        .adapter_id
        .set_max_width(adapter_id_bounds.size.width.max(120.0));
    inputs
        .adapter_package_path
        .set_max_width(adapter_package_bounds.size.width.max(160.0));
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
    inputs.adapter_id.paint(adapter_id_bounds, paint);
    inputs
        .adapter_package_path
        .paint(adapter_package_bounds, paint);
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
    paint_input_label(paint, adapter_id_bounds, "Adapter id");
    paint_input_label(paint, adapter_package_bounds, "Adapter package path");
    paint_input_label(paint, max_tokens_bounds, "Max tokens");
    paint_input_label(paint, temperature_bounds, "Temperature");
    paint_input_label(paint, top_bounds, "Top-k");
    paint_input_label(paint, probability_threshold_bounds, "Top-p");
    paint_input_label(paint, seed_bounds, "Seed");

    let mode_clip = workbench_section_clip_bounds(layout.mode_panel);
    paint.scene.push_clip(mode_clip);
    let mode_chunk_len = workbench_value_chunk_len(layout.mode_panel);
    let mut mode_y = sampling_mode_bounds.max_y() + 14.0;
    mode_y = paint_wrapped_label_line(
        paint,
        tool_profile_bounds.origin.x,
        mode_y,
        "Operation",
        pane_state.last_operation.as_deref().unwrap_or("-"),
        mode_chunk_len,
    );
    mode_y = paint_wrapped_label_line(
        paint,
        tool_profile_bounds.origin.x,
        mode_y,
        "Session",
        summary_session.as_str(),
        mode_chunk_len,
    );
    let _ = paint_wrapped_label_line(
        paint,
        tool_profile_bounds.origin.x,
        mode_y,
        "Attached",
        pane_state
            .active_session_adapter
            .as_ref()
            .map(|adapter| adapter.adapter_id.as_str())
            .unwrap_or("-"),
        mode_chunk_len,
    );
    paint.scene.pop_clip();

    let options_details_bounds = apple_fm_workbench_options_details_bounds(content_bounds);
    let options_clip = workbench_section_clip_bounds(layout.options_panel);
    paint.scene.push_clip(options_clip);
    let options_chunk_len = workbench_value_chunk_len(layout.options_panel);
    let mut options_y = options_details_bounds.origin.y + 6.0;
    options_y = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Bridge",
        runtime.bridge_status.as_deref().unwrap_or("-"),
        options_chunk_len,
    );
    options_y = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Available",
        join_models(runtime.available_models.as_slice()).as_str(),
        options_chunk_len,
    );
    options_y = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Guardrails",
        guardrails_summary.as_str(),
        options_chunk_len,
    );
    options_y = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Adapters",
        pane_state.adapter_preview.as_str(),
        options_chunk_len,
    );
    options_y = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Pending",
        pane_state.pending_request_id.as_deref().unwrap_or("-"),
        options_chunk_len,
    );
    let _ = paint_wrapped_label_line(
        paint,
        options_details_bounds.origin.x,
        options_y,
        "Last request",
        pane_state.last_request_id.as_deref().unwrap_or("-"),
        options_chunk_len,
    );
    if let Some(handoff) = pane_state.handoff_summary.as_deref() {
        let _ = paint_wrapped_label_line(
            paint,
            options_details_bounds.origin.x,
            options_y + 14.0,
            "Handoff",
            handoff,
            options_chunk_len,
        );
    }
    paint.scene.pop_clip();

    paint_output_panel(output_bounds, pane_state, paint);
    pane_state.event_log.set_title("");
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
        .adapter_id
        .event(
            event,
            apple_fm_workbench_adapter_id_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_fm_workbench_inputs
        .adapter_package_path
        .event(
            event,
            apple_fm_workbench_adapter_package_input_bounds(content_bounds),
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

fn paint_workbench_section_panel(
    bounds: Bounds,
    title: &str,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    if bounds.size.width <= 1.0 || bounds.size.height <= 1.0 {
        return;
    }

    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(workbench_panel_color())
            .with_border(workbench_panel_border_color(), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            4.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.85)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y,
            (bounds.size.width - 4.0).max(0.0),
            22.0,
        ))
        .with_background(accent.with_alpha(0.12)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("\\\\ {title}"),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 6.0),
        11.0,
        accent,
    ));
}

fn paint_workbench_status_cell(
    bounds: Bounds,
    label: &str,
    value: &str,
    value_color: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(workbench_panel_color())
            .with_border(workbench_panel_border_color(), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            2.0,
        ))
        .with_background(value_color.with_alpha(0.85)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 6.0),
        9.0,
        workbench_muted_color(),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 20.0),
        11.0,
        value_color,
    ));
}

fn paint_workbench_summary_band(
    bounds: Bounds,
    text: &str,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(0.10))
            .with_border(accent.with_alpha(0.72), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            10.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.85)),
    );
    let chunk_len = ((bounds.size.width - 30.0) / 6.4).floor().max(14.0) as usize;
    let line = split_text_for_display(text, chunk_len)
        .into_iter()
        .next()
        .unwrap_or_else(|| "SWIFT BRIDGE".to_string());
    paint.scene.draw_text(paint.text.layout_mono(
        &line,
        Point::new(bounds.origin.x + 16.0, bounds.origin.y + 9.0),
        11.0,
        workbench_text_color(),
    ));
}

fn workbench_section_clip_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 6.0,
        bounds.origin.y + 22.0,
        (bounds.size.width - 12.0).max(0.0),
        (bounds.size.height - 28.0).max(0.0),
    )
}

fn workbench_value_chunk_len(panel: Bounds) -> usize {
    let width = (panel.size.width - 150.0).max(48.0);
    ((width / 6.2).floor() as usize).max(8)
}

fn bridge_status_label(
    pane_state: &AppleFmWorkbenchPaneState,
    runtime: &AppleFmBridgeSnapshot,
) -> &'static str {
    if pane_state.pending_request_id.is_some() {
        "RUNNING"
    } else if runtime.is_ready() {
        "READY"
    } else if runtime.reachable {
        "REACHABLE"
    } else if pane_state.last_error.is_some() || runtime.last_error.is_some() {
        "ERROR"
    } else {
        "WAITING"
    }
}

fn bridge_status_accent(
    pane_state: &AppleFmWorkbenchPaneState,
    runtime: &AppleFmBridgeSnapshot,
) -> Hsla {
    match bridge_status_label(pane_state, runtime) {
        "RUNNING" => workbench_green_color(),
        "READY" => workbench_green_color(),
        "REACHABLE" => workbench_cyan_color(),
        "ERROR" => workbench_red_color(),
        _ => workbench_amber_color(),
    }
}

fn workbench_summary_text(
    capability_surface: &LocalRuntimeCapabilitySurface,
    pane_state: &AppleFmWorkbenchPaneState,
    runtime: &AppleFmBridgeSnapshot,
    operation: &str,
    session: &str,
) -> String {
    let capability_summary = local_runtime_capability_summary(capability_surface);
    let headline = if let Some(error) = pane_state
        .last_error
        .as_deref()
        .or(runtime.last_error.as_deref())
    {
        format!("FAULT {}", compact_workbench_value(error, 48))
    } else if let Some(action) = pane_state.last_action.as_deref() {
        compact_workbench_value(action, 52)
    } else {
        "Waiting for Apple FM bridge snapshot".to_string()
    };
    let request = pane_state
        .pending_request_id
        .as_deref()
        .map(|value| format!("REQUEST {}", compact_workbench_value(value, 24)))
        .or_else(|| {
            pane_state
                .last_request_id
                .as_deref()
                .map(|value| format!("LAST {}", compact_workbench_value(value, 24)))
        })
        .unwrap_or_else(|| "REQUEST idle".to_string());
    let handoff = pane_state
        .handoff_source_run_id
        .as_deref()
        .map(|value| format!("HANDOFF {}", compact_workbench_value(value, 18)))
        .unwrap_or_else(|| "HANDOFF none".to_string());
    format!(
        "{} // CAP {} // {headline} // OP {} // SESSION {} // {handoff} // {request}",
        capability_surface.workbench_label.to_ascii_uppercase(),
        compact_workbench_value(capability_summary.as_str(), 36),
        compact_workbench_value(operation, 18),
        compact_workbench_value(session, 18),
    )
}

fn normalize_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn compact_workbench_value(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars.max(1) {
        return if trimmed.is_empty() {
            "-".to_string()
        } else {
            trimmed.to_string()
        };
    }
    let head = max_chars.saturating_sub(9).max(4);
    let tail = 8.min(chars.len().saturating_sub(head));
    format!(
        "{}...{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len().saturating_sub(tail)..]
            .iter()
            .collect::<String>()
    )
}

fn workbench_panel_color() -> Hsla {
    Hsla::from_hex(0x101010)
}

fn workbench_panel_border_color() -> Hsla {
    Hsla::from_hex(0x3D3327)
}

fn workbench_text_color() -> Hsla {
    Hsla::from_hex(0xE8E3D7)
}

fn workbench_muted_color() -> Hsla {
    Hsla::from_hex(0x7F776D)
}

fn workbench_orange_color() -> Hsla {
    Hsla::from_hex(0xFF6A00)
}

fn workbench_amber_color() -> Hsla {
    Hsla::from_hex(0xFFB300)
}

fn workbench_green_color() -> Hsla {
    Hsla::from_hex(0x7DFF4A)
}

fn workbench_cyan_color() -> Hsla {
    Hsla::from_hex(0x46D9D3)
}

fn workbench_red_color() -> Hsla {
    Hsla::from_hex(0xD71414)
}

fn paint_output_panel(
    bounds: Bounds,
    pane_state: &AppleFmWorkbenchPaneState,
    paint: &mut PaintContext,
) {
    let sections = [
        (
            "Response",
            preview_or_placeholder(
                pane_state.output_preview.as_str(),
                "No Apple FM workbench response yet.",
            ),
        ),
        (
            "Adapters",
            preview_or_placeholder(pane_state.adapter_preview.as_str(), "-"),
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

    let mut y = bounds.origin.y + 6.0;
    for (label, value) in sections {
        if y + OUTPUT_SECTION_LABEL_GAP >= bounds.max_y() {
            break;
        }
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(bounds.origin.x + 10.0, y),
            10.0,
            workbench_muted_color(),
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
                workbench_text_color(),
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
