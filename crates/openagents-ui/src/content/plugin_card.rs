//! Plugin cards: our real plugins, shown inline in a chat answer
//! (`docs/web/plugin-card.md`).
//!
//! A [`PluginCard`] shows one plugin: an icon, its name, one line on what
//! it does, where it runs, and at most one action, a link that works
//! (getting the app it runs in, or starting it). A card with no working
//! action shows none: there is no disabled or placeholder button.
//! [`PluginCards`] lays cards out as a list that is one column at phone
//! width and wraps into more as the thread widens.
//!
//! The builders take finished words: the caller fills them from the plugin
//! catalog, never from a model's text. Every string is escaped.
//!
//! Styles live in `static/components/plugin-card.css`.

use maud::{Markup, Render, html};

use crate::actions::{ButtonLink, ButtonVariant, Color, ControlSize};
use crate::icons::{Icon, IconSize};

/// One plugin: `article.oa-plugin-card`.
#[derive(Clone, Debug)]
pub struct PluginCard {
    name: String,
    icon: Icon,
    summary: String,
    runs_on: Option<String>,
    action: Option<(String, String)>,
}

impl PluginCard {
    /// A card for the plugin `name`, drawn with `icon`, that does what
    /// `summary` says in one line.
    #[must_use]
    pub fn new(name: impl Into<String>, icon: Icon, summary: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            icon,
            summary: summary.into(),
            runs_on: None,
            action: None,
        }
    }

    /// Where the plugin runs, in a few plain words ("With Coder on your
    /// computer").
    #[must_use]
    pub fn runs_on(mut self, runs_on: impl Into<String>) -> Self {
        self.runs_on = Some(runs_on.into());
        self
    }

    /// The card's one action: a link reading `label` to `href`. Only pass
    /// an action that works where the card is shown.
    #[must_use]
    pub fn action(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.action = Some((label.into(), href.into()));
        self
    }
}

impl Render for PluginCard {
    fn render(&self) -> Markup {
        html! {
            article.oa-plugin-card aria-label=(self.name) {
                div.oa-plugin-card__head {
                    span.oa-plugin-card__icon { (self.icon.size(IconSize::Md)) }
                    h3.oa-plugin-card__name { (self.name) }
                }
                p.oa-plugin-card__summary { (self.summary) }
                @if let Some(runs_on) = &self.runs_on {
                    p.oa-plugin-card__where {
                        (Icon::Desktop.size(IconSize::Xs))
                        span { (runs_on) }
                    }
                }
                @if let Some((label, href)) = &self.action {
                    div.oa-plugin-card__action {
                        (ButtonLink::new(label.as_str(), href.as_str())
                            .variant(ButtonVariant::Outline)
                            .color(Color::Secondary)
                            .size(ControlSize::Sm))
                    }
                }
            }
        }
    }
}

/// A list of [`PluginCard`]s: `ul.oa-plugin-cards`. Renders nothing
/// without cards.
#[derive(Clone, Debug)]
pub struct PluginCards {
    label: String,
    cards: Vec<PluginCard>,
}

impl PluginCards {
    /// An empty list named `label` for screen readers ("Plugins").
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            cards: Vec::new(),
        }
    }

    /// Adds a card.
    #[must_use]
    pub fn card(mut self, card: PluginCard) -> Self {
        self.cards.push(card);
        self
    }

    /// Adds cards.
    #[must_use]
    pub fn cards(mut self, cards: impl IntoIterator<Item = PluginCard>) -> Self {
        self.cards.extend(cards);
        self
    }

    /// Whether the list has no cards.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

impl Render for PluginCards {
    fn render(&self) -> Markup {
        html! {
            @if !self.cards.is_empty() {
                ul.oa-plugin-cards aria-label=(self.label) {
                    @for card in &self.cards {
                        li.oa-plugin-cards__item { (card) }
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
    fn a_card_shows_its_words_escaped_and_only_a_real_action() {
        let html = PluginCard::new(
            "Project map",
            Icon::Maps,
            "Shows Coder how the project is laid out <first>.",
        )
        .runs_on("With Coder on your computer")
        .action("Get Coder", "/download")
        .render()
        .into_string();
        assert!(html.contains(r#"<article class="oa-plugin-card" aria-label="Project map">"#));
        assert!(html.contains(r#"<h3 class="oa-plugin-card__name">Project map</h3>"#));
        assert!(html.contains("laid out &lt;first&gt;."));
        assert!(html.contains("<span>With Coder on your computer</span>"));
        assert!(html.contains(r#"href="/download""#));
        assert!(html.contains("Get Coder"));

        let bare = PluginCard::new("Code finder", Icon::Search, "Finds TODO notes.")
            .render()
            .into_string();
        assert!(!bare.contains("oa-plugin-card__action"));
        assert!(!bare.contains("oa-plugin-card__where"));
        assert!(!bare.contains("<button"));
        assert!(!bare.contains("<a "));
    }

    #[test]
    fn the_list_wraps_each_card_and_an_empty_list_renders_nothing() {
        assert!(
            PluginCards::new("Plugins")
                .render()
                .into_string()
                .is_empty()
        );
        let html = PluginCards::new("Plugins")
            .cards([
                PluginCard::new("Project map", Icon::Maps, "One."),
                PluginCard::new("Code finder", Icon::Search, "Two."),
            ])
            .render()
            .into_string();
        assert!(html.starts_with(r#"<ul class="oa-plugin-cards" aria-label="Plugins">"#));
        assert_eq!(
            html.matches(r#"<li class="oa-plugin-cards__item">"#)
                .count(),
            2
        );
        assert!(!html.contains("style="));
    }
}
