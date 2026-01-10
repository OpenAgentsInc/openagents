use crate::components::atoms::{ToolStatus, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Child tool for nested display under Task tools
#[derive(Clone)]
pub struct ChildTool {
    pub tool_type: ToolType,
    pub name: String,
    pub params: String,
    pub status: ToolStatus,
    pub elapsed_secs: Option<f64>,
}

/// Compact tool call card - Zed-style one-line rendering
pub struct ToolCallCard {
    id: Option<ComponentId>,
    tool_type: ToolType,
    tool_name: String,
    status: ToolStatus,
    input: Option<String>,
    output: Option<String>,
    elapsed_secs: Option<f64>,
    expanded: bool,
    hovered: bool,
    /// Child tools for Task/subagent cards
    child_tools: Vec<ChildTool>,
    /// Scroll offset for child tools area
    child_scroll: f32,
    /// Whether to auto-scroll to bottom when children are added
    auto_scroll: bool,
    /// Last mouse position for scroll hit-testing
    last_mouse_pos: Option<Point>,
}

impl ToolCallCard {
    const HEADER_HEIGHT: f32 = 22.0;
    const LINE_HEIGHT: f32 = 18.0;
    const FONT_SIZE: f32 = 12.0;
    const DETAIL_FONT_SIZE: f32 = 11.0;

    /// Max visible child tools (height = 5 * HEADER_HEIGHT)
    const MAX_VISIBLE_CHILDREN: usize = 5;

    pub fn new(tool_type: ToolType, name: impl Into<String>) -> Self {
        Self {
            id: None,
            tool_type,
            tool_name: name.into(),
            status: ToolStatus::Pending,
            input: None,
            output: None,
            elapsed_secs: None,
            expanded: false, // Collapsed by default
            hovered: false,
            child_tools: Vec::new(),
            child_scroll: 0.0,
            auto_scroll: true, // Auto-scroll to bottom by default
            last_mouse_pos: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    pub fn input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn elapsed_secs(mut self, elapsed_secs: f64) -> Self {
        self.elapsed_secs = Some(elapsed_secs);
        self
    }

    pub fn set_elapsed_secs(&mut self, elapsed_secs: f64) {
        self.elapsed_secs = Some(elapsed_secs);
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn get_status(&self) -> ToolStatus {
        self.status
    }

    /// Check if this is a Task tool (has or can have children)
    pub fn is_task(&self) -> bool {
        self.tool_type == ToolType::Task
    }

    /// Add a child tool (for Task/subagent cards)
    pub fn add_child(&mut self, child: ChildTool) {
        self.child_tools.push(child);
        // Auto-scroll to bottom when a new child is added
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Scroll to show the most recent (bottom) child
    pub fn scroll_to_bottom(&mut self) {
        let total_children_height = self.child_tools.len() as f32 * Self::HEADER_HEIGHT;
        let visible_height =
            (Self::MAX_VISIBLE_CHILDREN as f32 * Self::HEADER_HEIGHT).min(total_children_height);
        let max_scroll = (total_children_height - visible_height).max(0.0);
        self.child_scroll = max_scroll;
    }

    /// Enable or disable auto-scroll to bottom
    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.auto_scroll = enabled;
    }

    /// Update a child tool's status by name and params
    pub fn update_child_status(&mut self, name: &str, params: &str, status: ToolStatus) {
        // Find matching child and update (search in reverse for most recent)
        for child in self.child_tools.iter_mut().rev() {
            if child.name == name && child.params.starts_with(&params[..params.len().min(30)]) {
                child.status = status;
                break;
            }
        }
    }

    /// Get number of child tools
    pub fn child_count(&self) -> usize {
        self.child_tools.len()
    }

    /// Truncate text to fit within available width (char-boundary safe)
    fn truncate_text(text: &str, available_width: f32, font_size: f32) -> String {
        let char_width = font_size * 0.6;
        let max_chars = (available_width / char_width) as usize;
        if text.len() <= max_chars {
            text.to_string()
        } else if max_chars > 3 {
            // Find valid UTF-8 char boundary
            let target = max_chars - 3;
            let mut end = target;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &text[..end])
        } else {
            "...".to_string()
        }
    }
}

impl Default for ToolCallCard {
    fn default() -> Self {
        Self::new(ToolType::Read, "read")
    }
}

impl Component for ToolCallCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 4.0;
        let x = bounds.origin.x + padding;
        let y = bounds.origin.y + (Self::HEADER_HEIGHT - Self::FONT_SIZE) / 2.0;

        // Hover background
        if self.hovered {
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    Self::HEADER_HEIGHT,
                ))
                .with_background(theme::bg::MUTED.with_alpha(0.3)),
            );
        }

        // Status dot (colored circle based on status)
        let dot_size = 8.0;
        let dot_y = bounds.origin.y + (Self::HEADER_HEIGHT - dot_size) / 2.0;
        let dot_color = match self.status {
            ToolStatus::Pending => theme::text::MUTED,
            ToolStatus::Running => theme::status::WARNING,
            ToolStatus::Success => theme::status::SUCCESS,
            ToolStatus::Error => theme::status::ERROR,
            ToolStatus::Cancelled => theme::text::MUTED,
        };
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, dot_y, dot_size, dot_size))
                .with_background(dot_color)
                .with_corner_radius(dot_size / 2.0),
        );

        // Tool name with input in parentheses: "Read(path/to/file.rs)"
        let name_x = x + dot_size + 8.0;
        let display_name = if let Some(input) = &self.input {
            // Truncate input if too long
            let max_input_len = 60;
            let truncated_input = if input.len() > max_input_len {
                let mut end = max_input_len;
                while end > 0 && !input.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &input[..end])
            } else {
                input.clone()
            };
            format!("{}({})", self.tool_name, truncated_input)
        } else {
            self.tool_name.clone()
        };

        let name_run = cx.text.layout_styled_mono(
            &display_name,
            Point::new(name_x, y),
            Self::FONT_SIZE,
            theme::text::PRIMARY,
            FontStyle::default(),
        );
        cx.scene.draw_text(name_run);

        // Status text on right (only for running with elapsed time)
        if self.status == ToolStatus::Running {
            if let Some(elapsed) = self.elapsed_secs {
                let status_text = format!("{:.1}s", elapsed);
                let status_x = bounds.origin.x + bounds.size.width - 50.0;
                let status_run = cx.text.layout_styled_mono(
                    &status_text,
                    Point::new(status_x, y),
                    Self::DETAIL_FONT_SIZE,
                    theme::status::WARNING,
                    FontStyle::default(),
                );
                cx.scene.draw_text(status_run);
            }
        }

        // Track height used by expanded content
        let mut expanded_content_height = 0.0;

        // Expanded content: show output with tree branch prefix
        if self.expanded {
            let indent = 16.0;
            let detail_y = bounds.origin.y + Self::HEADER_HEIGHT + 2.0;

            if let Some(output) = &self.output {
                // Format as "└ Read 25 lines" or similar
                let output_display = format!("└ {}", output);
                let output_run = cx.text.layout_styled_mono(
                    &output_display,
                    Point::new(x + indent, detail_y),
                    Self::DETAIL_FONT_SIZE,
                    theme::text::MUTED,
                    FontStyle::default(),
                );
                cx.scene.draw_text(output_run);
                expanded_content_height += Self::LINE_HEIGHT;
            }
        }

        // Render child tools for Task cards (below header AND expanded content)
        if !self.child_tools.is_empty() {
            let children_start_y = bounds.origin.y + Self::HEADER_HEIGHT + expanded_content_height;
            let max_children_height = Self::MAX_VISIBLE_CHILDREN as f32 * Self::HEADER_HEIGHT;
            let total_children_height = self.child_tools.len() as f32 * Self::HEADER_HEIGHT;
            let visible_height = total_children_height.min(max_children_height);

            // Child tools container background
            let container_bounds = Bounds::new(
                bounds.origin.x + 16.0, // indent
                children_start_y,
                bounds.size.width - 16.0,
                visible_height,
            );
            cx.scene.draw_quad(
                Quad::new(container_bounds).with_background(theme::bg::MUTED.with_alpha(0.3)),
            );

            // Clip to container bounds
            cx.scene.push_clip(container_bounds);

            // Render visible children
            let max_scroll = (total_children_height - visible_height).max(0.0);
            let scroll = self.child_scroll.clamp(0.0, max_scroll);
            let mut child_y = children_start_y - scroll;

            for child in &self.child_tools {
                if child_y + Self::HEADER_HEIGHT >= children_start_y
                    && child_y < children_start_y + visible_height
                {
                    let child_x = container_bounds.origin.x + 4.0;
                    let child_text_y =
                        child_y + (Self::HEADER_HEIGHT - Self::DETAIL_FONT_SIZE) / 2.0;

                    // Status dot (colored circle based on status)
                    let dot_size = 6.0;
                    let dot_y = child_y + (Self::HEADER_HEIGHT - dot_size) / 2.0;
                    let dot_color = match child.status {
                        ToolStatus::Pending => theme::text::MUTED,
                        ToolStatus::Running => theme::status::WARNING,
                        ToolStatus::Success => theme::status::SUCCESS,
                        ToolStatus::Error => theme::status::ERROR,
                        ToolStatus::Cancelled => theme::text::MUTED,
                    };
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(child_x, dot_y, dot_size, dot_size))
                            .with_background(dot_color)
                            .with_corner_radius(dot_size / 2.0),
                    );

                    // Child name with params: "Read(path/to/file.rs)"
                    let name_x = child_x + dot_size + 6.0;
                    let params_available =
                        container_bounds.size.width - (name_x - container_bounds.origin.x) - 60.0;
                    let child_display = if !child.params.is_empty() && params_available > 50.0 {
                        let params_truncated = Self::truncate_text(
                            &child.params,
                            params_available - child.name.len() as f32 * Self::DETAIL_FONT_SIZE * 0.6 - 10.0,
                            Self::DETAIL_FONT_SIZE,
                        );
                        format!("{}({})", child.name, params_truncated)
                    } else {
                        child.name.clone()
                    };
                    let name_run = cx.text.layout_styled_mono(
                        &child_display,
                        Point::new(name_x, child_text_y),
                        Self::DETAIL_FONT_SIZE,
                        theme::text::SECONDARY,
                        FontStyle::default(),
                    );
                    cx.scene.draw_text(name_run);

                    // Elapsed time for running tools
                    if child.status == ToolStatus::Running {
                        if let Some(elapsed) = child.elapsed_secs {
                            let status_x = container_bounds.origin.x + container_bounds.size.width - 50.0;
                            let status_text = format!("{:.1}s", elapsed);
                            let status_run = cx.text.layout_styled_mono(
                                &status_text,
                                Point::new(status_x, child_text_y),
                                Self::DETAIL_FONT_SIZE,
                                theme::status::WARNING,
                                FontStyle::default(),
                            );
                            cx.scene.draw_text(status_run);
                        }
                    }
                }
                child_y += Self::HEADER_HEIGHT;
            }

            cx.scene.pop_clip();

            // Scrollbar if needed
            if total_children_height > visible_height {
                let scrollbar_height = visible_height * (visible_height / total_children_height);
                let scrollbar_y =
                    children_start_y + (scroll / total_children_height) * visible_height;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        bounds.origin.x + bounds.size.width - 6.0,
                        scrollbar_y,
                        4.0,
                        scrollbar_height,
                    ))
                    .with_background(theme::text::MUTED.with_alpha(0.5)),
                );
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            Self::HEADER_HEIGHT,
        );

        // Child tools scroll area bounds
        let children_start_y = bounds.origin.y + Self::HEADER_HEIGHT;
        let max_children_height = Self::MAX_VISIBLE_CHILDREN as f32 * Self::HEADER_HEIGHT;
        let total_children_height = self.child_tools.len() as f32 * Self::HEADER_HEIGHT;
        let visible_height = total_children_height.min(max_children_height);
        let children_bounds = Bounds::new(
            bounds.origin.x + 16.0,
            children_start_y,
            bounds.size.width - 16.0,
            visible_height,
        );

        match event {
            InputEvent::MouseMove { x, y } => {
                // Track mouse position for scroll hit-testing
                self.last_mouse_pos = Some(Point::new(*x, *y));

                let was_hovered = self.hovered;
                self.hovered = header_bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && header_bounds.contains(Point::new(*x, *y)) {
                    self.expanded = !self.expanded;
                    return EventResult::Handled;
                }
            }
            InputEvent::Scroll { dy, .. } => {
                // Only handle scroll if mouse is within children bounds
                if !self.child_tools.is_empty() {
                    if let Some(mouse_pos) = self.last_mouse_pos {
                        if children_bounds.contains(mouse_pos) {
                            let max_scroll = (total_children_height - visible_height).max(0.0);
                            let new_scroll = (self.child_scroll - dy).clamp(0.0, max_scroll);

                            // Disable auto-scroll if user scrolled up manually
                            if new_scroll < max_scroll {
                                self.auto_scroll = false;
                            } else {
                                // Re-enable auto-scroll if user scrolled to bottom
                                self.auto_scroll = true;
                            }

                            self.child_scroll = new_scroll;
                            return EventResult::Handled;
                        }
                    }
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let mut height = Self::HEADER_HEIGHT;

        // Add expanded content height
        if self.expanded {
            if self.input.is_some() {
                height += Self::LINE_HEIGHT;
            }
            if self.output.is_some() {
                height += Self::LINE_HEIGHT;
            }
        }

        // Add child tools height (capped at MAX_VISIBLE_CHILDREN)
        if !self.child_tools.is_empty() {
            let children_height = (self.child_tools.len().min(Self::MAX_VISIBLE_CHILDREN) as f32)
                * Self::HEADER_HEIGHT;
            height += children_height;
        }

        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_card_new() {
        let card = ToolCallCard::new(ToolType::Read, "read_file");
        assert_eq!(card.tool_name(), "read_file");
        assert!(!card.is_expanded()); // Default collapsed
    }

    #[test]
    fn test_tool_call_card_builder() {
        let card = ToolCallCard::new(ToolType::Write, "write_file")
            .with_id(1)
            .status(ToolStatus::Success)
            .input("path: /src/main.rs")
            .output("File written successfully")
            .expanded(true);

        assert_eq!(card.id, Some(1));
        assert_eq!(card.get_status(), ToolStatus::Success);
        assert!(card.is_expanded());
    }

    #[test]
    fn test_toggle_expanded() {
        let mut card = ToolCallCard::new(ToolType::Read, "read");
        assert!(!card.is_expanded()); // Starts collapsed
        card.toggle_expanded();
        assert!(card.is_expanded());
    }

    #[test]
    fn test_size_hint_compact() {
        let card = ToolCallCard::new(ToolType::Read, "read");
        let (_, height) = card.size_hint();
        assert_eq!(height, Some(ToolCallCard::HEADER_HEIGHT)); // Just header when collapsed
    }

    #[test]
    fn test_truncate_text() {
        let long = "This is a very long path that needs truncation";
        let truncated = ToolCallCard::truncate_text(long, 100.0, 11.0);
        assert!(truncated.len() < long.len());
        assert!(truncated.ends_with("..."));
    }
}
