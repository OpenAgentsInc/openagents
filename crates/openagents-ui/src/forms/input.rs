use maud::{Markup, Render, html};

use super::{ControlSize, FieldAria, Variant};

/// The `type` of a text-like [`Input`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputType {
    #[default]
    Text,
    Email,
    Password,
    Search,
    Url,
    Tel,
    Number,
}

impl InputType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Email => "email",
            Self::Password => "password",
            Self::Search => "search",
            Self::Url => "url",
            Self::Tel => "tel",
            Self::Number => "number",
        }
    }
}

/// A native text-like `<input>` in the Apps SDK UI Input container.
#[derive(Clone, Debug)]
pub struct Input {
    name: String,
    input_type: InputType,
    value: Option<String>,
    placeholder: Option<String>,
    autocomplete: Option<String>,
    spellcheck: Option<bool>,
    inputmode: Option<String>,
    pattern: Option<String>,
    min: Option<String>,
    max: Option<String>,
    step: Option<String>,
    minlength: Option<u32>,
    maxlength: Option<u32>,
    aria: FieldAria,
    aria_label: Option<String>,
    variant: Variant,
    size: ControlSize,
    pill: bool,
    disabled: bool,
    readonly: bool,
    autofocus: bool,
    start: Option<Markup>,
    end: Option<Markup>,
}

impl Input {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input_type: InputType::Text,
            value: None,
            placeholder: None,
            autocomplete: None,
            spellcheck: None,
            inputmode: None,
            pattern: None,
            min: None,
            max: None,
            step: None,
            minlength: None,
            maxlength: None,
            aria: FieldAria::default(),
            aria_label: None,
            variant: Variant::Outline,
            size: ControlSize::Md,
            pill: false,
            disabled: false,
            readonly: false,
            autofocus: false,
            start: None,
            end: None,
        }
    }

    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.input_type = input_type;
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn autocomplete(mut self, autocomplete: impl Into<String>) -> Self {
        self.autocomplete = Some(autocomplete.into());
        self
    }

    /// The native `spellcheck` setting; `false` keeps a secret away from
    /// the browser spellchecker.
    pub fn spellcheck(mut self, spellcheck: bool) -> Self {
        self.spellcheck = Some(spellcheck);
        self
    }

    pub fn inputmode(mut self, inputmode: impl Into<String>) -> Self {
        self.inputmode = Some(inputmode.into());
        self
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn min(mut self, min: impl Into<String>) -> Self {
        self.min = Some(min.into());
        self
    }

    pub fn max(mut self, max: impl Into<String>) -> Self {
        self.max = Some(max.into());
        self
    }

    pub fn step(mut self, step: impl Into<String>) -> Self {
        self.step = Some(step.into());
        self
    }

    pub fn minlength(mut self, minlength: u32) -> Self {
        self.minlength = Some(minlength);
        self
    }

    pub fn maxlength(mut self, maxlength: u32) -> Self {
        self.maxlength = Some(maxlength);
        self
    }

    /// Element id. Defaults to the name. [`Input::aria`] sets it too.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.aria.id = Some(id.into());
        self
    }

    /// Wiring from a [`super::Field`].
    pub fn aria(mut self, aria: FieldAria) -> Self {
        self.aria.merge(aria);
        self
    }

    /// `aria-label` for inputs without a visible label.
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Content before the input, such as an icon.
    pub fn start_adornment(mut self, adornment: impl Render) -> Self {
        self.start = Some(adornment.render());
        self
    }

    /// Content after the input, such as an icon or a unit.
    pub fn end_adornment(mut self, adornment: impl Render) -> Self {
        self.end = Some(adornment.render());
        self
    }
}

impl Render for Input {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        html! {
            span.oa-input
                data-variant=(self.variant.as_str())
                data-size=(self.size.as_str())
                data-pill[self.pill]
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
                data-has-start-adornment[self.start.is_some()]
                data-has-end-adornment[self.end.is_some()]
            {
                @if let Some(start) = &self.start {
                    span.oa-input__adornment aria-hidden="true" { (start) }
                }
                input.oa-input__control
                    type=(self.input_type.as_str())
                    id=(id)
                    name=(self.name)
                    value=[self.value.as_deref()]
                    placeholder=[self.placeholder.as_deref()]
                    autocomplete=[self.autocomplete.as_deref()]
                    spellcheck=[self.spellcheck.map(|on| if on { "true" } else { "false" })]
                    inputmode=[self.inputmode.as_deref()]
                    pattern=[self.pattern.as_deref()]
                    min=[self.min.as_deref()]
                    max=[self.max.as_deref()]
                    step=[self.step.as_deref()]
                    minlength=[self.minlength]
                    maxlength=[self.maxlength]
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled]
                    readonly[self.readonly]
                    autofocus[self.autofocus];
                @if let Some(end) = &self.end {
                    span.oa-input__adornment { (end) }
                }
            }
        }
    }
}
