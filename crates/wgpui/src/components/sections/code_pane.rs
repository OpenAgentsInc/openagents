use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Clone, Debug)]
pub enum CodeLineKind {
    Add,
    Remove,
    Context,
}

#[derive(Clone, Debug)]
pub struct CodeLine {
    pub kind: CodeLineKind,
    pub text: String,
}

impl CodeLine {
    pub fn new(kind: CodeLineKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodeDiff {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub lines: Vec<CodeLine>,
}

impl CodeDiff {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            additions: 0,
            deletions: 0,
            lines: Vec::new(),
        }
    }

    pub fn additions(mut self, additions: usize) -> Self {
        self.additions = additions;
        self
    }

    pub fn deletions(mut self, deletions: usize) -> Self {
        self.deletions = deletions;
        self
    }

    pub fn lines(mut self, lines: Vec<CodeLine>) -> Self {
        self.lines = lines;
        self
    }
}

pub struct CodePane {
    id: Option<ComponentId>,
    diffs: Vec<CodeDiff>,
    scroll_offset: f32,
    content_height: f32,
    auto_scroll: bool,
    header_height: f32,
    line_height: f32,
}

impl CodePane {
    pub fn new() -> Self {
        Self {
            id: None,
            diffs: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            auto_scroll: true,
            header_height: 22.0,
            line_height: 14.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn set_diffs(&mut self, diffs: Vec<CodeDiff>) {
        self.diffs = diffs;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn push_diff(&mut self, diff: CodeDiff) {
        self.diffs.push(diff);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn clear(&mut self) {
        self.diffs.clear();
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
    }

    pub fn auto_scroll(mut self, auto: bool) -> Self {
        self.auto_scroll = auto;
        self
    }

    fn scroll_to_bottom(&mut self) {
        self.content_height = self.calculate_content_height();
        self.scroll_offset = self.content_height;
    }

    fn calculate_content_height(&self) -> f32 {
        let mut height = 0.0;
        for diff in &self.diffs {
            height += self.header_height;
            height += diff.lines.len() as f32 * self.line_height;
            height += theme::spacing::SM;
        }
        height
    }

    fn line_color(kind: &CodeLineKind) -> crate::Hsla {
        match kind {
            CodeLineKind::Add => theme::status::SUCCESS,
            CodeLineKind::Remove => theme::status::ERROR,
            CodeLineKind::Context => theme::text::PRIMARY,
        }
    }
}

impl Default for CodePane {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CodePane {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 24.0);
        let header_text = cx.text.layout(
            "Code",
            Point::new(header_bounds.origin.x + 10.0, header_bounds.origin.y + 6.0),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(header_text);

        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + header_bounds.size.height,
            bounds.size.width,
            bounds.size.height - header_bounds.size.height,
        );

        self.content_height = self.calculate_content_height();
        let max_scroll = (self.content_height - content_bounds.size.height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene.push_clip(content_bounds);

        let mut y = content_bounds.origin.y - self.scroll_offset;
        let line_x = content_bounds.origin.x + 10.0;

        for diff in &self.diffs {
            let header = format!("{} (+{}, -{})", diff.path, diff.additions, diff.deletions);
            let header_run = cx.text.layout(
                &header,
                Point::new(line_x, y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(header_run);
            y += self.header_height;

            for line in &diff.lines {
                if y > content_bounds.origin.y + content_bounds.size.height {
                    break;
                }
                let prefix = match line.kind {
                    CodeLineKind::Add => "+",
                    CodeLineKind::Remove => "-",
                    CodeLineKind::Context => " ",
                };
                let text = format!("{}{}", prefix, line.text);
                let run = cx.text.layout(
                    &text,
                    Point::new(line_x, y),
                    theme::font_size::XS,
                    Self::line_color(&line.kind),
                );
                cx.scene.draw_text(run);
                y += self.line_height;
            }

            y += theme::spacing::SM;
        }

        cx.scene.pop_clip();

        if self.content_height > content_bounds.size.height {
            let scrollbar_width = 5.0;
            let scrollbar_height =
                content_bounds.size.height * (content_bounds.size.height / self.content_height);
            let scrollbar_y = content_bounds.origin.y
                + (self.scroll_offset / self.content_height) * content_bounds.size.height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    content_bounds.origin.x + content_bounds.size.width - scrollbar_width - 2.0,
                    scrollbar_y,
                    scrollbar_width,
                    scrollbar_height.max(12.0),
                ))
                .with_background(theme::text::MUTED),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if let InputEvent::Scroll { dy, .. } = event {
            let header_height = 24.0;
            let content_height = bounds.size.height - header_height;
            let max_scroll = (self.content_height - content_height).max(0.0);
            self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max_scroll);
            return EventResult::Handled;
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}
