//! Thread feedback buttons (thumbs up/down).

use maud::{Markup, html};

/// Kind of feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackKind {
    /// Positive feedback
    ThumbsUp,
    /// Negative feedback
    ThumbsDown,
}

/// State of a feedback button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackState {
    /// Button is not selected
    Inactive,
    /// Button is selected
    Active,
}

impl FeedbackKind {
    /// Icon for the feedback type.
    fn icon(&self) -> &'static str {
        match self {
            FeedbackKind::ThumbsUp => "[+]",
            FeedbackKind::ThumbsDown => "[-]",
        }
    }

    /// Accessible label.
    fn label(&self) -> &'static str {
        match self {
            FeedbackKind::ThumbsUp => "Good response",
            FeedbackKind::ThumbsDown => "Bad response",
        }
    }
}

impl FeedbackState {
    /// CSS class for the state.
    fn class(&self) -> &'static str {
        match self {
            FeedbackState::Inactive => "opacity-50 hover:opacity-100",
            FeedbackState::Active => "opacity-100",
        }
    }
}

/// Render a feedback button.
///
/// # Arguments
/// * `kind` - Type of feedback (thumbs up/down)
/// * `state` - Current selection state
pub fn feedback_button(kind: FeedbackKind, state: FeedbackState) -> Markup {
    html! {
        button
            type="button"
            class={
                "text-lg p-1 hover:bg-secondary transition-opacity "
                (state.class())
            }
            data-feedback-kind=(format!("{:?}", kind))
            aria-pressed=(matches!(state, FeedbackState::Active))
            aria-label=(kind.label())
            title=(kind.label())
        {
            (kind.icon())
        }
    }
}
