//! TextLink, ported from Apps SDK UI `src/components/TextLink` (MIT).
//! Styles: `static/components/text-link.css`.

use maud::{Markup, Render, html};

use super::button::is_external;
use super::html::{Attrs, Tag, safe_url};

/// An inline `<a class="oa-text-link">`. Underlined unless `primary`.
/// Without an `href` it renders `<span role="button" tabindex="0">`, a
/// visual link that cannot be meta-clicked open (pair it with HTMX).
#[derive(Clone, Debug)]
pub struct TextLink {
    content: Markup,
    href: Option<String>,
    primary: bool,
    underline: Option<bool>,
    force_external: Option<bool>,
    attrs: Attrs,
}

impl TextLink {
    pub fn new(text: impl Into<String>, href: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            content: html! { (text) },
            href: Some(href.into()),
            primary: false,
            underline: None,
            force_external: None,
            attrs: Attrs::default(),
        }
    }

    /// A link with arbitrary inner markup (for example text plus an icon).
    pub fn with_content(content: impl Render, href: impl Into<String>) -> Self {
        Self {
            content: content.render(),
            ..Self::new("", href)
        }
    }

    /// A link-styled `<span role="button">` with no destination.
    pub fn without_href(text: impl Into<String>) -> Self {
        Self {
            href: None,
            ..Self::new(text, "")
        }
    }

    /// Primary color and no underline by default.
    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    /// Underline override (defaults to `!primary`).
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = Some(underline);
        self
    }

    /// Forces (or suppresses) external-link behavior instead of detecting it.
    pub fn force_external(mut self, external: bool) -> Self {
        self.force_external = Some(external);
        self
    }
}

impl_attrs!(TextLink);

impl Render for TextLink {
    fn render(&self) -> Markup {
        let underline = self.underline.unwrap_or(!self.primary);
        let tag = match &self.href {
            Some(href) => {
                let tag = Tag::new("a", "oa-text-link", &self.attrs);
                if self.force_external.unwrap_or_else(|| is_external(href)) {
                    tag.attr("target", "_blank")
                        .attr("rel", "noopener noreferrer")
                        .attr("href", &safe_url(href))
                } else {
                    tag.attr("href", &safe_url(href))
                }
            }
            None => Tag::new("span", "oa-text-link", &self.attrs)
                .attr("role", "button")
                .attr("tabindex", "0"),
        };
        tag.flag("data-primary", self.primary)
            .flag("data-underline", underline)
            .extra(&self.attrs)
            .close(self.content.clone())
    }
}
