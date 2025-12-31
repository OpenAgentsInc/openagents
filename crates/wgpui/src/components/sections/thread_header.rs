use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

pub struct ThreadHeader {
    id: Option<ComponentId>,
    title: String,
    subtitle: Option<String>,
    show_back_button: bool,
    show_menu_button: bool,
    back_hovered: bool,
    menu_hovered: bool,
    on_back: Option<Box<dyn FnMut()>>,
    on_menu: Option<Box<dyn FnMut()>>,
}

impl ThreadHeader {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            title: title.into(),
            subtitle: None,
            show_back_button: true,
            show_menu_button: true,
            back_hovered: false,
            menu_hovered: false,
            on_back: None,
            on_menu: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn show_back_button(mut self, show: bool) -> Self {
        self.show_back_button = show;
        self
    }

    pub fn show_menu_button(mut self, show: bool) -> Self {
        self.show_menu_button = show;
        self
    }

    pub fn on_back<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_back = Some(Box::new(f));
        self
    }

    pub fn on_menu<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_menu = Some(Box::new(f));
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    fn back_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_size = 32.0;
        let padding = theme::spacing::SM;
        Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + (bounds.size.height - btn_size) / 2.0,
            btn_size,
            btn_size,
        )
    }

    fn menu_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_size = 32.0;
        let padding = theme::spacing::SM;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_size,
            bounds.origin.y + (bounds.size.height - btn_size) / 2.0,
            btn_size,
            btn_size,
        )
    }
}

impl Default for ThreadHeader {
    fn default() -> Self {
        Self::new("Conversation")
    }
}

impl Component for ThreadHeader {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;
        let btn_size = 32.0;
        let mut content_x = bounds.origin.x + padding;

        if self.show_back_button {
            let back_bounds = self.back_button_bounds(&bounds);
            let bg = if self.back_hovered {
                theme::bg::HOVER
            } else {
                Hsla::transparent()
            };

            cx.scene
                .draw_quad(Quad::new(back_bounds).with_background(bg));

            let arrow = "\u{2190}";
            let arrow_size = theme::font_size::LG;
            let arrow_run = cx.text.layout(
                arrow,
                Point::new(
                    back_bounds.origin.x + (btn_size - arrow_size * 0.6) / 2.0,
                    back_bounds.origin.y + (btn_size - arrow_size) / 2.0,
                ),
                arrow_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(arrow_run);

            content_x += btn_size + padding;
        }

        let _content_end_x = if self.show_menu_button {
            bounds.origin.x + bounds.size.width - padding - btn_size - padding
        } else {
            bounds.origin.x + bounds.size.width - padding
        };

        let title_font_size = theme::font_size::LG;
        let title_y = if self.subtitle.is_some() {
            bounds.origin.y + bounds.size.height * 0.35 - title_font_size * 0.35
        } else {
            bounds.origin.y + bounds.size.height * 0.5 - title_font_size * 0.35
        };

        let title_run = cx.text.layout(
            &self.title,
            Point::new(content_x, title_y),
            title_font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        if let Some(subtitle) = &self.subtitle {
            let subtitle_font_size = theme::font_size::XS;
            let subtitle_y =
                bounds.origin.y + bounds.size.height * 0.65 - subtitle_font_size * 0.35;

            let subtitle_run = cx.text.layout(
                subtitle,
                Point::new(content_x, subtitle_y),
                subtitle_font_size,
                theme::text::MUTED,
            );
            cx.scene.draw_text(subtitle_run);
        }

        if self.show_menu_button {
            let menu_bounds = self.menu_button_bounds(&bounds);
            let bg = if self.menu_hovered {
                theme::bg::HOVER
            } else {
                Hsla::transparent()
            };

            cx.scene
                .draw_quad(Quad::new(menu_bounds).with_background(bg));

            let dots = "\u{22EE}";
            let dots_size = theme::font_size::LG;
            let dots_run = cx.text.layout(
                dots,
                Point::new(
                    menu_bounds.origin.x + (btn_size - dots_size * 0.4) / 2.0,
                    menu_bounds.origin.y + (btn_size - dots_size) / 2.0,
                ),
                dots_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(dots_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_back_hovered = self.back_hovered;
                let was_menu_hovered = self.menu_hovered;

                self.back_hovered =
                    self.show_back_button && self.back_button_bounds(&bounds).contains(point);
                self.menu_hovered =
                    self.show_menu_button && self.menu_button_bounds(&bounds).contains(point);

                if was_back_hovered != self.back_hovered || was_menu_hovered != self.menu_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);

                if self.show_back_button && self.back_button_bounds(&bounds).contains(point) {
                    if let Some(callback) = &mut self.on_back {
                        callback();
                    }
                    return EventResult::Handled;
                }

                if self.show_menu_button && self.menu_button_bounds(&bounds).contains(point) {
                    if let Some(callback) = &mut self.on_menu {
                        callback();
                    }
                    return EventResult::Handled;
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
        (None, Some(48.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_header_new() {
        let header = ThreadHeader::new("Test Thread");
        assert_eq!(header.title(), "Test Thread");
        assert!(header.show_back_button);
        assert!(header.show_menu_button);
    }

    #[test]
    fn test_thread_header_builder() {
        let header = ThreadHeader::new("Main")
            .with_id(1)
            .subtitle("3 messages")
            .show_back_button(false)
            .show_menu_button(true);

        assert_eq!(header.id, Some(1));
        assert_eq!(header.subtitle, Some("3 messages".to_string()));
        assert!(!header.show_back_button);
        assert!(header.show_menu_button);
    }

    #[test]
    fn test_thread_header_default() {
        let header = ThreadHeader::default();
        assert_eq!(header.title(), "Conversation");
    }

    #[test]
    fn test_thread_header_set_title() {
        let mut header = ThreadHeader::new("Old");
        header.set_title("New");
        assert_eq!(header.title(), "New");
    }

    #[test]
    fn test_thread_header_size_hint() {
        let header = ThreadHeader::new("Test");
        let (w, h) = header.size_hint();
        assert!(w.is_none());
        assert_eq!(h, Some(48.0));
    }
}
