use std::time::Duration;

use web_time::Instant;

use crate::clipboard::copy_to_clipboard;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::markdown::{MarkdownDocument, MarkdownLayout, MarkdownRenderer};
use crate::{Bounds, Cursor, InputEvent, MouseButton, Point, Quad, Size, TextSystem, theme};

pub struct MarkdownView {
    id: Option<ComponentId>,
    document: MarkdownDocument,
    renderer: MarkdownRenderer,
    layout: MarkdownLayout,
    hovered_block: Option<usize>,
    hovered_copy: Option<usize>,
    copied_block: Option<usize>,
    copied_at: Option<Instant>,
    copy_feedback_duration: Duration,
    copy_button_on_hover: bool,
    show_copy_button: bool,
    on_copy: Option<Box<dyn FnMut(String)>>,
    /// Cached height from last render (used for size_hint)
    last_rendered_height: f32,
    /// Width used for last render (to detect when re-measure is needed)
    last_rendered_width: f32,
}

impl MarkdownView {
    pub fn new(document: MarkdownDocument) -> Self {
        Self {
            id: None,
            document,
            renderer: MarkdownRenderer::new(),
            layout: MarkdownLayout::default(),
            hovered_block: None,
            hovered_copy: None,
            copied_block: None,
            copied_at: None,
            copy_feedback_duration: Duration::from_millis(1200),
            copy_button_on_hover: true,
            show_copy_button: true,
            on_copy: None,
            last_rendered_height: 0.0,
            last_rendered_width: 0.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn set_document(&mut self, document: MarkdownDocument) {
        self.document = document;
    }

    pub fn measure(&self, max_width: f32, text_system: &mut TextSystem) -> Size {
        self.renderer
            .measure(&self.document, max_width, text_system)
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    pub fn copy_button_on_hover(mut self, on_hover: bool) -> Self {
        self.copy_button_on_hover = on_hover;
        self
    }

    pub fn show_copy_button(mut self, show: bool) -> Self {
        self.show_copy_button = show;
        self
    }

    pub fn clear_hover(&mut self) {
        self.hovered_block = None;
        self.hovered_copy = None;
    }

    pub fn cursor(&self) -> Cursor {
        if self.hovered_copy.is_some() {
            Cursor::Pointer
        } else {
            Cursor::Default
        }
    }

    fn copy_label(&self, index: usize) -> &'static str {
        if self.is_copy_feedback(index) {
            "Copied"
        } else {
            "Copy"
        }
    }

    fn is_copy_feedback(&self, index: usize) -> bool {
        if self.copied_block != Some(index) {
            return false;
        }
        let Some(copied_at) = self.copied_at else {
            return false;
        };
        Instant::now().duration_since(copied_at) < self.copy_feedback_duration
    }

    fn clear_expired_copy(&mut self) {
        if let Some(copied_at) = self.copied_at {
            if Instant::now().duration_since(copied_at) >= self.copy_feedback_duration {
                self.copied_at = None;
                self.copied_block = None;
            }
        }
    }

    fn update_copy_bounds(&mut self, text_system: &mut TextSystem) {
        let font_size = theme::font_size::XS;
        let padding_x = theme::spacing::XS;
        let padding_y = theme::spacing::HALF;
        let now = Instant::now();
        let copied_block = self.copied_block;
        let copied_at = self.copied_at;
        let feedback_duration = self.copy_feedback_duration;

        for (index, block) in self.layout.code_blocks.iter_mut().enumerate() {
            let is_copied = copied_block == Some(index)
                && copied_at
                    .map(|t| now.duration_since(t) < feedback_duration)
                    .unwrap_or(false);
            let label = if is_copied { "Copied" } else { "Copy" };
            let text_width = text_system.measure(label, font_size);
            let button_width = text_width + padding_x * 2.0;
            let button_height = font_size + padding_y * 2.0;

            let x = block.header_bounds.origin.x + block.header_bounds.size.width - padding_x - button_width;
            let y =
                block.header_bounds.origin.y + (block.header_bounds.size.height - button_height) * 0.5;

            block.copy_bounds = Some(Bounds::new(x, y, button_width, button_height));
        }
    }

    fn draw_copy_buttons(&self, cx: &mut PaintContext) {
        if !self.show_copy_button {
            return;
        }

        let font_size = theme::font_size::XS;
        let padding_x = theme::spacing::XS;
        let text_y_offset = font_size * 0.55;

        for (index, block) in self.layout.code_blocks.iter().enumerate() {
            let show = !self.copy_button_on_hover
                || self.hovered_block == Some(index)
                || self.hovered_copy == Some(index)
                || self.is_copy_feedback(index);

            if !show {
                continue;
            }

            let Some(bounds) = block.copy_bounds else {
                continue;
            };

            let is_hovered = self.hovered_copy == Some(index);
            let is_copied = self.is_copy_feedback(index);

            let bg_color = if is_copied {
                theme::status::SUCCESS.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };

            let border_color = if is_hovered {
                theme::accent::PRIMARY.with_alpha(0.7)
            } else {
                theme::border::DEFAULT
            };

            cx.scene
                .draw_quad(Quad::new(bounds).with_background(bg_color).with_border(border_color, 1.0));

            let text_color = if is_copied {
                theme::status::SUCCESS
            } else if is_hovered {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            };

            let label = self.copy_label(index);
            let text_x = bounds.origin.x + padding_x;
            let text_y = bounds.origin.y + bounds.size.height * 0.5 - text_y_offset;
            let label_run = cx
                .text
                .layout(label, Point::new(text_x, text_y), font_size, text_color);
            cx.scene.draw_text(label_run);
        }
    }

    fn hit_copy_button(&self, point: Point) -> Option<usize> {
        for (index, block) in self.layout.code_blocks.iter().enumerate() {
            if let Some(bounds) = block.copy_bounds {
                if bounds.contains(point) {
                    return Some(index);
                }
            }
        }
        None
    }

    fn hit_code_block(&self, point: Point) -> Option<usize> {
        for (index, block) in self.layout.code_blocks.iter().enumerate() {
            if block.bounds.contains(point) {
                return Some(index);
            }
        }
        None
    }

    fn copy_code(&mut self, code: String) {
        if let Some(handler) = &mut self.on_copy {
            handler(code);
        } else {
            let _ = copy_to_clipboard(&code);
        }
    }
}

impl Component for MarkdownView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.clear_expired_copy();

        self.layout = self.renderer.render_with_layout(
            &self.document,
            bounds.origin,
            bounds.size.width,
            cx.text,
            cx.scene,
        );

        // Cache rendered dimensions for size_hint
        self.last_rendered_width = bounds.size.width;
        self.last_rendered_height = self.layout.size.height;

        self.update_copy_bounds(cx.text);
        self.draw_copy_buttons(cx);
    }

    fn event(&mut self, event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let new_hover_block = self.hit_code_block(point);
                let new_hover_copy = self.hit_copy_button(point);

                if new_hover_block != self.hovered_block || new_hover_copy != self.hovered_copy {
                    self.hovered_block = new_hover_block;
                    self.hovered_copy = new_hover_copy;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && self.show_copy_button {
                    let point = Point::new(*x, *y);
                    if let Some(index) = self.hit_copy_button(point) {
                        if let Some(code) = self.layout.code_blocks.get(index).map(|b| b.code.clone()) {
                            self.copy_code(code);
                            self.copied_block = Some(index);
                            self.copied_at = Some(Instant::now());
                            return EventResult::Handled;
                        }
                    }
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Return cached height if we've rendered at least once
        if self.last_rendered_height > 0.0 {
            (None, Some(self.last_rendered_height))
        } else {
            // Conservative estimate: ~24px per block (single line height)
            // More blocks = more lines, but start small to avoid wasted space
            let estimated_height = (self.document.blocks.len() as f32 * 24.0).max(24.0);
            (None, Some(estimated_height))
        }
    }
}
