use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wgpui::components::hud::{DotShape, DotsGrid, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::viz::panel::paint_shell as paint_viz_panel_shell;
use wgpui::viz::theme as viz_theme;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::LocalInferencePaneState;
use crate::campaign::{Campaign, CampaignId};
use crate::local_inference_runtime::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance,
    LocalInferenceExecutionSnapshot,
};
use crate::pane_renderer::{
    paint_label_line, paint_multiline_phrase, paint_source_badge, paint_state_summary,
};
use crate::probe::{ProbeSummary, ProbeSummaryId};
use crate::psionic::{PsionicEvalRef, PsionicEvalRefId};

const LATTICE_ROWS: usize = 12;
const LATTICE_COLS: usize = 24;
const SYNTHETIC_LAYER_BANDS: usize = 24;
const RIBBON_SEGMENTS: usize = 32;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &LocalInferencePaneState,
    runtime: &LocalInferenceExecutionSnapshot,
    campaigns: &[Campaign],
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime+viz", paint);

    let phase = animation_phase(runtime, pane_state);
    let active = runtime.busy || pane_state.pending_request_id.is_some();
    let accent = mesh_accent(active, runtime.artifact_present, runtime.is_ready());

    let field_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        content_bounds.origin.y + 34.0,
        content_bounds.size.width * 0.58,
        content_bounds.size.height * 0.48,
    );
    let layer_bounds = Bounds::new(
        field_bounds.origin.x,
        field_bounds.max_y() + 12.0,
        field_bounds.size.width,
        108.0,
    );
    let telemetry_bounds = Bounds::new(
        field_bounds.max_x() + 16.0,
        field_bounds.origin.y,
        content_bounds.max_x() - field_bounds.max_x() - 28.0,
        content_bounds.size.height * 0.68,
    );
    let ribbon_bounds = Bounds::new(
        field_bounds.origin.x,
        layer_bounds.max_y() + 12.0,
        content_bounds.size.width - 24.0,
        content_bounds.max_y() - layer_bounds.max_y() - 24.0,
    );

    paint_title_block(content_bounds, runtime, pane_state, active, accent, paint);
    paint_viz_panel_shell(field_bounds, accent, paint);
    paint_viz_panel_shell(layer_bounds, accent.with_alpha(0.88), paint);
    paint_viz_panel_shell(telemetry_bounds, accent.with_alpha(0.82), paint);
    paint_viz_panel_shell(ribbon_bounds, accent.with_alpha(0.78), paint);

    paint_decode_lattice(field_bounds, runtime, pane_state, phase, accent, paint);
    paint_layer_sweep(layer_bounds, runtime, pane_state, phase, accent, paint);
    paint_runtime_telemetry(telemetry_bounds, runtime, pane_state, active, accent, paint);
    paint_campaign_selection(ribbon_bounds, campaigns, runtime, pane_state, accent, paint);
}

fn paint_title_block(
    content_bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    active: bool,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let title = if active {
        "PSIONIC MESH  //  ACTIVE DECODE FIELD"
    } else if runtime.is_ready() {
        "PSIONIC MESH  //  READY STANDBY"
    } else if runtime.artifact_present {
        "PSIONIC MESH  //  UNLOADED"
    } else {
        "PSIONIC MESH  //  COLD SHELL"
    };
    let subtitle = if active {
        "Synthetic layer lattice driven by live Psionic / GPT-OSS runtime counters."
    } else {
        "Derived from runtime metrics and provenance. This is a visualization field, not raw tensor taps."
    };

    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 16.0,
        ),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        subtitle,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 34.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let summary = format!(
        "Field {}",
        if active {
            "tracking active decode"
        } else if runtime.is_ready() {
            "primed for next prompt"
        } else if runtime.artifact_present {
            "waiting for model warm"
        } else {
            "waiting for runtime artifact"
        }
    );
    let _ = paint_state_summary(
        paint,
        content_bounds.max_x() - 270.0,
        content_bounds.origin.y + 8.0,
        pane_state.load_state,
        summary.as_str(),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            content_bounds.max_x() - 156.0,
            content_bounds.origin.y + 18.0,
            136.0,
            2.0,
        ))
        .with_background(accent.with_alpha(0.65)),
    );
}

fn paint_decode_lattice(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "LATTICE",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.88),
    ));

    let matrix_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 28.0,
        bounds.size.width - 24.0,
        bounds.size.height - 40.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(26.0)
        .size(1.2)
        .color(accent.with_alpha(0.24))
        .animation_progress(phase);
    dots.paint(matrix_bounds, paint);

    let mut heatmap = Heatmap::new()
        .data(
            LATTICE_ROWS,
            LATTICE_COLS,
            build_lattice_matrix(runtime, pane_state, phase),
        )
        .range(0.0, 1.0)
        .gap(2.0)
        .color(accent.with_alpha(0.88));
    heatmap.paint(matrix_bounds, paint);
}

fn build_lattice_matrix(
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
) -> Vec<f32> {
    let mut matrix = vec![0.0; LATTICE_ROWS * LATTICE_COLS];
    // implement the logic to populate the matrix
    matrix
}

fn paint_layer_sweep(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    phase: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    // implement the logic to paint the layer sweep
}

fn paint_runtime_telemetry(
    bounds: Bounds,
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    active: bool,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    // implement the logic to paint the runtime telemetry
}

fn paint_campaign_selection(
    bounds: Bounds,
    campaigns: &[Campaign],
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    // implement the logic to paint the campaign selection
}

fn animation_phase(
    runtime: &LocalInferenceExecutionSnapshot,
    pane_state: &LocalInferencePaneState,
) -> f32 {
    // implement the logic to calculate the animation phase
    0.0
}

fn mesh_accent(
    active: bool,
    artifact_present: bool,
    is_ready: bool,
) -> Hsla {
    // implement the logic to calculate the mesh accent
    Hsla::new(0.0, 0.0, 0.0, 1.0)
}