use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Clone, Debug)]
pub enum TerminalStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub stream: TerminalStream,
    pub text: String,
}

impl TerminalLine {
    pub fn new(stream: TerminalStream, text: impl Into<String>) -> Self {
        Self {
            stream,
            text: text.into(),
        }
    }
}

pub struct TerminalPane {
    id: Option<ComponentId>,
    lines: Vec<TerminalLine>,
    scroll_offset: f32,
    content_height: f32,
    auto_scroll: bool,
    max_lines: usize,
    line_height: f32,
}

impl TerminalPane {
    pub fn new() -> Self {
        Self {
            id: None,
            lines: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            auto_scroll: true,
            max_lines: 2000,
            line_height: 14.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn push_line(&mut self, line: TerminalLine) {
        self.lines.push(line);
        if self.lines.len() > self.max_lines {
            let drop = self.lines.len() - self.max_lines;
            self.lines.drain(0..drop);
        }
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
    }

    pub fn auto_scroll(mut self, auto: bool) -> Self {
        self.auto_scroll = auto;
        self
    }

    fn scroll_to_bottom(&mut self) {
        self.content_height = self.lines.len() as f32 * self.line_height;
        self.scroll_offset = self.content_height;
    }

    fn line_color(stream: &TerminalStream) -> crate::Hsla {
        match stream {
            TerminalStream::Stdout => theme::text::PRIMARY,
            TerminalStream::Stderr => theme::status::ERROR,
        }
    }
}

impl Default for TerminalPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TerminalPane {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 24.0);
        let header_text = cx.text.layout_mono(
            "Terminal",
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

        self.content_height = self.lines.len() as f32 * self.line_height;
        let max_scroll = (self.content_height - content_bounds.size.height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene.push_clip(content_bounds);

        let mut y = content_bounds.origin.y - self.scroll_offset;
        let line_x = content_bounds.origin.x + 10.0;

        for line in &self.lines {
            if y > content_bounds.origin.y + content_bounds.size.height {
                break;
            }
            let run = cx.text.layout_mono(
                &line.text,
                Point::new(line_x, y),
                theme::font_size::XS,
                Self::line_color(&line.stream),
            );
            cx.scene.draw_text(run);
            y += self.line_height;
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
