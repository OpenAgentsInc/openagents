//! FRLM Panel - composite visualization for FRLM runs

use wgpui::components::{Component, EventContext, EventResult, PaintContext};
use wgpui::{Bounds, Hsla, InputEvent, Point, Quad, Size};

use super::budget::BudgetMeter;
use super::query_lane::QueryStatus;
use super::timeline::{FrlmTimeline, TimelineEntry};

/// Composite FRLM visualization panel
pub struct FrlmPanel {
    /// The timeline view
    timeline: FrlmTimeline,
    /// Budget meter
    budget: BudgetMeter,
    /// Whether panel is expanded
    expanded: bool,
    /// Panel title
    title: String,
    /// Colors
    bg_color: Hsla,
    border_color: Hsla,
    title_color: Hsla,
}

impl FrlmPanel {
    pub fn new() -> Self {
        Self {
            timeline: FrlmTimeline::new(),
            budget: BudgetMeter::new(),
            expanded: true,
            title: "FRLM Conductor".to_string(),
            bg_color: Hsla::new(0.0, 0.0, 0.06, 1.0),
            border_color: Hsla::new(0.0, 0.0, 0.2, 1.0),
            title_color: Hsla::new(0.0, 0.0, 0.85, 1.0),
        }
    }

    /// Set the run ID
    pub fn set_run_id(&mut self, run_id: impl Into<String>) {
        self.timeline.set_run_id(run_id);
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.timeline.clear();
        self.budget.set_budget(0, 0, 1000);
    }

    /// Update budget display
    pub fn set_budget(&mut self, spent_sats: u64, reserved_sats: u64, limit_sats: u64) {
        self.budget.set_budget(spent_sats, reserved_sats, limit_sats);
    }

    /// Add or update a sub-query
    pub fn update_query(
        &mut self,
        query_id: impl Into<String>,
        status: QueryStatus,
        start_ms: u64,
        end_ms: Option<u64>,
        provider_id: Option<String>,
    ) {
        self.timeline.update_entry(TimelineEntry {
            query_id: query_id.into(),
            status,
            start_ms,
            end_ms,
            provider_id,
        });
    }

    /// Set current time for timeline
    pub fn set_current_time(&mut self, now_ms: u64) {
        self.timeline.set_current_time(now_ms);
    }

    /// Toggle expansion
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Set expansion state
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Get timeline stats (total, pending, executing, complete)
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        self.timeline.stats()
    }
}

impl Default for FrlmPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FrlmPanel {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let title_height = 28.0;
        let budget_height = 24.0;
        let padding = 8.0;

        // Panel background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.bg_color)
                .with_border(self.border_color, 1.0)
                .with_corner_radius(4.0)
        );

        // Title bar
        let title_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: bounds.size.width,
                height: title_height,
            },
        };
        cx.scene.draw_quad(
            Quad::new(title_bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0))
                .with_corner_radius(4.0)
        );

        // Title text
        let title_run = cx.text.layout(
            &self.title,
            Point {
                x: bounds.origin.x + padding,
                y: bounds.origin.y + (title_height - 12.0) / 2.0,
            },
            12.0,
            self.title_color,
        );
        cx.scene.draw_text(title_run);

        // Expand/collapse indicator
        let indicator = if self.expanded { "▼" } else { "▶" };
        let indicator_run = cx.text.layout(
            indicator,
            Point {
                x: bounds.origin.x + bounds.size.width - 20.0,
                y: bounds.origin.y + (title_height - 12.0) / 2.0,
            },
            12.0,
            Hsla::new(0.0, 0.0, 0.5, 1.0),
        );
        cx.scene.draw_text(indicator_run);

        // Stats in title bar
        let (total, _pending, executing, complete) = self.stats();
        if total > 0 {
            let stats_text = format!("{}/{} ✓  {} ⏳", complete, total, executing);
            let stats_run = cx.text.layout(
                &stats_text,
                Point {
                    x: bounds.origin.x + bounds.size.width - 120.0,
                    y: bounds.origin.y + (title_height - 10.0) / 2.0,
                },
                10.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
            );
            cx.scene.draw_text(stats_run);
        }

        if !self.expanded {
            return;
        }

        // Budget meter (below title)
        let budget_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + padding,
                y: bounds.origin.y + title_height + padding,
            },
            size: Size {
                width: bounds.size.width - padding * 2.0,
                height: budget_height,
            },
        };
        self.budget.paint(budget_bounds, cx);

        // Timeline (rest of panel)
        let timeline_y = bounds.origin.y + title_height + budget_height + padding * 2.0;
        let timeline_height = bounds.size.height - title_height - budget_height - padding * 3.0;

        if timeline_height > 40.0 {
            let timeline_bounds = Bounds {
                origin: Point {
                    x: bounds.origin.x + padding,
                    y: timeline_y,
                },
                size: Size {
                    width: bounds.size.width - padding * 2.0,
                    height: timeline_height,
                },
            };
            self.timeline.paint(timeline_bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let title_height = 28.0;

        match event {
            InputEvent::MouseDown { x, y, .. } => {
                // Check if click is in title bar
                if *y >= bounds.origin.y && *y <= bounds.origin.y + title_height {
                    if *x >= bounds.origin.x && *x <= bounds.origin.x + bounds.size.width {
                        self.toggle_expanded();
                        return EventResult::Handled;
                    }
                }
            }
            InputEvent::Scroll { dy, .. } => {
                // Scroll the timeline if expanded
                if self.expanded {
                    self.timeline.scroll(-dy * 20.0);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.expanded {
            (Some(400.0), Some(300.0))
        } else {
            (Some(400.0), Some(28.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_creation() {
        let panel = FrlmPanel::new();
        assert!(panel.expanded);
    }

    #[test]
    fn test_update_query() {
        let mut panel = FrlmPanel::new();
        panel.update_query("sq-1", QueryStatus::Pending, 0, None, None);
        panel.update_query("sq-1", QueryStatus::Executing, 0, None, Some("provider-1".to_string()));
        panel.update_query("sq-1", QueryStatus::Complete, 0, Some(1000), Some("provider-1".to_string()));

        let (total, _, _, complete) = panel.stats();
        assert_eq!(total, 1);
        assert_eq!(complete, 1);
    }
}
