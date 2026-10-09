//! Answer components: the parts an answer draws inline so the reader can
//! start something from it (#11187): a titled card, cards side by side,
//! numbered steps with content, and tabs.
//!
//! Tabs need no script: each tab is a native radio button and CSS shows the
//! panel of the checked one (`static/components/answer.css`). Without
//! `:has()` support every panel shows, one under another, each under its
//! tab's name. `static/components/answer.js` only picks the starting tab
//! named for the reader's system (`data-oa-os`). Every string is escaped.

use maud::{Markup, Render, html};

/// A titled box: `section.oa-answer-card`.
#[derive(Clone, Debug)]
pub struct AnswerCard {
    title: String,
    body: Vec<Markup>,
}

impl AnswerCard {
    /// A card titled `title`.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: Vec::new(),
        }
    }

    /// Appends a block to the card.
    #[must_use]
    pub fn child(mut self, block: impl Render) -> Self {
        self.body.push(block.render());
        self
    }
}

impl Render for AnswerCard {
    fn render(&self) -> Markup {
        html! {
            section.oa-answer-card aria-label=(self.title) {
                h3.oa-answer-card__title { (self.title) }
                @if !self.body.is_empty() {
                    div.oa-answer-card__body { @for block in &self.body { (block) } }
                }
            }
        }
    }
}

/// Blocks side by side where the column allows, one under another on a
/// phone: `div.oa-answer-columns` in a size container.
#[derive(Clone, Debug, Default)]
pub struct AnswerColumns {
    children: Vec<Markup>,
}

impl AnswerColumns {
    /// No columns yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a column.
    #[must_use]
    pub fn child(mut self, block: impl Render) -> Self {
        self.children.push(block.render());
        self
    }
}

impl Render for AnswerColumns {
    fn render(&self) -> Markup {
        html! {
            div.oa-answer-columns-frame {
                div.oa-answer-columns { @for child in &self.children { div.oa-answer-columns__item { (child) } } }
            }
        }
    }
}

/// Numbered steps, each a short title and the blocks that do it:
/// `ol.oa-answer-steps`.
#[derive(Clone, Debug, Default)]
pub struct AnswerSteps {
    steps: Vec<(String, Vec<Markup>)>,
}

impl AnswerSteps {
    /// No steps yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a step titled `title` holding `body`.
    #[must_use]
    pub fn step(mut self, title: impl Into<String>, body: Vec<Markup>) -> Self {
        self.steps.push((title.into(), body));
        self
    }
}

impl Render for AnswerSteps {
    fn render(&self) -> Markup {
        html! {
            ol.oa-answer-steps {
                @for (n, (title, body)) in self.steps.iter().enumerate() {
                    li.oa-answer-step {
                        span.oa-answer-step__marker aria-hidden="true" { (n + 1) }
                        div.oa-answer-step__body {
                            p.oa-answer-step__title { (title) }
                            @for block in body { (block) }
                        }
                    }
                }
            }
        }
    }
}

/// The most tabs one set shows.
pub const MAX_TABS: usize = 6;

/// Tabs, one panel shown at a time: `div.oa-tabs`.
#[derive(Clone, Debug)]
pub struct Tabs {
    name: String,
    label: String,
    tabs: Vec<(String, Option<String>, Markup)>,
}

impl Tabs {
    /// A tab set named `label` for assistive technology. `name` groups its
    /// radio buttons and must be unique on the page; characters outside
    /// `[A-Za-z0-9_-]` are dropped.
    #[must_use]
    pub fn new(name: &str, label: impl Into<String>) -> Self {
        Self {
            name: name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect(),
            label: label.into(),
            tabs: Vec::new(),
        }
    }

    /// Appends a tab. Tabs past [`MAX_TABS`] are dropped.
    #[must_use]
    pub fn tab(mut self, label: impl Into<String>, panel: impl Render) -> Self {
        if self.tabs.len() < MAX_TABS {
            self.tabs.push((label.into(), None, panel.render()));
        }
        self
    }

    /// Appends a tab for one operating system (`unix` or `windows`); the
    /// page script starts on the reader's.
    #[must_use]
    pub fn tab_for(mut self, os: &str, label: impl Into<String>, panel: impl Render) -> Self {
        if self.tabs.len() < MAX_TABS {
            self.tabs
                .push((label.into(), Some(os.to_owned()), panel.render()));
        }
        self
    }
}

impl Render for Tabs {
    fn render(&self) -> Markup {
        html! {
            div.oa-tabs data-oa-tabs {
                div.oa-tabs__list role="radiogroup" aria-label=(self.label) {
                    @for (n, (label, os, _)) in self.tabs.iter().enumerate() {
                        label.oa-tabs__tab {
                            input.oa-tabs__input type="radio" name=(self.name) value=(n)
                                data-tab=(n) data-oa-os=[os.as_deref()] checked[n == 0];
                            span.oa-tabs__label { (label) }
                        }
                    }
                }
                @for (n, (label, _, panel)) in self.tabs.iter().enumerate() {
                    div.oa-tabs__panel data-tab=(n) {
                        p.oa-tabs__panel-label { (label) }
                        (panel)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ButtonLink;
    use crate::content::CodeBlock;

    #[test]
    fn cards_columns_and_steps_escape_and_nest() {
        let html = AnswerColumns::new()
            .child(
                AnswerCard::new("On the <web>")
                    .child(ButtonLink::new("Connect GitHub", "/projects")),
            )
            .child(
                AnswerCard::new("On your computer").child(AnswerSteps::new().step(
                    "Install Coder",
                    vec![CodeBlock::new("coder login").render()],
                )),
            )
            .render()
            .into_string();
        assert!(
            html.contains("<h3 class=\"oa-answer-card__title\">On the &lt;web&gt;</h3>"),
            "{html}"
        );
        assert!(html.contains("href=\"/projects\""), "{html}");
        assert!(
            html.contains("<span class=\"oa-answer-step__marker\" aria-hidden=\"true\">1</span>"),
            "{html}"
        );
        assert!(html.contains("data-oa-copy=\"coder login\""), "{html}");
        assert_eq!(html.matches("oa-answer-columns__item").count(), 2);
    }

    #[test]
    fn tabs_are_radios_with_a_panel_each() {
        let html = Tabs::new("os-1\"><x", "Install command")
            .tab_for("unix", "macOS and Linux", CodeBlock::new("a"))
            .tab_for("windows", "Windows", CodeBlock::new("b"))
            .render()
            .into_string();
        assert!(html.contains("name=\"os-1x\""), "{html}");
        assert_eq!(html.matches("type=\"radio\"").count(), 2);
        assert_eq!(html.matches(" checked").count(), 1);
        assert!(html.contains("data-oa-os=\"windows\""), "{html}");
        assert!(
            html.contains("class=\"oa-tabs__panel\" data-tab=\"1\""),
            "{html}"
        );
        assert!(
            !html.contains("<script") && !html.contains("style="),
            "{html}"
        );
    }
}
