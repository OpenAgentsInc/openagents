use crate::components::atoms::{ToolStatus, ToolStatusBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct TerminalHeader {
    id: Option<ComponentId>,
    command: String,
    working_dir: Option<String>,
    status: ToolStatus,
    exit_code: Option<i32>,
}

impl TerminalHeader {
    fn truncate_command(text: &str, max_width: f32, font_size: f32) -> String {
        if max_width <= 0.0 {
            return String::new();
        }
        let char_width = font_size * 0.6;
        let max_chars = (max_width / char_width).floor() as usize;
        if text.len() <= max_chars {
            return text.to_string();
        }
        if max_chars <= 3 {
            return "...".to_string();
        }
        let target = max_chars - 3;
        let mut end = target.min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &text[..end])
    }

    pub fn new(command: impl Into<String>) -> Self {
        Self {
            id: None,
            command: command.into(),
            working_dir: None,
            status: ToolStatus::Pending,
            exit_code: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    pub fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn get_status(&self) -> ToolStatus {
        self.status
    }
}

impl Default for TerminalHeader {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for TerminalHeader {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::CODE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;
        let mut x = bounds.origin.x + padding;
        let font_size = theme::font_size::XS;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;

        let mut badge = ToolStatusBadge::new(self.status).show_icon(false);
        let (badge_w, _badge_h) = badge.size_hint();
        let badge_width = badge_w.unwrap_or(8.0);
        badge.paint(
            Bounds::new(x, bounds.origin.y, badge_width, bounds.size.height),
            cx,
        );
        x += badge_width + theme::spacing::SM;

        let prompt = "$ ";
        let text_run = cx.text.layout_mono(
            prompt,
            Point::new(x, text_y),
            font_size,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(text_run);
        x += prompt.len() as f32 * font_size * 0.6;

        let available_width = (bounds.origin.x + bounds.size.width - padding - x).max(0.0);
        let command = Self::truncate_command(&self.command, available_width, font_size);
        let text_run = cx.text.layout_mono(
            &command,
            Point::new(x, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);

        // Status dot is rendered at the left edge; no trailing status label.
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(32.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_header_new() {
        let header = TerminalHeader::new("ls -la");
        assert_eq!(header.command(), "ls -la");
    }

    #[test]
    fn test_terminal_header_builder() {
        let header = TerminalHeader::new("cargo build")
            .with_id(1)
            .working_dir("/home/user/project")
            .status(ToolStatus::Success)
            .exit_code(0);

        assert_eq!(header.id, Some(1));
        assert_eq!(header.working_dir, Some("/home/user/project".to_string()));
        assert_eq!(header.exit_code, Some(0));
    }
}
