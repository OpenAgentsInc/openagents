//! Thread feedback section component.

use maud::{Markup, html};
use crate::acp::atoms::{feedback_button, FeedbackKind, FeedbackState};

/// Thread feedback section for rating responses.
pub struct ThreadFeedback {
    session_id: String,
    selected: Option<FeedbackKind>,
    show_comment: bool,
}

impl ThreadFeedback {
    /// Create a new thread feedback section.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            selected: None,
            show_comment: false,
        }
    }

    /// Set the selected feedback.
    pub fn selected(mut self, kind: FeedbackKind) -> Self {
        self.selected = Some(kind);
        self.show_comment = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let thumbs_up_state = if self.selected == Some(FeedbackKind::ThumbsUp) {
            FeedbackState::Active
        } else {
            FeedbackState::Inactive
        };

        let thumbs_down_state = if self.selected == Some(FeedbackKind::ThumbsDown) {
            FeedbackState::Active
        } else {
            FeedbackState::Inactive
        };

        html! {
            div class="px-4 py-3 border-t border-border bg-secondary/50" {
                div class="flex items-center gap-3" {
                    span class="text-sm text-muted-foreground" {
                        "How was this response?"
                    }

                    div class="flex gap-1" {
                        (feedback_button(FeedbackKind::ThumbsUp, thumbs_up_state))
                        (feedback_button(FeedbackKind::ThumbsDown, thumbs_down_state))
                    }
                }

                // Comment field (shown after selection)
                @if self.show_comment {
                    div class="mt-3" {
                        textarea
                            class="w-full px-3 py-2 text-sm bg-background border border-border focus:outline-none focus:border-primary resize-none"
                            rows="2"
                            placeholder="Add a comment (optional)..."
                            data-feedback-comment=(self.session_id)
                        {}

                        div class="mt-2 flex justify-end" {
                            button
                                type="button"
                                class="px-3 py-1 text-xs bg-primary text-primary-foreground"
                                data-submit-feedback=(self.session_id)
                            {
                                "Submit Feedback"
                            }
                        }
                    }
                }
            }
        }
    }
}
