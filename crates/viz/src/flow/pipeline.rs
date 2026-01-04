//! Pipeline - sequential stage visualization

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Flow, VizPrimitive};

/// A stage in a pipeline
#[derive(Clone)]
pub struct PipelineStage {
    #[allow(dead_code)]
    pub label: String,
    pub progress: f32, // 0.0 to 1.0
    pub active: bool,
    pub error: bool,
}

/// A linear pipeline with stages
pub struct Pipeline {
    stages: Vec<PipelineStage>,
    throughput: f32,
    source: Point,
    target: Point,
    stage_color: Hsla,
    active_color: Hsla,
    error_color: Hsla,
    connector_color: Hsla,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            throughput: 0.0,
            source: Point::ZERO,
            target: Point::ZERO,
            stage_color: Hsla::new(0.0, 0.0, 0.2, 1.0),
            active_color: Hsla::new(145.0 / 360.0, 0.8, 0.4, 1.0),
            error_color: Hsla::new(0.0, 0.9, 0.5, 1.0),
            connector_color: Hsla::new(0.0, 0.0, 0.3, 1.0),
        }
    }

    pub fn with_stages(mut self, labels: &[&str]) -> Self {
        self.stages = labels
            .iter()
            .map(|&label| PipelineStage {
                label: label.to_string(),
                progress: 0.0,
                active: false,
                error: false,
            })
            .collect();
        self
    }

    pub fn set_stage_progress(&mut self, index: usize, progress: f32) {
        if let Some(stage) = self.stages.get_mut(index) {
            stage.progress = progress.clamp(0.0, 1.0);
        }
    }

    pub fn set_stage_active(&mut self, index: usize, active: bool) {
        if let Some(stage) = self.stages.get_mut(index) {
            stage.active = active;
        }
    }

    pub fn set_stage_error(&mut self, index: usize, error: bool) {
        if let Some(stage) = self.stages.get_mut(index) {
            stage.error = error;
        }
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Pipeline {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.stages.is_empty() {
            return;
        }

        let stage_count = self.stages.len();
        let stage_width = bounds.size.width / stage_count as f32;
        let stage_height = bounds.size.height * 0.6;
        let connector_height = 4.0;

        for (i, stage) in self.stages.iter().enumerate() {
            let x = bounds.origin.x + i as f32 * stage_width;
            let y = bounds.origin.y + (bounds.size.height - stage_height) / 2.0;

            // Stage box
            let stage_bounds = Bounds {
                origin: Point { x: x + 4.0, y },
                size: Size {
                    width: stage_width - 8.0,
                    height: stage_height,
                },
            };

            let bg_color = if stage.error {
                self.error_color
            } else if stage.active {
                Hsla::new(
                    self.active_color.h,
                    self.active_color.s * 0.3,
                    self.active_color.l * 0.3,
                    1.0,
                )
            } else {
                self.stage_color
            };

            cx.scene.draw_quad(Quad::new(stage_bounds).with_background(bg_color));

            // Progress fill
            if stage.progress > 0.01 {
                let fill_color = if stage.error {
                    self.error_color
                } else {
                    self.active_color
                };

                let progress_bounds = Bounds {
                    origin: stage_bounds.origin,
                    size: Size {
                        width: stage_bounds.size.width * stage.progress,
                        height: stage_bounds.size.height,
                    },
                };
                cx.scene.draw_quad(Quad::new(progress_bounds).with_background(fill_color));
            }

            // Connector to next stage
            if i < stage_count - 1 {
                let conn_x = x + stage_width - 4.0;
                let conn_y = bounds.origin.y + bounds.size.height / 2.0 - connector_height / 2.0;

                let conn_bounds = Bounds {
                    origin: Point { x: conn_x, y: conn_y },
                    size: Size {
                        width: 8.0,
                        height: connector_height,
                    },
                };

                let conn_color = if stage.progress >= 1.0 && !stage.error {
                    self.active_color
                } else {
                    self.connector_color
                };

                cx.scene.draw_quad(Quad::new(conn_bounds).with_background(conn_color));
            }
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(200.0), Some(40.0))
    }
}

impl VizPrimitive for Pipeline {
    fn update(&mut self, value: f32) {
        self.throughput = value;
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.throughput = value;
    }
}

impl Flow for Pipeline {
    fn set_source(&mut self, point: Point) {
        self.source = point;
    }

    fn set_target(&mut self, point: Point) {
        self.target = point;
    }

    fn set_throughput(&mut self, value: f32) {
        self.throughput = value;
    }
}
