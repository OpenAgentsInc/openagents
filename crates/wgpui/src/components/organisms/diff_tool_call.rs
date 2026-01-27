use crate::components::atoms::ToolStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::{DiffHeader, DiffType};
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    Header,
}

pub struct DiffToolCall {
    id: Option<ComponentId>,
    file_path: String,
    lines: Vec<DiffLine>,
    additions: usize,
    deletions: usize,
    status: ToolStatus,
    diff_type: DiffType,
    expanded: bool,
}

impl DiffToolCall {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            id: None,
            file_path: file_path.into(),
            lines: Vec::new(),
            additions: 0,
            deletions: 0,
            status: ToolStatus::Pending,
            diff_type: DiffType::Unified,
            expanded: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn lines(mut self, lines: Vec<DiffLine>) -> Self {
        self.additions = lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Addition)
            .count();
        self.deletions = lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Deletion)
            .count();
        self.lines = lines;
        self
    }

    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    pub fn diff_type(mut self, diff_type: DiffType) -> Self {
        self.diff_type = diff_type;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    pub fn get_additions(&self) -> usize {
        self.additions
    }

    pub fn get_deletions(&self) -> usize {
        self.deletions
    }
}

impl Default for DiffToolCall {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for DiffToolCall {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_height = 32.0;
        let mut header = DiffHeader::new(&self.file_path)
            .additions(self.additions)
            .deletions(self.deletions)
            .diff_type(self.diff_type);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                header_height,
            ),
            cx,
        );

        if !self.expanded || self.lines.is_empty() {
            return;
        }

        let content_y = bounds.origin.y + padding + header_height + theme::spacing::XS;
        let content_width = bounds.size.width - padding * 2.0;
        let line_height = 20.0;
        let (old_width, new_width) = diff_line_number_widths(&self.lines);
        let show_numbers = old_width > 0 || new_width > 0;
        let old_width = old_width.max(1);
        let new_width = new_width.max(1);

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + padding,
                content_y,
                content_width,
                self.lines.len() as f32 * line_height + 8.0,
            ))
            .with_background(theme::bg::MUTED),
        );

        for (i, line) in self.lines.iter().enumerate() {
            let y = content_y + 4.0 + i as f32 * line_height;

            let (prefix, color) = match line.kind {
                DiffLineKind::Addition => ("+", theme::status::SUCCESS),
                DiffLineKind::Deletion => ("-", theme::status::ERROR),
                DiffLineKind::Context => (" ", theme::text::MUTED),
                DiffLineKind::Header => ("", theme::accent::PRIMARY),
            };
            let display = if line.kind == DiffLineKind::Header {
                line.content.clone()
            } else if show_numbers {
                let old_text = line.old_line.map(|n| n.to_string()).unwrap_or_default();
                let new_text = line.new_line.map(|n| n.to_string()).unwrap_or_default();
                format!(
                    "{:>old_width$} {:>new_width$} {} {}",
                    old_text,
                    new_text,
                    prefix,
                    line.content,
                    old_width = old_width,
                    new_width = new_width
                )
            } else {
                format!("{} {}", prefix, line.content)
            };
            let mut text = Text::new(&display)
                .font_size(theme::font_size::BASE)
                .color(color);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding + 8.0,
                    y,
                    content_width - 16.0,
                    line_height,
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
        let base_height = 28.0 + theme::spacing::SM * 2.0;
        let content_height = if self.expanded && !self.lines.is_empty() {
            self.lines.len() as f32 * 18.0 + 8.0 + theme::spacing::XS
        } else {
            0.0
        };
        (None, Some(base_height + content_height))
    }
}

fn diff_line_number_widths(lines: &[DiffLine]) -> (usize, usize) {
    let old_max = lines.iter().filter_map(|line| line.old_line).max();
    let new_max = lines.iter().filter_map(|line| line.new_line).max();
    let old_width = old_max.map(|value| value.to_string().len()).unwrap_or(0);
    let new_width = new_max.map(|value| value.to_string().len()).unwrap_or(0);
    (old_width, new_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_tool_call_new() {
        let diff = DiffToolCall::new("src/main.rs");
        assert_eq!(diff.file_path(), "src/main.rs");
        assert!(diff.is_expanded());
    }

    #[test]
    fn test_diff_tool_call_builder() {
        let lines = vec![
            DiffLine {
                kind: DiffLineKind::Addition,
                content: "new line".into(),
                old_line: None,
                new_line: Some(1),
            },
            DiffLine {
                kind: DiffLineKind::Deletion,
                content: "old line".into(),
                old_line: Some(1),
                new_line: None,
            },
        ];

        let diff = DiffToolCall::new("test.rs")
            .with_id(1)
            .lines(lines)
            .status(ToolStatus::Success);

        assert_eq!(diff.id, Some(1));
        assert_eq!(diff.get_additions(), 1);
        assert_eq!(diff.get_deletions(), 1);
    }

    #[test]
    fn test_toggle_expanded() {
        let mut diff = DiffToolCall::new("file.rs");
        assert!(diff.is_expanded());
        diff.toggle_expanded();
        assert!(!diff.is_expanded());
    }
}
