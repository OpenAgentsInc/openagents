use wgpui::components::{Component, ComponentId, EventResult};
use wgpui::{
    Bounds, Cursor, EventContext, Hsla, InputEvent, Key, MouseButton, NamedKey, PaintContext,
    Point, Quad, copy_to_clipboard, theme,
};

use crate::editor::Editor;
use crate::caret::Position;

struct CompositionState {
    base: Position,
    text: String,
}

pub struct EditorView {
    id: Option<ComponentId>,
    editor: Editor,
    font_size: f32,
    line_height: f32,
    padding: f32,
    gutter_padding: f32,
    background: Hsla,
    gutter_background: Hsla,
    border_color: Hsla,
    text_color: Hsla,
    gutter_text_color: Hsla,
    selection_color: Hsla,
    caret_color: Hsla,
    scroll_offset: f32,
    char_width: f32,
    gutter_width: f32,
    hover: bool,
    focused: bool,
    dragging: bool,
    composition: Option<CompositionState>,
    text_origin: Point,
    viewport_height: f32,
    bounds: Bounds,
    on_copy: Option<Box<dyn FnMut(String)>>,
}

impl EditorView {
    pub fn new(editor: Editor) -> Self {
        Self {
            id: None,
            editor,
            font_size: theme::font_size::SM,
            line_height: theme::font_size::SM * 1.4,
            padding: theme::spacing::SM,
            gutter_padding: theme::spacing::XS,
            background: theme::bg::APP,
            gutter_background: theme::bg::SURFACE,
            border_color: theme::border::DEFAULT,
            text_color: theme::text::PRIMARY,
            gutter_text_color: theme::text::MUTED,
            selection_color: theme::accent::PRIMARY.with_alpha(0.2),
            caret_color: theme::accent::PRIMARY,
            scroll_offset: 0.0,
            char_width: theme::font_size::SM * 0.6,
            gutter_width: 48.0,
            hover: false,
            focused: false,
            dragging: false,
            composition: None,
            text_origin: Point::ZERO,
            viewport_height: 0.0,
            bounds: Bounds::ZERO,
            on_copy: None,
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self::new(Editor::new(text))
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    pub fn set_text(&mut self, text: &str) {
        self.clear_composition();
        self.editor.set_text(text);
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    pub fn cursor(&self) -> Cursor {
        if self.hover || self.focused {
            Cursor::Text
        } else {
            Cursor::Default
        }
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn clear_hover(&mut self) {
        self.hover = false;
        self.dragging = false;
    }

    pub fn paste_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.clear_composition();
        self.editor.insert_text(text);
    }

    pub fn composition_start(&mut self, text: &str) {
        if !self.focused {
            return;
        }
        let base = self
            .editor
            .cursors()
            .first()
            .map(|cursor| cursor.position)
            .unwrap_or_else(Position::zero);
        self.composition = Some(CompositionState {
            base,
            text: text.to_string(),
        });
    }

    pub fn composition_update(&mut self, text: &str) {
        if !self.focused {
            return;
        }
        if let Some(composition) = &mut self.composition {
            composition.text = text.to_string();
        } else {
            self.composition_start(text);
        }
    }

    pub fn composition_end(&mut self, text: &str) {
        if !self.focused {
            self.composition = None;
            return;
        }
        let final_text = if text.is_empty() {
            self.composition
                .as_ref()
                .map(|composition| composition.text.clone())
                .unwrap_or_default()
        } else {
            text.to_string()
        };
        self.composition = None;
        if !final_text.is_empty() {
            self.editor.insert_text(&final_text);
        }
    }

    fn update_layout(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.bounds = bounds;
        self.char_width = cx.text.measure("M", self.font_size).max(1.0);
        self.line_height = self.font_size * 1.4;
        let line_count = self.editor.buffer().line_count();
        let digits = line_count.to_string().len().max(2);
        self.gutter_width = self.gutter_padding * 2.0 + digits as f32 * self.char_width;
        self.text_origin = Point::new(
            bounds.origin.x + self.padding + self.gutter_width,
            bounds.origin.y + self.padding,
        );
        self.viewport_height = (bounds.size.height - self.padding * 2.0).max(0.0);
    }

    fn point_to_position(&self, point: Point) -> Position {
        let x = point.x - self.text_origin.x;
        let y = point.y - self.text_origin.y + self.scroll_offset;
        let line = if self.line_height > 0.0 {
            (y / self.line_height).floor() as isize
        } else {
            0
        };
        let col = if self.char_width > 0.0 {
            (x / self.char_width).round() as isize
        } else {
            0
        };

        let line = line.clamp(0, self.editor.buffer().line_count().saturating_sub(1) as isize) as usize;
        let line_len = self.editor.buffer().line_len(line);
        let col = col.max(0) as usize;
        let col = col.min(line_len);

        Position { line, column: col }
    }

    fn copy_selection(&mut self) {
        if let Some(text) = self.editor.selected_text() {
            if let Some(handler) = &mut self.on_copy {
                handler(text);
            } else {
                let _ = copy_to_clipboard(&text);
            }
        }
    }

    fn cut_selection(&mut self) {
        if self.editor.selected_text().is_some() {
            self.copy_selection();
            self.editor.delete_backward();
        }
    }

    fn insert_tab(&mut self) {
        self.clear_composition();
        self.editor.insert_text("    ");
    }

    fn page_move(&mut self, delta: isize, select: bool) {
        let lines = (self.viewport_height / self.line_height).floor() as usize;
        let steps = lines.max(1);
        for _ in 0..steps {
            if delta < 0 {
                self.editor.move_up(select);
            } else {
                self.editor.move_down(select);
            }
        }
    }

    fn clear_composition(&mut self) {
        self.composition = None;
    }
}

impl Component for EditorView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.update_layout(bounds, cx);

        let line_count = self.editor.buffer().line_count();
        let content_height = line_count as f32 * self.line_height;
        let max_scroll = (content_height - self.viewport_height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(self.background).with_border(self.border_color, 1.0));

        let gutter_bounds = Bounds::new(
            bounds.origin.x + self.padding,
            bounds.origin.y,
            self.gutter_width,
            bounds.size.height,
        );
        cx.scene.draw_quad(
            Quad::new(gutter_bounds)
                .with_background(self.gutter_background)
                .with_border(self.border_color.with_alpha(0.4), 1.0),
        );

        let visible_start = if self.line_height > 0.0 {
            (self.scroll_offset / self.line_height).floor() as usize
        } else {
            0
        };
        let visible_lines = if self.line_height > 0.0 {
            (self.viewport_height / self.line_height).ceil() as usize + 1
        } else {
            0
        };
        let visible_end = (visible_start + visible_lines).min(line_count);

        if let Some(primary) = self.editor.cursors().first() {
            let line = primary.position.line;
            if line >= visible_start && line < visible_end {
                let y = self.text_origin.y + line as f32 * self.line_height - self.scroll_offset;
                let highlight_bounds = Bounds::new(
                    self.text_origin.x,
                    y,
                    bounds.size.width - (self.text_origin.x - bounds.origin.x) - self.padding,
                    self.line_height,
                );
                cx.scene.draw_quad(
                    Quad::new(highlight_bounds)
                        .with_background(theme::bg::ELEVATED.with_alpha(0.4)),
                );
            }
        }

        for selection in self.editor.selection_ranges_by_line() {
            let sel_start_line = selection.start.line;
            let sel_end_line = selection.end.line;

            for line in sel_start_line..=sel_end_line {
                if line < visible_start || line >= visible_end || line >= line_count {
                    continue;
                }

                let line_len = self.editor.buffer().line_len(line);
                let start_col = if line == sel_start_line {
                    selection.start.column
                } else {
                    0
                };
                let end_col = if line == sel_end_line {
                    selection.end.column
                } else {
                    line_len
                };

                if start_col == end_col {
                    continue;
                }

                let y = self.text_origin.y + line as f32 * self.line_height - self.scroll_offset;
                let x = self.text_origin.x + start_col as f32 * self.char_width;
                let width = (end_col - start_col) as f32 * self.char_width;
                let selection_bounds = Bounds::new(x, y, width, self.line_height);

                if y + self.line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                    continue;
                }

                cx.scene
                    .draw_quad(Quad::new(selection_bounds).with_background(self.selection_color));
            }
        }

        for line_idx in visible_start..visible_end {
            let y = self.text_origin.y + line_idx as f32 * self.line_height - self.scroll_offset;
            if y + self.line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                continue;
            }

            let line_text = self.editor.buffer().line_text(line_idx);
            let text_y = y + self.line_height * 0.5 - self.font_size * 0.55;
            let text_run = cx.text.layout(
                &line_text,
                Point::new(self.text_origin.x, text_y),
                self.font_size,
                self.text_color,
            );
            cx.scene.draw_text(text_run);

            let line_number = (line_idx + 1).to_string();
            let number_x = bounds.origin.x + self.padding + self.gutter_width
                - self.gutter_padding
                - (line_number.chars().count() as f32 * self.char_width);
            let number_run = cx.text.layout(
                &line_number,
                Point::new(number_x, text_y),
                self.font_size,
                self.gutter_text_color,
            );
            cx.scene.draw_text(number_run);
        }

        if self.focused {
            if let Some(composition) = &self.composition {
                let line = composition.base.line;
                if line >= visible_start && line < visible_end {
                    let y = self.text_origin.y + line as f32 * self.line_height - self.scroll_offset;
                    if y + self.line_height >= bounds.origin.y && y <= bounds.origin.y + bounds.size.height {
                        let x = self.text_origin.x + composition.base.column as f32 * self.char_width;
                        let text_len = composition.text.chars().count().max(1) as f32;
                        let width = text_len * self.char_width;
                        let text_y = y + self.line_height * 0.5 - self.font_size * 0.55;
                        let highlight_bounds = Bounds::new(x, y, width, self.line_height);
                        cx.scene.draw_quad(
                            Quad::new(highlight_bounds)
                                .with_background(theme::accent::PRIMARY.with_alpha(0.12)),
                        );
                        let run = cx.text.layout(
                            &composition.text,
                            Point::new(x, text_y),
                            self.font_size,
                            self.text_color,
                        );
                        cx.scene.draw_text(run);
                        let underline_bounds = Bounds::new(x, y + self.line_height - 2.0, width, 2.0);
                        cx.scene.draw_quad(
                            Quad::new(underline_bounds).with_background(theme::accent::PRIMARY),
                        );
                        let caret_x = x + (composition.text.chars().count() as f32 * self.char_width);
                        let caret_bounds = Bounds::new(caret_x, y + 2.0, 2.0, self.line_height - 4.0);
                        cx.scene
                            .draw_quad(Quad::new(caret_bounds).with_background(self.caret_color));
                    }
                }
            } else {
                for cursor in self.editor.cursors() {
                    let line = cursor.position.line;
                    if line < visible_start || line >= visible_end {
                        continue;
                    }
                    let col = cursor.position.column;
                    let x = self.text_origin.x + col as f32 * self.char_width;
                    let y = self.text_origin.y + line as f32 * self.line_height - self.scroll_offset;
                    let caret_bounds = Bounds::new(x, y + 2.0, 2.0, self.line_height - 4.0);
                    cx.scene
                        .draw_quad(Quad::new(caret_bounds).with_background(self.caret_color));
                }
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hover;
                self.hover = bounds.contains(point);
                if self.dragging && self.hover {
                    let pos = self.point_to_position(point);
                    if let Some(mut cursor) = self.editor.cursors().first().cloned() {
                        cursor.position = pos;
                        self.editor.set_cursors(vec![cursor]);
                    } else {
                        self.editor.set_cursor(pos);
                    }
                    return EventResult::Handled;
                }
                if was_hovered != self.hover {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    let inside = bounds.contains(point);
                    if inside {
                        self.focused = true;
                        self.dragging = true;
                        if let Some(id) = self.id {
                            cx.set_focus(id);
                        }
                        let pos = self.point_to_position(point);
                        self.editor.set_cursor(pos);
                        return EventResult::Handled;
                    }
                    if self.focused {
                        self.focused = false;
                        self.dragging = false;
                        cx.clear_focus();
                        return EventResult::Handled;
                    }
                }
            }
            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left && self.dragging {
                    self.dragging = false;
                    return EventResult::Handled;
                }
            }
            InputEvent::Scroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset + *dy).max(0.0);
                return EventResult::Handled;
            }
            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return EventResult::Ignored;
                }
                if self.composition.is_some() {
                    if matches!(key, Key::Named(NamedKey::Escape)) {
                        self.clear_composition();
                    }
                    return EventResult::Handled;
                }

                match key {
                    Key::Character(c) => {
                        let lower = c.to_lowercase();
                        if modifiers.ctrl || modifiers.meta {
                            match lower.as_str() {
                                "a" => self.editor.select_all(),
                                "c" => self.copy_selection(),
                                "x" => self.cut_selection(),
                                "z" => {
                                    if modifiers.shift {
                                        self.editor.redo();
                                    } else {
                                        self.editor.undo();
                                    }
                                }
                                "y" => self.editor.redo(),
                                _ => {}
                            }
                        } else {
                            self.editor.insert_text(c);
                        }
                        return EventResult::Handled;
                    }
                    Key::Named(named) => {
                        match named {
                            NamedKey::Enter => self.editor.insert_newline(),
                            NamedKey::Backspace => self.editor.delete_backward(),
                            NamedKey::Delete => self.editor.delete_forward(),
                            NamedKey::Tab => self.insert_tab(),
                            NamedKey::ArrowLeft => self.editor.move_left(modifiers.shift),
                            NamedKey::ArrowRight => self.editor.move_right(modifiers.shift),
                            NamedKey::ArrowUp => {
                                if modifiers.alt {
                                    self.editor.add_cursor_above();
                                } else {
                                    self.editor.move_up(modifiers.shift);
                                }
                            }
                            NamedKey::ArrowDown => {
                                if modifiers.alt {
                                    self.editor.add_cursor_below();
                                } else {
                                    self.editor.move_down(modifiers.shift);
                                }
                            }
                            NamedKey::Home => self.editor.move_line_start(modifiers.shift),
                            NamedKey::End => self.editor.move_line_end(modifiers.shift),
                            NamedKey::PageUp => self.page_move(-1, modifiers.shift),
                            NamedKey::PageDown => self.page_move(1, modifiers.shift),
                            _ => {}
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
        (None, None)
    }
}
