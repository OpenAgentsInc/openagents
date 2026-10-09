//! Document pages: the reading column and the facts grid. The other page
//! classes in `static/components/page.css` (`oa-page-eyebrow`,
//! `oa-page-lead`, `oa-page-meta`, `oa-page-actions`, `oa-page-section`,
//! `oa-card`, `oa-disclosure`, `oa-item-list`, `oa-item-title`, `oa-chart`,
//! `oa-canvas-panel`, `oa-event-list`) are plain classes on plain elements.

use maud::{Markup, Render, html};

/// The reading column of a scrolling page: `div.oa-page`, centered, with
/// the page's padding. [`PageColumn::wide`] widens it for tables.
#[derive(Clone, Debug)]
pub struct PageColumn {
    content: Markup,
    wide: bool,
}

impl PageColumn {
    /// A column holding `content`.
    #[must_use]
    pub fn new(content: impl Render) -> Self {
        Self {
            content: content.render(),
            wide: false,
        }
    }

    /// A wider column (64rem instead of 48rem), for pages of tables.
    #[must_use]
    pub fn wide(mut self) -> Self {
        self.wide = true;
        self
    }
}

impl Render for PageColumn {
    fn render(&self) -> Markup {
        html! {
            div.oa-page data-width=[self.wide.then_some("wide")] { (self.content) }
        }
    }
}

/// A grid of labelled values: `dl.oa-facts`, one `div > dt + dd` per fact.
/// A value may carry an id, so a page script can update it in place.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    id: Option<String>,
    facts: Vec<(String, Markup, Option<String>)>,
}

impl Facts {
    /// An empty grid.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The `dl`'s id.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Appends a fact (the term is escaped; the value renders).
    #[must_use]
    pub fn fact(mut self, term: impl Into<String>, value: impl Render) -> Self {
        self.facts.push((term.into(), value.render(), None));
        self
    }

    /// Appends a fact whose `dd` has the id `value_id`.
    #[must_use]
    pub fn fact_with_id(
        mut self,
        term: impl Into<String>,
        value: impl Render,
        value_id: impl Into<String>,
    ) -> Self {
        self.facts
            .push((term.into(), value.render(), Some(value_id.into())));
        self
    }
}

impl Render for Facts {
    fn render(&self) -> Markup {
        html! {
            dl.oa-facts id=[self.id.as_deref()] {
                @for (term, value, value_id) in &self.facts {
                    div { dt { (term) } dd id=[value_id.as_deref()] { (value) } }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_centers_content_and_widens_on_request() {
        let narrow = PageColumn::new(html! { p { "x" } }).render().into_string();
        assert_eq!(narrow, "<div class=\"oa-page\"><p>x</p></div>");
        let wide = PageColumn::new("<b>").wide().render().into_string();
        assert_eq!(wide, "<div class=\"oa-page\" data-width=\"wide\">&lt;b&gt;</div>");
    }

    #[test]
    fn facts_escape_terms_and_carry_value_ids() {
        let html = Facts::new()
            .id("totals")
            .fact("<Received>", "12 sats")
            .fact_with_id("Calls", "3", "calls")
            .render()
            .into_string();
        assert_eq!(
            html,
            "<dl class=\"oa-facts\" id=\"totals\"><div><dt>&lt;Received&gt;</dt><dd>12 sats</dd></div>\
<div><dt>Calls</dt><dd id=\"calls\">3</dd></div></dl>"
        );
    }
}
