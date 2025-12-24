//! Feedback button story.

use maud::{Markup, html};
use ui::acp::atoms::{feedback_button, FeedbackKind, FeedbackState};

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

fn row(content: Markup) -> Markup {
    html! { div class="flex gap-6 items-center flex-wrap" { (content) } }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn feedback_button_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Feedback Button"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Thumbs up/down buttons for thread rating."
        }

        (section_title("Inactive State"))
        (section(row(html! {
            (item("Thumbs Up", feedback_button(FeedbackKind::ThumbsUp, FeedbackState::Inactive)))
            (item("Thumbs Down", feedback_button(FeedbackKind::ThumbsDown, FeedbackState::Inactive)))
        })))

        (section_title("Active State"))
        (section(row(html! {
            (item("Thumbs Up", feedback_button(FeedbackKind::ThumbsUp, FeedbackState::Active)))
            (item("Thumbs Down", feedback_button(FeedbackKind::ThumbsDown, FeedbackState::Active)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{feedback_button, FeedbackKind, FeedbackState};

// Inactive thumbs up
feedback_button(FeedbackKind::ThumbsUp, FeedbackState::Inactive)

// Active (selected) thumbs down
feedback_button(FeedbackKind::ThumbsDown, FeedbackState::Active)"#))
    }
}
