//! Link cards: a grid of rectangular "learn about" cards, blog-post style,
//! where the whole card is one link (the homepage's new-chat state).
//!
//! A [`LinkCard`] shows an icon, a title, and a one-line description, all
//! inside one `<a>`, so a pointer, a tap, or the keyboard opens it. A card
//! whose target fails [`safe_href`] renders nothing: there is no card that
//! goes nowhere. Links that leave the site open in a new tab with
//! `noopener`.
//!
//! [`LinkCards`] lays cards out two small ones a row at phone width (the
//! icon inline with the title, the description cut to one line), two
//! roomier ones at tablet width, and three where the column allows (a container query, so the
//! grid follows the space it is given, not the window).
//!
//! Every string is escaped. Styles live in `static/components/link-card.css`.

use maud::{Markup, Render, html};

use super::{is_external, safe_href};
use crate::icons::{Icon, IconSize};

/// One card: `a.oa-link-card`.
#[derive(Clone, Debug)]
pub struct LinkCard {
    title: String,
    description: String,
    href: String,
    icon: Option<Icon>,
}

impl LinkCard {
    /// A card titled `title` that says `description` in one line and opens
    /// `href`.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        href: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            href: href.into(),
            icon: None,
        }
    }

    /// The icon drawn above the title.
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Whether the card renders: its target is a link the site allows.
    #[must_use]
    pub fn is_link(&self) -> bool {
        safe_href(&self.href).is_some()
    }
}

impl Render for LinkCard {
    fn render(&self) -> Markup {
        let Some(href) = safe_href(&self.href) else {
            return html! {};
        };
        let external = is_external(href);
        html! {
            a.oa-link-card href=(href)
                target=[external.then_some("_blank")]
                rel=[external.then_some("noopener noreferrer")] {
                @if let Some(icon) = self.icon {
                    span.oa-link-card__icon aria-hidden="true" { (icon.size(IconSize::Md)) }
                }
                span.oa-link-card__title { (self.title) }
                span.oa-link-card__description { (self.description) }
            }
        }
    }
}

/// A grid of [`LinkCard`]s: `ul.oa-link-cards`. Renders nothing without a
/// card that links somewhere.
#[derive(Clone, Debug)]
pub struct LinkCards {
    label: String,
    cards: Vec<LinkCard>,
}

impl LinkCards {
    /// An empty grid named `label` for screen readers ("Learn more").
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            cards: Vec::new(),
        }
    }

    /// Adds a card.
    #[must_use]
    pub fn card(mut self, card: LinkCard) -> Self {
        self.cards.push(card);
        self
    }

    /// Adds cards.
    #[must_use]
    pub fn cards(mut self, cards: impl IntoIterator<Item = LinkCard>) -> Self {
        self.cards.extend(cards);
        self
    }
}

impl Render for LinkCards {
    fn render(&self) -> Markup {
        let cards: Vec<&LinkCard> = self.cards.iter().filter(|c| c.is_link()).collect();
        html! {
            @if !cards.is_empty() {
                div.oa-link-cards-frame {
                    ul.oa-link-cards aria-label=(self.label) {
                        @for card in cards {
                            li.oa-link-cards__item { (card) }
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
    fn a_card_is_one_escaped_link_and_external_links_open_safely() {
        let html = LinkCard::new("Meet <Coder>", "In your terminal.", "/docs/coder")
            .icon(Icon::Terminal)
            .render()
            .into_string();
        assert!(html.starts_with(r#"<a class="oa-link-card" href="/docs/coder">"#));
        assert!(html.contains("Meet &lt;Coder&gt;"));
        assert!(
            html.contains(r#"<span class="oa-link-card__description">In your terminal.</span>"#)
        );
        assert!(html.contains("oa-link-card__icon"));
        assert!(!html.contains("target="));
        assert_eq!(html.matches("<a ").count(), 1);

        let out = LinkCard::new("Code", "On GitHub.", "https://github.com/x/y")
            .render()
            .into_string();
        assert!(out.contains(r#"target="_blank" rel="noopener noreferrer""#));
        assert!(!out.contains("oa-link-card__icon"));
    }

    #[test]
    fn a_card_that_goes_nowhere_is_left_out() {
        assert!(
            LinkCard::new("Bad", "x", "javascript:alert(1)")
                .render()
                .into_string()
                .is_empty()
        );
        assert!(LinkCards::new("Learn").render().into_string().is_empty());
        let html = LinkCards::new("Learn")
            .cards([
                LinkCard::new("One", "1", "/a"),
                LinkCard::new("Two", "2", ""),
                LinkCard::new("Three", "3", "/c"),
            ])
            .render()
            .into_string();
        assert!(html.contains(r#"<ul class="oa-link-cards" aria-label="Learn">"#));
        assert_eq!(
            html.matches(r#"<li class="oa-link-cards__item">"#).count(),
            2
        );
        assert!(!html.contains("style="));
    }
}
