use maud::{Markup, Render, html};

use super::{Choice, ControlSize, FieldAria, Variant};

/// A native `<select>` styled like Apps SDK UI SelectControl.
///
/// A placeholder renders as an empty, disabled first option; combined with
/// `required` the native validation blocks submission until a choice is made.
#[derive(Clone, Debug)]
pub struct Select {
    name: String,
    options: Vec<Item>,
    selected: Vec<String>,
    placeholder: Option<String>,
    multiple: bool,
    variant: Variant,
    size: ControlSize,
    pill: bool,
    block: bool,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
    form: Option<String>,
}

#[derive(Clone, Debug)]
enum Item {
    Option(Choice),
    Group(String, Vec<Choice>),
}

impl Select {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            options: Vec::new(),
            selected: Vec::new(),
            placeholder: None,
            multiple: false,
            variant: Variant::Outline,
            size: ControlSize::Md,
            pill: false,
            block: true,
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
            form: None,
        }
    }

    /// The id of the `<form>` this control belongs to, when it sits
    /// outside it (the `form` attribute).
    pub fn form(mut self, form: impl Into<String>) -> Self {
        self.form = Some(form.into());
        self
    }

    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(Item::Option(Choice::new(value, label)));
        self
    }

    pub fn disabled_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        let mut choice = Choice::new(value, label);
        choice.disabled = true;
        self.options.push(Item::Option(choice));
        self
    }

    /// An `<optgroup>` of `(value, label)` options.
    pub fn group<I, V, L>(mut self, label: impl Into<String>, options: I) -> Self
    where
        I: IntoIterator<Item = (V, L)>,
        V: Into<String>,
        L: Into<String>,
    {
        let choices = options
            .into_iter()
            .map(|(value, label)| Choice::new(value, label))
            .collect();
        self.options.push(Item::Group(label.into(), choices));
        self
    }

    /// Select a value; call again with `multiple(true)` to select several.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        if !self.multiple {
            self.selected.clear();
        }
        self.selected.push(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    /// Fill the available width (default) or size to content.
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

    fn is_selected(&self, value: &str) -> bool {
        self.selected.iter().any(|selected| selected == value)
    }

    fn choice(&self, choice: &Choice) -> Markup {
        html! {
            option value=(choice.value)
                selected[self.is_selected(&choice.value)]
                disabled[choice.disabled]
            { (choice.label) }
        }
    }
}

impl Render for Select {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        let nothing_selected = self.selected.is_empty();
        html! {
            span.oa-select
                data-variant=(self.variant.as_str())
                data-size=(self.size.as_str())
                data-block=(if self.block { "true" } else { "false" })
                data-pill[self.pill]
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
            {
                select.oa-select__control
                    id=(id)
                    name=(self.name)
                    form=[self.form.as_deref()]
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    multiple[self.multiple]
                    required[self.aria.required]
                    disabled[self.disabled]
                {
                    @if let Some(placeholder) = &self.placeholder {
                        option value="" disabled selected[nothing_selected] { (placeholder) }
                    }
                    @for item in &self.options {
                        @match item {
                            Item::Option(choice) => { (self.choice(choice)) }
                            Item::Group(label, choices) => {
                                optgroup label=(label) {
                                    @for choice in choices { (self.choice(choice)) }
                                }
                            }
                        }
                    }
                }
                @if !self.multiple {
                    svg.oa-select__chevron viewBox="0 0 11 6" fill="none" aria-hidden="true" {
                        path d="M1 1l4.5 4L10 1" stroke="currentColor" stroke-width="1.5"
                            stroke-linecap="round" stroke-linejoin="round" {}
                    }
                }
            }
        }
    }
}
