//! Timeline scrubber component for trajectory replay

use std::cell::RefCell;
use std::rc::Rc;

use autopilot::trajectory::{Step, StepType, Trajectory};
use wgpui::components::{Button, ButtonVariant};
use wgpui::{Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Quad, Text, theme};

use crate::views::fit_text;

/// Playback state for the timeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Paused,
    Playing,
    Seeking,
}

/// Step type filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepFilter {
    All,
    Thinking,
    ToolCalls,
    ToolResults,
    Assistant,
}

impl StepFilter {
    pub fn matches(&self, step: &Step) -> bool {
        match self {
            StepFilter::All => true,
            StepFilter::Thinking => matches!(&step.step_type, StepType::Thinking { .. }),
            StepFilter::ToolCalls => matches!(&step.step_type, StepType::ToolCall { .. }),
            StepFilter::ToolResults => matches!(&step.step_type, StepType::ToolResult { .. }),
            StepFilter::Assistant => matches!(&step.step_type, StepType::Assistant { .. }),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            StepFilter::All => "All",
            StepFilter::Thinking => "Thinking",
            StepFilter::ToolCalls => "Tool Calls",
            StepFilter::ToolResults => "Results",
            StepFilter::Assistant => "Assistant",
        }
    }
}

/// State for the timeline scrubber
pub struct TimelineState {
    pub trajectory: Option<Trajectory>,
    pub current_index: usize,
    pub playback_state: PlaybackState,
    pub playback_speed: f32,
    pub filter: StepFilter,
    /// Cached filtered indices
    filtered_indices: Vec<usize>,
}

impl TimelineState {
    pub fn new() -> Self {
        Self {
            trajectory: None,
            current_index: 0,
            playback_state: PlaybackState::Paused,
            playback_speed: 1.0,
            filter: StepFilter::All,
            filtered_indices: Vec::new(),
        }
    }

    pub fn load_trajectory(&mut self, trajectory: Trajectory) {
        self.trajectory = Some(trajectory);
        self.current_index = 0;
        self.playback_state = PlaybackState::Paused;
        self.update_filtered_indices();
    }

    pub fn update_filtered_indices(&mut self) {
        self.filtered_indices.clear();
        if let Some(ref traj) = self.trajectory {
            for (i, step) in traj.steps.iter().enumerate() {
                if self.filter.matches(step) {
                    self.filtered_indices.push(i);
                }
            }
        }
    }

    pub fn set_filter(&mut self, filter: StepFilter) {
        self.filter = filter;
        self.update_filtered_indices();
        // Clamp current index to valid range
        if !self.filtered_indices.is_empty() && self.current_index >= self.filtered_indices.len() {
            self.current_index = self.filtered_indices.len() - 1;
        }
    }

    pub fn total_steps(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn current_step(&self) -> Option<&Step> {
        let traj = self.trajectory.as_ref()?;
        let actual_idx = self.filtered_indices.get(self.current_index)?;
        traj.steps.get(*actual_idx)
    }

    pub fn current_actual_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.current_index).copied()
    }

    pub fn goto_next(&mut self) {
        if self.current_index + 1 < self.total_steps() {
            self.current_index += 1;
        }
    }

    pub fn goto_prev(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
    }

    pub fn goto_step(&mut self, index: usize) {
        if index < self.total_steps() {
            self.current_index = index;
        }
    }

    pub fn goto_position(&mut self, ratio: f32) {
        let total = self.total_steps();
        if total > 0 {
            let index = ((ratio * total as f32) as usize).min(total - 1);
            self.current_index = index;
        }
    }

    pub fn toggle_playback(&mut self) {
        self.playback_state = match self.playback_state {
            PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Seeking => PlaybackState::Paused,
        };
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.playback_speed = speed;
    }

    pub fn position_ratio(&self) -> f32 {
        let total = self.total_steps();
        if total == 0 {
            0.0
        } else {
            self.current_index as f32 / (total - 1).max(1) as f32
        }
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self::new()
    }
}

/// Step type colors
mod step_colors {
    use wgpui::Hsla;

    fn rgb(r: u8, g: u8, b: u8) -> Hsla {
        Hsla::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn user() -> Hsla { rgb(59, 130, 246) }       // blue
    pub fn assistant() -> Hsla { rgb(16, 185, 129) }  // green
    pub fn thinking() -> Hsla { rgb(245, 158, 11) }   // amber
    pub fn tool_call() -> Hsla { rgb(99, 102, 241) }  // indigo
    pub fn tool_result_success() -> Hsla { rgb(16, 185, 129) } // green
    pub fn tool_result_error() -> Hsla { rgb(239, 68, 68) }    // red
    pub fn system() -> Hsla { rgb(107, 114, 128) }    // gray
    pub fn subagent() -> Hsla { rgb(6, 182, 212) }    // cyan
}

pub fn step_color(step: &Step) -> Hsla {
    match &step.step_type {
        StepType::User { .. } => step_colors::user(),
        StepType::Assistant { .. } => step_colors::assistant(),
        StepType::Thinking { .. } => step_colors::thinking(),
        StepType::ToolCall { .. } => step_colors::tool_call(),
        StepType::ToolResult { success, .. } => {
            if *success { step_colors::tool_result_success() } else { step_colors::tool_result_error() }
        }
        StepType::SystemInit { .. } | StepType::SystemStatus { .. } => step_colors::system(),
        StepType::Subagent { .. } => step_colors::subagent(),
    }
}

pub fn step_label(step: &Step) -> String {
    match &step.step_type {
        StepType::User { .. } => "User".to_string(),
        StepType::Assistant { .. } => "Assistant".to_string(),
        StepType::Thinking { .. } => "Thinking".to_string(),
        StepType::ToolCall { tool, .. } => format!("Tool: {}", tool),
        StepType::ToolResult { success, .. } => {
            if *success { "Result: OK".to_string() } else { "Result: Error".to_string() }
        }
        StepType::SystemInit { model } => format!("Init: {}", model),
        StepType::SystemStatus { status } => format!("Status: {}", status),
        StepType::Subagent { agent_type, status, .. } => {
            format!("Subagent: {} ({:?})", agent_type, status)
        }
    }
}

/// Timeline scrubber component
pub struct TimelineScrubber {
    state: Rc<RefCell<TimelineState>>,
    play_button: Button,
    speed_1x: Button,
    speed_2x: Button,
    speed_4x: Button,
    filter_all: Button,
    filter_thinking: Button,
    filter_tools: Button,
    filter_results: Button,
    filter_assistant: Button,
    dragging: bool,
    on_step_change: Option<Box<dyn Fn(usize)>>,
}

impl TimelineScrubber {
    pub fn new(state: Rc<RefCell<TimelineState>>) -> Self {
        let state_ref = state.clone();
        let play_button = Button::new("Play")
            .variant(ButtonVariant::Primary)
            .on_click({
                let state = state_ref.clone();
                move || {
                    state.borrow_mut().toggle_playback();
                }
            });

        let speed_1x = Button::new("1x")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_speed(1.0);
                }
            });

        let speed_2x = Button::new("2x")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_speed(2.0);
                }
            });

        let speed_4x = Button::new("4x")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_speed(4.0);
                }
            });

        let filter_all = Button::new("All")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_filter(StepFilter::All);
                }
            });

        let filter_thinking = Button::new("Think")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_filter(StepFilter::Thinking);
                }
            });

        let filter_tools = Button::new("Tools")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_filter(StepFilter::ToolCalls);
                }
            });

        let filter_results = Button::new("Results")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_filter(StepFilter::ToolResults);
                }
            });

        let filter_assistant = Button::new("Asst")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_filter(StepFilter::Assistant);
                }
            });

        Self {
            state,
            play_button,
            speed_1x,
            speed_2x,
            speed_4x,
            filter_all,
            filter_thinking,
            filter_tools,
            filter_results,
            filter_assistant,
            dragging: false,
            on_step_change: None,
        }
    }

    pub fn on_step_change<F: Fn(usize) + 'static>(mut self, f: F) -> Self {
        self.on_step_change = Some(Box::new(f));
        self
    }

    fn scrubber_bounds(&self, bounds: Bounds) -> Bounds {
        let padding = theme::spacing::MD;
        let control_height = 32.0;
        let scrubber_y = bounds.origin.y + padding + control_height + theme::spacing::SM;
        let scrubber_height = 16.0;

        Bounds::new(
            bounds.origin.x + padding,
            scrubber_y,
            bounds.size.width - padding * 2.0,
            scrubber_height,
        )
    }

    fn position_from_x(&self, x: f32, bounds: Bounds) -> f32 {
        let scrubber = self.scrubber_bounds(bounds);
        ((x - scrubber.origin.x) / scrubber.size.width).clamp(0.0, 1.0)
    }
}

impl Component for TimelineScrubber {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let button_height = 28.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(theme::bg::SURFACE),
        );

        // Title row
        let title = if let Some(ref traj) = state.trajectory {
            let prompt_preview = if traj.prompt.len() > 40 {
                format!("{}...", &traj.prompt[..37])
            } else {
                traj.prompt.clone()
            };
            format!("Replay: {}", prompt_preview)
        } else {
            "No trajectory loaded".to_string()
        };

        let title = fit_text(cx, &title, theme::font_size::SM, bounds.size.width - padding * 2.0);
        let mut title_text = Text::new(&title)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY);
        title_text.paint(
            Bounds::new(bounds.origin.x + padding, bounds.origin.y + padding, bounds.size.width - padding * 2.0, line_height),
            cx,
        );

        // Control buttons row
        let button_y = bounds.origin.y + padding + line_height;
        let mut button_x = bounds.origin.x + padding;
        let button_spacing = 4.0;

        // Play/Pause button
        let play_label = match state.playback_state {
            PlaybackState::Playing => "Pause",
            _ => "Play",
        };
        drop(state); // Release borrow for button painting

        self.play_button = Button::new(play_label)
            .variant(ButtonVariant::Primary)
            .on_click({
                let state = self.state.clone();
                move || {
                    state.borrow_mut().toggle_playback();
                }
            });

        let play_bounds = Bounds::new(button_x, button_y, 60.0, button_height);
        self.play_button.paint(play_bounds, cx);
        button_x += 60.0 + button_spacing;

        // Speed buttons
        let state = self.state.borrow();
        let speeds = [(1.0, &mut self.speed_1x, "1x"), (2.0, &mut self.speed_2x, "2x"), (4.0, &mut self.speed_4x, "4x")];
        drop(state);

        for (speed, button, _label) in speeds {
            let state = self.state.borrow();
            let variant = if (state.playback_speed - speed).abs() < 0.01 {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            };
            drop(state);

            *button = Button::new(if speed == 1.0 { "1x" } else if speed == 2.0 { "2x" } else { "4x" })
                .variant(variant)
                .on_click({
                    let state = self.state.clone();
                    move || {
                        state.borrow_mut().set_speed(speed);
                    }
                });

            let btn_bounds = Bounds::new(button_x, button_y, 36.0, button_height);
            button.paint(btn_bounds, cx);
            button_x += 36.0 + button_spacing;
        }

        // Filter buttons (right side)
        let filter_start_x = bounds.origin.x + bounds.size.width - padding - (40.0 * 5.0 + button_spacing * 4.0);
        button_x = filter_start_x;

        let state = self.state.borrow();
        let filters = [
            (StepFilter::All, &mut self.filter_all, "All"),
            (StepFilter::Thinking, &mut self.filter_thinking, "Think"),
            (StepFilter::ToolCalls, &mut self.filter_tools, "Tools"),
            (StepFilter::ToolResults, &mut self.filter_results, "Results"),
            (StepFilter::Assistant, &mut self.filter_assistant, "Asst"),
        ];
        let current_filter = state.filter;
        drop(state);

        for (filter, button, label) in filters {
            let variant = if current_filter == filter {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            };

            *button = Button::new(label)
                .variant(variant)
                .on_click({
                    let state = self.state.clone();
                    move || {
                        state.borrow_mut().set_filter(filter);
                    }
                });

            let btn_bounds = Bounds::new(button_x, button_y, 48.0, button_height);
            button.paint(btn_bounds, cx);
            button_x += 48.0 + button_spacing;
        }

        // Scrubber track
        let scrubber = self.scrubber_bounds(bounds);

        // Track background
        cx.scene.draw_quad(
            Quad::new(scrubber).with_background(theme::border::DEFAULT),
        );

        // Draw step markers
        let state = self.state.borrow();
        let total = state.total_steps();
        if total > 0 && state.trajectory.is_some() {
            let traj = state.trajectory.as_ref().unwrap();
            let marker_width = (scrubber.size.width / total as f32).max(2.0).min(8.0);

            for (i, &actual_idx) in state.filtered_indices.iter().enumerate() {
                if let Some(step) = traj.steps.get(actual_idx) {
                    let x = scrubber.origin.x + (i as f32 / total as f32) * scrubber.size.width;
                    let color = step_color(step);

                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(x, scrubber.origin.y, marker_width, scrubber.size.height))
                            .with_background(color),
                    );
                }
            }

            // Playhead
            let playhead_x = scrubber.origin.x + state.position_ratio() * scrubber.size.width;
            let playhead_width = 4.0;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    playhead_x - playhead_width / 2.0,
                    scrubber.origin.y - 2.0,
                    playhead_width,
                    scrubber.size.height + 4.0,
                ))
                .with_background(theme::text::PRIMARY),
            );
        }

        // Step info
        let info_y = scrubber.origin.y + scrubber.size.height + theme::spacing::SM;

        if let Some(step) = state.current_step() {
            let step_num = state.current_index + 1;
            let total = state.total_steps();
            let label = step_label(step);
            let timestamp = step.timestamp.format("%H:%M:%S").to_string();

            let info = format!("[{}/{}] {} - {}", step_num, total, label, timestamp);
            let info = fit_text(cx, &info, theme::font_size::XS, bounds.size.width - padding * 2.0);

            let mut info_text = Text::new(&info)
                .font_size(theme::font_size::XS)
                .color(step_color(step));
            info_text.paint(
                Bounds::new(bounds.origin.x + padding, info_y, bounds.size.width - padding * 2.0, line_height),
                cx,
            );

            // Token info if available
            if step.tokens_in.is_some() || step.tokens_out.is_some() {
                let tokens_y = info_y + line_height;
                let tokens = format!(
                    "Tokens: in={} out={} cached={}",
                    step.tokens_in.unwrap_or(0),
                    step.tokens_out.unwrap_or(0),
                    step.tokens_cached.unwrap_or(0)
                );
                let tokens = fit_text(cx, &tokens, theme::font_size::XS, bounds.size.width - padding * 2.0);

                let mut tokens_text = Text::new(&tokens)
                    .font_size(theme::font_size::XS)
                    .color(theme::text::MUTED);
                tokens_text.paint(
                    Bounds::new(bounds.origin.x + padding, tokens_y, bounds.size.width - padding * 2.0, line_height),
                    cx,
                );
            }
        } else {
            let no_data = "No steps to display";
            let mut no_data_text = Text::new(no_data)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            no_data_text.paint(
                Bounds::new(bounds.origin.x + padding, info_y, bounds.size.width - padding * 2.0, line_height),
                cx,
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let button_height = 28.0;
        let button_y = bounds.origin.y + padding + line_height;
        let button_spacing = 4.0;
        let mut button_x = bounds.origin.x + padding;

        // Play button event
        let play_bounds = Bounds::new(button_x, button_y, 60.0, button_height);
        if self.play_button.event(event, play_bounds, cx).is_handled() {
            return EventResult::Handled;
        }
        button_x += 60.0 + button_spacing;

        // Speed button events
        for button in [&mut self.speed_1x, &mut self.speed_2x, &mut self.speed_4x] {
            let btn_bounds = Bounds::new(button_x, button_y, 36.0, button_height);
            if button.event(event, btn_bounds, cx).is_handled() {
                return EventResult::Handled;
            }
            button_x += 36.0 + button_spacing;
        }

        // Filter button events (right side)
        let filter_start_x = bounds.origin.x + bounds.size.width - padding - (40.0 * 5.0 + button_spacing * 4.0);
        button_x = filter_start_x;

        for button in [
            &mut self.filter_all,
            &mut self.filter_thinking,
            &mut self.filter_tools,
            &mut self.filter_results,
            &mut self.filter_assistant,
        ] {
            let btn_bounds = Bounds::new(button_x, button_y, 48.0, button_height);
            if button.event(event, btn_bounds, cx).is_handled() {
                return EventResult::Handled;
            }
            button_x += 48.0 + button_spacing;
        }

        // Scrubber drag handling
        let scrubber = self.scrubber_bounds(bounds);

        match event {
            InputEvent::MouseDown { x, y, .. } => {
                if scrubber.contains(wgpui::Point::new(*x, *y)) {
                    self.dragging = true;
                    let position = self.position_from_x(*x, bounds);
                    let mut state = self.state.borrow_mut();
                    state.playback_state = PlaybackState::Seeking;
                    let prev_index = state.current_index;
                    state.goto_position(position);
                    let new_index = state.current_index;
                    drop(state);

                    if prev_index != new_index {
                        if let Some(ref callback) = self.on_step_change {
                            callback(new_index);
                        }
                    }
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { .. } => {
                if self.dragging {
                    self.dragging = false;
                    let mut state = self.state.borrow_mut();
                    if state.playback_state == PlaybackState::Seeking {
                        state.playback_state = PlaybackState::Paused;
                    }
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseMove { x, .. } => {
                if self.dragging {
                    let position = self.position_from_x(*x, bounds);
                    let mut state = self.state.borrow_mut();
                    let prev_index = state.current_index;
                    state.goto_position(position);
                    let new_index = state.current_index;
                    drop(state);

                    if prev_index != new_index {
                        if let Some(ref callback) = self.on_step_change {
                            callback(new_index);
                        }
                    }
                    return EventResult::Handled;
                }
            }
            InputEvent::KeyDown { key, .. } => {
                use wgpui::input::{Key, NamedKey};
                let is_next = match key {
                    Key::Named(NamedKey::ArrowRight) => true,
                    Key::Character(s) if s == "n" => true,
                    _ => false,
                };
                let is_prev = match key {
                    Key::Named(NamedKey::ArrowLeft) => true,
                    Key::Character(s) if s == "p" => true,
                    _ => false,
                };
                let is_space = matches!(key, Key::Character(s) if s == " ");

                if is_next {
                    let mut state = self.state.borrow_mut();
                    let prev_index = state.current_index;
                    state.goto_next();
                    let new_index = state.current_index;
                    drop(state);

                    if prev_index != new_index {
                        if let Some(ref callback) = self.on_step_change {
                            callback(new_index);
                        }
                    }
                    return EventResult::Handled;
                } else if is_prev {
                    let mut state = self.state.borrow_mut();
                    let prev_index = state.current_index;
                    state.goto_prev();
                    let new_index = state.current_index;
                    drop(state);

                    if prev_index != new_index {
                        if let Some(ref callback) = self.on_step_change {
                            callback(new_index);
                        }
                    }
                    return EventResult::Handled;
                } else if is_space {
                    self.state.borrow_mut().toggle_playback();
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Width is flexible, height is fixed
        (None, Some(120.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopilot::trajectory::{TokenUsage, TrajectoryResult};
    use chrono::Utc;

    fn make_test_trajectory() -> Trajectory {
        let mut traj = Trajectory::new(
            "Test prompt".to_string(),
            "claude-sonnet".to_string(),
            "/test".to_string(),
            "abc123".to_string(),
            Some("main".to_string()),
        );

        traj.add_step(StepType::Thinking {
            content: "Let me think...".to_string(),
            signature: None,
        });

        traj.add_step(StepType::ToolCall {
            tool: "Read".to_string(),
            tool_id: "tool_1".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        });

        traj.add_step(StepType::ToolResult {
            tool_id: "tool_1".to_string(),
            success: true,
            output: Some("fn main() {}".to_string()),
        });

        traj.add_step(StepType::Assistant {
            content: "I found the file.".to_string(),
        });

        traj
    }

    #[test]
    fn test_timeline_state_new() {
        let state = TimelineState::new();
        assert!(state.trajectory.is_none());
        assert_eq!(state.current_index, 0);
        assert_eq!(state.playback_state, PlaybackState::Paused);
        assert_eq!(state.playback_speed, 1.0);
        assert_eq!(state.filter, StepFilter::All);
    }

    #[test]
    fn test_timeline_state_load_trajectory() {
        let mut state = TimelineState::new();
        let traj = make_test_trajectory();
        state.load_trajectory(traj);

        assert!(state.trajectory.is_some());
        assert_eq!(state.total_steps(), 4);
        assert_eq!(state.current_index, 0);
    }

    #[test]
    fn test_timeline_state_navigation() {
        let mut state = TimelineState::new();
        state.load_trajectory(make_test_trajectory());

        assert_eq!(state.current_index, 0);
        state.goto_next();
        assert_eq!(state.current_index, 1);
        state.goto_next();
        assert_eq!(state.current_index, 2);
        state.goto_prev();
        assert_eq!(state.current_index, 1);
        state.goto_step(3);
        assert_eq!(state.current_index, 3);
    }

    #[test]
    fn test_timeline_state_filter() {
        let mut state = TimelineState::new();
        state.load_trajectory(make_test_trajectory());

        assert_eq!(state.total_steps(), 4);

        state.set_filter(StepFilter::Thinking);
        assert_eq!(state.total_steps(), 1);

        state.set_filter(StepFilter::ToolCalls);
        assert_eq!(state.total_steps(), 1);

        state.set_filter(StepFilter::ToolResults);
        assert_eq!(state.total_steps(), 1);

        state.set_filter(StepFilter::Assistant);
        assert_eq!(state.total_steps(), 1);

        state.set_filter(StepFilter::All);
        assert_eq!(state.total_steps(), 4);
    }

    #[test]
    fn test_step_filter_matches() {
        let traj = make_test_trajectory();

        assert!(StepFilter::All.matches(&traj.steps[0]));
        assert!(StepFilter::Thinking.matches(&traj.steps[0]));
        assert!(!StepFilter::ToolCalls.matches(&traj.steps[0]));

        assert!(StepFilter::ToolCalls.matches(&traj.steps[1]));
        assert!(StepFilter::ToolResults.matches(&traj.steps[2]));
        assert!(StepFilter::Assistant.matches(&traj.steps[3]));
    }

    #[test]
    fn test_playback_toggle() {
        let mut state = TimelineState::new();
        assert_eq!(state.playback_state, PlaybackState::Paused);

        state.toggle_playback();
        assert_eq!(state.playback_state, PlaybackState::Playing);

        state.toggle_playback();
        assert_eq!(state.playback_state, PlaybackState::Paused);
    }

    #[test]
    fn test_position_ratio() {
        let mut state = TimelineState::new();
        state.load_trajectory(make_test_trajectory());

        assert_eq!(state.position_ratio(), 0.0);
        state.goto_step(3);
        assert_eq!(state.position_ratio(), 1.0);
        state.goto_step(1);
        let ratio = state.position_ratio();
        assert!(ratio > 0.0 && ratio < 1.0);
    }
}
