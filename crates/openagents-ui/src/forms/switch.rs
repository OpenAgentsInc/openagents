use maud::{Markup, Render, html};

use super::FieldAria;
use super::checkbox::join_ids;

/// Where a [`Switch`] label sits relative to the track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelPosition {
    Start,
    #[default]
    End,
}

impl LabelPosition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// A switch: `<input type="checkbox" role="switch">` styled as a track and thumb.
#[derive(Clone, Debug)]
pub struct Switch {
    name: String,
    label: Option<String>,
    aria_label: Option<String>,
    value: Option<String>,
    description: Option<String>,
    checked: bool,
    disabled: bool,
    label_position: LabelPosition,
    aria: FieldAria,
}

impl Switch {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: None,
            aria_label: None,
            value: None,
            description: None,
            checked: false,
            disabled: false,
            label_position: LabelPosition::End,
            aria: FieldAria::default(),
        }
    }

    /// Visible label text.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// `aria-label` for a switch without a visible label.
    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

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

    pub fn label_position(mut self, position: LabelPosition) -> Self {
        self.label_position = position;
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
}

impl Render for Switch {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        let description_id = format!("{id}-description");
        let described_by = join_ids(
            self.description.as_ref().map(|_| description_id.as_str()),
            self.aria.described_by.as_deref(),
        );
        html! {
            label.oa-switch
                for=(id)
                data-label-position=(self.label_position.as_str())
                data-disabled[self.disabled]
            {
                span.oa-switch__track {
                    input.oa-switch__input
                        type="checkbox"
                        role="switch"
                        id=(id)
                        name=(self.name)
                        value=[self.value.as_deref()]
                        aria-label=[self.aria_label.as_deref()]
                        aria-describedby=[described_by.as_deref()]
                        aria-invalid=[self.aria.invalid_attr()]
                        checked[self.checked]
                        disabled[self.disabled];
                    span.oa-switch__thumb aria-hidden="true" {}
                }
                @if self.label.is_some() || self.description.is_some() {
                    span.oa-switch__label {
                        @if let Some(label) = &self.label { span { (label) } }
                        @if let Some(description) = &self.description {
                            span.oa-switch__description id=(description_id) { (description) }
                        }
                    }
                }
            }
        }
    }
}
