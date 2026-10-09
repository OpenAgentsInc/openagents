//! The Markdown root and the prose elements: paragraph, heading, list and
//! list item, inline code.

use maud::{Markup, Render, html};

/// Text size of a [`MarkdownRoot`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkdownSize {
    /// Conversation and document text (16px).
    #[default]
    Md,
    /// Dense panels (14px).
    Sm,
}

impl MarkdownSize {
    fn attr(self) -> Option<&'static str> {
        match self {
            Self::Md => None,
            Self::Sm => Some("sm"),
        }
    }
}

/// The root of a block of Markdown content: `div.oa-markdown`.
///
/// It styles whatever it wraps, including the HTML the site's Markdown
/// renderer writes. That HTML is already escaped and link-checked by the
/// renderer, so it is passed as trusted markup:
///
/// ```
/// use maud::PreEscaped;
/// use openagents_ui::content::MarkdownRoot;
///
/// let rendered = "<p>Hello <code>world</code></p>\n".to_owned(); // markdown::render(..)
/// let root = MarkdownRoot::new(PreEscaped(rendered));
/// ```
///
/// A plain `&str` or `String` passed as content is escaped as text.
#[derive(Clone, Debug)]
pub struct MarkdownRoot {
    content: Markup,
    size: MarkdownSize,
    label: Option<String>,
    streaming: bool,
}

impl MarkdownRoot {
    /// Wraps `content`.
    #[must_use]
    pub fn new(content: impl Render) -> Self {
        Self {
            content: content.render(),
            size: MarkdownSize::default(),
            label: None,
            streaming: false,
        }
    }

    /// Text size.
    #[must_use]
    pub fn size(mut self, size: MarkdownSize) -> Self {
        self.size = size;
        self
    }

    /// An accessible label, for a region such as "Assistant message".
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Content still streaming in (`data-oa-streaming`): when a newer
    /// render replaces it, its height grows smoothly instead of jumping
    /// (`static/components/markdown-stream.js`).
    #[must_use]
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }
}

impl Render for MarkdownRoot {
    fn render(&self) -> Markup {
        html! {
            div.oa-markdown data-size=[self.size.attr()] aria-label=[self.label.as_deref()]
                data-oa-streaming=[self.streaming.then_some("")] {
                (self.content)
            }
        }
    }
}

/// A paragraph: `p.oa-paragraph`.
#[derive(Clone, Debug)]
pub struct Paragraph {
    content: Markup,
}

impl Paragraph {
    /// A paragraph of `content` (text is escaped).
    #[must_use]
    pub fn new(content: impl Render) -> Self {
        Self {
            content: content.render(),
        }
    }
}

impl Render for Paragraph {
    fn render(&self) -> Markup {
        html! { p.oa-paragraph { (self.content) } }
    }
}

/// A heading, `h1` to `h6`, with class `oa-heading` and `data-level`.
#[derive(Clone, Debug)]
pub struct Heading {
    level: u8,
    content: Markup,
    id: Option<String>,
}

impl Heading {
    /// A heading at `level`, clamped to 1..=6.
    #[must_use]
    pub fn new(level: u8, content: impl Render) -> Self {
        Self {
            level: level.clamp(1, 6),
            content: content.render(),
            id: None,
        }
    }

    /// An `id`, so `#anchor` links can reach it.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl Render for Heading {
    fn render(&self) -> Markup {
        let level = self.level.to_string();
        let id = self.id.as_deref();
        let c = &self.content;
        match self.level {
            1 => html! { h1.oa-heading data-level=(level) id=[id] { (c) } },
            2 => html! { h2.oa-heading data-level=(level) id=[id] { (c) } },
            3 => html! { h3.oa-heading data-level=(level) id=[id] { (c) } },
            4 => html! { h4.oa-heading data-level=(level) id=[id] { (c) } },
            5 => html! { h5.oa-heading data-level=(level) id=[id] { (c) } },
            _ => html! { h6.oa-heading data-level=(level) id=[id] { (c) } },
        }
    }
}

/// A list item: `li.oa-list-item`.
#[derive(Clone, Debug)]
pub struct ListItem {
    content: Markup,
}

impl ListItem {
    /// An item of `content` (text is escaped).
    #[must_use]
    pub fn new(content: impl Render) -> Self {
        Self {
            content: content.render(),
        }
    }
}

impl Render for ListItem {
    fn render(&self) -> Markup {
        html! { li.oa-list-item { (self.content) } }
    }
}

/// A list: `ul.oa-list` or `ol.oa-list`, with `data-variant`.
#[derive(Clone, Debug)]
pub struct List {
    ordered: bool,
    start: Option<u32>,
    items: Vec<ListItem>,
}

impl List {
    /// A bulleted list.
    #[must_use]
    pub fn unordered() -> Self {
        Self {
            ordered: false,
            start: None,
            items: Vec::new(),
        }
    }

    /// A numbered list.
    #[must_use]
    pub fn ordered() -> Self {
        Self {
            ordered: true,
            ..Self::unordered()
        }
    }

    /// The first number of an ordered list.
    #[must_use]
    pub fn start(mut self, start: u32) -> Self {
        self.start = Some(start);
        self
    }

    /// Appends an item of `content` (text is escaped).
    #[must_use]
    pub fn item(mut self, content: impl Render) -> Self {
        self.items.push(ListItem::new(content));
        self
    }

    /// Appends items.
    #[must_use]
    pub fn items<I, R>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: Render,
    {
        self.items.extend(items.into_iter().map(ListItem::new));
        self
    }
}

impl Render for List {
    fn render(&self) -> Markup {
        if self.ordered {
            let start = self.start.filter(|s| *s != 1);
            html! {
                ol.oa-list data-variant="ordered" start=[start] {
                    @for item in &self.items { (item) }
                }
            }
        } else {
            html! {
                ul.oa-list data-variant="unordered" {
                    @for item in &self.items { (item) }
                }
            }
        }
    }
}

/// Inline code: `code.oa-inline-code`. The text is escaped.
#[derive(Clone, Debug)]
pub struct InlineCode {
    text: String,
}

impl InlineCode {
    /// Inline code showing `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl Render for InlineCode {
    fn render(&self) -> Markup {
        html! { code.oa-inline-code { (self.text) } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::PreEscaped;

    #[test]
    fn root_wraps_trusted_renderer_output_and_escapes_text() {
        let html = MarkdownRoot::new(PreEscaped("<p>Hi</p>\n"))
            .render()
            .into_string();
        assert_eq!(html, "<div class=\"oa-markdown\"><p>Hi</p>\n</div>");

        let text = MarkdownRoot::new("<script>x</script>")
            .size(MarkdownSize::Sm)
            .label("Answer")
            .render()
            .into_string();
        assert!(text.contains("&lt;script&gt;"), "{text}");
        assert!(text.contains("data-size=\"sm\""), "{text}");
        assert!(text.contains("aria-label=\"Answer\""), "{text}");
    }

    #[test]
    fn prose_elements() {
        assert_eq!(
            Paragraph::new("a < b").render().into_string(),
            "<p class=\"oa-paragraph\">a &lt; b</p>"
        );
        assert_eq!(
            Heading::new(9, "T").id("t").render().into_string(),
            "<h6 class=\"oa-heading\" data-level=\"6\" id=\"t\">T</h6>"
        );
        assert!(
            Heading::new(0, "T")
                .render()
                .into_string()
                .starts_with("<h1 ")
        );
        assert_eq!(
            List::ordered()
                .start(3)
                .items(["a", "<b>"])
                .render()
                .into_string(),
            "<ol class=\"oa-list\" data-variant=\"ordered\" start=\"3\"><li class=\"oa-list-item\">a</li><li class=\"oa-list-item\">&lt;b&gt;</li></ol>"
        );
        assert_eq!(
            List::unordered().item("x").render().into_string(),
            "<ul class=\"oa-list\" data-variant=\"unordered\"><li class=\"oa-list-item\">x</li></ul>"
        );
        assert_eq!(
            InlineCode::new("<T>").render().into_string(),
            "<code class=\"oa-inline-code\">&lt;T&gt;</code>"
        );
    }
}
