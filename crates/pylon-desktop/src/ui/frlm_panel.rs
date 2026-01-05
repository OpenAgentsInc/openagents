//! FRLM Panel - visualization for FRLM conductor state using viz crate

use viz::{FrlmPanel, QueryStatus};
use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, SubQueryDisplayStatus};

/// Draw the FRLM panel showing conductor state using viz::frlm::FrlmPanel
pub fn draw_frlm_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Initialize panel if needed
    if state.frlm_panel.is_none() {
        state.frlm_panel = Some(FrlmPanel::new());
    }

    let panel = state.frlm_panel.as_mut().unwrap();

    // Update panel from state
    if let Some(ref run) = state.frlm_active_run {
        panel.set_run_id(&run.run_id);
        panel.set_budget(
            run.budget_used_sats,
            0, // reserved
            run.budget_remaining_sats + run.budget_used_sats,
        );
    } else {
        panel.clear();
    }

    // Update query statuses
    for (query_id, status) in &state.frlm_subquery_status {
        let (query_status, duration_ms, provider_id) = match status {
            SubQueryDisplayStatus::Pending => (QueryStatus::Pending, None, None),
            SubQueryDisplayStatus::Submitted { .. } => (QueryStatus::Submitted, None, None),
            SubQueryDisplayStatus::Executing { provider_id } => {
                (QueryStatus::Executing, None, Some(provider_id.clone()))
            }
            SubQueryDisplayStatus::Complete { duration_ms } => {
                (QueryStatus::Complete, Some(*duration_ms), None)
            }
            SubQueryDisplayStatus::Failed { .. } => (QueryStatus::Failed, None, None),
            SubQueryDisplayStatus::Timeout => (QueryStatus::Timeout, None, None),
        };
        panel.update_query(query_id, query_status, 0, duration_ms, provider_id);
    }

    // Paint using Component trait
    let bounds = Bounds {
        origin: Point::new(x, y),
        size: Size::new(width, height),
    };
    let scale_factor = 1.0;
    let mut paint_cx = PaintContext::new(scene, text, scale_factor);
    panel.paint(bounds, &mut paint_cx);
}

/// Draw FRLM idle state (when no run is active)
#[allow(dead_code)]
pub fn draw_frlm_idle(
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

    // Idle state message
    let idle_text = "No active run";
    let idle_run = text.layout(
        idle_text,
        Point::new(x + 8.0, y + header_height + 20.0),
        11.0,
        text_dim(),
    );
    scene.draw_text(idle_run);

    // Stats summary
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

// Color helpers
#[allow(dead_code)]
fn panel_bg() -> Hsla {
    Hsla::new(220.0 / 360.0, 0.15, 0.08, 1.0)
}

#[allow(dead_code)]
fn text_dim() -> Hsla {
    Hsla::new(0.0, 0.0, 0.5, 1.0)
}
