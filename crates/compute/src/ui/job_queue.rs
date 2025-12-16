//! Job queue panel showing active jobs

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, Size, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Job queue panel
pub struct JobQueue {
    state: Arc<AppState>,
}

impl JobQueue {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        let white_70 = Hsla::new(0.0, 0.0, 1.0, 0.7);
        let white_40 = Hsla::new(0.0, 0.0, 1.0, 0.4);
        let white_10 = Hsla::new(0.0, 0.0, 1.0, 0.1);

        // Panel background
        scene.draw_quad(Quad {
            bounds,
            background: Some(white_10),
            corner_radii: CornerRadii::uniform(4.0),
            ..Default::default()
        });

        // Title
        let title = text_system.layout(
            "JOBS",
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 10.0,
            },
            9.0,
            white_40,
        );
        scene.draw_text(title);

        // Get active jobs
        let jobs = self.state.active_jobs.get();

        if jobs.is_empty() {
            let status = text_system.layout(
                "No active jobs",
                Point {
                    x: bounds.origin.x + 10.0,
                    y: bounds.origin.y + 35.0,
                },
                10.0,
                white_40,
            );
            scene.draw_text(status);
            return;
        }

        // List jobs (up to 4)
        let row_height = 20.0;
        for (i, job) in jobs.iter().take(4).enumerate() {
            let y = bounds.origin.y + 30.0 + (i as f32 * row_height);

            // Job row background
            scene.draw_quad(Quad {
                bounds: Bounds {
                    origin: Point { x: bounds.origin.x + 6.0, y },
                    size: Size {
                        width: bounds.size.width - 12.0,
                        height: row_height - 2.0,
                    },
                },
                background: Some(Hsla::new(0.0, 0.0, 1.0, 0.05)),
                corner_radii: CornerRadii::uniform(2.0),
                ..Default::default()
            });

            // Job ID (truncated)
            let job_id_short = if job.id.len() > 8 {
                format!("{}...", &job.id[..8])
            } else {
                job.id.clone()
            };
            let id_label = text_system.layout(
                &job_id_short,
                Point { x: bounds.origin.x + 12.0, y: y + 3.0 },
                9.0,
                white_40,
            );
            scene.draw_text(id_label);

            // Status (no colors, just white text with varying opacity)
            let status_text = match &job.status {
                crate::domain::JobStatus::Pending => "Pending",
                crate::domain::JobStatus::PaymentRequired { .. } => "Payment",
                crate::domain::JobStatus::Processing { .. } => "Working",
                crate::domain::JobStatus::Completed { .. } => "Done",
                crate::domain::JobStatus::Failed { .. } => "Failed",
            };
            let status_width = text_system.measure(status_text, 9.0);
            let status_label = text_system.layout(
                status_text,
                Point {
                    x: bounds.origin.x + bounds.size.width - status_width - 16.0,
                    y: y + 3.0,
                },
                9.0,
                white_70,
            );
            scene.draw_text(status_label);
        }
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
