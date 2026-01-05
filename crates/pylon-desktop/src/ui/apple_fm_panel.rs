//! Apple FM Tools Panel - visualization for Apple FM tool selection and execution

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{AppleFmToolCall, FmVizState, ToolCallStatus};

/// Draw the Apple FM tools panel
pub fn draw_apple_fm_tools_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg())
        .with_corner_radius(4.0),
    );

    // Header
    let header_height = 24.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, header_height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.12, 1.0))
        .with_corner_radius(4.0),
    );

    // Title
    let title = "APPLE FM TOOLS";
    let title_run = text.layout(title, Point::new(x + 8.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(title_run);

    // Tool count
    let count_text = format!("{} calls", state.apple_fm_tool_calls.len());
    let count_width = text.measure(&count_text, 9.0);
    let count_run = text.layout(
        &count_text,
        Point::new(x + width - count_width - 8.0, y + 7.0),
        9.0,
        text_dim(),
    );
    scene.draw_text(count_run);

    let mut cy = y + header_height + 8.0;

    // Current tool call (if any)
    if let Some(ref call) = state.current_tool_call {
        draw_active_tool_indicator(scene, text, call, x + 8.0, cy, width - 16.0);
        cy += 36.0;
    }

    // Recent tool calls (scrollable list)
    let available_height = height - (cy - y) - 8.0;
    let row_height = 24.0;
    let max_visible = (available_height / row_height) as usize;

    for call in state.apple_fm_tool_calls.iter().rev().take(max_visible) {
        draw_tool_call_row(scene, text, call, x + 8.0, cy, width - 16.0);
        cy += row_height;
    }

    // Show "more" indicator if needed
    if state.apple_fm_tool_calls.len() > max_visible {
        let more_count = state.apple_fm_tool_calls.len() - max_visible;
        let more_text = format!("... +{} more", more_count);
        let more_run = text.layout(
            &more_text,
            Point::new(x + 8.0, y + height - 16.0),
            9.0,
            text_dim(),
        );
        scene.draw_text(more_run);
    }
}

/// Draw the active tool indicator with pulsing effect
fn draw_active_tool_indicator(
    scene: &mut Scene,
    text: &mut TextSystem,
    call: &AppleFmToolCall,
    x: f32,
    y: f32,
    width: f32,
) {
    // Background with active color
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, 32.0),
        })
        .with_background(Hsla::new(45.0 / 360.0, 0.3, 0.15, 1.0))
        .with_corner_radius(4.0),
    );

    // Pulsing dot (orange for executing)
    let dot_size = 10.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 6.0, y + (32.0 - dot_size) / 2.0),
            size: Size::new(dot_size, dot_size),
        })
        .with_background(Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0))
        .with_corner_radius(dot_size / 2.0),
    );

    // Tool name
    let name_run = text.layout(
        &call.tool_name,
        Point::new(x + 22.0, y + 6.0),
        11.0,
        accent_orange(),
    );
    scene.draw_text(name_run);

    // Status label
    let status_run = text.layout(
        "executing...",
        Point::new(x + 22.0, y + 18.0),
        9.0,
        text_dim(),
    );
    scene.draw_text(status_run);
}

/// Draw a single tool call row
fn draw_tool_call_row(
    scene: &mut Scene,
    text: &mut TextSystem,
    call: &AppleFmToolCall,
    x: f32,
    y: f32,
    width: f32,
) {
    // Status dot
    let dot_size = 8.0;
    let dot_color = match call.status {
        ToolCallStatus::Pending => Hsla::new(0.0, 0.0, 0.4, 1.0),
        ToolCallStatus::Executing => Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0),
        ToolCallStatus::Complete => Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0),
        ToolCallStatus::Failed => Hsla::new(0.0, 0.85, 0.5, 1.0),
    };

    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y + (20.0 - dot_size) / 2.0),
            size: Size::new(dot_size, dot_size),
        })
        .with_background(dot_color)
        .with_corner_radius(dot_size / 2.0),
    );

    // Tool name
    let name_run = text.layout(
        &call.tool_name,
        Point::new(x + 14.0, y + 4.0),
        10.0,
        text_white(),
    );
    scene.draw_text(name_run);

    // Duration (if complete)
    if let Some(completed_at) = call.completed_at {
        let duration_ms = completed_at.saturating_sub(call.started_at);
        let duration_str = if duration_ms >= 1000 {
            format!("{}s", duration_ms / 1000)
        } else {
            format!("{}ms", duration_ms)
        };
        let dur_width = text.measure(&duration_str, 9.0);
        let dur_run = text.layout(
            &duration_str,
            Point::new(x + width - dur_width, y + 5.0),
            9.0,
            text_dim(),
        );
        scene.draw_text(dur_run);
    }
}

/// Draw tool usage statistics
#[allow(dead_code)]
fn draw_tool_stats(
    scene: &mut Scene,
    text: &mut TextSystem,
    calls: &[AppleFmToolCall],
    x: f32,
    y: f32,
    _width: f32,
) {
    // Count by tool
    let mut tool_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for call in calls {
        *tool_counts.entry(&call.tool_name).or_insert(0) += 1;
    }

    // Total and success rate
    let total = calls.len();
    let complete = calls.iter().filter(|c| c.status == ToolCallStatus::Complete).count();
    let success_rate = if total > 0 {
        (complete as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    let stats_text = format!("{} calls | {}% success", total, success_rate);
    let stats_run = text.layout(&stats_text, Point::new(x, y), 9.0, text_dim());
    scene.draw_text(stats_run);
}

// Color helpers
fn panel_bg() -> Hsla {
    Hsla::new(220.0 / 360.0, 0.15, 0.08, 1.0)
}

fn text_dim() -> Hsla {
    Hsla::new(0.0, 0.0, 0.5, 1.0)
}

fn text_white() -> Hsla {
    Hsla::new(0.0, 0.0, 0.9, 1.0)
}

fn accent_orange() -> Hsla {
    Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0)
}
