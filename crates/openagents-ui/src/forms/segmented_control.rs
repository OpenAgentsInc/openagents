use maud::{Markup, Render, html};

use super::{Choice, ControlSize, FieldAria};

/// A segmented control: a native radio group styled as segments. Arrow keys
/// move the selection and the form submits the chosen value, without JS.
#[derive(Clone, Debug)]
pub struct SegmentedControl {
    name: String,
    options: Vec<SegmentOption>,
    selected: Option<String>,
    size: ControlSize,
    pill: bool,
    block: bool,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
}

#[derive(Clone, Debug)]
struct SegmentOption {
    choice: Choice,
    icon: Option<Markup>,
    icon_only: bool,
}

impl SegmentedControl {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            options: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            pill: true,
            block: false,
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
        }
    }

    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(SegmentOption {
            choice: Choice::new(value, label),
            icon: None,
            icon_only: false,
        });
        self
    }

    /// An option with an icon before its label.
    pub fn option_with_icon(
        mut self,
        value: impl Into<String>,
        label: impl Into<String>,
        icon: impl Render,
    ) -> Self {
        self.options.push(SegmentOption {
            choice: Choice::new(value, label),
            icon: Some(icon.render()),
            icon_only: false,
        });
        self
    }

    /// An icon-only option; the label becomes its `aria-label`.
    pub fn icon_option(
        mut self,
        value: impl Into<String>,
        label: impl Into<String>,
        icon: impl Render,
    ) -> Self {
        self.options.push(SegmentOption {
            choice: Choice::new(value, label),
            icon: Some(icon.render()),
            icon_only: true,
        });
        self
    }

    pub fn disabled_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        let mut choice = Choice::new(value, label);
        choice.disabled = true;
        self.options.push(SegmentOption {
            choice,
            icon: None,
            icon_only: false,
        });
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected = Some(value.into());
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Rounded ends (`data-pill`); on by default, as in Apps SDK UI.
    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    /// Fill the available width (`data-block`).
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
}

impl Render for SegmentedControl {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        html! {
            div.oa-segmented-control
                id=(id)
                role="radiogroup"
                data-size=(self.size.as_str())
                data-pill[self.pill]
                data-block[self.block]
                aria-label=[self.aria_label.as_deref()]
                aria-labelledby=[self.aria.labelled_by.as_deref()]
                aria-describedby=[self.aria.described_by.as_deref()]
                aria-invalid=[self.aria.invalid_attr()]
            {
                @for (index, option) in self.options.iter().enumerate() {
                    @let option_id = format!("{id}-{index}");
                    @let disabled = self.disabled || option.choice.disabled;
                    @let checked = self.selected.as_deref() == Some(option.choice.value.as_str());
                    label.oa-segmented-control__option for=(option_id) data-disabled[disabled] {
                        input.oa-segmented-control__input
                            type="radio"
                            id=(option_id)
                            name=(self.name)
                            value=(option.choice.value)
                            aria-label=[option.icon_only.then_some(option.choice.label.as_str())]
                            checked[checked]
                            required[self.aria.required]
                            disabled[disabled];
                        @if let Some(icon) = &option.icon {
                            span.oa-segmented-control__text aria-hidden="true" { (icon) }
                        }
                        @if !option.icon_only {
                            span.oa-segmented-control__text { (option.choice.label) }
                        }
                    }
                }
            }
        }
    }
}
