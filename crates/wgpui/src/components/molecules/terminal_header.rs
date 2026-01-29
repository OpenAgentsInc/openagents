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
        let (badge_w, badge_h) = badge.size_hint();
        badge.paint(
            Bounds::new(
                x,
                bounds.origin.y,
                badge_w.unwrap_or(8.0),
                badge_h.unwrap_or(bounds.size.height),
            ),
            cx,
        );
        x += badge_w.unwrap_or(8.0) + theme::spacing::SM;

        let prompt = "$ ";
        let text_run = cx.text.layout_mono(
            prompt,
            Point::new(x, text_y),
            font_size,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(text_run);
        x += prompt.len() as f32 * font_size * 0.6;

        let text_run = cx.text.layout_mono(
            &self.command,
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
