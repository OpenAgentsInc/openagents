use maud::{Markup, Render, html};

use super::FieldAria;

/// A native `<input type="checkbox">` with its own `<label>`.
///
/// Unchecked boxes submit nothing, as with any native checkbox; the value
/// defaults to `"on"`.
#[derive(Clone, Debug)]
pub struct Checkbox {
    name: String,
    label: String,
    value: Option<String>,
    description: Option<String>,
    checked: bool,
    disabled: bool,
    label_right: bool,
    aria: FieldAria,
}

impl Checkbox {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            value: None,
            description: None,
            checked: false,
            disabled: false,
            label_right: false,
            aria: FieldAria::default(),
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Secondary text under the label, linked with `aria-describedby`.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Put the label before the box (`data-orientation="right"`).
    pub fn label_first(mut self, label_first: bool) -> Self {
        self.label_right = label_first;
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

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.aria.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.aria.required = required;
        self
    }
}

impl Render for Checkbox {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        let description_id = format!("{id}-description");
        let described_by = join_ids(
            self.description.as_ref().map(|_| description_id.as_str()),
            self.aria.described_by.as_deref(),
        );
        html! {
            label.oa-checkbox
                for=(id)
                data-orientation=[self.label_right.then_some("right")]
                data-disabled[self.disabled]
            {
                span.oa-checkbox__box {
                    input.oa-checkbox__input
                        type="checkbox"
                        id=(id)
                        name=(self.name)
                        value=[self.value.as_deref()]
                        aria-describedby=[described_by.as_deref()]
                        aria-invalid=[self.aria.invalid_attr()]
                        checked[self.checked]
                        required[self.aria.required]
                        disabled[self.disabled];
                    span.oa-checkbox__mark aria-hidden="true" {}
                }
                span.oa-checkbox__label {
                    span { (self.label) }
                    @if let Some(description) = &self.description {
                        span.oa-checkbox__description id=(description_id) { (description) }
                    }
                }
            }
        }
    }
}

/// Join two optional space-separated id lists.
pub(crate) fn join_ids(first: Option<&str>, second: Option<&str>) -> Option<String> {
    match (first, second) {
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (Some(a), None) => Some(a.to_owned()),
        (None, Some(b)) => Some(b.to_owned()),
        (None, None) => None,
    }
}
