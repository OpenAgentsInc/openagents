//! Plan cards: the plans side by side, as the pricing page shows them.
//!
//! A [`PlanCard`] shows one plan: its name, its price in the accent color,
//! a short lead ("Everything in Free, and:"), a checklist of what it
//! includes (each item may carry small grey lines under it), and one
//! action at the bottom: a link that works. A card with no action shows
//! none; there is no disabled or placeholder button. [`PlanCards`] lays the
//! cards out in a row that stacks into one column at phone width, under an
//! optional centered heading and one-line lead.
//!
//! The builders take finished words; every string is escaped.
//!
//! Styles live in `static/components/plan-card.css`.

use maud::{Markup, Render, html};

use crate::actions::{ButtonLink, ButtonVariant, Color, ControlSize};
use crate::icons::{Icon, IconSize};

/// One checklist line and the small lines under it.
#[derive(Clone, Debug)]
struct Item {
    text: String,
    notes: Vec<String>,
}

/// One plan: `article.oa-plan-card`.
#[derive(Clone, Debug)]
pub struct PlanCard {
    name: String,
    price: String,
    per: Option<String>,
    note: Option<String>,
    lead: Option<String>,
    items: Vec<Item>,
    action: Option<(String, String)>,
    featured: bool,
}

impl PlanCard {
    /// A card for the plan `name` at `price` ("Free", "$20").
    #[must_use]
    pub fn new(name: impl Into<String>, price: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            price: price.into(),
            per: None,
            note: None,
            lead: None,
            items: Vec::new(),
            action: None,
            featured: false,
        }
    }

    /// The period after the price, drawn smaller ("/mo.").
    #[must_use]
    pub fn per(mut self, per: impl Into<String>) -> Self {
        self.per = Some(per.into());
        self
    }

    /// One short line under the price ("3-day free trial").
    #[must_use]
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The line over the checklist ("Everything in Free, and:").
    #[must_use]
    pub fn lead(mut self, lead: impl Into<String>) -> Self {
        self.lead = Some(lead.into());
        self
    }

    /// Adds a checklist line.
    #[must_use]
    pub fn item(self, text: impl Into<String>) -> Self {
        self.item_with(text, std::iter::empty::<String>())
    }

    /// Adds a checklist line with small grey lines under it.
    #[must_use]
    pub fn item_with(
        mut self,
        text: impl Into<String>,
        notes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.items.push(Item {
            text: text.into(),
            notes: notes.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// The card's one action: a link reading `label` to `href`. Only pass
    /// an action that works where the card is shown.
    #[must_use]
    pub fn action(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.action = Some((label.into(), href.into()));
        self
    }

    /// The plan to point people at: an accented border and a solid button.
    #[must_use]
    pub fn featured(mut self, featured: bool) -> Self {
        self.featured = featured;
        self
    }
}

impl Render for PlanCard {
    fn render(&self) -> Markup {
        html! {
            article.oa-plan-card data-featured[self.featured] aria-label=(self.name) {
                h2.oa-plan-card__name { (self.name) }
                p.oa-plan-card__price {
                    span.oa-plan-card__amount { (self.price) }
                    @if let Some(per) = &self.per {
                        span.oa-plan-card__per { (per) }
                    }
                }
                @if let Some(note) = &self.note {
                    p.oa-plan-card__note { (note) }
                }
                @if let Some(lead) = &self.lead {
                    p.oa-plan-card__lead { (lead) }
                }
                @if !self.items.is_empty() {
                    ul.oa-plan-card__items {
                        @for item in &self.items {
                            li.oa-plan-card__item {
                                span.oa-plan-card__check aria-hidden="true" {
                                    (Icon::Check.size(IconSize::Sm))
                                }
                                span.oa-plan-card__text {
                                    span { (item.text) }
                                    @for note in &item.notes {
                                        span.oa-plan-card__item-note { (note) }
                                    }
                                }
                            }
                        }
                    }
                }
                @if let Some((label, href)) = &self.action {
                    div.oa-plan-card__action {
                        @if self.featured {
                            (ButtonLink::new(label.as_str(), href.as_str())
                                .size(ControlSize::Lg)
                                .block(true))
                        } @else {
                            (ButtonLink::new(label.as_str(), href.as_str())
                                .variant(ButtonVariant::Outline)
                                .color(Color::Secondary)
                                .size(ControlSize::Lg)
                                .block(true))
                        }
                    }
                }
            }
        }
    }
}

/// The plans side by side: `section.oa-plan-cards`, with an optional
/// centered heading and lead. Renders nothing without cards.
#[derive(Clone, Debug)]
pub struct PlanCards {
    label: String,
    heading: Option<String>,
    lead: Option<String>,
    cards: Vec<PlanCard>,
}

impl PlanCards {
    /// An empty row named `label` for screen readers ("Plans").
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            heading: None,
            lead: None,
            cards: Vec::new(),
        }
    }

    /// The page heading over the cards, drawn large and centered (`h1`).
    #[must_use]
    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    /// One line under the heading.
    #[must_use]
    pub fn lead(mut self, lead: impl Into<String>) -> Self {
        self.lead = Some(lead.into());
        self
    }

    /// Adds a card.
    #[must_use]
    pub fn card(mut self, card: PlanCard) -> Self {
        self.cards.push(card);
        self
    }

    /// Adds cards.
    #[must_use]
    pub fn cards(mut self, cards: impl IntoIterator<Item = PlanCard>) -> Self {
        self.cards.extend(cards);
        self
    }
}

impl Render for PlanCards {
    fn render(&self) -> Markup {
        html! {
            @if !self.cards.is_empty() {
                section.oa-plan-cards aria-label=(self.label) {
                    @if self.heading.is_some() || self.lead.is_some() {
                        header.oa-plan-cards__head {
                            @if let Some(heading) = &self.heading {
                                h1.oa-plan-cards__heading { (heading) }
                            }
                            @if let Some(lead) = &self.lead {
                                p.oa-plan-cards__lead { (lead) }
                            }
                        }
                    }
                    ul.oa-plan-cards__list {
                        @for card in &self.cards {
                            li.oa-plan-cards__item { (card) }
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
    fn a_card_shows_its_price_checklist_notes_and_only_a_real_action() {
        let html = PlanCard::new("Pro", "$20")
            .per("/mo.")
            .lead("Everything in Free, and:")
            .item_with("6,000 minutes <a month>", ["2 vCPU, 8 GB"])
            .item("Uploads")
            .action("Start", "/settings")
            .featured(true)
            .render()
            .into_string();
        assert!(html.contains(r#"<article class="oa-plan-card" data-featured aria-label="Pro">"#));
        assert!(html.contains(r#"<span class="oa-plan-card__amount">$20</span>"#));
        assert!(html.contains("6,000 minutes &lt;a month&gt;"));
        assert!(html.contains(r#"<span class="oa-plan-card__item-note">2 vCPU, 8 GB</span>"#));
        assert_eq!(html.matches("<li ").count(), 2);
        assert!(html.contains(r#"href="/settings""#));

        let bare = PlanCard::new("Free", "Free").render().into_string();
        assert!(!bare.contains("oa-plan-card__action"));
        assert!(!bare.contains("data-featured"));
        assert!(!bare.contains("<a ") && !bare.contains("<button"));
    }

    #[test]
    fn the_row_has_its_heading_and_an_empty_row_renders_nothing() {
        assert!(PlanCards::new("Plans").render().into_string().is_empty());
        let html = PlanCards::new("Plans")
            .heading("Pricing")
            .lead("Start free.")
            .cards([PlanCard::new("Free", "Free"), PlanCard::new("Pro", "$20")])
            .render()
            .into_string();
        assert!(html.starts_with(r#"<section class="oa-plan-cards" aria-label="Plans">"#));
        assert!(html.contains(r#"<h1 class="oa-plan-cards__heading">Pricing</h1>"#));
        assert_eq!(
            html.matches(r#"<li class="oa-plan-cards__item">"#).count(),
            2
        );
        assert!(!html.contains("style="));
    }
}
