//! RLM Execution Panel - visualization for RLM (Recursive Language Model) execution

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, RlmIteration};

/// Draw the RLM execution panel
pub fn draw_rlm_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Only show when RLM is active or has history
    if !state.rlm_active && state.rlm_iterations.is_empty() {
        return;
    }

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
    let title = "RLM EXECUTION";
    let title_run = text.layout(title, Point::new(x + 8.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(title_run);

    // Active indicator
    if state.rlm_active {
        let active_run = text.layout(
            "RUNNING",
            Point::new(x + width - 60.0, y + 7.0),
            9.0,
            accent_green(),
        );
        scene.draw_text(active_run);
    }

    let mut cy = y + header_height + 8.0;

    // Iteration counter
    draw_iteration_counter(scene, text, &state.rlm_iterations, x + 8.0, cy, width - 16.0);
    cy += 28.0;

    // Command timeline
    let available_height = height - (cy - y) - 8.0;
    let row_height = 28.0;
    let max_visible = (available_height / row_height) as usize;

    for iter in state.rlm_iterations.iter().rev().take(max_visible) {
        draw_iteration_row(scene, text, iter, x + 8.0, cy, width - 16.0);
        cy += row_height;
    }
}

/// Draw iteration counter with progress indicator
fn draw_iteration_counter(
    scene: &mut Scene,
    text: &mut TextSystem,
    iterations: &[RlmIteration],
    x: f32,
    y: f32,
    width: f32,
) {
    let count = iterations.len();
    let count_text = format!("Iteration: {}", count);
    let count_run = text.layout(&count_text, Point::new(x, y + 4.0), 12.0, text_white());
    scene.draw_text(count_run);

    // Progress bar (based on command types seen)
    let run_count = iterations.iter().filter(|i| i.command_type == "Run").count();
    let code_count = iterations.iter().filter(|i| i.command_type == "RunCode").count();
    let final_count = iterations.iter().filter(|i| i.command_type == "Final").count();

    let bar_x = x + 100.0;
    let bar_width = width - 100.0;
    let bar_height = 12.0;
    let bar_y = y + 6.0;

    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(bar_x, bar_y),
            size: Size::new(bar_width, bar_height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.15, 1.0))
        .with_corner_radius(2.0),
    );

    // Segments for each command type
    let total = (run_count + code_count + final_count).max(1);
    let mut segment_x = bar_x;

    // Run commands (blue)
    if run_count > 0 {
        let seg_width = bar_width * (run_count as f32 / total as f32);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(segment_x, bar_y),
                size: Size::new(seg_width, bar_height),
            })
            .with_background(Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0))
            .with_corner_radius(2.0),
        );
        segment_x += seg_width;
    }

    // RunCode commands (orange)
    if code_count > 0 {
        let seg_width = bar_width * (code_count as f32 / total as f32);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(segment_x, bar_y),
                size: Size::new(seg_width, bar_height),
            })
            .with_background(Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0))
            .with_corner_radius(2.0),
        );
        segment_x += seg_width;
    }

    // Final commands (green)
    if final_count > 0 {
        let seg_width = bar_width * (final_count as f32 / total as f32);
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(segment_x, bar_y),
                size: Size::new(seg_width, bar_height),
            })
            .with_background(Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0))
            .with_corner_radius(2.0),
        );
    }
}

/// Draw a single iteration row
fn draw_iteration_row(
    scene: &mut Scene,
    text: &mut TextSystem,
    iter: &RlmIteration,
    x: f32,
    y: f32,
    width: f32,
) {
    // Command type indicator
    let (type_color, type_label) = match iter.command_type.as_str() {
        "Run" => (Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0), "RUN"),
        "RunCode" => (Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0), "CODE"),
        "Final" => (Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0), "FINAL"),
        _ => (Hsla::new(0.0, 0.0, 0.5, 1.0), "???"),
    };

    // Type badge
    let badge_width = 40.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y + 2.0),
            size: Size::new(badge_width, 18.0),
        })
        .with_background(type_color.with_alpha(0.3))
        .with_corner_radius(2.0),
    );
    let type_run = text.layout(type_label, Point::new(x + 4.0, y + 4.0), 10.0, type_color);
    scene.draw_text(type_run);

    // Iteration number
    let iter_text = format!("#{}", iter.iteration);
    let iter_run = text.layout(&iter_text, Point::new(x + badge_width + 8.0, y + 4.0), 10.0, text_dim());
    scene.draw_text(iter_run);

    // Executed command preview (truncated)
    let preview_x = x + badge_width + 40.0;
    let _preview_width = width - badge_width - 80.0;
    let preview = if iter.executed.len() > 40 {
        format!("{}...", &iter.executed[..37])
    } else {
        iter.executed.clone()
    };
    let _preview_run = text.layout(&preview, Point::new(preview_x, y + 4.0), 9.0, text_dim());
    // Don't draw preview if it would overflow - just show duration

    // Duration
    let dur_str = if iter.duration_ms >= 1000 {
        format!("{}s", iter.duration_ms / 1000)
    } else {
        format!("{}ms", iter.duration_ms)
    };
    let dur_width = text.measure(&dur_str, 9.0);
    let dur_run = text.layout(
        &dur_str,
        Point::new(x + width - dur_width, y + 5.0),
        9.0,
        text_dim(),
    );
    scene.draw_text(dur_run);
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

fn accent_green() -> Hsla {
    Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0)
}
