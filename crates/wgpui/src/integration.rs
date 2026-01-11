use crate::components::atoms::{Mode, Model};
use crate::components::hud::{Command, CommandPalette, Notifications, StatusBar, StatusItem};
use crate::components::organisms::{ThreadEntry, ThreadEntryType};
use crate::components::sections::{MessageEditor, ThreadFeedback, ThreadHeader, ThreadView};
use crate::components::{Component, ComponentId, EventContext, EventResult, PaintContext, Text};
use crate::{Bounds, InputEvent};

pub struct ChatApplication {
    id: Option<ComponentId>,
    header: ThreadHeader,
    view: ThreadView,
    editor: MessageEditor,
    feedback: ThreadFeedback,
    status_bar: StatusBar,
    command_palette: CommandPalette,
    notifications: Notifications,
    show_feedback: bool,
}

impl ChatApplication {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            header: ThreadHeader::new(title),
            view: ThreadView::new().auto_scroll(true),
            editor: MessageEditor::new().mode(Mode::Normal),
            feedback: ThreadFeedback::new(),
            status_bar: StatusBar::new().items(vec![
                StatusItem::mode("mode", Mode::Normal).left(),
                StatusItem::model("model", Model::Codex).right(),
            ]),
            command_palette: CommandPalette::new().commands(vec![
                Command::new("new", "New Conversation").keybinding("Cmd+N"),
                Command::new("clear", "Clear Messages").keybinding("Cmd+K"),
                Command::new("export", "Export Chat").keybinding("Cmd+E"),
            ]),
            notifications: Notifications::new(),
            show_feedback: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.view
            .push_entry(ThreadEntry::new(ThreadEntryType::User, Text::new(content)));
    }

    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.view.push_entry(ThreadEntry::new(
            ThreadEntryType::Assistant,
            Text::new(content),
        ));
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.editor = MessageEditor::new().mode(mode);
        self.status_bar.update_item(
            "mode",
            crate::components::hud::StatusItemContent::Mode(mode),
        );
    }

    pub fn set_model(&mut self, model: Model) {
        self.status_bar.update_item(
            "model",
            crate::components::hud::StatusItemContent::Model(model),
        );
    }

    pub fn show_feedback(&mut self) {
        self.show_feedback = true;
    }

    pub fn hide_feedback(&mut self) {
        self.show_feedback = false;
    }

    pub fn open_command_palette(&mut self) {
        self.command_palette.open();
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.notifications.info(message);
    }

    pub fn notify_success(&mut self, message: impl Into<String>) {
        self.notifications.success(message);
    }

    pub fn notify_error(&mut self, message: impl Into<String>) {
        self.notifications.error(message);
    }
}

impl Default for ChatApplication {
    fn default() -> Self {
        Self::new("New Conversation")
    }
}

impl Component for ChatApplication {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let status_height = 28.0;
        let header_height = 48.0;
        let editor_height = 64.0;
        let feedback_height = if self.show_feedback { 80.0 } else { 0.0 };

        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );
        self.header.paint(header_bounds, cx);

        let view_top = bounds.origin.y + header_height;
        let view_height =
            bounds.size.height - header_height - editor_height - feedback_height - status_height;

        let view_bounds = Bounds::new(bounds.origin.x, view_top, bounds.size.width, view_height);
        self.view.paint(view_bounds, cx);

        if self.show_feedback {
            let feedback_bounds = Bounds::new(
                bounds.origin.x,
                view_top + view_height,
                bounds.size.width,
                feedback_height,
            );
            self.feedback.paint(feedback_bounds, cx);
        }

        let editor_y = if self.show_feedback {
            view_top + view_height + feedback_height
        } else {
            view_top + view_height
        };
        let editor_bounds =
            Bounds::new(bounds.origin.x, editor_y, bounds.size.width, editor_height);
        self.editor.paint(editor_bounds, cx);

        self.status_bar.paint(bounds, cx);

        self.notifications.paint(bounds, cx);

        self.command_palette.paint(bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if self.command_palette.is_open() {
            return self.command_palette.event(event, bounds, cx);
        }

        if let result @ EventResult::Handled = self.notifications.event(event, bounds, cx) {
            return result;
        }

        let status_height = 28.0;
        let header_height = 48.0;
        let editor_height = 64.0;
        let feedback_height = if self.show_feedback { 80.0 } else { 0.0 };

        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );
        if let result @ EventResult::Handled = self.header.event(event, header_bounds, cx) {
            return result;
        }

        let view_top = bounds.origin.y + header_height;
        let view_height =
            bounds.size.height - header_height - editor_height - feedback_height - status_height;

        let view_bounds = Bounds::new(bounds.origin.x, view_top, bounds.size.width, view_height);
        if let result @ EventResult::Handled = self.view.event(event, view_bounds, cx) {
            return result;
        }

        if self.show_feedback {
            let feedback_bounds = Bounds::new(
                bounds.origin.x,
                view_top + view_height,
                bounds.size.width,
                feedback_height,
            );
            if let result @ EventResult::Handled = self.feedback.event(event, feedback_bounds, cx) {
                return result;
            }
        }

        let editor_y = if self.show_feedback {
            view_top + view_height + feedback_height
        } else {
            view_top + view_height
        };
        let editor_bounds =
            Bounds::new(bounds.origin.x, editor_y, bounds.size.width, editor_height);
        if let result @ EventResult::Handled = self.editor.event(event, editor_bounds, cx) {
            return result;
        }

        self.status_bar.event(event, bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_application_new() {
        let app = ChatApplication::new("Test Chat");
        assert!(app.id.is_none());
        assert!(!app.show_feedback);
    }

    #[test]
    fn test_chat_application_with_id() {
        let app = ChatApplication::new("Test").with_id(42);
        assert_eq!(app.id, Some(42));
    }

    #[test]
    fn test_add_messages() {
        let mut app = ChatApplication::new("Test");
        app.add_user_message("Hello");
        app.add_assistant_message("Hi there!");
    }

    #[test]
    fn test_mode_and_model() {
        let mut app = ChatApplication::new("Test");
        app.set_mode(Mode::Plan);
        app.set_model(Model::CodexSonnet);
    }

    #[test]
    fn test_feedback_visibility() {
        let mut app = ChatApplication::new("Test");
        assert!(!app.show_feedback);

        app.show_feedback();
        assert!(app.show_feedback);

        app.hide_feedback();
        assert!(!app.show_feedback);
    }

    #[test]
    fn test_command_palette() {
        let mut app = ChatApplication::new("Test");
        assert!(!app.command_palette.is_open());

        app.open_command_palette();
        assert!(app.command_palette.is_open());
    }

    #[test]
    fn test_notifications() {
        let mut app = ChatApplication::new("Test");
        app.notify("Info message");
        app.notify_success("Success!");
        app.notify_error("Error occurred");

        assert_eq!(app.notifications.count(), 3);
    }

    #[test]
    fn test_default() {
        let app = ChatApplication::default();
        assert_eq!(app.header.title(), "New Conversation");
    }
}
