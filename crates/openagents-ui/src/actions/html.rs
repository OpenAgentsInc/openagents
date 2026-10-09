//! A tiny element writer. Maud cannot take attribute names at runtime, and
//! every builder accepts caller attributes (`hx-*`, `aria-*`), so the outer
//! element of each component is written here with escaping, and its
//! children come from Maud.

use maud::{Markup, PreEscaped};

/// Caller-supplied `id`, extra classes and extra attributes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attrs {
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub extra: Vec<(String, Option<String>)>,
}

/// Escapes text for use in HTML content or a double-quoted attribute value.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.' | '@'))
        && !name.to_ascii_lowercase().starts_with("on")
}

/// Neutralizes script-bearing URL schemes; everything else passes through
/// (and is escaped when written).
pub(crate) fn safe_url(url: &str) -> String {
    let scheme: String = url
        .trim_start()
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .take(11)
        .collect::<String>()
        .to_ascii_lowercase();
    if scheme.starts_with("javascript:")
        || scheme.starts_with("vbscript:")
        || (scheme.starts_with("data:") && !scheme.starts_with("data:image"))
    {
        "about:blank".to_string()
    } else {
        url.to_string()
    }
}

pub(crate) struct Tag {
    name: &'static str,
    out: String,
}

impl Tag {
    pub fn new(name: &'static str, class: &str, attrs: &Attrs) -> Self {
        let mut tag = Self {
            name,
            out: format!("<{name}"),
        };
        if let Some(id) = &attrs.id {
            tag = tag.attr("id", id);
        }
        let mut classes = class.to_string();
        for extra in &attrs.classes {
            if !extra.trim().is_empty() {
                if !classes.is_empty() {
                    classes.push(' ');
                }
                classes.push_str(extra.trim());
            }
        }
        if !classes.is_empty() {
            tag = tag.attr("class", &classes);
        }
        tag
    }

    pub fn attr(mut self, name: &str, value: &str) -> Self {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        self.out.push_str(&escape(value));
        self.out.push('"');
        self
    }

    pub fn attr_opt(self, name: &str, value: Option<&str>) -> Self {
        match value {
            Some(value) => self.attr(name, value),
            None => self,
        }
    }

    /// A boolean attribute, written bare (`data-pill`, `disabled`).
    pub fn flag(mut self, name: &str, on: bool) -> Self {
        if on {
            self.out.push(' ');
            self.out.push_str(name);
        }
        self
    }

    /// Writes the caller's extra attributes; call last.
    pub fn extra(mut self, attrs: &Attrs) -> Self {
        for (name, value) in &attrs.extra {
            if !valid_name(name) {
                debug_assert!(false, "invalid or event-handler attribute name: {name}");
                continue;
            }
            self = match value {
                Some(value) => self.attr(name, value),
                None => self.flag(name, true),
            };
        }
        self
    }

    pub fn close(mut self, children: Markup) -> Markup {
        self.out.push('>');
        self.out.push_str(&children.into_string());
        self.out.push_str("</");
        self.out.push_str(self.name);
        self.out.push('>');
        PreEscaped(self.out)
    }

    /// For void elements such as `img`.
    pub fn void(mut self) -> Markup {
        self.out.push('>');
        PreEscaped(self.out)
    }
}
