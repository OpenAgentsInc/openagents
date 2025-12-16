//! Earnings panel showing stats

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Earnings display panel
pub struct EarningsPanel {
    state: Arc<AppState>,
}

impl EarningsPanel {
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
            "EARNINGS",
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 10.0,
            },
            9.0,
            white_40,
        );
        scene.draw_text(title);

        // Get earnings data
        let earnings = self.state.earnings.get();
        let today_sats = earnings.today_sats / 1000;
        let today_jobs = earnings.jobs_today;
        let week_sats = earnings.week_sats / 1000;
        let week_jobs = earnings.jobs_week;

        // Today's earnings
        let today_text = format!("Today: {} sats ({})", today_sats, today_jobs);
        let today_label = text_system.layout(
            &today_text,
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 32.0,
            },
            10.0,
            white_70,
        );
        scene.draw_text(today_label);

        // Week's earnings
        let week_text = format!("Week: {} sats ({})", week_sats, week_jobs);
        let week_label = text_system.layout(
            &week_text,
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 52.0,
            },
            10.0,
            white_40,
        );
        scene.draw_text(week_label);
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
