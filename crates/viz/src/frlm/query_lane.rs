//! Query lane - individual sub-query progress visualization

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

/// Status of a sub-query
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    Pending,
    Submitted,
    Executing,
    Complete,
    Failed,
    Timeout,
}

impl QueryStatus {
    pub fn color(&self) -> Hsla {
        match self {
            QueryStatus::Pending => Hsla::new(0.0, 0.0, 0.4, 1.0),      // Gray
            QueryStatus::Submitted => Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0),  // Blue
            QueryStatus::Executing => Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0),  // Orange (active)
            QueryStatus::Complete => Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0),  // Green
            QueryStatus::Failed => Hsla::new(0.0, 0.85, 0.5, 1.0),              // Red
            QueryStatus::Timeout => Hsla::new(280.0 / 360.0, 0.6, 0.5, 1.0),    // Purple
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            QueryStatus::Pending => "pending",
            QueryStatus::Submitted => "submitted",
            QueryStatus::Executing => "executing",
            QueryStatus::Complete => "complete",
            QueryStatus::Failed => "failed",
            QueryStatus::Timeout => "timeout",
        }
    }
}

/// A single sub-query lane in the timeline
pub struct QueryLane {
    query_id: String,
    status: QueryStatus,
    progress: f32,  // 0.0 to 1.0 for executing queries
    duration_ms: Option<u64>,
    provider_id: Option<String>,

    // Animation
    display_progress: f32,
    pulse_intensity: f32,
}

impl QueryLane {
    pub fn new(query_id: impl Into<String>) -> Self {
        Self {
            query_id: query_id.into(),
            status: QueryStatus::Pending,
            progress: 0.0,
            duration_ms: None,
            provider_id: None,
            display_progress: 0.0,
            pulse_intensity: 0.0,
        }
    }

    pub fn with_status(mut self, status: QueryStatus) -> Self {
        if self.status != status {
            self.pulse_intensity = 1.0;  // Trigger pulse on status change
        }
        self.status = status;
        self
    }

    pub fn set_status(&mut self, status: QueryStatus) {
        if self.status != status {
            self.pulse_intensity = 1.0;
        }
        self.status = status;
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    pub fn set_duration(&mut self, duration_ms: u64) {
        self.duration_ms = Some(duration_ms);
    }

    pub fn set_provider(&mut self, provider_id: impl Into<String>) {
        self.provider_id = Some(provider_id.into());
    }

    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    pub fn status(&self) -> QueryStatus {
        self.status
    }
}

impl Default for QueryLane {
    fn default() -> Self {
        Self::new("query")
    }
}

impl Component for QueryLane {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Animate progress
        self.display_progress += (self.progress - self.display_progress) * 0.15;

        // Decay pulse
        self.pulse_intensity = (self.pulse_intensity - 0.03).max(0.0);

        let height = bounds.size.height;
        let status_size = height.min(16.0);
        let padding = 4.0;

        // Status indicator (left side)
        let status_color = self.status.color();
        let status_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + padding,
                y: bounds.origin.y + (height - status_size) / 2.0,
            },
            size: Size {
                width: status_size,
                height: status_size,
            },
        };

        // Glow effect on pulse
        if self.pulse_intensity > 0.1 {
            let glow_size = status_size * (1.0 + 0.5 * self.pulse_intensity);
            let glow_bounds = Bounds {
                origin: Point {
                    x: status_bounds.origin.x - (glow_size - status_size) / 2.0,
                    y: status_bounds.origin.y - (glow_size - status_size) / 2.0,
                },
                size: Size {
                    width: glow_size,
                    height: glow_size,
                },
            };
            let glow_color = Hsla::new(status_color.h, status_color.s, status_color.l, self.pulse_intensity * 0.4);
            cx.scene.draw_quad(
                Quad::new(glow_bounds)
                    .with_background(glow_color)
                    .with_corner_radius(glow_size / 2.0)
            );
        }

        // Status dot
        cx.scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(status_color)
                .with_corner_radius(status_size / 2.0)
        );

        // Progress bar (for executing queries)
        let bar_x = bounds.origin.x + padding + status_size + padding;
        let bar_width = bounds.size.width - bar_x + bounds.origin.x - padding;
        let bar_height = 4.0;
        let bar_y = bounds.origin.y + (height - bar_height) / 2.0;

        if bar_width > 10.0 {
            // Background track
            let track_bounds = Bounds {
                origin: Point { x: bar_x, y: bar_y },
                size: Size { width: bar_width, height: bar_height },
            };
            cx.scene.draw_quad(
                Quad::new(track_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 0.15, 1.0))
                    .with_corner_radius(2.0)
            );

            // Progress fill
            let fill_width = bar_width * self.display_progress;
            if fill_width > 0.001 {
                let fill_bounds = Bounds {
                    origin: Point { x: bar_x, y: bar_y },
                    size: Size { width: fill_width, height: bar_height },
                };
                cx.scene.draw_quad(
                    Quad::new(fill_bounds)
                        .with_background(status_color)
                        .with_corner_radius(2.0)
                );
            }

            // Query ID label (below progress bar)
            let label_y = bar_y + bar_height + 2.0;
            if label_y + 10.0 < bounds.origin.y + height {
                let short_id = if self.query_id.len() > 12 {
                    format!("{}...", &self.query_id[..8])
                } else {
                    self.query_id.clone()
                };

                let text_color = Hsla::new(0.0, 0.0, 0.6, 1.0);
                let text_run = cx.text.layout(
                    &short_id,
                    Point { x: bar_x, y: label_y },
                    9.0,
                    text_color,
                );
                cx.scene.draw_text(text_run);
            }
        }

        // Duration label (if complete)
        if let Some(duration) = self.duration_ms {
            let duration_str = if duration >= 1000 {
                format!("{}s", duration / 1000)
            } else {
                format!("{}ms", duration)
            };

            let text_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
            let text_x = bounds.origin.x + bounds.size.width - 40.0;
            let text_y = bounds.origin.y + (height - 10.0) / 2.0;

            let text_run = cx.text.layout(
                &duration_str,
                Point { x: text_x, y: text_y },
                10.0,
                text_color,
            );
            cx.scene.draw_text(text_run);
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(200.0), Some(24.0))
    }
}
