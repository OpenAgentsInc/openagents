//! FRLM Panel - visualization for FRLM conductor state

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, FrlmRunState, SubQueryDisplayStatus};

/// Draw the FRLM panel showing conductor state
pub fn draw_frlm_panel(
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
    let title = "FRLM CONDUCTOR";
    let title_run = text.layout(title, Point::new(x + 8.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(title_run);

    // Stats (if active run)
    if let Some(ref run) = state.frlm_active_run {
        draw_active_run(scene, text, run, state, x, y + header_height, width, height - header_height);
    } else {
        // Idle state
        let idle_text = "No active run";
        let idle_run = text.layout(
            idle_text,
            Point::new(x + 8.0, y + header_height + 20.0),
            11.0,
            text_dim(),
        );
        scene.draw_text(idle_run);

        // Show stats summary
        let stats_text = format!(
            "Runs completed: {}  |  Total cost: {} sats",
            state.frlm_runs_completed,
            state.frlm_total_cost_sats
        );
        let stats_run = text.layout(
            &stats_text,
            Point::new(x + 8.0, y + header_height + 40.0),
            10.0,
            text_dim(),
        );
        scene.draw_text(stats_run);
    }
}

fn draw_active_run(
    scene: &mut Scene,
    text: &mut TextSystem,
    run: &FrlmRunState,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let padding = 8.0;
    let mut cy = y + padding;

    // Run ID
    let short_id = if run.run_id.len() > 12 {
        format!("Run: {}...", &run.run_id[..8])
    } else {
        format!("Run: {}", run.run_id)
    };
    let run_id_run = text.layout(&short_id, Point::new(x + padding, cy), 11.0, accent_cyan());
    scene.draw_text(run_id_run);
    cy += 18.0;

    // Progress: X/Y queries
    let progress_text = format!(
        "{}/{} queries complete",
        run.completed_queries,
        run.pending_queries + run.completed_queries
    );
    let progress_run = text.layout(&progress_text, Point::new(x + padding, cy), 10.0, text_white());
    scene.draw_text(progress_run);
    cy += 16.0;

    // Budget bar
    let budget_width = width - padding * 2.0;
    let budget_height = 16.0;
    draw_budget_bar(
        scene,
        text,
        x + padding,
        cy,
        budget_width,
        budget_height,
        run.budget_used_sats,
        run.budget_remaining_sats + run.budget_used_sats,
    );
    cy += budget_height + 12.0;

    // Sub-query timeline
    let timeline_height = height - (cy - y) - padding;
    if timeline_height > 40.0 {
        draw_subquery_timeline(scene, text, state, x + padding, cy, budget_width, timeline_height);
    }
}

fn draw_budget_bar(
    scene: &mut Scene,
    text: &mut TextSystem,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spent: u64,
    limit: u64,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.15, 1.0))
        .with_corner_radius(2.0),
    );

    // Fill
    let ratio = if limit > 0 {
        (spent as f32 / limit as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    if ratio > 0.001 {
        let fill_width = width * ratio;
        let fill_color = if ratio > 0.9 {
            Hsla::new(0.0, 0.85, 0.5, 1.0) // Red
        } else if ratio > 0.7 {
            Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0) // Orange
        } else {
            accent_green() // Green
        };

        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(x, y),
                size: Size::new(fill_width, height),
            })
            .with_background(fill_color)
            .with_corner_radius(2.0),
        );
    }

    // Label
    let label = format!("{} / {} sats", spent, limit);
    let label_run = text.layout(&label, Point::new(x + 4.0, y + 2.0), 10.0, text_white());
    scene.draw_text(label_run);
}

fn draw_subquery_timeline(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Header
    let header_run = text.layout("SUB-QUERIES", Point::new(x, y), 9.0, text_dim());
    scene.draw_text(header_run);

    let lane_y = y + 14.0;
    let lane_height = 20.0;
    let max_visible = ((height - 14.0) / lane_height) as usize;

    // Sort queries by status (executing first, then pending, then complete)
    let mut queries: Vec<_> = state.frlm_subquery_status.iter().collect();
    queries.sort_by(|(_, a), (_, b)| {
        fn status_order(s: &SubQueryDisplayStatus) -> u8 {
            match s {
                SubQueryDisplayStatus::Executing { .. } => 0,
                SubQueryDisplayStatus::Submitted { .. } => 1,
                SubQueryDisplayStatus::Pending => 2,
                SubQueryDisplayStatus::Complete { .. } => 3,
                SubQueryDisplayStatus::Failed { .. } => 4,
                SubQueryDisplayStatus::Timeout => 5,
            }
        }
        status_order(a).cmp(&status_order(b))
    });

    for (i, (query_id, status)) in queries.iter().enumerate().take(max_visible) {
        let qy = lane_y + i as f32 * lane_height;
        draw_query_lane(scene, text, query_id, status, x, qy, width, lane_height - 2.0);
    }

    // Show count if more queries
    if queries.len() > max_visible {
        let more = queries.len() - max_visible;
        let more_text = format!("... +{} more", more);
        let more_run = text.layout(
            &more_text,
            Point::new(x, lane_y + max_visible as f32 * lane_height),
            9.0,
            text_dim(),
        );
        scene.draw_text(more_run);
    }
}

fn draw_query_lane(
    scene: &mut Scene,
    text: &mut TextSystem,
    query_id: &str,
    status: &SubQueryDisplayStatus,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Status dot
    let dot_size = height * 0.5;
    let dot_y = y + (height - dot_size) / 2.0;

    let (color, status_label) = match status {
        SubQueryDisplayStatus::Pending => (Hsla::new(0.0, 0.0, 0.4, 1.0), "pending"),
        SubQueryDisplayStatus::Submitted { .. } => (Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0), "submitted"),
        SubQueryDisplayStatus::Executing { .. } => (Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0), "executing"),
        SubQueryDisplayStatus::Complete { duration_ms } => {
            let dur = if *duration_ms >= 1000 {
                format!("{}s", duration_ms / 1000)
            } else {
                format!("{}ms", duration_ms)
            };
            (accent_green(), &*format!("done {}", dur))
        }
        SubQueryDisplayStatus::Failed { .. } => (Hsla::new(0.0, 0.85, 0.5, 1.0), "failed"),
        SubQueryDisplayStatus::Timeout => (Hsla::new(280.0 / 360.0, 0.6, 0.5, 1.0), "timeout"),
    };

    // Draw status dot
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, dot_y),
            size: Size::new(dot_size, dot_size),
        })
        .with_background(color)
        .with_corner_radius(dot_size / 2.0),
    );

    // Query ID (shortened)
    let short_id = if query_id.len() > 10 {
        format!("{}...", &query_id[..7])
    } else {
        query_id.to_string()
    };
    let id_run = text.layout(
        &short_id,
        Point::new(x + dot_size + 6.0, y + 2.0),
        10.0,
        text_white(),
    );
    scene.draw_text(id_run);

    // Status label (right side)
    let status_str = match status {
        SubQueryDisplayStatus::Complete { duration_ms } => {
            if *duration_ms >= 1000 {
                format!("{}s", duration_ms / 1000)
            } else {
                format!("{}ms", duration_ms)
            }
        }
        _ => status_label.to_string(),
    };
    let status_width = text.measure(&status_str, 9.0);
    let status_run = text.layout(
        &status_str,
        Point::new(x + width - status_width - 4.0, y + 3.0),
        9.0,
        text_dim(),
    );
    scene.draw_text(status_run);
}

// Color helpers
fn panel_bg() -> Hsla {
    Hsla::new(220.0 / 360.0, 0.15, 0.08, 1.0)
}

fn accent_cyan() -> Hsla {
    Hsla::new(180.0 / 360.0, 0.8, 0.5, 1.0)
}

fn accent_green() -> Hsla {
    Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0)
}

fn text_dim() -> Hsla {
    Hsla::new(0.0, 0.0, 0.5, 1.0)
}

fn text_white() -> Hsla {
    Hsla::new(0.0, 0.0, 0.9, 1.0)
}
