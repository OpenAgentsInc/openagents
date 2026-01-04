//! FRLM Timeline - horizontal visualization of sub-queries over time

use std::collections::HashMap;
use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use super::query_lane::QueryStatus;

/// Timeline entry for a sub-query
#[derive(Clone)]
pub struct TimelineEntry {
    pub query_id: String,
    pub status: QueryStatus,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub provider_id: Option<String>,
}

/// FRLM Timeline showing sub-queries over time
pub struct FrlmTimeline {
    /// Run ID
    run_id: Option<String>,
    /// Timeline entries indexed by query_id
    entries: HashMap<String, TimelineEntry>,
    /// Time range (start, end) in ms
    time_range: (u64, u64),
    /// Scroll offset for vertical scrolling
    scroll_offset: f32,
    /// Maximum visible lanes
    max_visible_lanes: usize,
    /// Lane height
    lane_height: f32,
    /// Header height
    header_height: f32,
    /// Colors
    bg_color: Hsla,
    grid_color: Hsla,
    header_color: Hsla,
}

impl FrlmTimeline {
    pub fn new() -> Self {
        Self {
            run_id: None,
            entries: HashMap::new(),
            time_range: (0, 1000),
            scroll_offset: 0.0,
            max_visible_lanes: 10,
            lane_height: 28.0,
            header_height: 24.0,
            bg_color: Hsla::new(0.0, 0.0, 0.08, 1.0),
            grid_color: Hsla::new(0.0, 0.0, 0.15, 1.0),
            header_color: Hsla::new(0.0, 0.0, 0.12, 1.0),
        }
    }

    /// Set the run ID
    pub fn set_run_id(&mut self, run_id: impl Into<String>) {
        self.run_id = Some(run_id.into());
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.time_range = (0, 1000);
        self.scroll_offset = 0.0;
    }

    /// Add or update a timeline entry
    pub fn update_entry(&mut self, entry: TimelineEntry) {
        // Update time range
        if self.entries.is_empty() {
            self.time_range.0 = entry.start_ms;
        }
        self.time_range.0 = self.time_range.0.min(entry.start_ms);

        if let Some(end) = entry.end_ms {
            self.time_range.1 = self.time_range.1.max(end);
        } else {
            // For running queries, extend timeline to "now" + some padding
            self.time_range.1 = self.time_range.1.max(entry.start_ms + 1000);
        }

        self.entries.insert(entry.query_id.clone(), entry);
    }

    /// Set the current time (for extending timeline)
    pub fn set_current_time(&mut self, now_ms: u64) {
        self.time_range.1 = self.time_range.1.max(now_ms);
    }

    /// Scroll the timeline
    pub fn scroll(&mut self, delta: f32) {
        let max_scroll = ((self.entries.len() as f32 - self.max_visible_lanes as f32) * self.lane_height).max(0.0);
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, max_scroll);
    }

    /// Get entries sorted by start time
    fn sorted_entries(&self) -> Vec<&TimelineEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by_key(|e| e.start_ms);
        entries
    }

    /// Convert time to x position
    fn time_to_x(&self, time_ms: u64, content_x: f32, content_width: f32) -> f32 {
        let duration = (self.time_range.1 - self.time_range.0).max(1) as f32;
        let t = (time_ms - self.time_range.0) as f32 / duration;
        content_x + t * content_width
    }

    /// Get summary stats
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        let total = self.entries.len();
        let pending = self.entries.values().filter(|e| e.status == QueryStatus::Pending).count();
        let executing = self.entries.values().filter(|e| e.status == QueryStatus::Executing || e.status == QueryStatus::Submitted).count();
        let complete = self.entries.values().filter(|e| e.status == QueryStatus::Complete).count();
        (total, pending, executing, complete)
    }
}

impl Default for FrlmTimeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FrlmTimeline {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.bg_color)
                .with_corner_radius(4.0)
        );

        // Header
        let header_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: bounds.size.width,
                height: self.header_height,
            },
        };
        cx.scene.draw_quad(
            Quad::new(header_bounds)
                .with_background(self.header_color)
                .with_corner_radius(4.0)
        );

        // Header text
        let (total, _pending, executing, complete) = self.stats();
        let header_text = if let Some(ref run_id) = self.run_id {
            let short_id = if run_id.len() > 8 { &run_id[..8] } else { run_id };
            format!("FRLM Run {} | {}/{} complete | {} active", short_id, complete, total, executing)
        } else {
            format!("FRLM Timeline | {}/{} complete", complete, total)
        };

        let header_text_run = cx.text.layout(
            &header_text,
            Point {
                x: bounds.origin.x + 8.0,
                y: bounds.origin.y + (self.header_height - 11.0) / 2.0,
            },
            11.0,
            Hsla::new(0.0, 0.0, 0.8, 1.0),
        );
        cx.scene.draw_text(header_text_run);

        // Time markers in header
        let time_width = bounds.size.width - 100.0;
        let time_x = bounds.origin.x + 100.0;
        let duration = self.time_range.1 - self.time_range.0;

        // Draw time markers (every second or so)
        if duration > 0 {
            let marker_interval = if duration > 10000 { 5000 } else if duration > 5000 { 2000 } else { 1000 };
            let mut t = (self.time_range.0 / marker_interval) * marker_interval;
            while t <= self.time_range.1 {
                if t >= self.time_range.0 {
                    let x = self.time_to_x(t, time_x, time_width);
                    let label = format!("{}s", (t - self.time_range.0) / 1000);
                    let label_run = cx.text.layout(
                        &label,
                        Point { x: x - 10.0, y: bounds.origin.y + self.header_height - 12.0 },
                        9.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                    );
                    cx.scene.draw_text(label_run);

                    // Vertical grid line
                    let line_bounds = Bounds {
                        origin: Point { x, y: bounds.origin.y + self.header_height },
                        size: Size { width: 1.0, height: bounds.size.height - self.header_height },
                    };
                    cx.scene.draw_quad(Quad::new(line_bounds).with_background(self.grid_color));
                }
                t += marker_interval;
            }
        }

        // Content area
        let content_y = bounds.origin.y + self.header_height;
        let content_height = bounds.size.height - self.header_height;
        let visible_lanes = (content_height / self.lane_height) as usize;

        // Draw lanes
        let entries = self.sorted_entries();
        let start_idx = (self.scroll_offset / self.lane_height) as usize;

        for (i, entry) in entries.iter().enumerate().skip(start_idx).take(visible_lanes + 1) {
            let lane_y = content_y + (i - start_idx) as f32 * self.lane_height - (self.scroll_offset % self.lane_height);

            if lane_y + self.lane_height < content_y || lane_y > bounds.origin.y + bounds.size.height {
                continue;
            }

            // Lane background (alternating)
            if i % 2 == 0 {
                let lane_bounds = Bounds {
                    origin: Point { x: bounds.origin.x, y: lane_y },
                    size: Size { width: bounds.size.width, height: self.lane_height },
                };
                cx.scene.draw_quad(
                    Quad::new(lane_bounds)
                        .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0))
                );
            }

            // Query ID label (left side)
            let short_id = if entry.query_id.len() > 10 {
                format!("{}...", &entry.query_id[..7])
            } else {
                entry.query_id.clone()
            };
            let id_run = cx.text.layout(
                &short_id,
                Point { x: bounds.origin.x + 4.0, y: lane_y + (self.lane_height - 10.0) / 2.0 },
                10.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
            );
            cx.scene.draw_text(id_run);

            // Time span bar
            let start_x = self.time_to_x(entry.start_ms, time_x, time_width);
            let end_x = if let Some(end) = entry.end_ms {
                self.time_to_x(end, time_x, time_width)
            } else {
                self.time_to_x(self.time_range.1, time_x, time_width)
            };

            let bar_height = self.lane_height * 0.5;
            let bar_y = lane_y + (self.lane_height - bar_height) / 2.0;
            let bar_width = (end_x - start_x).max(4.0);

            let bar_bounds = Bounds {
                origin: Point { x: start_x, y: bar_y },
                size: Size { width: bar_width, height: bar_height },
            };

            let color = entry.status.color();
            cx.scene.draw_quad(
                Quad::new(bar_bounds)
                    .with_background(color)
                    .with_corner_radius(2.0)
            );

            // Pulsing effect for executing queries
            if entry.status == QueryStatus::Executing {
                let pulse_color = Hsla::new(color.h, color.s, color.l + 0.1, 0.3);
                let pulse_bounds = Bounds {
                    origin: Point { x: start_x - 2.0, y: bar_y - 2.0 },
                    size: Size { width: bar_width + 4.0, height: bar_height + 4.0 },
                };
                cx.scene.draw_quad(
                    Quad::new(pulse_bounds)
                        .with_background(pulse_color)
                        .with_corner_radius(3.0)
                );
            }
        }

        // Scrollbar (if needed)
        let total_height = entries.len() as f32 * self.lane_height;
        if total_height > content_height {
            let scrollbar_height = (content_height / total_height * content_height).max(20.0);
            let scrollbar_y = content_y + (self.scroll_offset / total_height * content_height);

            let scrollbar_bounds = Bounds {
                origin: Point {
                    x: bounds.origin.x + bounds.size.width - 6.0,
                    y: scrollbar_y,
                },
                size: Size { width: 4.0, height: scrollbar_height },
            };
            cx.scene.draw_quad(
                Quad::new(scrollbar_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 0.3, 1.0))
                    .with_corner_radius(2.0)
            );
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(400.0), Some(200.0))
    }
}
