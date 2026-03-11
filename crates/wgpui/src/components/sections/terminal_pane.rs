use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    title: String,
    lines: Vec<TerminalLine>,
    scroll_offset: f32,
    content_height: f32,
    auto_scroll: bool,
    max_lines: usize,
    line_height: f32,
    show_frame: bool,
    code_block_style: bool,
}

impl TerminalPane {
    pub fn new() -> Self {
        Self {
            id: None,
            title: "Terminal".to_string(),
            lines: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            auto_scroll: true,
            max_lines: 2000,
            line_height: 14.0,
            show_frame: true,
            code_block_style: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
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

    pub fn show_frame(mut self, show_frame: bool) -> Self {
        self.show_frame = show_frame;
        self
    }

    pub fn set_show_frame(&mut self, show_frame: bool) {
        self.show_frame = show_frame;
    }

    pub fn code_block_style(mut self, code_block_style: bool) -> Self {
        self.code_block_style = code_block_style;
        self
    }

    pub fn set_code_block_style(&mut self, code_block_style: bool) {
        self.code_block_style = code_block_style;
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
        if self.show_frame {
            cx.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
        }

        let header_height = if self.title.trim().is_empty() { 0.0 } else { 24.0 };
        if header_height > 0.0 {
            let header_bounds =
                Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, header_height);
            let header_text = cx.text.layout_mono(
                &self.title,
                Point::new(header_bounds.origin.x + 10.0, header_bounds.origin.y + 6.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(header_text);
        }

        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + header_height,
            bounds.size.width,
            bounds.size.height - header_height,
        );
        let mut text_bounds = content_bounds;
        if self.code_block_style {
            let code_bounds = Bounds::new(
                content_bounds.origin.x + 15.0,
                content_bounds.origin.y + 10.0,
                (content_bounds.size.width - 30.0).max(0.0),
                (content_bounds.size.height - 25.0).max(0.0),
            );
            cx.scene.draw_quad(
                Quad::new(code_bounds)
                    .with_background(Hsla::from_hex(0x0E0E0F))
                    .with_border(Hsla::from_hex(0x21242C), 1.0)
                    .with_corner_radius(8.0),
            );
            text_bounds = Bounds::new(
                code_bounds.origin.x + 15.0,
                code_bounds.origin.y + 15.0,
                (code_bounds.size.width - 30.0).max(0.0),
                (code_bounds.size.height - 30.0).max(0.0),
            );
        }

        self.content_height = self.lines.len() as f32 * self.line_height;
        let max_scroll = (self.content_height - text_bounds.size.height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene.push_clip(text_bounds);

        let mut y = text_bounds.origin.y - self.scroll_offset;
        let line_x = text_bounds.origin.x;

        for line in &self.lines {
            if y > text_bounds.origin.y + text_bounds.size.height {
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

        if self.content_height > text_bounds.size.height {
            let scrollbar_width = 5.0;
            let scrollbar_height =
                text_bounds.size.height * (text_bounds.size.height / self.content_height);
            let scrollbar_y = text_bounds.origin.y
                + (self.scroll_offset / self.content_height) * text_bounds.size.height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    text_bounds.origin.x + text_bounds.size.width - scrollbar_width - 2.0,
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
            let header_height = if self.title.trim().is_empty() { 0.0 } else { 24.0 };
            let content_height = if self.code_block_style {
                (bounds.size.height - header_height - 40.0).max(0.0)
            } else {
                bounds.size.height - header_height
            };
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
