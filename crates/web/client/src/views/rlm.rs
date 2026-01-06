//! RLM (Recursive Language Model) visualization page
//!
//! Interactive "execution movie" showing RLM processing documents through
//! structure discovery, chunking, extraction, and synthesis phases.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::markdown::{MarkdownParser, MarkdownRenderer};
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::{
    AppState, RlmChunkState, RlmConnectionStatus, RlmDemoTrace, RlmPhase, RlmStepStatus,
    RlmTraceEventType,
};

/// Embedded trace JSON for demo playback
const DEMO_TRACE_JSON: &str = include_str!("../../assets/rlm-demo-trace.json");

// ============================================================================
// V2 Color Palette (from spec)
// ============================================================================

// Background colors
fn bg_dark() -> Hsla {
    Hsla::from_hex(0x08090a)
}

fn bg_panel() -> Hsla {
    Hsla::from_hex(0x0d0f11)
}

fn border_color() -> Hsla {
    Hsla::from_hex(0x1d2328)
}

// Text colors
fn text_primary() -> Hsla {
    Hsla::from_hex(0xf7f8f8)
}

fn text_muted() -> Hsla {
    Hsla::from_hex(0x9aa4ad)
}

// State colors
fn state_pending() -> Hsla {
    Hsla::from_hex(0x3a424a)
}

fn state_active() -> Hsla {
    Hsla::from_hex(0xe6b450)
}

fn state_complete() -> Hsla {
    Hsla::from_hex(0x23d18b)
}

fn state_error() -> Hsla {
    Hsla::from_hex(0xf44747)
}

// ============================================================================
// V2 Typography (from spec)
// ============================================================================

const FONT_TITLE: f32 = 20.0;
const FONT_HEADER: f32 = 14.0;
const FONT_BODY: f32 = 13.0;
const FONT_TABLE: f32 = 13.0;
const FONT_SMALL: f32 = 12.0;

// ============================================================================
// Legacy color helpers (for compatibility during transition)
// ============================================================================

#[allow(dead_code)]
fn accent_cyan() -> Hsla {
    Hsla::from_hex(0x7fd3e5)
}

#[allow(dead_code)]
fn accent_green() -> Hsla {
    state_complete()
}

#[allow(dead_code)]
fn accent_orange() -> Hsla {
    state_active()
}

#[allow(dead_code)]
fn accent_red() -> Hsla {
    state_error()
}

#[allow(dead_code)]
fn phase_pending() -> Hsla {
    state_pending()
}

#[allow(dead_code)]
fn phase_processing() -> Hsla {
    state_active()
}

#[allow(dead_code)]
fn phase_complete() -> Hsla {
    state_complete()
}

#[allow(dead_code)]
fn phase_error() -> Hsla {
    state_error()
}

#[allow(dead_code)]
fn panel_bg() -> Hsla {
    bg_panel()
}

#[allow(dead_code)]
fn panel_border() -> Hsla {
    border_color()
}

// ============================================================================
// V2 Text Wrapping Helper
// ============================================================================

fn wrap_text(text_system: &mut TextSystem, text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in words {
            let test = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };
            let width = text_system.measure(&test, font_size);
            if width > max_width && !current_line.is_empty() {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                current_line = test;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

// ============================================================================
// V2 Main Entry Point
// ============================================================================

/// Build the RLM visualization page (V2 Layout)
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
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(bg_dark()));

    // Dots grid background (subtle)
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.08))
        .distance(40.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Layout
    let padding = 20.0;
    let content_width = (width - padding * 2.0).min(1400.0);
    let content_x = (width - content_width) / 2.0;
    let content_y = padding;
    let content_height = height - padding * 2.0;

    // Frame animation
    if !state.rlm.frame_started {
        state.rlm.frame_started = true;
    }
    let frame_progress = state.rlm.frame_animator.update(AnimatorState::Entering);

    // Main frame
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    Frame::corners()
        .line_color(border_color())
        .bg_color(bg_panel())
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.05))
        .border_color(border_color())
        .stroke_width(1.0)
        .corner_length(20.0)
        .animation_progress(frame_progress)
        .paint(Bounds::new(content_x, content_y, content_width, content_height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let inner_padding = 16.0;
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0;
    let mut y = content_y + inner_padding;

    // ========================================================================
    // TOP BAR (60px)
    // ========================================================================
    let top_bar_height = render_top_bar(scene, text_system, state, inner_x, y, inner_width);
    y += top_bar_height + 16.0;

    // ========================================================================
    // MAIN CONTENT: Two-column layout
    // ========================================================================
    let remaining_height = content_y + content_height - y - inner_padding;
    let left_width = inner_width * 0.45;
    let right_width = inner_width * 0.55 - 16.0;
    let gap = 16.0;

    // LEFT COLUMN: Pipeline + Workset + Chunk Grid
    render_left_column(scene, text_system, state, inner_x, y, left_width, remaining_height);

    // RIGHT COLUMN: Inspector
    render_inspector(scene, text_system, state, inner_x + left_width + gap, y, right_width, remaining_height);
}

// ============================================================================
// V2 Top Bar
// ============================================================================

fn render_top_bar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let bar_height = 50.0;

    // Title
    let title = "RLM VISUALIZER";
    let title_run = text_system.layout(title, Point::new(x, y + 8.0), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    // Scenario info (query preview)
    let query_preview = if let Some(trace) = &state.rlm.trace {
        let truncated: String = trace.query.chars().take(60).collect();
        if trace.query.len() > 60 {
            format!("{}...", truncated)
        } else {
            truncated
        }
    } else {
        "No scenario loaded".to_string()
    };
    let scenario_x = x + 180.0;
    let scenario_run = text_system.layout(&query_preview, Point::new(scenario_x, y + 12.0), FONT_BODY, text_muted());
    scene.draw_text(scenario_run);

    // Controls (right side)
    let controls_right = x + width;
    let button_height = 28.0;
    let button_y = y + 6.0;

    // Speed button
    let speed_text = format!("SPEED: {:.1}x", state.rlm.playback_speed);
    let speed_width = text_system.measure(&speed_text, FONT_SMALL) + 16.0;
    let speed_x = controls_right - speed_width;
    state.rlm.speed_button_bounds = Bounds::new(speed_x, button_y, speed_width, button_height);
    let speed_bg = if state.rlm.speed_button_hovered {
        border_color()
    } else {
        bg_panel()
    };
    scene.draw_quad(
        Quad::new(state.rlm.speed_button_bounds)
            .with_background(speed_bg)
            .with_border(border_color(), 1.0),
    );
    let speed_run = text_system.layout(&speed_text, Point::new(speed_x + 8.0, button_y + 7.0), FONT_SMALL, text_muted());
    scene.draw_text(speed_run);

    // Restart button
    let restart_text = "RESTART";
    let restart_width = text_system.measure(restart_text, FONT_SMALL) + 16.0;
    let restart_x = speed_x - restart_width - 8.0;
    state.rlm.restart_button_bounds = Bounds::new(restart_x, button_y, restart_width, button_height);
    let restart_bg = if state.rlm.restart_button_hovered {
        border_color()
    } else {
        bg_panel()
    };
    scene.draw_quad(
        Quad::new(state.rlm.restart_button_bounds)
            .with_background(restart_bg)
            .with_border(border_color(), 1.0),
    );
    let restart_run = text_system.layout(restart_text, Point::new(restart_x + 8.0, button_y + 7.0), FONT_SMALL, text_muted());
    scene.draw_text(restart_run);

    // Run/Pause/Replay button - context-aware label
    let run_text = match state.rlm.connection_status {
        RlmConnectionStatus::Streaming => "PAUSE",
        RlmConnectionStatus::Complete => "REPLAY",
        _ => "RUN",
    };
    let run_width = text_system.measure(run_text, FONT_SMALL) + 24.0;
    let run_x = restart_x - run_width - 8.0;
    state.rlm.run_button_bounds = Bounds::new(run_x, button_y, run_width, button_height);
    let (run_bg, run_border, run_text_color) = if state.rlm.run_button_hovered {
        (state_active().with_alpha(0.4), state_active(), text_primary())
    } else if state.rlm.connection_status == RlmConnectionStatus::Streaming {
        (state_active().with_alpha(0.25), state_active(), state_active())
    } else {
        (state_complete().with_alpha(0.2), state_complete(), state_complete())
    };
    scene.draw_quad(
        Quad::new(state.rlm.run_button_bounds)
            .with_background(run_bg)
            .with_border(run_border, 1.0),
    );
    let run_run = text_system.layout(run_text, Point::new(run_x + 12.0, button_y + 7.0), FONT_SMALL, run_text_color);
    scene.draw_text(run_run);

    // Status badge
    let (status_text, status_color) = match state.rlm.connection_status {
        RlmConnectionStatus::Idle => ("READY", state_pending()),
        RlmConnectionStatus::Connecting => ("CONNECTING", state_active()),
        RlmConnectionStatus::Streaming => ("STREAMING", state_active()),
        RlmConnectionStatus::Complete => ("COMPLETE", state_complete()),
        RlmConnectionStatus::Error => ("ERROR", state_error()),
    };
    let badge_width = text_system.measure(status_text, FONT_SMALL) + 16.0;
    let badge_x = run_x - badge_width - 16.0;
    scene.draw_quad(
        Quad::new(Bounds::new(badge_x, button_y, badge_width, button_height))
            .with_background(status_color.with_alpha(0.15))
            .with_border(status_color, 1.0),
    );
    let status_run = text_system.layout(status_text, Point::new(badge_x + 8.0, button_y + 7.0), FONT_SMALL, status_color);
    scene.draw_text(status_run);

    bar_height
}

// ============================================================================
// V2 Left Column: Pipeline + Workset + Chunk Grid
// ============================================================================

fn render_left_column(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let mut current_y = y;

    // PIPELINE BOXES (with inline mini-map in EXTRACT)
    let pipeline_height = render_pipeline_boxes(scene, text_system, state, x, current_y, width);
    current_y += pipeline_height + 8.0;

    // NOW: line - shows current activity
    let now_height = render_now_line(scene, text_system, state, x, current_y, width);
    current_y += now_height + 8.0;

    // Separator
    scene.draw_quad(
        Quad::new(Bounds::new(x, current_y, width, 1.0))
            .with_background(border_color()),
    );
    current_y += 8.0;

    // WORKSET TABLE - fills remaining space (no chunk grid)
    let table_height = height - (current_y - y) - 8.0;
    render_workset_table(scene, text_system, state, x, current_y, width, table_height);
}

fn render_pipeline_boxes(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let box_height = 60.0;
    let gap = 8.0;
    // Check if routing is complete (we're at or past chunking)
    let routing_complete = is_phase_complete_v2(state.rlm.current_phase, RlmPhase::StructureDiscovery);

    let phases = [
        ("ROUTE", RlmPhase::StructureDiscovery, format!("sections: {}", if routing_complete { "8" } else { "--" })),
        ("CHUNK", RlmPhase::Chunking, format!("chunks: {}", state.rlm.total_chunks)),
        ("EXTRACT", RlmPhase::Extraction, format!("{}/{}", state.rlm.processed_chunks, state.rlm.total_chunks)),
        ("VERIFY", RlmPhase::Synthesis, if state.rlm.current_phase == RlmPhase::Complete { "PASS".to_string() } else { "--".to_string() }),
    ];

    let box_width = (width - gap * 3.0) / 4.0;

    for (i, (label, phase, metric)) in phases.iter().enumerate() {
        let box_x = x + (box_width + gap) * i as f32;

        // Determine state - unified visual rules:
        // RUN: 2px border + filled background
        // OK: 1px border, no fill
        // PEND: muted border/text
        let is_active = state.rlm.current_phase == *phase;
        let is_complete = is_phase_complete_v2(state.rlm.current_phase, *phase);

        let (bg_color, border_color_val, border_width, text_color) = if is_active {
            // RUN state - filled background, prominent
            (state_active().with_alpha(0.2), state_active(), 2.0, state_active())
        } else if is_complete {
            // OK state - just border, no fill
            (bg_panel(), state_complete(), 1.0, state_complete())
        } else {
            // PEND state - muted
            (bg_panel(), state_pending(), 1.0, text_muted())
        };

        // Box background
        scene.draw_quad(
            Quad::new(Bounds::new(box_x, y, box_width, box_height))
                .with_background(bg_color)
                .with_border(border_color_val, border_width),
        );

        // Phase name
        let label_run = text_system.layout(label, Point::new(box_x + 8.0, y + 10.0), FONT_HEADER, text_color);
        scene.draw_text(label_run);

        // For EXTRACT phase, show inline mini-map instead of just numbers
        if *phase == RlmPhase::Extraction && state.rlm.total_chunks > 0 {
            // Mini-map: small squares showing chunk status
            let mini_size = 6.0;
            let mini_gap = 2.0;
            let mini_y = y + 32.0;
            let max_visible = 8.min(state.rlm.total_chunks);

            for j in 0..max_visible {
                let mini_x = box_x + 8.0 + (mini_size + mini_gap) * j as f32;
                let chunk_color = if let Some(chunk) = state.rlm.chunks.get(j) {
                    match chunk.status {
                        RlmStepStatus::Pending => state_pending(),
                        RlmStepStatus::Processing => state_active(),
                        RlmStepStatus::Complete => state_complete(),
                        RlmStepStatus::Error => state_error(),
                    }
                } else {
                    state_pending()
                };

                // Fill for processing, border-only for others
                let is_processing = state.rlm.chunks.get(j).map(|c| c.status == RlmStepStatus::Processing).unwrap_or(false);
                if is_processing {
                    scene.draw_quad(
                        Quad::new(Bounds::new(mini_x, mini_y, mini_size, mini_size))
                            .with_background(chunk_color.with_alpha(0.5))
                            .with_border(chunk_color, 1.0),
                    );
                } else {
                    scene.draw_quad(
                        Quad::new(Bounds::new(mini_x, mini_y, mini_size, mini_size))
                            .with_border(chunk_color, 1.0),
                    );
                }
            }

            // Show count after mini-map
            let count_x = box_x + 8.0 + (mini_size + mini_gap) * max_visible as f32 + 4.0;
            let count_text = format!("{}/{}", state.rlm.processed_chunks, state.rlm.total_chunks);
            let count_run = text_system.layout(&count_text, Point::new(count_x, mini_y - 2.0), FONT_SMALL, text_muted());
            scene.draw_text(count_run);
        } else {
            // Metric for other phases
            let metric_run = text_system.layout(metric, Point::new(box_x + 8.0, y + 34.0), FONT_SMALL, text_muted());
            scene.draw_text(metric_run);
        }
    }

    box_height
}

/// Render the NOW: line showing current activity
fn render_now_line(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let line_height = 20.0;

    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, line_height))
            .with_background(bg_dark()),
    );

    // Build the NOW: text based on current state
    let now_text = if state.rlm.connection_status == RlmConnectionStatus::Complete {
        "NOW: Complete".to_string()
    } else if let Some(chunk_id) = state.rlm.active_chunk_id {
        if let Some(chunk) = state.rlm.chunks.get(chunk_id) {
            let section = chunk.section_title.as_deref().unwrap_or("--");
            let section_short: String = section.chars().take(30).collect();
            format!("NOW: EXTRACT chunk {}/{} — {}", chunk_id + 1, state.rlm.total_chunks, section_short)
        } else {
            format!("NOW: EXTRACT chunk {}/{}", chunk_id + 1, state.rlm.total_chunks)
        }
    } else {
        match state.rlm.current_phase {
            RlmPhase::Idle => "NOW: Ready".to_string(),
            RlmPhase::StructureDiscovery => "NOW: ROUTE — discovering sections...".to_string(),
            RlmPhase::Chunking => "NOW: CHUNK — splitting into semantic chunks...".to_string(),
            RlmPhase::Extraction => "NOW: EXTRACT — waiting...".to_string(),
            RlmPhase::Synthesis => "NOW: VERIFY — synthesizing answer...".to_string(),
            RlmPhase::Complete => "NOW: Complete".to_string(),
        }
    };

    let now_run = text_system.layout(&now_text, Point::new(x + 8.0, y + 3.0), FONT_SMALL, state_active());
    scene.draw_text(now_run);

    line_height
}

fn render_workset_table(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Table header
    let header_height = 24.0;
    let col_id_width = 30.0;
    let col_section_width = width - col_id_width - 80.0;
    let col_status_width = 40.0;
    let _col_rel_width = 40.0;

    // Header background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, header_height))
            .with_background(bg_dark()),
    );

    // Header text
    let header_y = y + 5.0;
    let id_run = text_system.layout("ID", Point::new(x + 8.0, header_y), FONT_TABLE, text_muted());
    scene.draw_text(id_run);
    let section_run = text_system.layout("SECTION", Point::new(x + col_id_width + 8.0, header_y), FONT_TABLE, text_muted());
    scene.draw_text(section_run);
    let st_run = text_system.layout("ST", Point::new(x + col_id_width + col_section_width + 8.0, header_y), FONT_TABLE, text_muted());
    scene.draw_text(st_run);
    let rel_run = text_system.layout("REL", Point::new(x + col_id_width + col_section_width + col_status_width + 8.0, header_y), FONT_TABLE, text_muted());
    scene.draw_text(rel_run);

    // Clear and rebuild row bounds
    state.rlm.table_row_bounds.clear();

    // Table rows
    let row_height = 24.0;
    let mut row_y = y + header_height;
    let max_rows = ((height - header_height) / row_height) as usize;

    for (i, chunk) in state.rlm.chunks.iter().enumerate().take(max_rows) {
        let row_bounds = Bounds::new(x, row_y, width, row_height);
        state.rlm.table_row_bounds.push(row_bounds);

        // Row background
        let is_selected = state.rlm.selected_chunk_id == Some(i);
        let is_active = state.rlm.active_chunk_id == Some(i);
        let row_bg = if is_selected {
            state_active().with_alpha(0.15)
        } else if is_active {
            state_active().with_alpha(0.08)
        } else if i % 2 == 0 {
            bg_panel()
        } else {
            bg_dark()
        };
        scene.draw_quad(Quad::new(row_bounds).with_background(row_bg));

        // Active indicator
        if is_active {
            scene.draw_quad(
                Quad::new(Bounds::new(x, row_y, 3.0, row_height))
                    .with_background(state_active()),
            );
        }

        // Selection border
        if is_selected {
            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_border(state_active(), 1.0),
            );
        }

        let text_y = row_y + 5.0;

        // ID
        let id_text = format!("{}", chunk.chunk_id);
        let id_run = text_system.layout(&id_text, Point::new(x + 8.0, text_y), FONT_TABLE, text_primary());
        scene.draw_text(id_run);

        // Section (truncated)
        let section = chunk.section_title.as_deref().unwrap_or("--");
        let section_truncated: String = section.chars().take(25).collect();
        let section_display = if section.len() > 25 { format!("{}...", section_truncated) } else { section_truncated };
        let section_run = text_system.layout(&section_display, Point::new(x + col_id_width + 8.0, text_y), FONT_TABLE, text_primary());
        scene.draw_text(section_run);

        // Status - with unified visual encoding
        let (status_text, status_color) = match chunk.status {
            RlmStepStatus::Pending => ("PEND", state_pending()),
            RlmStepStatus::Processing => ("RUN", state_active()),
            RlmStepStatus::Complete => ("OK", state_complete()),
            RlmStepStatus::Error => ("ERR", state_error()),
        };

        // For RUN status, add background highlight to make it pop
        if chunk.status == RlmStepStatus::Processing {
            let status_bg_bounds = Bounds::new(
                x + col_id_width + col_section_width + 4.0,
                row_y + 2.0,
                col_status_width - 4.0,
                row_height - 4.0,
            );
            scene.draw_quad(
                Quad::new(status_bg_bounds)
                    .with_background(state_active().with_alpha(0.3)),
            );
        }

        let status_run = text_system.layout(status_text, Point::new(x + col_id_width + col_section_width + 8.0, text_y), FONT_TABLE, status_color);
        scene.draw_text(status_run);

        // Relevance - show as percentage (91% not 0.91)
        let rel_text = chunk.relevance.map(|r| format!("{}%", (r * 100.0).round() as i32)).unwrap_or_else(|| "--".to_string());
        let rel_run = text_system.layout(&rel_text, Point::new(x + col_id_width + col_section_width + col_status_width + 8.0, text_y), FONT_TABLE, text_muted());
        scene.draw_text(rel_run);

        row_y += row_height;
    }
}

// Chunk grid replaced by inline mini-map in EXTRACT box
#[allow(dead_code)]
fn render_chunk_grid_v2(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    _width: f32,
) {
    let cell_size = 28.0;
    let gap = 6.0;
    let total_chunks = state.rlm.total_chunks.max(8);

    // Clear and rebuild grid bounds
    state.rlm.chunk_grid_bounds.clear();

    for i in 0..total_chunks {
        let col = i % 8;
        let cell_x = x + (cell_size + gap) * col as f32;
        let cell_bounds = Bounds::new(cell_x, y, cell_size, cell_size);

        // Store bounds for click detection
        state.rlm.chunk_grid_bounds.push(cell_bounds);

        // Determine state
        let (border_color_val, fill) = if let Some(chunk) = state.rlm.chunks.get(i) {
            match chunk.status {
                RlmStepStatus::Pending => (state_pending(), false),
                RlmStepStatus::Processing => (state_active(), true),
                RlmStepStatus::Complete => (state_complete(), false),
                RlmStepStatus::Error => (state_error(), false),
            }
        } else {
            (state_pending(), false)
        };

        // Is selected?
        let is_selected = state.rlm.selected_chunk_id == Some(i);

        // Cell background
        let bg = if fill {
            border_color_val.with_alpha(0.3)
        } else if is_selected {
            state_active().with_alpha(0.2)
        } else {
            bg_panel()
        };

        scene.draw_quad(
            Quad::new(cell_bounds)
                .with_background(bg)
                .with_border(border_color_val, if is_selected { 2.0 } else { 1.0 }),
        );

        // Cell ID
        let id_text = format!("{}", i);
        let id_width = text_system.measure(&id_text, FONT_TABLE);
        let id_x = cell_x + (cell_size - id_width) / 2.0;
        let id_run = text_system.layout(&id_text, Point::new(id_x, y + 7.0), FONT_TABLE, text_primary());
        scene.draw_text(id_run);
    }
}

// ============================================================================
// V2 Inspector (Right Column)
// ============================================================================

fn render_inspector(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let padding = 12.0;
    let mut current_y = y;

    // Panel background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, width, height))
            .with_background(bg_panel())
            .with_border(border_color(), 1.0),
    );

    current_y += padding;

    // Title: Selected chunk info
    let title = if let Some(chunk_id) = state.rlm.selected_chunk_id {
        if let Some(chunk) = state.rlm.chunks.get(chunk_id) {
            let section = chunk.section_title.as_deref().unwrap_or("Unknown");
            format!("Selected: Chunk {} \"{}\"", chunk_id, section)
        } else {
            format!("Selected: Chunk {}", chunk_id)
        }
    } else {
        "INSPECTOR".to_string()
    };
    let title_run = text_system.layout(&title, Point::new(x + padding, current_y), FONT_HEADER, text_primary());
    scene.draw_text(title_run);
    current_y += 28.0;

    // Calculate pane heights - now with 4 sections when answer is available
    let available_height = height - (current_y - y) - padding;
    let pane_gap = 8.0;
    let has_answer = state.rlm.final_answer.is_some();
    let citations_height = 60.0; // Fixed small height for citations

    let (excerpt_height, findings_height, answer_height) = if has_answer {
        let remaining = available_height - citations_height - pane_gap * 3.0;
        let excerpt = remaining * 0.3;
        let findings = remaining * 0.3;
        let answer = remaining * 0.4;
        (excerpt, findings, answer)
    } else {
        let each = (available_height - pane_gap) / 2.0;
        (each, each, 0.0)
    };

    // EXCERPT PANE
    state.rlm.excerpt_bounds = Bounds::new(x + padding, current_y, width - padding * 2.0, excerpt_height);
    render_scrollable_pane(
        scene, text_system, state,
        "EXCERPT",
        state.rlm.selected_chunk_id
            .and_then(|id| state.rlm.chunks.get(id))
            .and_then(|c| c.content_preview.as_deref())
            .unwrap_or("Select a chunk to view its content"),
        state.rlm.excerpt_scroll,
        state.rlm.excerpt_bounds,
    );
    current_y += excerpt_height + pane_gap;

    // FINDINGS PANE
    state.rlm.findings_bounds = Bounds::new(x + padding, current_y, width - padding * 2.0, findings_height);
    render_scrollable_pane(
        scene, text_system, state,
        "FINDINGS",
        state.rlm.selected_chunk_id
            .and_then(|id| state.rlm.chunks.get(id))
            .and_then(|c| c.findings.as_deref())
            .unwrap_or("Extraction results will appear here"),
        state.rlm.findings_scroll,
        state.rlm.findings_bounds,
    );
    current_y += findings_height + pane_gap;

    // CITATIONS PANE (shows all chunks with findings as clickable references)
    if has_answer {
        let citations_bounds = Bounds::new(x + padding, current_y, width - padding * 2.0, citations_height);

        scene.draw_quad(
            Quad::new(citations_bounds)
                .with_background(bg_dark())
                .with_border(border_color(), 1.0),
        );

        let header_run = text_system.layout("CITATIONS", Point::new(citations_bounds.x() + 8.0, citations_bounds.y() + 4.0), FONT_SMALL, text_muted());
        scene.draw_text(header_run);

        // List chunks with findings as citations
        let mut citation_x = citations_bounds.x() + 8.0;
        let citation_y = citations_bounds.y() + 22.0;

        for chunk in state.rlm.chunks.iter() {
            if chunk.findings.is_some() {
                let section_short: String = chunk.section_title.as_deref().unwrap_or("--").chars().take(15).collect();
                let citation_text = format!("[{}] {}", chunk.chunk_id, section_short);
                let citation_width = text_system.measure(&citation_text, FONT_SMALL);

                // Check if fits on current line
                if citation_x + citation_width > citations_bounds.x() + citations_bounds.width() - 8.0 {
                    break; // Don't wrap, just stop
                }

                // Highlight if this is the selected chunk
                let is_selected = state.rlm.selected_chunk_id == Some(chunk.chunk_id);
                let citation_color = if is_selected { state_active() } else { state_complete() };

                let citation_run = text_system.layout(&citation_text, Point::new(citation_x, citation_y), FONT_SMALL, citation_color);
                scene.draw_text(citation_run);

                citation_x += citation_width + 12.0;
            }
        }

        current_y += citations_height + pane_gap;
    }

    // FINAL ANSWER PANE (only if available)
    if let Some(answer) = &state.rlm.final_answer {
        state.rlm.answer_bounds = Bounds::new(x + padding, current_y, width - padding * 2.0, answer_height);

        // Answer pane with special styling
        scene.draw_quad(
            Quad::new(state.rlm.answer_bounds)
                .with_background(state_complete().with_alpha(0.05))
                .with_border(state_complete().with_alpha(0.3), 1.0),
        );

        // Header
        let header_run = text_system.layout("FINAL ANSWER", Point::new(state.rlm.answer_bounds.x() + 8.0, state.rlm.answer_bounds.y() + 8.0), FONT_HEADER, state_complete());
        scene.draw_text(header_run);

        // Render markdown content
        let content_x = state.rlm.answer_bounds.x() + 8.0;
        let content_y = state.rlm.answer_bounds.y() + 32.0;
        let content_width = state.rlm.answer_bounds.width() - 16.0;

        // Parse and render markdown
        let parser = MarkdownParser::new();
        let doc = parser.parse(answer);
        let renderer = MarkdownRenderer::new();
        renderer.render_with_layout(
            &doc,
            Point::new(content_x, content_y),
            content_width,
            text_system,
            scene,
        );
    }
}

fn render_scrollable_pane(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    _state: &AppState,
    title: &str,
    content: &str,
    scroll_offset: f32,
    bounds: Bounds,
) {
    // Pane background
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(bg_dark())
            .with_border(border_color(), 1.0),
    );

    // Header
    let header_color = match title {
        "EXCERPT" => text_muted(),
        "FINDINGS" => state_active(),
        _ => text_primary(),
    };
    let header_run = text_system.layout(title, Point::new(bounds.x() + 8.0, bounds.y() + 8.0), FONT_HEADER, header_color);
    scene.draw_text(header_run);

    // Content with wrapping
    let content_y = bounds.y() + 32.0;
    let content_width = bounds.width() - 16.0;
    let wrapped_lines = wrap_text(text_system, content, content_width, FONT_BODY);

    let line_height = 18.0;
    let max_lines = ((bounds.height() - 40.0) / line_height) as usize;
    let start_line = (scroll_offset / line_height) as usize;

    for (i, line) in wrapped_lines.iter().skip(start_line).take(max_lines).enumerate() {
        let line_y = content_y + (i as f32 * line_height);
        let line_run = text_system.layout(line, Point::new(bounds.x() + 8.0, line_y), FONT_BODY, text_primary());
        scene.draw_text(line_run);
    }
}

/// Helper to check if a phase is complete relative to current phase
fn is_phase_complete_v2(current: RlmPhase, check: RlmPhase) -> bool {
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

    // Apply playback speed to elapsed time
    let raw_elapsed = now.saturating_sub(state.rlm.trace_start_time);
    let elapsed = (raw_elapsed as f32 * state.rlm.playback_speed) as u64;
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
                relevance: None,
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
            relevance,
        } => {
            if let Some(chunk) = state.rlm.chunks.get_mut(*chunk_id) {
                chunk.findings = Some(findings.clone());
                chunk.relevance = Some(*relevance);
                chunk.status = RlmStepStatus::Complete;
            }
            state.rlm.processed_chunks += 1;
            state.rlm.streaming_text = format!("Extracted: {}...", &findings.chars().take(80).collect::<String>());
            // Auto-select completed chunk for inspector display
            state.rlm.selected_chunk_id = Some(*chunk_id);
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
    let point = Point::new(x, y);
    state.rlm.run_button_hovered = state.rlm.run_button_bounds.contains(point);
    state.rlm.restart_button_hovered = state.rlm.restart_button_bounds.contains(point);
    state.rlm.speed_button_hovered = state.rlm.speed_button_bounds.contains(point);
}

/// Handle mouse click
pub(crate) fn handle_rlm_click(state: &mut AppState, x: f32, y: f32) -> bool {
    let point = Point::new(x, y);

    // Run button click - toggle playback
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

    // Restart button click
    if state.rlm.restart_button_bounds.contains(point) {
        reset_and_restart_trace(state);
        return true;
    }

    // Speed button click - cycle through speeds
    if state.rlm.speed_button_bounds.contains(point) {
        state.rlm.playback_speed = match state.rlm.playback_speed {
            s if s < 0.75 => 1.0,
            s if s < 1.25 => 1.5,
            s if s < 1.75 => 2.0,
            _ => 0.5,
        };
        return true;
    }

    // Workset table row clicks
    for (i, bounds) in state.rlm.table_row_bounds.iter().enumerate() {
        if bounds.contains(point) {
            state.rlm.selected_chunk_id = Some(i);
            return true;
        }
    }

    // Chunk grid clicks
    for (i, bounds) in state.rlm.chunk_grid_bounds.iter().enumerate() {
        if bounds.contains(point) {
            state.rlm.selected_chunk_id = Some(i);
            return true;
        }
    }

    false
}

/// Handle keyboard events
pub(crate) fn handle_rlm_keydown(state: &mut AppState, key: &str) -> bool {
    match key {
        "ArrowUp" => {
            // Select previous chunk
            if let Some(current) = state.rlm.selected_chunk_id {
                if current > 0 {
                    state.rlm.selected_chunk_id = Some(current - 1);
                }
            } else if !state.rlm.chunks.is_empty() {
                state.rlm.selected_chunk_id = Some(state.rlm.chunks.len() - 1);
            }
            true
        }
        "ArrowDown" => {
            // Select next chunk
            if let Some(current) = state.rlm.selected_chunk_id {
                if current + 1 < state.rlm.chunks.len() {
                    state.rlm.selected_chunk_id = Some(current + 1);
                }
            } else if !state.rlm.chunks.is_empty() {
                state.rlm.selected_chunk_id = Some(0);
            }
            true
        }
        "r" | "R" => {
            // Restart trace playback
            reset_and_restart_trace(state);
            true
        }
        " " => {
            // Toggle playback (space key)
            if state.rlm.connection_status == RlmConnectionStatus::Streaming {
                state.rlm.connection_status = RlmConnectionStatus::Idle;
                state.rlm.demo_mode = false;
                state.rlm.auto_restart = false;
            } else {
                reset_and_restart_trace(state);
            }
            true
        }
        _ => false,
    }
}

/// Handle scroll events for Inspector panes
pub(crate) fn handle_rlm_scroll(state: &mut AppState, x: f32, y: f32, delta_y: f32) -> bool {
    let point = Point::new(x, y);
    let scroll_amount = delta_y * 20.0; // Scroll sensitivity

    // Excerpt pane scroll
    if state.rlm.excerpt_bounds.contains(point) {
        state.rlm.excerpt_scroll = (state.rlm.excerpt_scroll + scroll_amount).max(0.0);
        return true;
    }

    // Findings pane scroll
    if state.rlm.findings_bounds.contains(point) {
        state.rlm.findings_scroll = (state.rlm.findings_scroll + scroll_amount).max(0.0);
        return true;
    }

    // Answer pane scroll
    if state.rlm.answer_bounds.contains(point) {
        state.rlm.answer_scroll = (state.rlm.answer_scroll + scroll_amount).max(0.0);
        return true;
    }

    false
}
