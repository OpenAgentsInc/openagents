//! Suggestion chips: questions to ask with one tap, as on the ChatGPT home
//! page, and links to a page a reply points to.
//!
//! A chip that asks is its own small `<form method="post">` with hidden
//! fields and one submit button, so it works without JavaScript; with
//! [`SuggestionChips::enhanced`] it posts with HTMX (`hx-swap="none"`), as
//! the enhanced [`super::Composer`] does. A chip that opens a page is a
//! plain link.
//!
//! Styles live in `static/components/suggestions.css`.

use maud::{Markup, Render, html};

/// One chip in a [`SuggestionChips`] row.
#[derive(Clone, Debug)]
pub struct SuggestionChip {
    label: String,
    target: Target,
}

#[derive(Clone, Debug)]
enum Target {
    /// Post `fields` to `action`.
    Send {
        action: String,
        fields: Vec<(String, String)>,
    },
    /// Open `href`.
    Link { href: String },
}

impl SuggestionChip {
    /// A chip reading `label` that posts `fields` (name, value) to `action`.
    #[must_use]
    pub fn send(
        label: impl Into<String>,
        action: impl Into<String>,
        fields: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        Self {
            label: label.into(),
            target: Target::Send {
                action: action.into(),
                fields: fields.into_iter().collect(),
            },
        }
    }

    /// A chip reading `label` that opens `href`.
    #[must_use]
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            target: Target::Link { href: href.into() },
        }
    }
}

/// A row of [`SuggestionChip`]s: `div.oa-suggestions[role=group]`. Renders
/// nothing without chips.
#[derive(Clone, Debug)]
pub struct SuggestionChips {
    label: String,
    id: Option<String>,
    enhanced: bool,
    chips: Vec<SuggestionChip>,
}

impl SuggestionChips {
    /// An empty row named `label` for screen readers ("Suggestions").
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            id: None,
            enhanced: false,
            chips: Vec::new(),
        }
    }

    /// The element id.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Chips that ask post with HTMX and swap nothing; the page's own
    /// responses (out-of-band swaps, `HX-Redirect`) say what happened.
    #[must_use]
    pub fn enhanced(mut self, enhanced: bool) -> Self {
        self.enhanced = enhanced;
        self
    }

    /// Adds a chip.
    #[must_use]
    pub fn chip(mut self, chip: SuggestionChip) -> Self {
        self.chips.push(chip);
        self
    }

    /// Adds chips.
    #[must_use]
    pub fn chips(mut self, chips: impl IntoIterator<Item = SuggestionChip>) -> Self {
        self.chips.extend(chips);
        self
    }

    /// Whether the row has no chips.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chips.is_empty()
    }
}

impl Render for SuggestionChips {
    fn render(&self) -> Markup {
        let enhanced = self.enhanced;
        html! {
            @if !self.chips.is_empty() {
                div class="oa-suggestions" role="group" aria-label=(self.label) id=[self.id.as_deref()] {
                    @for chip in &self.chips {
                        @match &chip.target {
                            Target::Send { action, fields } => {
                                form class="oa-suggestion-form" action=(action) method="post"
                                    hx-post=[enhanced.then_some(action.as_str())]
                                    hx-swap=[enhanced.then_some("none")]
                                    hx-sync=[enhanced.then_some("this:drop")]
                                    hx-disabled-elt=[enhanced.then_some("find button")] {
                                    @for (name, value) in fields {
                                        input type="hidden" name=(name) value=(value);
                                    }
                                    button type="submit" class="oa-suggestion-chip" { (chip.label) }
                                }
                            }
                            Target::Link { href } => {
                                a class="oa-suggestion-chip" href=(href) { (chip.label) }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_post_their_fields_or_link_and_an_empty_row_renders_nothing() {
        assert!(
            SuggestionChips::new("Suggestions")
                .render()
                .into_string()
                .is_empty()
        );
        let html = SuggestionChips::new("Suggestions")
            .id("chat-suggestions")
            .chip(SuggestionChip::send(
                "Who are you?",
                "/chat",
                [("q".to_owned(), "Who are you?".to_owned())],
            ))
            .chip(SuggestionChip::link(
                "Connect a computer",
                "/docs/connect-a-computer",
            ))
            .render()
            .into_string();
        assert!(html.contains(r#"<div class="oa-suggestions" role="group" aria-label="Suggestions" id="chat-suggestions">"#));
        assert!(html.contains(r#"<form class="oa-suggestion-form" action="/chat" method="post">"#));
        assert!(html.contains(r#"<input type="hidden" name="q" value="Who are you?">"#));
        assert!(
            html.contains(
                r#"<button type="submit" class="oa-suggestion-chip">Who are you?</button>"#
            )
        );
        assert!(html.contains(r#"<a class="oa-suggestion-chip" href="/docs/connect-a-computer">Connect a computer</a>"#));
        assert!(!html.contains("hx-post"));
        let enhanced = SuggestionChips::new("Suggestions")
            .enhanced(true)
            .chip(SuggestionChip::send("Ask", "/chat/1", []))
            .render()
            .into_string();
        assert!(enhanced.contains(r#"hx-post="/chat/1" hx-swap="none""#));
    }
}
