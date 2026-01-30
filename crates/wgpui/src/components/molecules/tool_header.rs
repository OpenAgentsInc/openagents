use crate::components::atoms::{ToolIcon, ToolStatus, ToolStatusBadge, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, theme};

pub struct ToolHeader {
    id: Option<ComponentId>,
    tool_type: ToolType,
    tool_name: String,
    status: ToolStatus,
    duration: Option<String>,
}

impl ToolHeader {
    fn truncate_label(text: &str, max_width: f32, font_size: f32) -> String {
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

    pub fn new(tool_type: ToolType, name: impl Into<String>) -> Self {
        Self {
            id: None,
            tool_type,
            tool_name: name.into(),
            status: ToolStatus::Pending,
            duration: None,
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

    pub fn duration(mut self, duration: impl Into<String>) -> Self {
        self.duration = Some(duration.into());
        self
    }

    pub fn tool_type(&self) -> ToolType {
        self.tool_type
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn get_status(&self) -> ToolStatus {
        self.status
    }

    pub fn set_status(&mut self, status: ToolStatus) {
        self.status = status;
    }
}

impl Default for ToolHeader {
    fn default() -> Self {
        Self::new(ToolType::Unknown, "unknown")
    }
}

impl Component for ToolHeader {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut x = bounds.origin.x;
        let icon_size = 20.0;

        let mut badge = ToolStatusBadge::new(self.status);
        let (badge_w, _badge_h) = badge.size_hint();
        badge.paint(
            Bounds::new(
                x,
                bounds.origin.y,
                badge_w.unwrap_or(8.0),
                bounds.size.height,
            ),
            cx,
        );
        x += badge_w.unwrap_or(8.0) + theme::spacing::SM;

        let mut icon = ToolIcon::new(self.tool_type).size(icon_size);
        icon.paint(
            Bounds::new(x, bounds.origin.y, icon_size, bounds.size.height),
            cx,
        );
        x += icon_size + theme::spacing::SM;

        let font_size = theme::font_size::XS;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;
        let mut available_width =
            (bounds.origin.x + bounds.size.width - theme::spacing::SM - x).max(0.0);
        if let Some(dur) = &self.duration {
            let dur_width = dur.len() as f32 * font_size * 0.6 + theme::spacing::SM;
            available_width = (available_width - dur_width).max(0.0);
        }
        let tool_name = Self::truncate_label(&self.tool_name, available_width, font_size);
        let text_run = cx.text.layout_mono(
            &tool_name,
            Point::new(x, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);
        x += tool_name.len() as f32 * font_size * 0.6 + theme::spacing::MD;

        if let Some(dur) = &self.duration {
            let text_run = cx.text.layout_mono(
                dur,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(text_run);
        }
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
        (None, Some(24.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_header_new() {
        let header = ToolHeader::new(ToolType::Bash, "bash");
        assert_eq!(header.tool_type(), ToolType::Bash);
        assert_eq!(header.tool_name(), "bash");
    }

    #[test]
    fn test_tool_header_builder() {
        let header = ToolHeader::new(ToolType::Read, "read")
            .with_id(1)
            .status(ToolStatus::Success)
            .duration("1.2s");

        assert_eq!(header.id, Some(1));
        assert_eq!(header.get_status(), ToolStatus::Success);
        assert_eq!(header.duration, Some("1.2s".to_string()));
    }

    #[test]
    fn test_set_status() {
        let mut header = ToolHeader::new(ToolType::Write, "write");
        header.set_status(ToolStatus::Running);
        assert_eq!(header.get_status(), ToolStatus::Running);
    }
}
