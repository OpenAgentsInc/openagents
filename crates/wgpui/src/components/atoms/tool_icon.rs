use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolType {
    #[default]
    Read,
    Write,
    Edit,
    Bash,
    Search,
    Glob,
    Grep,
    List,
    Task,
    WebFetch,
    Unknown,
}

impl ToolType {
    /// Get the single-character icon for this tool type
    pub fn icon(&self) -> &'static str {
        match self {
            ToolType::Read => "R",
            ToolType::Write => "W",
            ToolType::Edit => "E",
            ToolType::Bash => "$",
            ToolType::Search => "?",
            ToolType::Glob => "*",
            ToolType::Grep => "G",
            ToolType::List => "L",
            ToolType::Task => "T",
            ToolType::WebFetch => "@",
            ToolType::Unknown => "?",
        }
    }

    /// Get the accent color for this tool type
    pub fn color(&self) -> Hsla {
        match self {
            ToolType::Read => theme::accent::PRIMARY,
            ToolType::Write => theme::status::SUCCESS,
            ToolType::Edit => theme::status::WARNING,
            ToolType::Bash => theme::accent::SECONDARY,
            ToolType::Search | ToolType::Glob | ToolType::Grep => theme::accent::PRIMARY,
            ToolType::List => theme::text::MUTED,
            ToolType::Task => theme::accent::PURPLE,
            ToolType::WebFetch => theme::accent::PRIMARY,
            ToolType::Unknown => theme::text::MUTED,
        }
    }
}

pub struct ToolIcon {
    id: Option<ComponentId>,
    tool_type: ToolType,
    size: f32,
    show_background: bool,
}

impl ToolIcon {
    pub fn new(tool_type: ToolType) -> Self {
        Self {
            id: None,
            tool_type,
            size: 20.0,
            show_background: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn show_background(mut self, show: bool) -> Self {
        self.show_background = show;
        self
    }

    pub fn tool_type(&self) -> ToolType {
        self.tool_type
    }
}

impl Default for ToolIcon {
    fn default() -> Self {
        Self::new(ToolType::default())
    }
}

impl Component for ToolIcon {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let icon_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - self.size) / 2.0,
            bounds.origin.y + (bounds.size.height - self.size) / 2.0,
            self.size,
            self.size,
        );

        if self.show_background {
            cx.scene.draw_quad(
                Quad::new(icon_bounds)
                    .with_background(theme::bg::MUTED)
                    .with_border(self.tool_type.color(), 1.0),
            );
        }

        let font_size = self.size * 0.6;
        let text_x = icon_bounds.origin.x + (self.size - font_size * 0.6) / 2.0;
        let text_y = icon_bounds.origin.y + self.size * 0.5 - font_size * 0.55;

        let text_run = cx.text.layout(
            self.tool_type.icon(),
            crate::Point::new(text_x, text_y),
            font_size,
            self.tool_type.color(),
        );
        cx.scene.draw_text(text_run);
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(self.size), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_icon_new() {
        let icon = ToolIcon::new(ToolType::Bash);
        assert_eq!(icon.tool_type(), ToolType::Bash);
    }

    #[test]
    fn test_tool_icon_builder() {
        let icon = ToolIcon::new(ToolType::Read)
            .with_id(1)
            .size(24.0)
            .show_background(false);

        assert_eq!(icon.id, Some(1));
        assert_eq!(icon.size, 24.0);
        assert!(!icon.show_background);
    }

    #[test]
    fn test_tool_type_icons() {
        assert_eq!(ToolType::Bash.icon(), "$");
        assert_eq!(ToolType::Read.icon(), "R");
        assert_eq!(ToolType::Write.icon(), "W");
    }
}
