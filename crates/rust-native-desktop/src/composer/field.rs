//! Native composer field: text, selection, caret, and IME over `ComposerDraft`.
use super::{ComposerDraft, Input, Stamp};
use crate::input::{SurfaceInput, TextInput};
use crate::text::{Fonts, Paragraph, font};
use crate::{Frame, PxRect};
use rust_native::edit::{Movement, Selection};
use rust_native::layout::display::Weight;
use rust_native::style::{Color, TextAlign};
use std::rc::Rc;

/// What the focused field asks its application to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Edited,
    Send,
    Unhandled,
}

#[cfg_attr(not(feature = "window"), allow(dead_code))]
enum ClipboardResult {
    Paste(Option<String>),
    Cut(bool),
}

#[derive(Default)]
pub struct Field {
    pub draft: ComposerDraft,
    pub focused: bool,
    paragraph: Option<Rc<Paragraph>>,
    offset: f32,
    dragging: bool,
    pub caret: (f32, f32),
    clipboard: Option<(Stamp, std::sync::mpsc::Receiver<ClipboardResult>)>,
    waker: Option<crate::Waker>,
    measured_height: std::cell::RefCell<Option<(String, f32, f32)>>,
}

impl Field {
    /// Grow from one line to eight lines, then scroll within the field.
    pub fn height(&self, width: f32) -> f32 {
        let text = self.text();
        if let Some((previous, previous_width, height)) = self.measured_height.borrow().as_ref()
            && previous == text
            && *previous_width == width
        {
            return *height;
        }
        use rust_native::layout::{MeasureRun, Measurer, shape::ShapingMeasurer};
        let font = font(15.0, Weight::Regular, false);
        let mut measurer = ShapingMeasurer::new();
        let measured = measurer.measure(
            text,
            &[MeasureRun {
                font,
                start16: 0,
                end16: text.encode_utf16().count() as u32,
            }],
            Some((width - 28.0).max(1.0)),
        );
        let mut lines = measured.map_or(1, |measured| measured.lines.len().max(1));
        if text.ends_with('\n') {
            lines += 1;
        }
        let height = (lines as f32 * 21.0 + 28.0).clamp(56.0, 196.0);
        *self.measured_height.borrow_mut() = Some((text.to_owned(), width, height));
        height
    }
    pub fn start(&mut self, waker: crate::Waker) {
        self.waker = Some(waker);
    }
    /// Apply a clipboard result only to the unchanged editing lifetime that requested it.
    pub fn poll_clipboard(&mut self, at_ms: u64) -> bool {
        let Some((stamp, receive)) = &self.clipboard else {
            return false;
        };
        match receive.try_recv() {
            Ok(result) => {
                let stamp = stamp.clone();
                self.clipboard = None;
                match result {
                    ClipboardResult::Paste(Some(text)) => {
                        self.draft.apply(&stamp, Input::Paste(&text), at_ms).is_ok()
                    }
                    ClipboardResult::Cut(true) => {
                        self.draft.apply(&stamp, Input::Text(""), at_ms).is_ok()
                    }
                    _ => false,
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.clipboard = None;
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
        }
    }
    #[cfg(feature = "window")]
    fn clipboard(&mut self, key: &str) {
        if self.clipboard.is_some() {
            return;
        }
        let Ok(stamp) = self.draft.stamp() else {
            return;
        };
        let text = self
            .draft
            .editor()
            .map(|editor| editor.selected_text().to_owned())
            .unwrap_or_default();
        let paste = matches!(key, "v" | "V");
        let cut = matches!(key, "x" | "X");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let waker = self.waker.clone();
        let spawned = std::thread::Builder::new()
            .name("composer-clipboard".into())
            .spawn(move || {
                let result = if paste {
                    ClipboardResult::Paste(crate::input::paste())
                } else {
                    ClipboardResult::Cut(crate::input::copy(&text) && cut)
                };
                let _ = send.send(result);
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
        if spawned.is_ok() {
            self.clipboard = Some((stamp, receive));
        }
    }
    pub fn version(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (
            self.draft.editor().map(|e| e.revision()),
            self.focused,
            self.offset.to_bits(),
        )
            .hash(&mut hash);
        hash.finish()
    }
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    pub fn text(&self) -> &str {
        self.draft.editor().map_or("", |editor| editor.text())
    }
    fn apply(&mut self, input: Input<'_>, at_ms: u64) {
        if let Ok(stamp) = self.draft.stamp() {
            let _ = self.draft.apply(&stamp, input, at_ms);
        }
    }
    /// Translate native input. Marked text never submits on Enter.
    pub fn input(&mut self, event: TextInput<'_>, at_ms: u64) -> Action {
        if !self.focused {
            return Action::Unhandled;
        }
        match event {
            TextInput::FocusLost => {
                self.apply(Input::CancelComposition, at_ms);
                self.focused = false;
            }
            TextInput::CancelComposition => self.apply(Input::CancelComposition, at_ms),
            TextInput::Preedit { text: "", .. } => self.apply(Input::CancelComposition, at_ms),
            TextInput::Preedit { text, selection } => {
                let (anchor, caret) = selection.unwrap_or((text.len(), text.len()));
                self.apply(
                    Input::Preedit {
                        text,
                        selection: Selection { anchor, caret },
                    },
                    at_ms,
                );
            }
            TextInput::Commit(text) => self.apply(Input::Commit(text), at_ms),
            TextInput::Key {
                key,
                text,
                command,
                alt,
                shift,
            } => {
                let composing = self
                    .draft
                    .editor()
                    .is_some_and(|editor| editor.is_composing());
                if composing {
                    return Action::Edited;
                }
                match key {
                    "a" | "A" if command => self.apply(Input::SelectAll, at_ms),
                    "z" | "Z" if command => {
                        self.apply(if shift { Input::Redo } else { Input::Undo }, at_ms)
                    }
                    "y" | "Y" if command => self.apply(Input::Redo, at_ms),
                    "c" | "C" | "x" | "X" | "v" | "V" if command => {
                        #[cfg(feature = "window")]
                        self.clipboard(key);
                    }
                    "ArrowLeft" | "ArrowRight" | "Home" | "End" => {
                        let movement = match key {
                            "Home" => {
                                if command {
                                    Movement::Start
                                } else {
                                    Movement::LineStart
                                }
                            }
                            "End" => {
                                if command {
                                    Movement::End
                                } else {
                                    Movement::LineEnd
                                }
                            }
                            "ArrowLeft" if command && cfg!(target_os = "macos") => {
                                Movement::LineStart
                            }
                            "ArrowRight" if command && cfg!(target_os = "macos") => {
                                Movement::LineEnd
                            }
                            "ArrowLeft" if alt || command => Movement::PreviousWord,
                            "ArrowRight" if alt || command => Movement::NextWord,
                            "ArrowLeft" => Movement::PreviousGrapheme,
                            _ => Movement::NextGrapheme,
                        };
                        self.apply(
                            Input::Move {
                                movement,
                                extend: shift,
                            },
                            at_ms,
                        );
                    }
                    "ArrowUp" | "ArrowDown" => {
                        if command {
                            self.apply(
                                Input::Move {
                                    movement: if key == "ArrowUp" {
                                        Movement::Start
                                    } else {
                                        Movement::End
                                    },
                                    extend: shift,
                                },
                                at_ms,
                            );
                        } else if let Some(paragraph) = &self.paragraph {
                            let selection = self.draft.editor().unwrap().selection();
                            let index = paragraph
                                .lines
                                .iter()
                                .rposition(|line| line.start <= selection.caret)
                                .unwrap_or(0);
                            let target = if key == "ArrowUp" {
                                index.saturating_sub(1)
                            } else {
                                (index + 1).min(paragraph.lines.len().saturating_sub(1))
                            };
                            if let Some(line) = paragraph.lines.get(target) {
                                let column =
                                    selection.caret.saturating_sub(paragraph.lines[index].start);
                                let mut caret = (line.start + column).min(line.end);
                                while !paragraph.text.is_char_boundary(caret) {
                                    caret -= 1;
                                }
                                self.apply(
                                    Input::Select(Selection {
                                        anchor: if shift { selection.anchor } else { caret },
                                        caret,
                                    }),
                                    at_ms,
                                );
                            }
                        }
                    }
                    "Backspace" => self.apply(Input::Backspace, at_ms),
                    "Delete" => self.apply(Input::Delete, at_ms),
                    "Enter" if !shift => return Action::Send,
                    "Enter" => self.apply(Input::Text("\n"), at_ms),
                    "Escape" => {
                        self.focused = false;
                    }
                    "Tab" => {
                        self.focused = false;
                        return Action::Unhandled;
                    }
                    _ if !command => {
                        if let Some(text) =
                            text.filter(|text| !text.chars().any(|c| c.is_control()))
                        {
                            self.apply(Input::Text(text), at_ms);
                        } else {
                            return Action::Unhandled;
                        }
                    }
                    _ => return Action::Unhandled,
                }
            }
        }
        Action::Edited
    }

    fn hit(&self, x: f32, y: f32, fonts: &mut Fonts) -> usize {
        let Some(paragraph) = &self.paragraph else {
            return self.text().len();
        };
        let index = ((y - 14.0 + self.offset).max(0.0) / paragraph.line_height()) as usize;
        let Some(line) = paragraph.lines.get(index) else {
            return self.text().len();
        };
        let mut result = line.start;
        let mut best = f32::MAX;
        for byte in (line.start..=line.end).filter(|byte| paragraph.text.is_char_boundary(*byte)) {
            let width = fonts.advance(&paragraph.text[line.start..byte], paragraph.font);
            let distance = (14.0 + width - x).abs();
            if distance < best {
                best = distance;
                result = byte;
            }
        }
        result
    }

    pub fn pointer(&mut self, event: SurfaceInput, fonts: &mut Fonts, at_ms: u64) {
        match event {
            SurfaceInput::Down { x, y, shift } => {
                self.focused = true;
                self.dragging = true;
                let caret = self.hit(x, y, fonts);
                let anchor = if shift {
                    self.draft.editor().map_or(caret, |e| e.selection().anchor)
                } else {
                    caret
                };
                self.apply(Input::Select(Selection { anchor, caret }), at_ms);
            }
            SurfaceInput::Move { x, y } | SurfaceInput::Up { x, y } if self.dragging => {
                let caret = self.hit(x, y, fonts);
                let anchor = self.draft.editor().unwrap().selection().anchor;
                self.apply(Input::Select(Selection { anchor, caret }), at_ms);
                if matches!(event, SurfaceInput::Up { .. }) {
                    self.dragging = false;
                }
            }
            SurfaceInput::Wheel { dy, .. } => {
                self.offset = (self.offset - dy).max(0.0);
            }
            _ => {}
        }
    }

    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect, scale: f32, fonts: &mut Fonts) {
        let clip = frame.clip_to(rect);
        frame.fill(rect, 10.0 * scale, Color::rgb(25, 29, 35));
        frame.stroke(
            rect,
            10.0 * scale,
            scale,
            if self.focused {
                Color::rgb(116, 143, 174)
            } else {
                Color::rgb(57, 63, 73)
            },
        );
        let font = font(15.0, Weight::Regular, false);
        let text = self.text().to_owned();
        let paragraph = fonts.editable_paragraph(&text, font, (rect.w / scale - 28.0).max(1.0));
        let height = paragraph.line_height();
        let selection = self
            .draft
            .editor()
            .map_or(Selection::collapsed(0), |e| e.selection());
        let range = selection.range();
        let caret_line = paragraph
            .lines
            .iter()
            .rposition(|line| line.start <= selection.caret)
            .unwrap_or(0);
        let caret_y = caret_line as f32 * height;
        let available = rect.h / scale - 28.0;
        if self.focused {
            if caret_y < self.offset {
                self.offset = caret_y;
            }
            if caret_y + height > self.offset + available {
                self.offset = caret_y + height - available;
            }
        }
        self.offset = self
            .offset
            .clamp(0.0, (paragraph.height - available).max(0.0));
        for (index, line) in paragraph.lines.iter().enumerate() {
            let y = rect.y + (14.0 + index as f32 * height - self.offset) * scale;
            let lo = range.start.max(line.start).min(line.end);
            let hi = range.end.min(line.end).max(lo);
            if lo < hi {
                let left = fonts.advance(&text[line.start..lo], font);
                let width = fonts.advance(&text[lo..hi], font);
                frame.fill(
                    PxRect {
                        x: rect.x + (14.0 + left) * scale,
                        y,
                        w: width * scale,
                        h: height * scale,
                    },
                    0.0,
                    Color::rgb(53, 78, 105),
                );
            }
        }
        if text.is_empty() {
            let placeholder = fonts.paragraph("Message OpenAgents…", font, None);
            fonts.draw(
                frame,
                &placeholder,
                rect.x + 14.0 * scale,
                rect.y + 14.0 * scale,
                rect.w / scale - 28.0,
                TextAlign::Start,
                scale,
                Color::rgb(137, 144, 155),
            );
        } else {
            fonts.draw(
                frame,
                &paragraph,
                rect.x + 14.0 * scale,
                rect.y + (14.0 - self.offset) * scale,
                rect.w / scale - 28.0,
                TextAlign::Start,
                scale,
                Color::rgb(230, 232, 235),
            );
        }
        let line = paragraph.lines.get(caret_line);
        let left = line.map_or(0.0, |line| {
            fonts.advance(&text[line.start..selection.caret.min(text.len())], font)
        });
        self.caret = (14.0 + left, 14.0 + caret_y - self.offset);
        if self.focused {
            frame.fill(
                PxRect {
                    x: rect.x + self.caret.0 * scale,
                    y: rect.y + self.caret.1 * scale,
                    w: scale.max(1.0),
                    h: height * scale,
                },
                0.0,
                Color::rgb(222, 231, 243),
            );
        }
        if let Some(marked) = self.draft.editor().and_then(|e| e.marked_range()) {
            for (index, line) in paragraph.lines.iter().enumerate() {
                let lo = marked.start.max(line.start).min(line.end);
                let hi = marked.end.min(line.end).max(lo);
                if lo < hi {
                    let left = fonts.advance(&text[line.start..lo], font);
                    let width = fonts.advance(&text[lo..hi], font);
                    let y = rect.y + (14.0 + (index + 1) as f32 * height - self.offset) * scale;
                    frame.line(
                        (rect.x + (14.0 + left) * scale, y),
                        (rect.x + (14.0 + left + width) * scale, y),
                        scale,
                        Color::rgb(169, 199, 234),
                    );
                }
            }
        }
        self.paragraph = Some(paragraph);
        frame.restore_clip(clip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::{Element, Node, View, style::Style};

    fn field() -> Field {
        let view = View::<()>::new(
            "test",
            1,
            Node {
                key: "input".into(),
                style: Style::default(),
                element: Element::Composer {
                    token: "send".into(),
                    placeholder: "Message".into(),
                    max_bytes: 1024,
                    enabled: true,
                    busy: false,
                    stop: None,
                    choices: vec![],
                    draft: None,
                    focus: true,
                },
            },
        )
        .validate()
        .unwrap();
        let mut field = Field {
            focused: true,
            ..Field::default()
        };
        field.draft.mount(&view, "input").unwrap();
        field
    }

    #[test]
    fn each_trailing_space_moves_the_caret_and_remains_selectable() {
        let mut field = field();
        let mut fonts = Fonts::new();
        let mut frame = Frame::transparent(400, 100);
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 100.0,
        };
        field.input(TextInput::Commit("hello"), 0);
        field.paint(&mut frame, rect, 1.0, &mut fonts);
        let initial = field.caret.0;
        for index in 1..=3 {
            field.input(
                TextInput::Key {
                    key: " ",
                    text: Some(" "),
                    command: false,
                    alt: false,
                    shift: false,
                },
                index,
            );
            field.paint(&mut frame, rect, 1.0, &mut fonts);
            assert!(field.caret.0 > initial);
            let text = field.text();
            assert_eq!(text, format!("hello{}", " ".repeat(index as usize)));
            assert_eq!(
                field.paragraph.as_ref().unwrap().lines.last().unwrap().end,
                text.len()
            );
            assert_eq!(
                field.hit(field.caret.0, field.caret.1, &mut fonts),
                text.len()
            );
        }
        field.pointer(
            SurfaceInput::Down {
                x: initial,
                y: 14.0,
                shift: false,
            },
            &mut fonts,
            4,
        );
        field.pointer(
            SurfaceInput::Up {
                x: field.caret.0,
                y: 14.0,
            },
            &mut fonts,
            5,
        );
        field.input(
            TextInput::Key {
                key: "End",
                text: None,
                command: true,
                alt: false,
                shift: true,
            },
            6,
        );
        assert!(field.draft.editor().unwrap().selected_text().ends_with(' '));
    }
    #[test]
    fn delayed_paste_cannot_replace_a_newer_edit() {
        let mut field = field();
        let (send, receive) = std::sync::mpsc::channel();
        field.clipboard = Some((field.draft.stamp().unwrap(), receive));
        field.input(TextInput::Commit("newer"), 0);
        send.send(ClipboardResult::Paste(Some("old clipboard".into())))
            .unwrap();
        assert!(!field.poll_clipboard(1));
        assert_eq!(field.text(), "newer");
        let (send, receive) = std::sync::mpsc::channel();
        field.clipboard = Some((field.draft.stamp().unwrap(), receive));
        send.send(ClipboardResult::Paste(Some(" paste".into())))
            .unwrap();
        assert!(field.poll_clipboard(2));
        assert_eq!(field.text(), "newer paste");
    }

    #[test]
    fn final_newline_places_caret_on_empty_next_line() {
        let mut field = field();
        field.input(TextInput::Commit("hello \n"), 0);
        let mut fonts = Fonts::new();
        field.paint(
            &mut Frame::transparent(400, 100),
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 100.0,
            },
            1.0,
            &mut fonts,
        );
        assert_eq!(field.caret.0, 14.0);
        assert!(field.caret.1 > 14.0);
    }
}
