//! Source chips (citations) and their favicons.

use maud::{Markup, Render, html};

use super::{is_external, safe_href};

/// A site's small round icon: `span.oa-favicon`, decorative.
///
/// The site's policy loads images from its own origin only, so an image is
/// drawn only from a site path (`/static/...`); otherwise, and by default,
/// the favicon is the first letter of the site's name.
#[derive(Clone, Debug)]
pub struct Favicon {
    letter: String,
    src: Option<String>,
}

impl Favicon {
    /// A letter favicon for the site called `name` (for example a domain).
    #[must_use]
    pub fn new(name: &str) -> Self {
        let name = name
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.");
        let letter = name
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        Self { letter, src: None }
    }

    /// An image on this site to draw instead of the letter. Ignored unless
    /// it is a site path (starts with one `/`, no `..`).
    #[must_use]
    pub fn src(mut self, src: &str) -> Self {
        let own = src.starts_with('/')
            && !src.starts_with("//")
            && !src.contains("..")
            && !src.contains('\\')
            && safe_href(src).is_some();
        self.src = own.then(|| src.to_owned());
        self
    }
}

impl Render for Favicon {
    fn render(&self) -> Markup {
        html! {
            span.oa-favicon aria-hidden="true" {
                @if let Some(src) = &self.src {
                    img src=(src) alt="" width="16" height="16" loading="lazy" decoding="async";
                } @else {
                    (self.letter)
                }
            }
        }
    }
}

/// How a [`Source`] chip is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceVariant {
    /// A small pill inline with text (the reference's `CompactSource`).
    #[default]
    Compact,
    /// A larger chip leading with the favicon, for a list of sources (the
    /// reference's `LeadingSource`).
    Leading,
}

impl SourceVariant {
    fn attr(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Leading => "leading",
        }
    }
}

/// A source chip: favicon, site name, and an optional "+N" for more
/// sources. A link when its target passes [`safe_href`]; external links
/// open in a new tab with `noopener noreferrer`. Any other target draws
/// the chip as plain text.
#[derive(Clone, Debug)]
pub struct Source {
    label: String,
    href: Option<String>,
    variant: SourceVariant,
    favicon: Option<Favicon>,
    extra: usize,
}

impl Source {
    /// A chip named `label` (escaped) pointing at `href`.
    #[must_use]
    pub fn new(label: impl Into<String>, href: &str) -> Self {
        Self {
            label: label.into(),
            href: safe_href(href).map(str::to_owned),
            variant: SourceVariant::default(),
            favicon: None,
            extra: 0,
        }
    }

    /// The variant.
    #[must_use]
    pub fn variant(mut self, variant: SourceVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The favicon (default: the label's first letter).
    #[must_use]
    pub fn favicon(mut self, favicon: Favicon) -> Self {
        self.favicon = Some(favicon);
        self
    }

    /// How many more sources this chip stands for, drawn as "+N".
    #[must_use]
    pub fn extra(mut self, extra: usize) -> Self {
        self.extra = extra;
        self
    }

    fn inner(&self) -> Markup {
        let favicon = self
            .favicon
            .clone()
            .unwrap_or_else(|| Favicon::new(&self.label));
        html! {
            (favicon)
            span.oa-source__label { (self.label) }
            @if self.extra > 0 {
                span.oa-source__extra { "+" (self.extra) }
            }
        }
    }
}

impl Render for Source {
    fn render(&self) -> Markup {
        let variant = self.variant.attr();
        match &self.href {
            Some(href) if is_external(href) => html! {
                a.oa-source data-variant=(variant) href=(href) target="_blank" rel="noopener noreferrer" { (self.inner()) }
            },
            Some(href) => html! {
                a.oa-source data-variant=(variant) href=(href) { (self.inner()) }
            },
            None => html! {
                span.oa-source data-variant=(variant) { (self.inner()) }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favicon_is_a_letter_or_an_own_image() {
        assert_eq!(
            Favicon::new("https://www.example.com")
                .render()
                .into_string(),
            "<span class=\"oa-favicon\" aria-hidden=\"true\">E</span>"
        );
        for elsewhere in [
            "https://evil.example/f.png",
            "//evil.example/f.png",
            "/static/../x",
        ] {
            let html = Favicon::new("a").src(elsewhere).render().into_string();
            assert!(!html.contains("<img"), "{elsewhere}");
        }
        assert!(
            Favicon::new("a")
                .src("/static/favicons/github.png")
                .render()
                .into_string()
                .contains("<img src=\"/static/favicons/github.png\" alt=\"\"")
        );
    }

    #[test]
    fn source_links_are_constrained() {
        let html = Source::new("example.com", "https://example.com/a")
            .extra(2)
            .render()
            .into_string();
        assert!(
            html.starts_with(
                "<a class=\"oa-source\" data-variant=\"compact\" href=\"https://example.com/a\" target=\"_blank\" rel=\"noopener noreferrer\">"
            ),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"oa-source__extra\">+2</span>"),
            "{html}"
        );

        let html = Source::new("<x>", "javascript:alert(1)")
            .variant(SourceVariant::Leading)
            .render()
            .into_string();
        assert!(html.starts_with("<span class=\"oa-source\" data-variant=\"leading\">"));
        assert!(!html.contains("javascript"), "{html}");
        assert!(html.contains("&lt;x&gt;"), "{html}");

        let html = Source::new("Docs", "/docs").render().into_string();
        assert!(html.contains("href=\"/docs\""), "{html}");
        assert!(!html.contains("target"), "{html}");
    }
}
