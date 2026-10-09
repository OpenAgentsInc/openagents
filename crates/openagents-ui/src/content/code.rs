//! Code blocks and the sticky action bar that heads them.

use maud::{Markup, Render, html};

use crate::actions::{ButtonVariant, Color, ControlSize, CopyButton};

/// A toolbar that sticks to the top of its scroll container while its
/// block is in view: `div.oa-sticky-action-bar`. A code block uses one as
/// its header (language on the left, copy on the right); other content can
/// use it too. Set `--sticky-action-bar-top` to clear a fixed page header.
#[derive(Clone, Debug, Default)]
pub struct StickyActionBar {
    label: Option<Markup>,
    actions: Vec<Markup>,
    aria_label: Option<String>,
}

impl StickyActionBar {
    /// An empty bar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The leading label (text is escaped).
    #[must_use]
    pub fn label(mut self, label: impl Render) -> Self {
        self.label = Some(label.render());
        self
    }

    /// Appends a trailing action, such as a button.
    #[must_use]
    pub fn action(mut self, action: impl Render) -> Self {
        self.actions.push(action.render());
        self
    }

    /// The bar's accessible name, used when it has actions.
    #[must_use]
    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl Render for StickyActionBar {
    fn render(&self) -> Markup {
        let has_actions = !self.actions.is_empty();
        let role = has_actions.then_some("toolbar");
        let aria = if has_actions {
            self.aria_label.as_deref()
        } else {
            None
        };
        html! {
            div.oa-sticky-action-bar role=[role] aria-label=[aria] {
                @if let Some(label) = &self.label {
                    span.oa-sticky-action-bar__label { (label) }
                }
                @if has_actions {
                    div.oa-sticky-action-bar__actions {
                        @for action in &self.actions { (action) }
                    }
                }
            }
        }
    }
}

/// A block of code: a surface (`div.oa-code-block`), an optional sticky
/// header with the language and a copy action, and a scrolling pane with
/// `pre > code`. Code is monospace and never wraps unless [`wrap`] is set;
/// long lines scroll sideways inside the pane.
///
/// The copy action is a [`CopyButton`] labelled "Copy" that carries the
/// code in `data-oa-copy`; the CopyButton script hook
/// ([`crate::actions::COPY_BUTTON_JS`], part of [`crate::script()`])
/// copies it in one click, with no inline script.
///
/// [`wrap`]: CodeBlock::wrap
#[derive(Clone, Debug)]
pub struct CodeBlock {
    code: String,
    highlighted: Option<Markup>,
    language: Option<String>,
    copyable: bool,
    copy_icon: Option<Markup>,
    wrap: bool,
}

impl CodeBlock {
    /// A block showing `code` as escaped text.
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            highlighted: None,
            language: None,
            copyable: true,
            copy_icon: None,
            wrap: false,
        }
    }

    /// The language: shown in the header and set as `language-<name>` on
    /// the `code` element. Characters outside `[A-Za-z0-9+#._-]` are
    /// dropped.
    #[must_use]
    pub fn language(mut self, language: &str) -> Self {
        let clean: String = language
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '_' | '-'))
            .take(32)
            .collect();
        self.language = (!clean.is_empty()).then_some(clean);
        self
    }

    /// Already highlighted markup to show in place of the plain text, for
    /// example spans with Prism-style `token` classes. The caller vouches
    /// that it is escaped.
    #[must_use]
    pub fn highlighted(mut self, markup: Markup) -> Self {
        self.highlighted = Some(markup);
        self
    }

    /// Whether to offer the copy action (default: yes).
    #[must_use]
    pub fn copyable(mut self, copyable: bool) -> Self {
        self.copyable = copyable;
        self
    }

    /// An icon for the copy action, drawn before its "Copy" text.
    #[must_use]
    pub fn copy_icon(mut self, icon: impl Render) -> Self {
        self.copy_icon = Some(icon.render());
        self
    }

    /// Wrap long lines instead of scrolling (default: no).
    #[must_use]
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    fn copy_button(&self) -> Markup {
        let mut button = CopyButton::new(self.code.clone())
            .label("Copy")
            .aria_label("Copy code")
            .size(ControlSize::Sm)
            .variant(ButtonVariant::Ghost)
            .color(Color::Secondary);
        if let Some(icon) = &self.copy_icon {
            button = button.copy_icon(icon.clone());
        }
        button.render()
    }
}

impl Render for CodeBlock {
    fn render(&self) -> Markup {
        let language = self.language.as_deref();
        let code_class = language.map(|l| format!("oa-code-block__code language-{l}"));
        let header = (language.is_some() || self.copyable).then(|| {
            let mut bar = StickyActionBar::new().aria_label("Code actions");
            if let Some(language) = language {
                bar = bar.label(language);
            }
            if self.copyable {
                bar = bar.action(self.copy_button());
            }
            bar
        });
        let pane_label = language.map_or_else(|| "Code".to_owned(), |l| format!("{l} code"));
        html! {
            div.oa-code-block data-language=[language] data-wrap[self.wrap] {
                @if let Some(header) = &header { (header) }
                div.oa-code-block__pane tabindex="0" role="region" aria-label=(pane_label) {
                    pre.oa-code-block__pre {
                        code class=(code_class.as_deref().unwrap_or("oa-code-block__code")) {
                            @if let Some(markup) = &self.highlighted {
                                (markup)
                            } @else {
                                (self.code)
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
    use maud::PreEscaped;

    #[test]
    fn code_is_escaped_and_scrolls_in_a_pane() {
        let html = CodeBlock::new("fn main() { println!(\"<hi>\"); }")
            .language("rust\" onclick=\"x")
            .render()
            .into_string();
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(!html.contains("onclick="), "{html}");
        assert!(html.contains("data-language=\"rustonclickx\""), "{html}");
        assert!(
            html.contains("class=\"oa-code-block__code language-rustonclickx\""),
            "{html}"
        );
        assert!(html.contains("class=\"oa-code-block__pane\" tabindex=\"0\""));
        assert!(html.contains("role=\"toolbar\" aria-label=\"Code actions\""));
        // The copy action is a CopyButton carrying the (unescaped) code.
        assert!(
            html.contains("data-oa-copy=\"fn main() { println!(&quot;&lt;hi&gt;&quot;); }\""),
            "{html}"
        );
        assert!(html.contains("<span>Copy</span>"), "{html}");
        assert!(html.contains("aria-label=\"Copy code\""), "{html}");
        assert!(!html.contains("data-wrap"), "{html}");
    }

    #[test]
    fn options() {
        let html = CodeBlock::new("x")
            .copyable(false)
            .wrap(true)
            .render()
            .into_string();
        assert!(!html.contains("oa-sticky-action-bar"), "{html}");
        assert!(html.contains("data-wrap"), "{html}");
        assert!(html.contains("class=\"oa-code-block__code\""), "{html}");

        let html = CodeBlock::new("ignored")
            .highlighted(PreEscaped(
                "<span class=\"token keyword\">fn</span>".to_owned(),
            ))
            .copy_icon(PreEscaped("<svg></svg>"))
            .render()
            .into_string();
        assert!(html.contains("<span class=\"token keyword\">fn</span>"));
        assert!(!html.contains(">ignored<"));
        assert!(
            html.contains("data-copy-icon=\"copy\"><svg></svg></span>"),
            "{html}"
        );
        assert!(html.contains("data-oa-copy=\"ignored\""), "{html}");
    }

    #[test]
    fn bar_without_actions_is_not_a_toolbar() {
        let html = StickyActionBar::new()
            .label("<b>")
            .aria_label("x")
            .render()
            .into_string();
        assert_eq!(
            html,
            "<div class=\"oa-sticky-action-bar\"><span class=\"oa-sticky-action-bar__label\">&lt;b&gt;</span></div>"
        );
    }
}
