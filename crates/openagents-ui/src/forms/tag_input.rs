use maud::{Markup, Render, html};

use super::{ControlSize, FieldAria};

/// A tag input. Without JavaScript it is a plain text field holding the tags
/// joined by the delimiter (`"a, b, c"`); the `oaTagInput` Alpine component
/// turns entries into removable tags and still submits one field with the
/// same name, joined by the delimiter. Parse it on the server with
/// [`TagInput::parse`].
#[derive(Clone, Debug)]
pub struct TagInput {
    name: String,
    tags: Vec<String>,
    placeholder: Option<String>,
    delimiter: char,
    max: Option<u32>,
    size: ControlSize,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
}

impl TagInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tags: Vec::new(),
            placeholder: None,
            delimiter: ',',
            max: None,
            size: ControlSize::Md,
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
        }
    }

    /// Initial tags.
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Separator between tags, `,` by default.
    pub fn delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Maximum number of tags (enforced by the Alpine component).
    pub fn max(mut self, max: u32) -> Self {
        self.max = Some(max);
        self
    }

    /// Size: `Md` through `Xl3` (Apps SDK UI TagInput sizes).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.aria.id = Some(id.into());
        self
    }

    pub fn aria(mut self, aria: FieldAria) -> Self {
        self.aria.merge(aria);
        self
    }

    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.aria.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.aria.required = required;
        self
    }

    /// Split a submitted value into trimmed, non-empty, de-duplicated tags.
    pub fn parse(value: &str, delimiter: char) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();
        for tag in value.split(delimiter).map(str::trim) {
            if !tag.is_empty() && !tags.iter().any(|existing| existing == tag) {
                tags.push(tag.to_owned());
            }
        }
        tags
    }
}

impl Render for TagInput {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        let joined = self.tags.join(&format!("{} ", self.delimiter));
        let size = match self.size {
            ControlSize::Xs3 | ControlSize::Xs2 | ControlSize::Xs | ControlSize::Sm => {
                ControlSize::Md
            }
            other => other,
        };
        html! {
            div.oa-tag-input
                x-data="oaTagInput"
                data-size=(size.as_str())
                data-delimiter=(self.delimiter)
                data-max=[self.max]
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
            {
                input.oa-tag-input__control
                    type="text"
                    id=(id)
                    name=(self.name)
                    value=(joined)
                    placeholder=[self.placeholder.as_deref()]
                    autocomplete="off"
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled];
            }
        }
    }
}
