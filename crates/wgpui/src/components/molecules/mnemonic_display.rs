//! Mnemonic display molecule for showing seed phrases.
//!
//! Displays BIP-39 mnemonic words in a grid with optional blur and copy button.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Mnemonic display component
pub struct MnemonicDisplay {
    id: Option<ComponentId>,
    words: Vec<String>,
    revealed: bool,
    copied: bool,
    on_copy: Option<Box<dyn FnMut(String)>>,
    on_reveal_toggle: Option<Box<dyn FnMut(bool)>>,
    hovered_word: Option<usize>,
    copy_hovered: bool,
    reveal_hovered: bool,
}

impl MnemonicDisplay {
    pub fn new(words: Vec<String>) -> Self {
        Self {
            id: None,
            words,
            revealed: false,
            copied: false,
            on_copy: None,
            on_reveal_toggle: None,
            hovered_word: None,
            copy_hovered: false,
            reveal_hovered: false,
        }
    }

    pub fn from_phrase(phrase: &str) -> Self {
        let words: Vec<String> = phrase.split_whitespace().map(|s| s.to_string()).collect();
        Self::new(words)
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn revealed(mut self, revealed: bool) -> Self {
        self.revealed = revealed;
        self
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    pub fn on_reveal_toggle<F>(mut self, f: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_reveal_toggle = Some(Box::new(f));
        self
    }

    fn word_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let padding = 12.0;
        let header_height = 40.0;
        let cols = 4;
        let _rows = self.words.len().div_ceil(cols);

        let word_w = (bounds.size.width - padding * 2.0 - 12.0 * (cols as f32 - 1.0)) / cols as f32;
        let word_h = 32.0;
        let gap = 8.0;

        let row = index / cols;
        let col = index % cols;

        let x = bounds.origin.x + padding + col as f32 * (word_w + 12.0);
        let y = bounds.origin.y + header_height + row as f32 * (word_h + gap);

        Bounds::new(x, y, word_w, word_h)
    }

    fn button_bounds(&self, bounds: &Bounds) -> (Bounds, Bounds) {
        let padding = 12.0;
        let btn_w = 80.0;
        let btn_h = 28.0;
        let gap = 8.0;

        let reveal_btn = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_w * 2.0 - gap,
            bounds.origin.y + 6.0,
            btn_w,
            btn_h,
        );

        let copy_btn = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_w,
            bounds.origin.y + 6.0,
            btn_w,
            btn_h,
        );

        (reveal_btn, copy_btn)
    }

    fn full_phrase(&self) -> String {
        self.words.join(" ")
    }
}

impl Component for MnemonicDisplay {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let header_height = 40.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Title
        let title = cx.text.layout(
            "Recovery Phrase",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 10.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title);

        // Warning
        let warning = cx.text.layout(
            "Keep this phrase secure and never share it",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 26.0),
            theme::font_size::XS,
            Hsla::new(0.0, 0.7, 0.55, 1.0), // Warning red
        );
        cx.scene.draw_text(warning);

        // Buttons
        let (reveal_bounds, copy_bounds) = self.button_bounds(&bounds);

        // Reveal button
        let reveal_bg = if self.reveal_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::MUTED
        };
        cx.scene.draw_quad(
            Quad::new(reveal_bounds)
                .with_background(reveal_bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let reveal_text = if self.revealed { "Hide" } else { "Reveal" };
        let reveal_label = cx.text.layout(
            reveal_text,
            Point::new(reveal_bounds.origin.x + 20.0, reveal_bounds.origin.y + 6.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(reveal_label);

        // Copy button
        let copy_bg = if self.copy_hovered {
            theme::bg::HOVER
        } else if self.copied {
            Hsla::new(120.0, 0.5, 0.25, 1.0)
        } else {
            theme::bg::MUTED
        };
        cx.scene.draw_quad(
            Quad::new(copy_bounds)
                .with_background(copy_bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let copy_text = if self.copied { "Copied!" } else { "Copy" };
        let copy_label = cx.text.layout(
            copy_text,
            Point::new(copy_bounds.origin.x + 18.0, copy_bounds.origin.y + 6.0),
            theme::font_size::SM,
            if self.copied {
                Hsla::new(120.0, 0.7, 0.5, 1.0)
            } else {
                theme::text::PRIMARY
            },
        );
        cx.scene.draw_text(copy_label);

        // Words grid
        for (i, word) in self.words.iter().enumerate() {
            let word_bounds = self.word_bounds(&bounds, i);

            let bg = if self.hovered_word == Some(i) {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };

            cx.scene.draw_quad(
                Quad::new(word_bounds)
                    .with_background(bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            // Word number
            let num = format!("{}.", i + 1);
            let num_run = cx.text.layout(
                &num,
                Point::new(word_bounds.origin.x + 6.0, word_bounds.origin.y + 8.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(num_run);

            // Word (or blur)
            let display_word = if self.revealed {
                word.clone()
            } else {
                "\u{2022}\u{2022}\u{2022}\u{2022}".to_string() // Dots for hidden
            };
            let word_run = cx.text.layout(
                &display_word,
                Point::new(word_bounds.origin.x + 26.0, word_bounds.origin.y + 8.0),
                theme::font_size::SM,
                if self.revealed {
                    theme::text::PRIMARY
                } else {
                    theme::text::DISABLED
                },
            );
            cx.scene.draw_text(word_run);
        }

        // Word count indicator
        let cols = 4;
        let rows = self.words.len().div_ceil(cols);
        let word_h = 32.0;
        let gap = 8.0;
        let footer_y = bounds.origin.y + header_height + rows as f32 * (word_h + gap) + 8.0;

        let count_text = format!("{} words", self.words.len());
        let count_run = cx.text.layout(
            &count_text,
            Point::new(bounds.origin.x + padding, footer_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(count_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let (reveal_bounds, copy_bounds) = self.button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_reveal = self.reveal_hovered;
                let was_copy = self.copy_hovered;
                let was_word = self.hovered_word;

                self.reveal_hovered = reveal_bounds.contains(point);
                self.copy_hovered = copy_bounds.contains(point);

                self.hovered_word = None;
                for i in 0..self.words.len() {
                    if self.word_bounds(&bounds, i).contains(point) {
                        self.hovered_word = Some(i);
                        break;
                    }
                }

                if was_reveal != self.reveal_hovered
                    || was_copy != self.copy_hovered
                    || was_word != self.hovered_word
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if reveal_bounds.contains(point) {
                        self.revealed = !self.revealed;
                        if let Some(callback) = &mut self.on_reveal_toggle {
                            callback(self.revealed);
                        }
                        return EventResult::Handled;
                    }

                    if copy_bounds.contains(point) {
                        self.copied = true;
                        let phrase = self.full_phrase();
                        if let Some(callback) = &mut self.on_copy {
                            callback(phrase);
                        }
                        return EventResult::Handled;
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
        let header_height = 40.0;
        let cols = 4;
        let rows = self.words.len().div_ceil(cols);
        let word_h = 32.0;
        let gap = 8.0;
        let footer = 32.0;

        let height = header_height + rows as f32 * (word_h + gap) + footer;
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_display() {
        let words = vec![
            "abandon".to_string(),
            "ability".to_string(),
            "able".to_string(),
            "about".to_string(),
        ];
        let display = MnemonicDisplay::new(words);
        assert!(!display.revealed);
        assert_eq!(display.words.len(), 4);
    }

    #[test]
    fn test_from_phrase() {
        let display = MnemonicDisplay::from_phrase("one two three four five six");
        assert_eq!(display.words.len(), 6);
    }

    #[test]
    fn test_full_phrase() {
        let display = MnemonicDisplay::from_phrase("one two three");
        assert_eq!(display.full_phrase(), "one two three");
    }
}
