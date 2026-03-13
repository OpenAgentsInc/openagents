use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, theme};

use crate::app_state::{LocalInferencePaneInputs, LocalInferencePaneState, PaneKind, RenderState};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::local_runtime_capabilities::{
    LocalRuntimeCapabilitySurface, local_runtime_capability_summary,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    local_inference_max_tokens_input_bounds, local_inference_prompt_input_bounds,
    local_inference_refresh_button_bounds, local_inference_requested_model_input_bounds,
    local_inference_run_button_bounds, local_inference_temperature_input_bounds,
    local_inference_top_k_input_bounds, local_inference_top_p_input_bounds,
    local_inference_unload_button_bounds, local_inference_warm_button_bounds, pane_content_bounds,
};

pub fn paint(
    content_bounds: Bounds,
    pane_state: &LocalInferencePaneState,
    capability_surface: &LocalRuntimeCapabilitySurface,
    runtime: &LocalInferenceExecutionSnapshot,
    inputs: &mut LocalInferencePaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let refresh_bounds = local_inference_refresh_button_bounds(content_bounds);
    let warm_bounds = local_inference_warm_button_bounds(content_bounds);
    let unload_bounds = local_inference_unload_button_bounds(content_bounds);
    let run_bounds = local_inference_run_button_bounds(content_bounds);
    let prompt_bounds = local_inference_prompt_input_bounds(content_bounds);
    let requested_model_bounds = local_inference_requested_model_input_bounds(content_bounds);
    let max_tokens_bounds = local_inference_max_tokens_input_bounds(content_bounds);
    let temperature_bounds = local_inference_temperature_input_bounds(content_bounds);
    let top_k_bounds = local_inference_top_k_input_bounds(content_bounds);
    let top_p_bounds = local_inference_top_p_input_bounds(content_bounds);

    paint_action_button(refresh_bounds, "Refresh runtime", paint);
    paint_action_button(warm_bounds, "Warm model", paint);
    paint_action_button(unload_bounds, "Unload model", paint);
    paint_action_button(run_bounds, "Run prompt", paint);

    let title_x = run_bounds.max_x() + 16.0;
    paint.scene.draw_text(paint.text.layout(
        capability_surface.workbench_label,
        Point::new(title_x, content_bounds.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    let capability_summary = local_runtime_capability_summary(capability_surface);
    paint.scene.draw_text(paint.text.layout(
        capability_summary.as_str(),
        Point::new(title_x, content_bounds.origin.y + 34.0),
        11.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "Runtime: {}",
        if runtime.busy || pane_state.pending_request_id.is_some() {
            "running"
        } else if runtime.is_ready() {
            "ready"
        } else if runtime.artifact_present {
            "not loaded"
        } else if runtime.reachable {
            "reachable"
        } else {
            "waiting"
        }
    );
    let _ = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        refresh_bounds.max_y() + 12.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    inputs
        .prompt
        .set_max_width(prompt_bounds.size.width.max(180.0));
    inputs
        .requested_model
        .set_max_width(requested_model_bounds.size.width.max(120.0));
    inputs
        .max_tokens
        .set_max_width(max_tokens_bounds.size.width.max(72.0));
    inputs
        .temperature
        .set_max_width(temperature_bounds.size.width.max(72.0));
    inputs
        .top_k
        .set_max_width(top_k_bounds.size.width.max(72.0));
    inputs
        .top_p
        .set_max_width(top_p_bounds.size.width.max(72.0));

    inputs.prompt.paint(prompt_bounds, paint);
    inputs.requested_model.paint(requested_model_bounds, paint);
    inputs.max_tokens.paint(max_tokens_bounds, paint);
    inputs.temperature.paint(temperature_bounds, paint);
    inputs.top_k.paint(top_k_bounds, paint);
    inputs.top_p.paint(top_p_bounds, paint);

    paint.scene.draw_text(paint.text.layout(
        "Prompt",
        Point::new(prompt_bounds.origin.x, prompt_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Requested model",
        Point::new(
            requested_model_bounds.origin.x,
            requested_model_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Max tokens",
        Point::new(
            max_tokens_bounds.origin.x,
            max_tokens_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Temperature",
        Point::new(
            temperature_bounds.origin.x,
            temperature_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Top-k",
        Point::new(top_k_bounds.origin.x, top_k_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Top-p",
        Point::new(top_p_bounds.origin.x, top_p_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let mut line_y = requested_model_bounds.max_y() + 20.0;
    let endpoint = if runtime.base_url.trim().is_empty() {
        "-"
    } else {
        runtime.base_url.as_str()
    };
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Endpoint",
        endpoint,
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Configured",
        runtime.configured_model.as_deref().unwrap_or("-"),
    );
    line_y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Model path",
        runtime.configured_model_path.as_deref().unwrap_or("-"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Backend",
        if runtime.backend_label.trim().is_empty() {
            "-"
        } else {
            runtime.backend_label.as_str()
        },
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Artifact",
        if runtime.artifact_present {
            "present"
        } else {
            "missing"
        },
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Ready model",
        runtime.ready_model.as_deref().unwrap_or("-"),
    );
    let available_models = join_models(runtime.available_models.as_slice());
    line_y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Available",
        available_models.as_str(),
    );
    let loaded_models = join_models(runtime.loaded_models.as_slice());
    line_y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Loaded",
        loaded_models.as_str(),
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
        "Served model",
        pane_state.last_model.as_deref().unwrap_or("-"),
    );

    if let Some(metrics) = pane_state.last_metrics.as_ref() {
        let metrics_summary = format!(
            "prompt={} eval={} total={}ms",
            metrics.prompt_eval_count.unwrap_or(0),
            metrics.eval_count.unwrap_or(0),
            metrics
                .total_duration_ns
                .map(|value| value / 1_000_000)
                .unwrap_or(0)
        );
        line_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Metrics",
            metrics_summary.as_str(),
        );
    }

    if let Some(provenance) = pane_state.last_provenance.as_ref() {
        line_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Backend",
            provenance.backend.as_str(),
        );
        line_y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Prompt digest",
            provenance.normalized_prompt_digest.as_str(),
        );
        line_y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Options digest",
            provenance.normalized_options_digest.as_str(),
        );
    }

    let output_value = if pane_state.output_preview.trim().is_empty() {
        "No workbench response yet.".to_string()
    } else if pane_state.output_chars > pane_state.output_preview.chars().count() {
        format!(
            "{}\n\n[truncated {} chars]",
            pane_state.output_preview,
            pane_state
                .output_chars
                .saturating_sub(pane_state.output_preview.chars().count())
        )
    } else {
        pane_state.output_preview.clone()
    };
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y + 8.0,
        "Output",
        output_value.as_str(),
    );
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::LocalInference)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .local_inference_inputs
        .prompt
        .event(
            event,
            local_inference_prompt_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .local_inference_inputs
        .requested_model
        .event(
            event,
            local_inference_requested_model_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .local_inference_inputs
        .max_tokens
        .event(
            event,
            local_inference_max_tokens_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .local_inference_inputs
        .temperature
        .event(
            event,
            local_inference_temperature_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .local_inference_inputs
        .top_k
        .event(
            event,
            local_inference_top_k_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .local_inference_inputs
        .top_p
        .event(
            event,
            local_inference_top_p_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled
}

fn join_models(models: &[String]) -> String {
    if models.is_empty() {
        "-".to_string()
    } else {
        models.join(", ")
    }
}
