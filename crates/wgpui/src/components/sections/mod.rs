mod message_editor;
mod thread_feedback;
mod thread_header;
mod thread_view;
mod trajectory_view;

pub use message_editor::MessageEditor;
pub use thread_feedback::{FeedbackRating, ThreadFeedback};
pub use thread_header::ThreadHeader;
pub use thread_view::ThreadView;
pub use trajectory_view::{TrajectoryEntry, TrajectoryView};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::atoms::Mode;
    use crate::components::organisms::{ThreadEntry, ThreadEntryType as EntryType};
    use crate::components::Text;

    #[test]
    fn test_sections_exports() {
        let _header = ThreadHeader::new("Test");
        let _view = ThreadView::new();
        let _editor = MessageEditor::new();
        let _feedback = ThreadFeedback::new();
        let _trajectory = TrajectoryView::new();
    }

    #[test]
    fn test_full_thread_layout() {
        let header = ThreadHeader::new("Conversation")
            .subtitle("3 messages")
            .show_back_button(true);

        let mut view = ThreadView::new().auto_scroll(true);
        view.push_entry(ThreadEntry::new(EntryType::User, Text::new("Hello")));
        view.push_entry(ThreadEntry::new(EntryType::Assistant, Text::new("Hi there!")));

        let editor = MessageEditor::new()
            .mode(Mode::Normal)
            .placeholder("Type a message...");

        assert_eq!(header.title(), "Conversation");
        assert_eq!(view.entry_count(), 2);
        assert_eq!(editor.value(), "");
    }
}
