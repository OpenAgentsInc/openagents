//! Main replay view component
//!
//! Integrates timeline scrubber, step content display, receipts panel, and diff view.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use autopilot::replay::load_trajectory;
use autopilot::trajectory::{Step, StepType, Trajectory};
use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Quad, Text, theme};

use crate::views::diff_view::{DiffState, DiffView, FileDiff};
use crate::views::receipts::{ReceiptsPanel, ReceiptsState};
use crate::views::timeline::{step_color, step_label, TimelineScrubber, TimelineState};
use crate::views::fit_text;

/// Combined state for the replay view
pub struct ReplayState {
    pub timeline: Rc<RefCell<TimelineState>>,
    pub receipts: Rc<RefCell<ReceiptsState>>,
    pub diff: Rc<RefCell<DiffState>>,
    pub content_scroll_offset: f32,
}

impl ReplayState {
    pub fn new() -> Self {
        Self {
            timeline: Rc::new(RefCell::new(TimelineState::new())),
            receipts: Rc::new(RefCell::new(ReceiptsState::new())),
            diff: Rc::new(RefCell::new(DiffState::new())),
            content_scroll_offset: 0.0,
        }
    }

    /// Load a trajectory from a JSON file
    pub fn load_trajectory_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let trajectory = load_trajectory(path)?;
        self.load_trajectory(trajectory);
        Ok(())
    }

    /// Load a trajectory directly
    pub fn load_trajectory(&mut self, trajectory: Trajectory) {
        self.timeline.borrow_mut().load_trajectory(trajectory.clone());
        self.receipts.borrow_mut().load_trajectory(trajectory);
        self.diff.borrow_mut().clear();
        self.content_scroll_offset = 0.0;

        // Update diff view for current step if it's a tool call
        self.update_diff_for_current_step();
    }

    /// Update diff view based on current step
    pub fn update_diff_for_current_step(&mut self) {
        let timeline = self.timeline.borrow();
        if let Some(step) = timeline.current_step() {
            if let StepType::ToolCall { tool, input, .. } = &step.step_type {
                // Check if this is an Edit tool call
                if tool == "Edit" {
                    if let Some(old_string) = input.get("old_string").and_then(|v| v.as_str()) {
                        if let Some(new_string) = input.get("new_string").and_then(|v| v.as_str()) {
                            let file_path = input.get("file_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");

                            // Create a simple diff
                            let mut diff = FileDiff::new(file_path.to_string());
                            for (i, line) in old_string.lines().enumerate() {
                                diff.add_line(crate::views::diff_view::DiffLine::Removed {
                                    line_num: i + 1,
                                    content: line.to_string(),
                                });
                            }
                            for (i, line) in new_string.lines().enumerate() {
                                diff.add_line(crate::views::diff_view::DiffLine::Added {
                                    line_num: i + 1,
                                    content: line.to_string(),
                                });
                            }

                            drop(timeline);
                            self.diff.borrow_mut().set_diff(diff);
                            return;
                        }
                    }
                }
            }
        }

        // Clear diff if not an Edit tool call
        drop(timeline);
        self.diff.borrow_mut().clear();
    }

    pub fn current_step(&self) -> Option<Step> {
        self.timeline.borrow().current_step().cloned()
    }
}

impl Default for ReplayState {
    fn default() -> Self {
        Self::new()
    }
}

/// Main replay view component
pub struct ReplayView {
    state: Rc<RefCell<ReplayState>>,
    timeline: TimelineScrubber,
    receipts: ReceiptsPanel,
    diff_view: DiffView,
    show_receipts: bool,
    show_diff: bool,
}

impl ReplayView {
    pub fn new(state: Rc<RefCell<ReplayState>>) -> Self {
        let timeline_state = state.borrow().timeline.clone();
        let receipts_state = state.borrow().receipts.clone();
        let diff_state = state.borrow().diff.clone();

        let state_for_callback = state.clone();
        let timeline = TimelineScrubber::new(timeline_state)
            .on_step_change(move |_idx| {
                state_for_callback.borrow_mut().update_diff_for_current_step();
            });

        let receipts = ReceiptsPanel::new(receipts_state);
        let diff_view = DiffView::new(diff_state);

        Self {
            state,
            timeline,
            receipts,
            diff_view,
            show_receipts: true,
            show_diff: true,
        }
    }

    pub fn set_show_receipts(&mut self, show: bool) {
        self.show_receipts = show;
    }

    pub fn set_show_diff(&mut self, show: bool) {
        self.show_diff = show;
    }

    /// Calculate bounds for each component
    fn layout(&self, bounds: Bounds) -> ReplayLayout {
        let timeline_height = 120.0;
        let receipts_width = if self.show_receipts { 250.0 } else { 0.0 };

        let timeline_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            timeline_height,
        );

        let content_y = bounds.origin.y + timeline_height;
        let content_height = bounds.size.height - timeline_height;

        let receipts_bounds = if self.show_receipts {
            Bounds::new(
                bounds.origin.x + bounds.size.width - receipts_width,
                content_y,
                receipts_width,
                content_height,
            )
        } else {
            Bounds::new(0.0, 0.0, 0.0, 0.0)
        };

        let main_width = bounds.size.width - receipts_width;

        // Split main content area between step content and diff (if showing diff)
        let (content_bounds, diff_bounds) = if self.show_diff {
            let diff_height = content_height * 0.4;
            let step_height = content_height - diff_height;

            (
                Bounds::new(bounds.origin.x, content_y, main_width, step_height),
                Bounds::new(bounds.origin.x, content_y + step_height, main_width, diff_height),
            )
        } else {
            (
                Bounds::new(bounds.origin.x, content_y, main_width, content_height),
                Bounds::new(0.0, 0.0, 0.0, 0.0),
            )
        };

        ReplayLayout {
            timeline: timeline_bounds,
            content: content_bounds,
            receipts: receipts_bounds,
            diff: diff_bounds,
        }
    }
}

struct ReplayLayout {
    timeline: Bounds,
    content: Bounds,
    receipts: Bounds,
    diff: Bounds,
}

impl Component for ReplayView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let layout = self.layout(bounds);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(theme::bg::APP),
        );

        // Timeline scrubber
        self.timeline.paint(layout.timeline, cx);

        // Step content area
        self.paint_step_content(layout.content, cx);

        // Receipts panel
        if self.show_receipts && layout.receipts.size.width > 0.0 {
            self.receipts.paint(layout.receipts, cx);
        }

        // Diff view
        if self.show_diff && layout.diff.size.height > 0.0 {
            self.diff_view.paint(layout.diff, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let layout = self.layout(bounds);

        // Timeline events
        if self.timeline.event(event, layout.timeline, cx).is_handled() {
            return EventResult::Handled;
        }

        // Receipts events
        if self.show_receipts && layout.receipts.size.width > 0.0 {
            if self.receipts.event(event, layout.receipts, cx).is_handled() {
                return EventResult::Handled;
            }
        }

        // Diff view events
        if self.show_diff && layout.diff.size.height > 0.0 {
            if self.diff_view.event(event, layout.diff, cx).is_handled() {
                return EventResult::Handled;
            }
        }

        // Content scroll
        if let InputEvent::Scroll { dy, .. } = event {
            // For scroll, we'll just handle it if cursor is likely in content area
            let mut state = self.state.borrow_mut();
            state.content_scroll_offset = (state.content_scroll_offset - dy).max(0.0);
            return EventResult::Handled;
        }

        EventResult::Ignored
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

impl ReplayView {
    fn paint_step_content(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let mut y = bounds.origin.y + padding - state.content_scroll_offset;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(theme::bg::SURFACE),
        );

        let timeline = state.timeline.borrow();

        if let Some(step) = timeline.current_step() {
            // Step header
            let step_num = timeline.current_index + 1;
            let total = timeline.total_steps();
            let label = step_label(step);
            let color = step_color(step);

            let header = format!("Step {}/{}: {}", step_num, total, label);
            let header = fit_text(cx, &header, theme::font_size::BASE, available_width);

            let mut header_text = Text::new(&header)
                .font_size(theme::font_size::BASE)
                .color(color);
            header_text.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height * 1.2),
                cx,
            );
            y += line_height * 1.2;

            // Timestamp
            let timestamp = step.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
            let mut ts_text = Text::new(&timestamp)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            ts_text.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height + theme::spacing::SM;

            // Step content based on type
            match &step.step_type {
                StepType::User { content } => {
                    self.paint_content_block(bounds, &mut y, "User Message", content, theme::accent::BLUE, cx);
                }
                StepType::Assistant { content } => {
                    self.paint_content_block(bounds, &mut y, "Assistant Response", content, theme::accent::GREEN, cx);
                }
                StepType::Thinking { content, signature } => {
                    if let Some(sig) = signature {
                        let sig_text = format!("Signature: {}", &sig[..sig.len().min(32)]);
                        let mut sig_label = Text::new(&sig_text)
                            .font_size(theme::font_size::XS)
                            .color(theme::text::MUTED);
                        sig_label.paint(
                            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                            cx,
                        );
                        y += line_height;
                    }
                    self.paint_content_block(bounds, &mut y, "Thinking", content, theme::accent::PRIMARY, cx);
                }
                StepType::ToolCall { tool, tool_id, input } => {
                    // Tool name
                    let mut tool_label = Text::new(&format!("Tool: {}", tool))
                        .font_size(theme::font_size::SM)
                        .color(theme::accent::BLUE);
                    tool_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height;

                    // Tool ID
                    let id_preview = &tool_id[..tool_id.len().min(24)];
                    let mut id_label = Text::new(&format!("ID: {}", id_preview))
                        .font_size(theme::font_size::XS)
                        .color(theme::text::MUTED);
                    id_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height + theme::spacing::SM;

                    // Input JSON
                    let input_str = serde_json::to_string_pretty(input).unwrap_or_default();
                    self.paint_content_block(bounds, &mut y, "Input", &input_str, theme::text::PRIMARY, cx);
                }
                StepType::ToolResult { tool_id, success, output } => {
                    // Status
                    let status_color = if *success { theme::accent::GREEN } else { theme::accent::RED };
                    let status_text = if *success { "SUCCESS" } else { "FAILED" };

                    let mut status_label = Text::new(status_text)
                        .font_size(theme::font_size::SM)
                        .color(status_color);
                    status_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height;

                    // Tool ID
                    let id_preview = &tool_id[..tool_id.len().min(24)];
                    let mut id_label = Text::new(&format!("ID: {}", id_preview))
                        .font_size(theme::font_size::XS)
                        .color(theme::text::MUTED);
                    id_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height + theme::spacing::SM;

                    // Output
                    if let Some(out) = output {
                        // Truncate very long outputs
                        let truncated = if out.len() > 2000 {
                            format!("{}...\n[truncated, {} more chars]", &out[..2000], out.len() - 2000)
                        } else {
                            out.clone()
                        };
                        self.paint_content_block(bounds, &mut y, "Output", &truncated, theme::text::PRIMARY, cx);
                    } else {
                        let mut no_output = Text::new("(no output)")
                            .font_size(theme::font_size::XS)
                            .color(theme::text::MUTED);
                        no_output.paint(
                            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                            cx,
                        );
                    }
                }
                StepType::SystemInit { model } => {
                    let mut model_label = Text::new(&format!("Model: {}", model))
                        .font_size(theme::font_size::SM)
                        .color(theme::text::PRIMARY);
                    model_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                }
                StepType::SystemStatus { status } => {
                    let mut status_label = Text::new(&format!("Status: {}", status))
                        .font_size(theme::font_size::SM)
                        .color(theme::text::PRIMARY);
                    status_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                }
                StepType::Subagent { agent_id, agent_type, status, summary } => {
                    // Subagent info
                    let mut type_label = Text::new(&format!("Type: {}", agent_type))
                        .font_size(theme::font_size::SM)
                        .color(theme::accent::BLUE);
                    type_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height;

                    let id_preview = &agent_id[..agent_id.len().min(24)];
                    let mut id_label = Text::new(&format!("ID: {}", id_preview))
                        .font_size(theme::font_size::XS)
                        .color(theme::text::MUTED);
                    id_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height;

                    let status_color = match status {
                        autopilot::trajectory::SubagentStatus::Started => theme::accent::PRIMARY,
                        autopilot::trajectory::SubagentStatus::Done => theme::accent::GREEN,
                        autopilot::trajectory::SubagentStatus::Error => theme::accent::RED,
                    };
                    let mut status_label = Text::new(&format!("Status: {:?}", status))
                        .font_size(theme::font_size::SM)
                        .color(status_color);
                    status_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height + theme::spacing::SM;

                    if let Some(sum) = summary {
                        self.paint_content_block(bounds, &mut y, "Summary", sum, theme::text::PRIMARY, cx);
                    }
                }
            }

            // Token info
            if step.tokens_in.is_some() || step.tokens_out.is_some() {
                y += theme::spacing::MD;
                let tokens = format!(
                    "Tokens: in={} out={} cached={}",
                    step.tokens_in.unwrap_or(0),
                    step.tokens_out.unwrap_or(0),
                    step.tokens_cached.unwrap_or(0)
                );
                let mut tokens_text = Text::new(&tokens)
                    .font_size(theme::font_size::XS)
                    .color(theme::text::MUTED);
                tokens_text.paint(
                    Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                    cx,
                );
            }
        } else {
            let mut no_step = Text::new("No step selected")
                .font_size(theme::font_size::SM)
                .color(theme::text::MUTED);
            no_step.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        }
    }

    fn paint_content_block(
        &self,
        bounds: Bounds,
        y: &mut f32,
        label: &str,
        content: &str,
        color: wgpui::Hsla,
        cx: &mut PaintContext,
    ) {
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let code_line_height = theme::font_size::XS * 1.4;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        // Label
        let mut label_text = Text::new(label)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        label_text.paint(
            Bounds::new(bounds.origin.x + padding, *y, available_width, line_height),
            cx,
        );
        *y += line_height;

        // Content background
        let content_lines: Vec<&str> = content.lines().take(50).collect();
        let content_height = (content_lines.len() as f32 * code_line_height).max(code_line_height);

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + padding,
                *y,
                available_width,
                content_height + theme::spacing::SM * 2.0,
            ))
            .with_background(theme::bg::APP),
        );

        *y += theme::spacing::SM;

        // Content lines
        for line in content_lines {
            let line = fit_text(cx, line, theme::font_size::XS, available_width - theme::spacing::SM * 2.0);
            let mut line_text = Text::new(&line)
                .font_size(theme::font_size::XS)
                .color(color);
            line_text.paint(
                Bounds::new(
                    bounds.origin.x + padding + theme::spacing::SM,
                    *y,
                    available_width - theme::spacing::SM * 2.0,
                    code_line_height,
                ),
                cx,
            );
            *y += code_line_height;
        }

        // Show truncation indicator
        if content.lines().count() > 50 {
            let more = format!("... {} more lines", content.lines().count() - 50);
            let mut more_text = Text::new(&more)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            more_text.paint(
                Bounds::new(
                    bounds.origin.x + padding + theme::spacing::SM,
                    *y,
                    available_width - theme::spacing::SM * 2.0,
                    code_line_height,
                ),
                cx,
            );
            *y += code_line_height;
        }

        *y += theme::spacing::SM;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopilot::trajectory::TokenUsage;

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
            input: serde_json::json!({"file_path": "src/main.rs"}),
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
    fn test_replay_state_new() {
        let state = ReplayState::new();
        assert!(state.current_step().is_none());
        assert_eq!(state.content_scroll_offset, 0.0);
    }

    #[test]
    fn test_replay_state_load_trajectory() {
        let mut state = ReplayState::new();
        let traj = make_test_trajectory();
        state.load_trajectory(traj);

        assert!(state.current_step().is_some());
        assert_eq!(state.timeline.borrow().total_steps(), 4);
    }

    #[test]
    fn test_replay_view_layout() {
        let state = Rc::new(RefCell::new(ReplayState::new()));
        let view = ReplayView::new(state);

        let bounds = Bounds::new(0.0, 0.0, 1000.0, 800.0);
        let layout = view.layout(bounds);

        // Timeline should be at the top
        assert_eq!(layout.timeline.origin.y, 0.0);
        assert_eq!(layout.timeline.size.height, 120.0);

        // Content should be below timeline
        assert_eq!(layout.content.origin.y, 120.0);

        // Receipts should be on the right
        assert!(layout.receipts.origin.x > layout.content.origin.x);
    }

    #[test]
    fn test_replay_view_hide_receipts() {
        let state = Rc::new(RefCell::new(ReplayState::new()));
        let mut view = ReplayView::new(state);

        view.set_show_receipts(false);

        let bounds = Bounds::new(0.0, 0.0, 1000.0, 800.0);
        let layout = view.layout(bounds);

        // Receipts should have no width
        assert_eq!(layout.receipts.size.width, 0.0);

        // Content should take full width minus nothing
        assert_eq!(layout.content.size.width, 1000.0);
    }
}
