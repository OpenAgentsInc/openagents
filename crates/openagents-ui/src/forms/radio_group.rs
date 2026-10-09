use maud::{Markup, Render, html};

use super::{Choice, FieldAria};

/// Layout of a [`RadioGroup`] (`data-direction`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Row,
    Col,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Col => "col",
        }
    }
}

/// A group of native `<input type="radio">` elements sharing one name.
///
/// Wrap it in a group [`super::Field`] (`Field::new(..).group(true)`) for a
/// `<fieldset>`/`<legend>`; the group gets `role="radiogroup"` and the
/// field's `aria-labelledby`/`aria-describedby`/`aria-invalid`.
#[derive(Clone, Debug)]
pub struct RadioGroup {
    name: String,
    options: Vec<Choice>,
    selected: Option<String>,
    direction: Direction,
    block: bool,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
}

impl RadioGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            options: Vec::new(),
            selected: None,
            direction: Direction::Col,
            block: false,
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
        }
    }

    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(Choice::new(value, label));
        self
    }

    /// An option with secondary text under its label.
    pub fn option_with_description(
        mut self,
        value: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let mut choice = Choice::new(value, label);
        choice.description = Some(description.into());
        self.options.push(choice);
        self
    }

    pub fn disabled_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        let mut choice = Choice::new(value, label);
        choice.disabled = true;
        self.options.push(choice);
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected = Some(value.into());
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Make each option fill the row (`data-block`).
    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
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
}

impl Render for RadioGroup {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        html! {
            div.oa-radio-group
                id=(id)
                role="radiogroup"
                data-direction=(self.direction.as_str())
                data-block[self.block]
                aria-label=[self.aria_label.as_deref()]
                aria-labelledby=[self.aria.labelled_by.as_deref()]
                aria-describedby=[self.aria.described_by.as_deref()]
                aria-invalid=[self.aria.invalid_attr()]
                aria-required=[self.aria.required.then_some("true")]
            {
                @for (index, option) in self.options.iter().enumerate() {
                    @let option_id = format!("{id}-{index}");
                    @let disabled = self.disabled || option.disabled;
                    @let description_id = format!("{option_id}-description");
                    label.oa-radio for=(option_id) data-disabled[disabled] {
                        span.oa-radio__indicator-wrap {
                            input.oa-radio__input
                                type="radio"
                                id=(option_id)
                                name=(self.name)
                                value=(option.value)
                                aria-describedby=[option.description.as_ref().map(|_| description_id.as_str())]
                                checked[self.selected.as_deref() == Some(option.value.as_str())]
                                required[self.aria.required]
                                disabled[disabled];
                            span.oa-radio__item aria-hidden="true" {}
                        }
                        span {
                            (option.label)
                            @if let Some(description) = &option.description {
                                span.oa-radio__description id=(description_id) { (description) }
                            }
                        }
                    }
                }
            }
        }
    }
}
