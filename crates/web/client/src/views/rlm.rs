//! RLM (Recursive Language Model) visualization page
//!
//! Interactive "execution movie" showing RLM processing documents through
//! structure discovery, chunking, extraction, and synthesis phases.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{theme, Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::{
    AppState, RlmChunkState, RlmConnectionStatus, RlmDemoTrace, RlmPhase, RlmStepStatus,
    RlmTraceEventType,
};

/// Embedded trace JSON for demo playback
const DEMO_TRACE_JSON: &str = include_str!("../../assets/rlm-demo-trace.json");

// Color scheme
fn accent_cyan() -> Hsla {
    Hsla::from_hex(0x7fd3e5)
}

fn accent_green() -> Hsla {
    Hsla::from_hex(0x00ff88)
}

fn accent_orange() -> Hsla {
    Hsla::from_hex(0xff9900)
}

fn accent_red() -> Hsla {
    Hsla::from_hex(0xff4444)
}

fn phase_pending() -> Hsla {
    Hsla::new(0.0, 0.0, 0.4, 1.0)
}

fn phase_processing() -> Hsla {
    accent_orange()
}

fn phase_complete() -> Hsla {
    accent_green()
}

fn phase_error() -> Hsla {
    accent_red()
}

fn panel_bg() -> Hsla {
    Hsla::from_hex(0x05070b)
}

fn panel_border() -> Hsla {
    Hsla::from_hex(0x2a3640)
}

/// Build the RLM visualization page
pub(crate) fn build_rlm_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Demo mode simulation
    if state.rlm.demo_mode {
        tick_demo(state);
    }

    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.18))
        .distance(34.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Layout
    let padding = 22.0;
    let content_width = (width - padding * 2.0).min(1200.0);
    let content_x = (width - content_width) / 2.0;
    let card_y = padding;
    let card_height = height - padding * 2.0;
    let card_bounds = Bounds::new(content_x, card_y, content_width, card_height);

    // Frame animation
    if !state.rlm.frame_started {
        state.rlm.frame_started = true;
    }
    let frame_progress = state.rlm.frame_animator.update(AnimatorState::Entering);

    // Main frame
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    Frame::corners()
        .line_color(Hsla::new(0.0, 0.0, 1.0, 0.75))
        .bg_color(Hsla::new(0.0, 0.0, 0.0, 0.4))
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.16))
        .border_color(Hsla::new(0.0, 0.0, 1.0, 0.1))
        .stroke_width(1.0)
        .corner_length(26.0)
        .animation_progress(frame_progress)
        .paint(card_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let inner_padding = 26.0;
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0;
    let mut y = card_y + inner_padding;

    // ========================================================================
    // Header
    // ========================================================================
    draw_header(scene, text_system, state, inner_x, y, inner_width);
    y += 50.0;

    // ========================================================================
    // Input Section
    // ========================================================================
    let input_height = draw_input_section(scene, text_system, state, inner_x, y, inner_width, scale_factor);
    y += input_height + 20.0;

    // ========================================================================
    // Timeline / Progress Bar
    // ========================================================================
    draw_timeline(scene, text_system, state, inner_x, y, inner_width);
    y += 50.0;

    // ========================================================================
    // Main Content Area (two-column layout)
    // ========================================================================
    let remaining_height = card_y + card_height - y - inner_padding;
    draw_main_content(scene, text_system, state, inner_x, y, inner_width, remaining_height);
}

fn draw_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
) {
    // Title
    let title = "RLM EXECUTION VISUALIZER";
    let title_run = text_system.layout(title, Point::new(x, y), 18.0, accent_cyan());
    scene.draw_text(title_run);

    // Status badge
    let (status_text, status_color) = match state.rlm.connection_status {
        RlmConnectionStatus::Idle => ("READY", phase_pending()),
        RlmConnectionStatus::Connecting => ("CONNECTING", accent_orange()),
        RlmConnectionStatus::Streaming => ("STREAMING", accent_green()),
        RlmConnectionStatus::Complete => ("COMPLETE", accent_green()),
        RlmConnectionStatus::Error => ("ERROR", accent_red()),
    };

    let badge_width = text_system.measure(status_text, 10.0) + 12.0;
    let badge_x = x + width - badge_width;
    scene.draw_quad(
        Quad::new(Bounds::new(badge_x, y, badge_width, 18.0))
            .with_background(status_color.with_alpha(0.2))
            .with_border(status_color, 1.0),
    );
    let status_run = text_system.layout(status_text, Point::new(badge_x + 6.0, y + 3.0), 10.0, status_color);
    scene.draw_text(status_run);

    // Subtitle
    let subtitle = "DSPy-Powered Document Analysis: Route -> Extract -> Reduce -> Verify";
    let subtitle_run = text_system.layout(subtitle, Point::new(x, y + 24.0), 11.0, theme::text::MUTED);
    scene.draw_text(subtitle_run);
}

fn draw_input_section(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
    scale_factor: f32,
) -> f32 {
    let query_height = 32.0;
    let context_height = 60.0;
    let button_width = 80.0;
    let gap = 12.0;

    // Query input label
    let query_label = "Query:";
    let label_run = text_system.layout(query_label, Point::new(x, y), 11.0, theme::text::SECONDARY);
    scene.draw_text(label_run);

    // Query input field
    let query_input_x = x + 50.0;
    let query_input_width = width - 50.0 - button_width - gap;
    state.rlm.query_input_bounds = Bounds::new(query_input_x, y - 2.0, query_input_width, query_height);

    // Query input background
    scene.draw_quad(
        Quad::new(state.rlm.query_input_bounds)
            .with_background(panel_bg())
            .with_border(if state.rlm.query_input.is_focused() { accent_cyan() } else { panel_border() }, 1.0),
    );

    // Render query input
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.rlm.query_input.paint(state.rlm.query_input_bounds, &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Run button
    let button_x = x + width - button_width;
    state.rlm.run_button_bounds = Bounds::new(button_x, y - 2.0, button_width, query_height);
    let button_bg = if state.rlm.run_button_hovered { accent_cyan().with_alpha(0.3) } else { accent_cyan().with_alpha(0.15) };
    scene.draw_quad(
        Quad::new(state.rlm.run_button_bounds)
            .with_background(button_bg)
            .with_border(accent_cyan(), 1.0),
    );
    let run_text = if state.rlm.connection_status == RlmConnectionStatus::Streaming { "STOP" } else { "RUN" };
    let run_width = text_system.measure(run_text, 12.0);
    let run_x = button_x + (button_width - run_width) / 2.0;
    let run_run = text_system.layout(run_text, Point::new(run_x, y + 7.0), 12.0, accent_cyan());
    scene.draw_text(run_run);

    let current_y = y + query_height + gap;

    // Context input label
    let context_label = "Document:";
    let label_run = text_system.layout(context_label, Point::new(x, current_y), 11.0, theme::text::SECONDARY);
    scene.draw_text(label_run);

    // Context input field
    let context_input_x = x + 70.0;
    let context_input_width = width - 70.0;
    state.rlm.context_input_bounds = Bounds::new(context_input_x, current_y - 2.0, context_input_width, context_height);

    // Context input background
    scene.draw_quad(
        Quad::new(state.rlm.context_input_bounds)
            .with_background(panel_bg())
            .with_border(if state.rlm.context_input.is_focused() { accent_cyan() } else { panel_border() }, 1.0),
    );

    // Render context input
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.rlm.context_input.paint(state.rlm.context_input_bounds, &mut cx);

    query_height + gap + context_height
}

fn draw_timeline(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
) {
    // Phase labels and progress (DSPy terminology)
    let phases = [
        ("Route", RlmPhase::StructureDiscovery),
        ("Chunk", RlmPhase::Chunking),
        ("Extract", RlmPhase::Extraction),
        ("Reduce", RlmPhase::Synthesis),
    ];

    let phase_width = width / phases.len() as f32;
    let bar_height = 6.0;
    let bar_y = y + 20.0;

    // Track background
    scene.draw_quad(
        Quad::new(Bounds::new(x, bar_y, width, bar_height))
            .with_background(Hsla::new(0.0, 0.0, 0.15, 1.0)),
    );

    for (i, (label, phase)) in phases.iter().enumerate() {
        let phase_x = x + phase_width * i as f32;

        // Determine phase status
        let (color, filled) = if state.rlm.current_phase == *phase {
            (phase_processing(), true)
        } else if is_phase_complete(state.rlm.current_phase, *phase) {
            (phase_complete(), true)
        } else {
            (phase_pending(), false)
        };

        // Phase segment
        if filled {
            scene.draw_quad(
                Quad::new(Bounds::new(phase_x, bar_y, phase_width - 2.0, bar_height))
                    .with_background(color),
            );
        }

        // Phase label
        let label_width = text_system.measure(label, 10.0);
        let label_x = phase_x + (phase_width - label_width) / 2.0;
        let label_run = text_system.layout(label, Point::new(label_x, y), 10.0, color);
        scene.draw_text(label_run);

        // Phase indicator dot
        let dot_x = phase_x + phase_width / 2.0 - 4.0;
        let dot_y = bar_y + bar_height + 6.0;
        scene.draw_quad(
            Quad::new(Bounds::new(dot_x, dot_y, 8.0, 8.0))
                .with_background(color)
                .with_corner_radius(4.0),
        );
    }

    // Current phase label
    let current_label = state.rlm.phase_label();
    let current_run = text_system.layout(
        &format!("Phase: {}", current_label),
        Point::new(x, bar_y + bar_height + 20.0),
        11.0,
        theme::text::SECONDARY,
    );
    scene.draw_text(current_run);

    // Progress info
    if state.rlm.total_chunks > 0 {
        let progress = format!("Chunks: {}/{}", state.rlm.processed_chunks, state.rlm.total_chunks);
        let progress_width = text_system.measure(&progress, 11.0);
        let progress_run = text_system.layout(
            &progress,
            Point::new(x + width - progress_width, bar_y + bar_height + 20.0),
            11.0,
            accent_cyan(),
        );
        scene.draw_text(progress_run);
    }

    // Store timeline bounds for interaction
    state.rlm.timeline_slider_bounds = Bounds::new(x, bar_y, width, bar_height);
}

fn draw_main_content(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let left_width = width * 0.35;
    let right_width = width * 0.65 - 16.0;
    let gap = 16.0;

    // Left panel: Phase & Chunk Overview
    draw_phases_panel(scene, text_system, state, x, y, left_width, height);

    // Right panel: Detail View
    draw_detail_panel(scene, text_system, state, x + left_width + gap, y, right_width, height);
}

fn draw_phases_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(panel_bg())
            .with_border(panel_border(), 1.0),
    );

    let padding = 12.0;
    let mut current_y = y + padding;

    // Panel title
    let title = "EXECUTION PHASES";
    let title_run = text_system.layout(title, Point::new(x + padding, current_y), 11.0, accent_cyan());
    scene.draw_text(title_run);
    current_y += 24.0;

    // Phase list (DSPy terminology)
    let phases = [
        ("Routing", RlmPhase::StructureDiscovery, "Selecting relevant sections"),
        ("Chunking", RlmPhase::Chunking, "Splitting into semantic chunks"),
        ("Extraction (CoT)", RlmPhase::Extraction, "Chain-of-thought per chunk"),
        ("Reduce + Verify", RlmPhase::Synthesis, "Combining and validating"),
    ];

    for (label, phase, desc) in phases {
        let (icon, color) = get_phase_status_icon(state.rlm.current_phase, phase);

        // Phase row
        let icon_run = text_system.layout(icon, Point::new(x + padding, current_y), 11.0, color);
        scene.draw_text(icon_run);

        let label_run = text_system.layout(label, Point::new(x + padding + 18.0, current_y), 11.0, color);
        scene.draw_text(label_run);
        current_y += 16.0;

        // Description (when active)
        if state.rlm.current_phase == phase {
            let desc_run = text_system.layout(desc, Point::new(x + padding + 18.0, current_y), 9.0, theme::text::MUTED);
            scene.draw_text(desc_run);
            current_y += 14.0;

            // Show chunk progress for extraction phase
            if phase == RlmPhase::Extraction && state.rlm.total_chunks > 0 {
                draw_chunk_grid(scene, state, x + padding, current_y, width - padding * 2.0, 60.0);
                current_y += 70.0;
            }
        }

        current_y += 8.0;
    }

    // Error display
    if let Some(error) = &state.rlm.error {
        current_y += 8.0;
        scene.draw_quad(
            Quad::new(Bounds::new(x + padding, current_y, width - padding * 2.0, 40.0))
                .with_background(accent_red().with_alpha(0.1))
                .with_border(accent_red(), 1.0),
        );
        let error_run = text_system.layout(error, Point::new(x + padding + 8.0, current_y + 12.0), 10.0, accent_red());
        scene.draw_text(error_run);
    }
}

fn draw_chunk_grid(
    scene: &mut Scene,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    _height: f32,
) {
    let chunk_size = 10.0;
    let gap = 3.0;
    let cols = ((width - gap) / (chunk_size + gap)) as usize;
    let total = state.rlm.total_chunks.max(1);

    for (i, chunk) in state.rlm.chunks.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let chunk_x = x + (col as f32) * (chunk_size + gap);
        let chunk_y = y + (row as f32) * (chunk_size + gap);

        let color = match chunk.status {
            RlmStepStatus::Pending => phase_pending(),
            RlmStepStatus::Processing => phase_processing(),
            RlmStepStatus::Complete => phase_complete(),
            RlmStepStatus::Error => phase_error(),
        };

        scene.draw_quad(
            Quad::new(Bounds::new(chunk_x, chunk_y, chunk_size, chunk_size))
                .with_background(color)
                .with_corner_radius(2.0),
        );
    }

    // Fill remaining slots with pending
    for i in state.rlm.chunks.len()..total {
        let col = i % cols;
        let row = i / cols;
        let chunk_x = x + (col as f32) * (chunk_size + gap);
        let chunk_y = y + (row as f32) * (chunk_size + gap);

        scene.draw_quad(
            Quad::new(Bounds::new(chunk_x, chunk_y, chunk_size, chunk_size))
                .with_background(phase_pending())
                .with_corner_radius(2.0),
        );
    }
}

fn draw_detail_panel(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(panel_bg())
            .with_border(panel_border(), 1.0),
    );

    let padding = 12.0;
    let mut current_y = y + padding;

    // Panel title
    let title = if state.rlm.current_phase == RlmPhase::Extraction {
        if let Some(chunk_id) = state.rlm.active_chunk_id {
            format!("CHUNK {} DETAIL", chunk_id + 1)
        } else {
            "DETAIL VIEW".to_string()
        }
    } else {
        "DETAIL VIEW".to_string()
    };
    let title_run = text_system.layout(&title, Point::new(x + padding, current_y), 11.0, accent_cyan());
    scene.draw_text(title_run);
    current_y += 28.0;

    // Active chunk info
    if let Some(chunk_id) = state.rlm.active_chunk_id {
        if let Some(chunk) = state.rlm.chunks.get(chunk_id) {
            if let Some(title) = &chunk.section_title {
                let section_label = format!("Section: {}", title);
                let section_run = text_system.layout(&section_label, Point::new(x + padding, current_y), 11.0, theme::text::SECONDARY);
                scene.draw_text(section_run);
                current_y += 20.0;
            }

            if let Some(preview) = &chunk.content_preview {
                let preview_truncated: String = preview.chars().take(200).collect();
                let preview_run = text_system.layout(&preview_truncated, Point::new(x + padding, current_y), 10.0, theme::text::MUTED);
                scene.draw_text(preview_run);
                current_y += 40.0;
            }

            if let Some(findings) = &chunk.findings {
                current_y += 8.0;
                let findings_label = "Findings:";
                let findings_label_run = text_system.layout(findings_label, Point::new(x + padding, current_y), 10.0, accent_green());
                scene.draw_text(findings_label_run);
                current_y += 16.0;

                let findings_truncated: String = findings.chars().take(500).collect();
                let findings_run = text_system.layout(&findings_truncated, Point::new(x + padding, current_y), 10.0, theme::text::PRIMARY);
                scene.draw_text(findings_run);
                current_y += 60.0;
            }
        }
    }

    // Streaming text
    if !state.rlm.streaming_text.is_empty() {
        current_y += 8.0;
        let stream_label = "LLM Response:";
        let stream_label_run = text_system.layout(stream_label, Point::new(x + padding, current_y), 10.0, accent_orange());
        scene.draw_text(stream_label_run);
        current_y += 16.0;

        // Streaming text box
        let box_height = height - (current_y - y) - padding * 2.0;
        scene.draw_quad(
            Quad::new(Bounds::new(x + padding, current_y, width - padding * 2.0, box_height.max(60.0)))
                .with_background(Hsla::new(0.0, 0.0, 0.0, 0.3))
                .with_border(panel_border(), 1.0),
        );

        let text_truncated: String = state.rlm.streaming_text.chars().take(1000).collect();
        let text_run = text_system.layout(&text_truncated, Point::new(x + padding + 8.0, current_y + 8.0), 10.0, theme::text::PRIMARY);
        scene.draw_text(text_run);
    }

    // Final answer
    if let Some(answer) = &state.rlm.final_answer {
        current_y += 8.0;

        // Final answer header
        let answer_label = "FINAL ANSWER";
        let answer_label_run = text_system.layout(answer_label, Point::new(x + padding, current_y), 11.0, accent_green());
        scene.draw_text(answer_label_run);
        current_y += 20.0;

        // Answer box
        let box_height = height - (current_y - y) - padding;
        scene.draw_quad(
            Quad::new(Bounds::new(x + padding, current_y, width - padding * 2.0, box_height.max(80.0)))
                .with_background(accent_green().with_alpha(0.05))
                .with_border(accent_green().with_alpha(0.3), 1.0),
        );

        let answer_truncated: String = answer.chars().take(2000).collect();
        let answer_run = text_system.layout(&answer_truncated, Point::new(x + padding + 8.0, current_y + 8.0), 11.0, theme::text::PRIMARY);
        scene.draw_text(answer_run);
    }
}

fn get_phase_status_icon(current: RlmPhase, phase: RlmPhase) -> (&'static str, Hsla) {
    if current == phase {
        (">", phase_processing())
    } else if is_phase_complete(current, phase) {
        ("=", phase_complete())
    } else {
        (" ", phase_pending())
    }
}

fn is_phase_complete(current: RlmPhase, check: RlmPhase) -> bool {
    let order = |p: RlmPhase| -> u8 {
        match p {
            RlmPhase::Idle => 0,
            RlmPhase::StructureDiscovery => 1,
            RlmPhase::Chunking => 2,
            RlmPhase::Extraction => 3,
            RlmPhase::Synthesis => 4,
            RlmPhase::Complete => 5,
        }
    };
    order(current) > order(check)
}

/// Load the embedded demo trace
fn load_demo_trace() -> Option<RlmDemoTrace> {
    serde_json::from_str(DEMO_TRACE_JSON).ok()
}

/// Initialize trace playback if not already loaded
fn ensure_trace_loaded(state: &mut AppState) {
    if state.rlm.trace.is_none() {
        state.rlm.trace = load_demo_trace();
        // Pre-populate query and document from trace
        if let Some(trace) = &state.rlm.trace {
            state.rlm.query_input.set_value(&trace.query);
            state.rlm.context_input.set_value(&trace.document_preview);
        }
    }
}

/// Tick the demo using trace playback
fn tick_demo(state: &mut AppState) {
    let now = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() as u64)
        .unwrap_or(0);

    // Ensure trace is loaded
    ensure_trace_loaded(state);

    // Initialize playback
    if state.rlm.trace_start_time == 0 {
        state.rlm.trace_start_time = now;
        state.rlm.trace_event_idx = 0;
        state.rlm.connection_status = RlmConnectionStatus::Streaming;
        return;
    }

    let Some(trace) = state.rlm.trace.as_ref() else {
        return;
    };

    let elapsed = now.saturating_sub(state.rlm.trace_start_time);
    let trace_len = trace.events.len();
    let last_event_time = trace.events.last().map(|e| e.t).unwrap_or(0);

    // Collect events to apply (clone to avoid borrow conflict)
    let mut events_to_apply = Vec::new();
    while state.rlm.trace_event_idx < trace_len {
        let event = &trace.events[state.rlm.trace_event_idx];
        if event.t > elapsed {
            break;
        }
        events_to_apply.push(event.clone());
        state.rlm.trace_event_idx += 1;
    }

    // Apply collected events
    for event in events_to_apply {
        apply_trace_event(state, &event);
    }

    // Auto-restart after completion + 5 second delay
    if state.rlm.current_phase == RlmPhase::Complete && state.rlm.auto_restart {
        if elapsed > last_event_time + 5000 {
            reset_and_restart_trace(state);
        }
    }
}

/// Apply a single trace event to update state
fn apply_trace_event(state: &mut AppState, event: &crate::state::RlmTraceEvent) {
    match &event.event {
        RlmTraceEventType::PhaseStart { phase } => {
            state.rlm.current_phase = match phase.as_str() {
                "routing" => RlmPhase::StructureDiscovery,
                "chunking" => RlmPhase::Chunking,
                "extraction" => RlmPhase::Extraction,
                "synthesis" => RlmPhase::Synthesis,
                _ => state.rlm.current_phase,
            };
            state.rlm.connection_status = RlmConnectionStatus::Streaming;
        }
        RlmTraceEventType::RoutingResult { sections } => {
            state.rlm.streaming_text = format!("Identified sections: {}", sections);
        }
        RlmTraceEventType::ChunkCreated {
            id,
            section,
            preview,
        } => {
            state.rlm.chunks.push(RlmChunkState {
                chunk_id: *id,
                section_title: Some(section.clone()),
                content_preview: Some(preview.clone()),
                findings: None,
                status: RlmStepStatus::Pending,
            });
            state.rlm.total_chunks = state.rlm.chunks.len();
        }
        RlmTraceEventType::ExtractionStart { chunk_id } => {
            if let Some(chunk) = state.rlm.chunks.get_mut(*chunk_id) {
                chunk.status = RlmStepStatus::Processing;
            }
            state.rlm.active_chunk_id = Some(*chunk_id);
            if let Some(chunk) = state.rlm.chunks.get(*chunk_id) {
                state.rlm.streaming_text = format!(
                    "Extracting from: {}",
                    chunk.section_title.as_deref().unwrap_or("Unknown")
                );
            }
        }
        RlmTraceEventType::ExtractionComplete {
            chunk_id,
            findings,
            relevance: _,
        } => {
            if let Some(chunk) = state.rlm.chunks.get_mut(*chunk_id) {
                chunk.findings = Some(findings.clone());
                chunk.status = RlmStepStatus::Complete;
            }
            state.rlm.processed_chunks += 1;
            state.rlm.streaming_text = format!("Extracted: {}...", &findings.chars().take(80).collect::<String>());
        }
        RlmTraceEventType::SynthesisComplete {
            answer,
            confidence: _,
        } => {
            state.rlm.final_answer = Some(answer.clone());
            state.rlm.streaming_text.clear();
        }
        RlmTraceEventType::Complete => {
            state.rlm.current_phase = RlmPhase::Complete;
            state.rlm.connection_status = RlmConnectionStatus::Complete;
        }
    }
}

/// Reset state and restart trace playback
fn reset_and_restart_trace(state: &mut AppState) {
    // Preserve trace data
    let trace = state.rlm.trace.take();
    let auto_restart = state.rlm.auto_restart;

    // Reset execution state
    state.rlm.reset_execution();

    // Restore trace and restart
    state.rlm.trace = trace;
    state.rlm.auto_restart = auto_restart;
    state.rlm.demo_mode = true;
    state.rlm.trace_start_time = 0;
    state.rlm.trace_event_idx = 0;

    // Re-populate inputs from trace
    if let Some(trace) = &state.rlm.trace {
        state.rlm.query_input.set_value(&trace.query);
        state.rlm.context_input.set_value(&trace.document_preview);
    }
}

/// Handle mouse move for hover detection
pub(crate) fn handle_rlm_mouse_move(state: &mut AppState, x: f32, y: f32) {
    state.rlm.run_button_hovered = state.rlm.run_button_bounds.contains(Point::new(x, y));
}

/// Handle mouse click
pub(crate) fn handle_rlm_click(state: &mut AppState, x: f32, y: f32) -> bool {
    let point = Point::new(x, y);

    // Run button click
    if state.rlm.run_button_bounds.contains(point) {
        if state.rlm.connection_status == RlmConnectionStatus::Streaming {
            // Stop
            state.rlm.connection_status = RlmConnectionStatus::Idle;
            state.rlm.demo_mode = false;
            state.rlm.auto_restart = false;
        } else {
            // Start trace playback demo
            reset_and_restart_trace(state);
        }
        return true;
    }

    false
}
