use crate::components::atoms::{ToolStatus, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::ToolHeader;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub file: String,
    pub line: u32,
    pub content: String,
}

pub struct SearchToolCall {
    id: Option<ComponentId>,
    query: String,
    matches: Vec<SearchMatch>,
    status: ToolStatus,
    expanded: bool,
}

impl SearchToolCall {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            id: None,
            query: query.into(),
            matches: Vec::new(),
            status: ToolStatus::Pending,
            expanded: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn matches(mut self, matches: Vec<SearchMatch>) -> Self {
        self.matches = matches;
        self
    }

    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Default for SearchToolCall {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for SearchToolCall {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::SURFACE));

        let header_height = 32.0;
        let tool_name = format!("grep \"{}\"", self.query);
        let mut header = ToolHeader::new(ToolType::Search, &tool_name).status(self.status);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                header_height,
            ),
            cx,
        );

        let summary_y = bounds.origin.y + padding + header_height + 4.0;
        let summary = format!("{} matches found", self.matches.len());
        let mut summary_text = Text::new(&summary)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        summary_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                summary_y,
                bounds.size.width - padding * 2.0,
                16.0,
            ),
            cx,
        );

        if !self.expanded || self.matches.is_empty() {
            return;
        }

        let content_y = summary_y + 20.0;
        let content_width = bounds.size.width - padding * 2.0;
        let match_height = 42.0;

        for (i, m) in self.matches.iter().take(10).enumerate() {
            let y = content_y + i as f32 * match_height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    content_width,
                    match_height - 4.0,
                ))
                .with_background(theme::bg::MUTED),
            );

            let file_line = format!("{}:{}", m.file, m.line);
            let mut file_text = Text::new(&file_line)
                .font_size(theme::font_size::XS)
                .color(theme::accent::PRIMARY);
            file_text.paint(
                Bounds::new(
                    bounds.origin.x + padding + 8.0,
                    y + 4.0,
                    content_width - 16.0,
                    14.0,
                ),
                cx,
            );

            let mut content_text = Text::new(&m.content)
                .font_size(theme::font_size::XS)
                .color(theme::text::PRIMARY);
            content_text.paint(
                Bounds::new(
                    bounds.origin.x + padding + 8.0,
                    y + 18.0,
                    content_width - 16.0,
                    14.0,
                ),
                cx,
            );
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
        let base_height = 28.0 + 20.0 + theme::spacing::SM * 2.0;
        let matches_height = if self.expanded && !self.matches.is_empty() {
            (self.matches.len().min(10) as f32) * 36.0
        } else {
            0.0
        };
        (None, Some(base_height + matches_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_tool_call_new() {
        let search = SearchToolCall::new("TODO");
        assert_eq!(search.query(), "TODO");
        assert!(search.is_expanded());
    }

    #[test]
    fn test_search_tool_call_builder() {
        let matches = vec![
            SearchMatch {
                file: "main.rs".into(),
                line: 10,
                content: "TODO: fix this".into(),
            },
            SearchMatch {
                file: "lib.rs".into(),
                line: 20,
                content: "TODO: refactor".into(),
            },
        ];

        let search = SearchToolCall::new("TODO")
            .with_id(1)
            .matches(matches)
            .status(ToolStatus::Success);

        assert_eq!(search.id, Some(1));
        assert_eq!(search.match_count(), 2);
    }

    #[test]
    fn test_toggle_expanded() {
        let mut search = SearchToolCall::new("test");
        assert!(search.is_expanded());
        search.toggle_expanded();
        assert!(!search.is_expanded());
    }
}
