use crate::components::atoms::ToolStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::TerminalHeader;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

pub struct TerminalToolCall {
    id: Option<ComponentId>,
    command: String,
    output: String,
    status: ToolStatus,
    exit_code: Option<i32>,
    expanded: bool,
}

impl TerminalToolCall {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            id: None,
            command: command.into(),
            output: String::new(),
            status: ToolStatus::Pending,
            exit_code: None,
            expanded: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = output.into();
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

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn append_output(&mut self, text: &str) {
        self.output.push_str(text);
    }

    pub fn set_complete(&mut self, exit_code: i32) {
        self.exit_code = Some(exit_code);
        self.status = if exit_code == 0 {
            ToolStatus::Success
        } else {
            ToolStatus::Error
        };
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn get_output(&self) -> &str {
        &self.output
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Default for TerminalToolCall {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for TerminalToolCall {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::APP)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_height = 28.0;
        let mut header = TerminalHeader::new(&self.command).status(self.status);
        if let Some(code) = self.exit_code {
            header = header.exit_code(code);
        }
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                header_height,
            ),
            cx,
        );

        if !self.expanded || self.output.is_empty() {
            return;
        }

        let output_y = bounds.origin.y + padding + header_height + theme::spacing::XS;
        let output_height = bounds.size.height - padding * 2.0 - header_height - theme::spacing::XS;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + padding,
                output_y,
                bounds.size.width - padding * 2.0,
                output_height,
            ))
            .with_background(theme::bg::MUTED),
        );

        let mut output_text = Text::new(&self.output)
            .font_size(theme::font_size::SM)
            .color(theme::text::SECONDARY);
        output_text.paint(
            Bounds::new(
                bounds.origin.x + padding + 8.0,
                output_y + 8.0,
                bounds.size.width - padding * 2.0 - 16.0,
                output_height - 16.0,
            ),
            cx,
        );
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
        let base_height = 28.0 + theme::spacing::SM * 2.0;
        let output_height = if self.expanded && !self.output.is_empty() {
            80.0 + theme::spacing::XS
        } else {
            0.0
        };
        (None, Some(base_height + output_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_tool_call_new() {
        let tool = TerminalToolCall::new("ls -la");
        assert_eq!(tool.command(), "ls -la");
        assert!(tool.is_expanded());
    }

    #[test]
    fn test_terminal_tool_call_builder() {
        let tool = TerminalToolCall::new("cargo build")
            .with_id(1)
            .output("Compiling...")
            .status(ToolStatus::Running)
            .exit_code(0)
            .expanded(false);

        assert_eq!(tool.id, Some(1));
        assert_eq!(tool.get_output(), "Compiling...");
        assert!(!tool.is_expanded());
    }

    #[test]
    fn test_append_output() {
        let mut tool = TerminalToolCall::new("echo test");
        tool.append_output("test\n");
        assert_eq!(tool.get_output(), "test\n");
    }

    #[test]
    fn test_set_complete() {
        let mut tool = TerminalToolCall::new("exit 1");
        tool.set_complete(1);
        assert_eq!(tool.status, ToolStatus::Error);
        assert_eq!(tool.exit_code, Some(1));
    }
}
