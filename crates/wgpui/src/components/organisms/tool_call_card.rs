use crate::components::atoms::{ToolStatus, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::ToolHeader;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

pub struct ToolCallCard {
    id: Option<ComponentId>,
    tool_type: ToolType,
    tool_name: String,
    status: ToolStatus,
    input: Option<String>,
    output: Option<String>,
    expanded: bool,
}

impl ToolCallCard {
    pub fn new(tool_type: ToolType, name: impl Into<String>) -> Self {
        Self {
            id: None,
            tool_type,
            tool_name: name.into(),
            status: ToolStatus::Pending,
            input: None,
            output: None,
            expanded: true,
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
}

impl Default for ToolCallCard {
    fn default() -> Self {
        Self::new(ToolType::Read, "read")
    }
}

impl Component for ToolCallCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_height = 28.0;
        let mut header = ToolHeader::new(self.tool_type, &self.tool_name).status(self.status);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                header_height,
            ),
            cx,
        );

        if !self.expanded {
            return;
        }

        let content_y = bounds.origin.y + padding + header_height + theme::spacing::XS;
        let content_width = bounds.size.width - padding * 2.0;
        let mut y = content_y;

        if let Some(input) = &self.input {
            let label_height = 16.0;
            let mut label = Text::new("Input:")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            label.paint(
                Bounds::new(bounds.origin.x + padding, y, content_width, label_height),
                cx,
            );
            y += label_height + 4.0;

            let input_height = 40.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    content_width,
                    input_height,
                ))
                .with_background(theme::bg::MUTED),
            );
            let mut input_text = Text::new(input)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            input_text.paint(
                Bounds::new(bounds.origin.x + padding + 8.0, y + 8.0, content_width - 16.0, input_height - 16.0),
                cx,
            );
            y += input_height + theme::spacing::SM;
        }

        if let Some(output) = &self.output {
            let label_height = 16.0;
            let mut label = Text::new("Output:")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            label.paint(
                Bounds::new(bounds.origin.x + padding, y, content_width, label_height),
                cx,
            );
            y += label_height + 4.0;

            let output_height = 40.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    content_width,
                    output_height,
                ))
                .with_background(theme::bg::MUTED),
            );
            let mut output_text = Text::new(output)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            output_text.paint(
                Bounds::new(bounds.origin.x + padding + 8.0, y + 8.0, content_width - 16.0, output_height - 16.0),
                cx,
            );
        }
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = if self.expanded {
            let base = 28.0 + theme::spacing::SM * 2.0;
            let input_height = if self.input.is_some() { 60.0 + theme::spacing::SM } else { 0.0 };
            let output_height = if self.output.is_some() { 60.0 } else { 0.0 };
            base + input_height + output_height
        } else {
            28.0 + theme::spacing::SM * 2.0
        };
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
        assert!(card.is_expanded());
    }

    #[test]
    fn test_tool_call_card_builder() {
        let card = ToolCallCard::new(ToolType::Write, "write_file")
            .with_id(1)
            .status(ToolStatus::Success)
            .input("path: /src/main.rs")
            .output("File written successfully")
            .expanded(false);

        assert_eq!(card.id, Some(1));
        assert_eq!(card.get_status(), ToolStatus::Success);
        assert!(!card.is_expanded());
    }

    #[test]
    fn test_toggle_expanded() {
        let mut card = ToolCallCard::new(ToolType::Read, "read");
        assert!(card.is_expanded());
        card.toggle_expanded();
        assert!(!card.is_expanded());
    }
}
