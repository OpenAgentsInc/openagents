//! Timeline widget for visualizing workflow execution.

use crate::lane::{Lane, LaneId};
use crate::step::{Step, StepId, StepStatus};
use coder_domain::ids::RunId;
use coder_ui_runtime::Signal;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use wgpui::{Bounds, CornerRadii, Hsla, InputEvent, NamedKey, Point, Quad};

/// Timeline colors.
pub mod colors {
    use wgpui::Hsla;

    pub const BACKGROUND: Hsla = Hsla::new(220.0 / 360.0, 0.15, 0.1, 1.0);
    pub const LANE_BG: Hsla = Hsla::new(220.0 / 360.0, 0.1, 0.12, 1.0);
    pub const LANE_BORDER: Hsla = Hsla::new(220.0 / 360.0, 0.1, 0.2, 1.0);
    pub const GRID_LINE: Hsla = Hsla::new(0.0, 0.0, 0.2, 0.3);
    pub const TIME_LABEL: Hsla = Hsla::new(0.0, 0.0, 0.5, 1.0);
    pub const LABEL_FG: Hsla = Hsla::new(0.0, 0.0, 0.8, 1.0);
    pub const STEP_BORDER: Hsla = Hsla::new(0.0, 0.0, 0.3, 1.0);
    pub const PROGRESS_BG: Hsla = Hsla::new(0.0, 0.0, 0.15, 1.0);
}

/// Timeline widget.
pub struct Timeline {
    /// The run being visualized.
    run_id: RunId,
    /// Lanes in the timeline.
    lanes: Vec<Lane>,
    /// Horizontal scroll offset (pixels).
    scroll_x: Signal<f32>,
    /// Vertical scroll offset (pixels).
    scroll_y: Signal<f32>,
    /// Time scale (pixels per millisecond).
    time_scale: Signal<f32>,
    /// Lane label width.
    label_width: f32,
    /// Default lane height.
    lane_height: f32,
    /// Whether timeline has focus.
    focused: bool,
    /// Currently selected step.
    selected_step: Option<(LaneId, StepId)>,
    /// Whether to show time grid.
    show_grid: bool,
    /// Grid interval in milliseconds.
    grid_interval_ms: u64,
}

impl Timeline {
    /// Create a new timeline for a run.
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            lanes: Vec::new(),
            scroll_x: Signal::new(0.0),
            scroll_y: Signal::new(0.0),
            time_scale: Signal::new(0.1), // 0.1 pixels per ms = 100px per second
            label_width: 120.0,
            lane_height: 60.0,
            focused: false,
            selected_step: None,
            show_grid: true,
            grid_interval_ms: 1000, // 1 second intervals
        }
    }

    /// Get the run ID.
    pub fn run_id(&self) -> RunId {
        self.run_id
    }

    /// Add a lane to the timeline.
    pub fn add_lane(&mut self, lane: Lane) {
        self.lanes.push(lane);
    }

    /// Get a lane by ID.
    pub fn get_lane(&self, id: LaneId) -> Option<&Lane> {
        self.lanes.iter().find(|l| l.id == id)
    }

    /// Get a mutable lane by ID.
    pub fn get_lane_mut(&mut self, id: LaneId) -> Option<&mut Lane> {
        self.lanes.iter_mut().find(|l| l.id == id)
    }

    /// Add a step to a lane.
    pub fn add_step(&mut self, lane_id: LaneId, step: Step) {
        if let Some(lane) = self.get_lane_mut(lane_id) {
            lane.add_step(step);
        }
    }

    /// Update a step's status.
    pub fn update_step_status(&mut self, lane_id: LaneId, step_id: StepId, status: StepStatus) {
        if let Some(lane) = self.get_lane_mut(lane_id) {
            if let Some(step) = lane.get_step_mut(step_id) {
                step.status = status;
            }
        }
    }

    /// Get all lanes.
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }

    /// Get lane count.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Set time scale.
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale.set(scale.clamp(0.001, 1.0));
    }

    /// Zoom in (increase scale).
    pub fn zoom_in(&mut self) {
        let current = self.time_scale.get_untracked();
        self.set_time_scale(current * 1.5);
    }

    /// Zoom out (decrease scale).
    pub fn zoom_out(&mut self) {
        let current = self.time_scale.get_untracked();
        self.set_time_scale(current / 1.5);
    }

    /// Scroll to show a specific time.
    pub fn scroll_to_time(&mut self, time_ms: u64) {
        let scale = self.time_scale.get_untracked();
        let x = time_ms as f32 * scale;
        self.scroll_x.set(x);
    }

    /// Get the total time span.
    pub fn total_duration_ms(&self) -> u64 {
        self.lanes
            .iter()
            .filter_map(|l| l.latest_end())
            .max()
            .unwrap_or(0)
    }

    /// Get the total content width.
    fn content_width(&self) -> f32 {
        let duration = self.total_duration_ms();
        let scale = self.time_scale.get_untracked();
        self.label_width + (duration as f32 * scale) + 100.0
    }

    /// Get the total content height.
    fn content_height(&self) -> f32 {
        self.lanes
            .iter()
            .map(|l| l.calculate_height(self.lane_height))
            .sum()
    }

    /// Convert time to x position.
    fn time_to_x(&self, time_ms: u64, bounds: Bounds) -> f32 {
        let scale = self.time_scale.get_untracked();
        let scroll = self.scroll_x.get_untracked();
        bounds.origin.x + self.label_width + (time_ms as f32 * scale) - scroll
    }

    /// Convert x position to time.
    fn x_to_time(&self, x: f32, bounds: Bounds) -> u64 {
        let scale = self.time_scale.get_untracked();
        let scroll = self.scroll_x.get_untracked();
        let relative_x = x - bounds.origin.x - self.label_width + scroll;
        (relative_x / scale).max(0.0) as u64
    }

    /// Select a step.
    pub fn select_step(&mut self, lane_id: LaneId, step_id: StepId) {
        self.selected_step = Some((lane_id, step_id));
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected_step = None;
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new(RunId::new())
    }
}

impl Widget for Timeline {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(colors::BACKGROUND),
        );

        let scroll_y = self.scroll_y.get_untracked();

        // Draw time grid
        if self.show_grid {
            self.paint_time_grid(bounds, cx);
        }

        // Draw lanes
        let mut y = bounds.origin.y - scroll_y;
        for lane in &self.lanes {
            let lane_height = lane.calculate_height(self.lane_height);

            if y + lane_height > bounds.origin.y && y < bounds.origin.y + bounds.size.height {
                self.paint_lane(bounds, y, lane, cx);
            }

            y += lane_height;
        }

        // Draw time ruler at top
        self.paint_time_ruler(bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown { position, .. } => {
                if bounds.contains(*position) {
                    self.focused = true;
                    // Check if clicking on a step
                    if let Some((lane_id, step_id)) = self.hit_test_step(bounds, *position) {
                        self.select_step(lane_id, step_id);
                        return EventResult::Handled;
                    }
                    self.clear_selection();
                    return EventResult::Handled;
                } else {
                    self.focused = false;
                }
            }
            InputEvent::Wheel { delta, modifiers, .. } => {
                if modifiers.command() || modifiers.ctrl {
                    // Zoom with Cmd/Ctrl + wheel
                    if delta.y > 0.0 {
                        self.zoom_in();
                    } else {
                        self.zoom_out();
                    }
                } else if modifiers.shift {
                    // Horizontal scroll with Shift + wheel
                    let max_scroll = (self.content_width() - bounds.size.width).max(0.0);
                    self.scroll_x.update(|x| {
                        *x = (*x - delta.y * 40.0).clamp(0.0, max_scroll);
                    });
                } else {
                    // Vertical scroll
                    let max_scroll = (self.content_height() - bounds.size.height).max(0.0);
                    self.scroll_y.update(|y| {
                        *y = (*y - delta.y * 40.0).clamp(0.0, max_scroll);
                    });
                }
                return EventResult::Handled;
            }
            InputEvent::KeyDown { key, .. } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                match key {
                    wgpui::Key::Character(c) if c == "+" || c == "=" => {
                        self.zoom_in();
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "-" => {
                        self.zoom_out();
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "0" => {
                        self.set_time_scale(0.1);
                        return EventResult::Handled;
                    }
                    wgpui::Key::Named(NamedKey::Home) => {
                        self.scroll_x.set(0.0);
                        return EventResult::Handled;
                    }
                    wgpui::Key::Named(NamedKey::End) => {
                        let max = (self.content_width() - bounds.size.width).max(0.0);
                        self.scroll_x.set(max);
                        return EventResult::Handled;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }
}

impl Timeline {
    /// Paint the time grid.
    fn paint_time_grid(&self, bounds: Bounds, cx: &mut PaintContext) {
        let scale = self.time_scale.get_untracked();
        let scroll_x = self.scroll_x.get_untracked();

        // Calculate appropriate interval based on zoom
        let interval = self.calculate_grid_interval(scale);
        let interval_px = interval as f32 * scale;

        // Only draw if interval is reasonable
        if interval_px < 20.0 {
            return;
        }

        let start_time = (scroll_x / scale) as u64;
        let start_time = (start_time / interval) * interval;

        let content_start = bounds.origin.x + self.label_width;
        let content_width = bounds.size.width - self.label_width;

        let mut time = start_time;
        loop {
            let x = self.time_to_x(time, bounds);
            if x > bounds.origin.x + bounds.size.width {
                break;
            }

            if x >= content_start {
                // Draw grid line
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, bounds.origin.y + 24.0, 1.0, bounds.size.height - 24.0))
                        .with_background(colors::GRID_LINE),
                );
            }

            time += interval;
        }
    }

    /// Calculate appropriate grid interval for current zoom.
    fn calculate_grid_interval(&self, scale: f32) -> u64 {
        let target_px = 100.0; // Target ~100px between grid lines
        let interval_ms = (target_px / scale) as u64;

        // Round to nice values
        if interval_ms < 100 {
            100
        } else if interval_ms < 500 {
            500
        } else if interval_ms < 1000 {
            1000
        } else if interval_ms < 5000 {
            5000
        } else if interval_ms < 10000 {
            10000
        } else if interval_ms < 30000 {
            30000
        } else if interval_ms < 60000 {
            60000
        } else {
            ((interval_ms / 60000) + 1) * 60000
        }
    }

    /// Paint the time ruler.
    fn paint_time_ruler(&self, bounds: Bounds, cx: &mut PaintContext) {
        let ruler_height = 24.0;
        let scale = self.time_scale.get_untracked();
        let scroll_x = self.scroll_x.get_untracked();

        // Draw ruler background
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + self.label_width,
                bounds.origin.y,
                bounds.size.width - self.label_width,
                ruler_height,
            ))
            .with_background(colors::LANE_BG),
        );

        // Draw time labels
        let interval = self.calculate_grid_interval(scale);
        let start_time = (scroll_x / scale) as u64;
        let start_time = (start_time / interval) * interval;

        let mut time = start_time;
        loop {
            let x = self.time_to_x(time, bounds);
            if x > bounds.origin.x + bounds.size.width {
                break;
            }

            if x >= bounds.origin.x + self.label_width {
                let label = format_time(time);
                let run = cx.text.layout(
                    &label,
                    Point::new(x + 4.0, bounds.origin.y + 4.0),
                    11.0,
                    colors::TIME_LABEL,
                );
                cx.scene.draw_text(run);
            }

            time += interval;
        }
    }

    /// Paint a lane.
    fn paint_lane(&self, bounds: Bounds, y: f32, lane: &Lane, cx: &mut PaintContext) {
        let lane_height = lane.calculate_height(self.lane_height);

        // Draw lane background
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                y,
                bounds.size.width,
                lane_height,
            ))
            .with_background(colors::LANE_BG),
        );

        // Draw lane border
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                y + lane_height - 1.0,
                bounds.size.width,
                1.0,
            ))
            .with_background(colors::LANE_BORDER),
        );

        // Draw lane label
        let run = cx.text.layout(
            &lane.label,
            Point::new(bounds.origin.x + 8.0, y + (self.lane_height / 2.0) - 6.0),
            12.0,
            colors::LABEL_FG,
        );
        cx.scene.draw_text(run);

        // Draw steps
        if !lane.collapsed {
            for step in &lane.steps {
                self.paint_step(bounds, y, step, cx);
            }
        }
    }

    /// Paint a step.
    fn paint_step(&self, bounds: Bounds, lane_y: f32, step: &Step, cx: &mut PaintContext) {
        let scale = self.time_scale.get_untracked();

        let start_time = step.start_time.unwrap_or(0);
        let x = self.time_to_x(start_time, bounds);
        let width = step.visual_width(scale);

        // Skip if not visible
        if x + width < bounds.origin.x + self.label_width || x > bounds.origin.x + bounds.size.width
        {
            return;
        }

        let step_y = lane_y + 8.0;
        let step_height = self.lane_height - 16.0;

        // Check if selected
        let is_selected = self
            .selected_step
            .map(|(_, sid)| sid == step.id)
            .unwrap_or(false);

        // Draw step background
        let bg_color = step.status.color();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, step_y, width, step_height))
                .with_background(bg_color)
                .with_corner_radii(CornerRadii::uniform(4.0)),
        );

        // Draw selection border
        if is_selected {
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x - 2.0, step_y - 2.0, width + 4.0, step_height + 4.0))
                    .with_border(Hsla::new(0.0, 0.0, 1.0, 1.0), 2.0)
                    .with_corner_radii(CornerRadii::uniform(6.0)),
            );
        }

        // Draw progress bar for running steps
        if step.status == StepStatus::Running {
            if let Some(progress) = step.progress {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, step_y, width, step_height))
                        .with_background(colors::PROGRESS_BG)
                        .with_corner_radii(CornerRadii::uniform(4.0)),
                );
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, step_y, width * progress, step_height))
                        .with_background(bg_color)
                        .with_corner_radii(CornerRadii::uniform(4.0)),
                );
            }
        }

        // Draw step name
        let text_x = x + 8.0;
        let text_y = step_y + (step_height / 2.0) - 6.0;
        let run = cx.text.layout(
            &step.name,
            Point::new(text_x, text_y),
            11.0,
            Hsla::new(0.0, 0.0, 1.0, 1.0),
        );
        cx.scene.draw_text(run);

        // Draw duration if completed
        if let Some(duration) = step.formatted_duration() {
            let run = cx.text.layout(
                &duration,
                Point::new(text_x, text_y + 14.0),
                10.0,
                Hsla::new(0.0, 0.0, 0.8, 1.0),
            );
            cx.scene.draw_text(run);
        }
    }

    /// Hit test for clicking on a step.
    fn hit_test_step(&self, bounds: Bounds, point: Point) -> Option<(LaneId, StepId)> {
        let scroll_y = self.scroll_y.get_untracked();
        let scale = self.time_scale.get_untracked();

        let mut y = bounds.origin.y - scroll_y;
        for lane in &self.lanes {
            let lane_height = lane.calculate_height(self.lane_height);

            if point.y >= y && point.y < y + lane_height && !lane.collapsed {
                // Check steps in this lane
                for step in &lane.steps {
                    let start_time = step.start_time.unwrap_or(0);
                    let x = self.time_to_x(start_time, bounds);
                    let width = step.visual_width(scale);
                    let step_y = y + 8.0;
                    let step_height = self.lane_height - 16.0;

                    if point.x >= x
                        && point.x < x + width
                        && point.y >= step_y
                        && point.y < step_y + step_height
                    {
                        return Some((lane.id, step.id));
                    }
                }
            }

            y += lane_height;
        }

        None
    }
}

/// Format time in milliseconds to a readable string.
fn format_time(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60000;
        let secs = (ms % 60000) / 1000;
        format!("{}:{:02}", mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_creation() {
        let run_id = RunId::new();
        let timeline = Timeline::new(run_id);

        assert_eq!(timeline.run_id(), run_id);
        assert!(timeline.lanes().is_empty());
    }

    #[test]
    fn test_add_lane() {
        let mut timeline = Timeline::default();
        let lane = Lane::new(LaneId::new(1), "Agent 1");
        timeline.add_lane(lane);

        assert_eq!(timeline.lane_count(), 1);
    }

    #[test]
    fn test_add_step_to_lane() {
        let mut timeline = Timeline::default();
        let lane_id = LaneId::new(1);
        timeline.add_lane(Lane::new(lane_id, "Agent"));

        let step = Step::new(StepId::new(1), timeline.run_id(), "Step 1");
        timeline.add_step(lane_id, step);

        assert_eq!(timeline.get_lane(lane_id).unwrap().step_count(), 1);
    }

    #[test]
    fn test_zoom() {
        let mut timeline = Timeline::default();
        let initial_scale = timeline.time_scale.get_untracked();

        timeline.zoom_in();
        assert!(timeline.time_scale.get_untracked() > initial_scale);

        timeline.zoom_out();
        timeline.zoom_out();
        assert!(timeline.time_scale.get_untracked() < initial_scale);
    }

    #[test]
    fn test_selection() {
        let mut timeline = Timeline::default();
        let lane_id = LaneId::new(1);
        let step_id = StepId::new(1);

        timeline.select_step(lane_id, step_id);
        assert_eq!(timeline.selected_step, Some((lane_id, step_id)));

        timeline.clear_selection();
        assert!(timeline.selected_step.is_none());
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time(500), "500ms");
        assert_eq!(format_time(2500), "2.5s");
        assert_eq!(format_time(65000), "1:05");
    }

    #[test]
    fn test_total_duration() {
        let mut timeline = Timeline::default();
        let lane_id = LaneId::new(1);
        timeline.add_lane(Lane::new(lane_id, "Agent"));

        let mut step = Step::new(StepId::new(1), timeline.run_id(), "Step");
        step.start(0);
        step.complete(5000, None);
        timeline.add_step(lane_id, step);

        assert_eq!(timeline.total_duration_ms(), 5000);
    }
}
