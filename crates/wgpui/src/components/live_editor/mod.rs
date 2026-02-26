mod block;
mod cursor;

include!("state.rs");
include!("editing.rs");
include!("layout_paint.rs");
include!("input_handlers.rs");

impl Component for LiveEditor {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.paint_component(bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        self.handle_component_event(event, bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let line_height = self.style.font_size * self.style.line_height;
        let height = self.lines.len() as f32 * line_height + self.style.padding * 2.0;
        (None, Some(height.max(100.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = LiveEditor::new("Hello\nWorld");
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.content(), "Hello\nWorld");
    }

    #[test]
    fn test_empty_editor() {
        let editor = LiveEditor::new("");
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content(), "");
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = LiveEditor::new("Hello\nWorld");
        assert_eq!(editor.cursor.line, 0);
        assert_eq!(editor.cursor.column, 0);

        editor.move_cursor_right();
        assert_eq!(editor.cursor.column, 1);

        editor.move_cursor_down();
        assert_eq!(editor.cursor.line, 1);
        assert_eq!(editor.cursor.column, 1);

        editor.move_cursor_to_line_end();
        assert_eq!(editor.cursor.column, 5);
    }

    #[test]
    fn test_insert_char() {
        let mut editor = LiveEditor::new("Hello");
        editor.cursor.column = 5;
        editor.insert_char('!');
        assert_eq!(editor.content(), "Hello!");
    }

    #[test]
    fn test_insert_newline() {
        let mut editor = LiveEditor::new("HelloWorld");
        editor.cursor.column = 5;
        editor.insert_newline();
        assert_eq!(editor.content(), "Hello\nWorld");
        assert_eq!(editor.cursor.line, 1);
        assert_eq!(editor.cursor.column, 0);
    }

    #[test]
    fn test_delete_backward() {
        let mut editor = LiveEditor::new("Hello");
        editor.cursor.column = 5;
        editor.delete_backward();
        assert_eq!(editor.content(), "Hell");
    }

    #[test]
    fn test_delete_at_line_start() {
        let mut editor = LiveEditor::new("Hello\nWorld");
        editor.cursor.line = 1;
        editor.cursor.column = 0;
        editor.delete_backward();
        assert_eq!(editor.content(), "HelloWorld");
    }
}
