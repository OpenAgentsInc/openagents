use maud::{Markup, Render, html};

use super::{ControlSize, FieldAria, Variant};

/// A native `<input type="date">` in the Input container. Values use the
/// HTML date format `YYYY-MM-DD`.
#[derive(Clone, Debug)]
pub struct DatePicker {
    name: String,
    value: Option<String>,
    min: Option<String>,
    max: Option<String>,
    variant: Variant,
    size: ControlSize,
    pill: bool,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
}

impl DatePicker {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
            min: None,
            max: None,
            variant: Variant::Outline,
            size: ControlSize::Md,
            pill: false,
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
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

impl Render for DatePicker {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        html! {
            span.oa-input
                data-variant=(self.variant.as_str())
                data-size=(self.size.as_str())
                data-pill[self.pill]
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
            {
                input.oa-input__control
                    type="date"
                    id=(id)
                    name=(self.name)
                    value=[self.value.as_deref()]
                    min=[self.min.as_deref()]
                    max=[self.max.as_deref()]
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled];
            }
        }
    }
}

/// Two native date inputs, start and end, in one Input container. Each input
/// has its own name and an accessible label ("Start date", "End date" by
/// default). Use it inside a group [`super::Field`].
#[derive(Clone, Debug)]
pub struct DateRangePicker {
    start_name: String,
    end_name: String,
    start: Option<String>,
    end: Option<String>,
    min: Option<String>,
    max: Option<String>,
    start_label: String,
    end_label: String,
    variant: Variant,
    size: ControlSize,
    pill: bool,
    disabled: bool,
    aria: FieldAria,
}

impl DateRangePicker {
    pub fn new(start_name: impl Into<String>, end_name: impl Into<String>) -> Self {
        Self {
            start_name: start_name.into(),
            end_name: end_name.into(),
            start: None,
            end: None,
            min: None,
            max: None,
            start_label: "Start date".to_owned(),
            end_label: "End date".to_owned(),
            variant: Variant::Outline,
            size: ControlSize::Md,
            pill: false,
            disabled: false,
            aria: FieldAria::default(),
        }
    }

    pub fn start(mut self, value: impl Into<String>) -> Self {
        self.start = Some(value.into());
        self
    }

    pub fn end(mut self, value: impl Into<String>) -> Self {
        self.end = Some(value.into());
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

    /// Accessible labels for the two inputs.
    pub fn labels(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.start_label = start.into();
        self.end_label = end.into();
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

    /// Id prefix: the inputs get `{id}` and `{id}-end`.
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

impl Render for DateRangePicker {
    fn render(&self) -> Markup {
        let id = self
            .aria
            .id
            .clone()
            .unwrap_or_else(|| self.start_name.clone());
        let end_id = format!("{id}-end");
        // The end input cannot precede the start; the start cannot follow the end.
        let end_min = self.start.as_deref().or(self.min.as_deref());
        let start_max = self.end.as_deref().or(self.max.as_deref());
        html! {
            span.oa-input
                role="group"
                data-range
                data-variant=(self.variant.as_str())
                data-size=(self.size.as_str())
                data-pill[self.pill]
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
                aria-labelledby=[self.aria.labelled_by.as_deref()]
            {
                input.oa-input__control
                    type="date"
                    id=(id)
                    name=(self.start_name)
                    value=[self.start.as_deref()]
                    min=[self.min.as_deref()]
                    max=[start_max]
                    aria-label=(self.start_label)
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled];
                span.oa-input__separator aria-hidden="true" { "–" }
                input.oa-input__control
                    type="date"
                    id=(end_id)
                    name=(self.end_name)
                    value=[self.end.as_deref()]
                    min=[end_min]
                    max=[self.max.as_deref()]
                    aria-label=(self.end_label)
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled];
            }
        }
    }
}
