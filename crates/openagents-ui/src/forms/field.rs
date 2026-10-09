use maud::{Markup, Render, html};

use super::FieldAria;

/// Label, description and error around one control.
///
/// A single control renders `<div class="oa-field">` with `<label for=id>`.
/// A group (RadioGroup, SegmentedControl, DateRangePicker, a set of
/// checkboxes) renders `<fieldset class="oa-field">` with a `<legend>`.
/// Description and error get the ids `{id}-description` and `{id}-error`,
/// which [`Field::aria`] passes to the control as `aria-describedby`.
#[derive(Clone, Debug)]
pub struct Field {
    id: String,
    label: String,
    description: Option<String>,
    error: Option<String>,
    required: bool,
    optional: bool,
    group: bool,
    control: Option<Markup>,
}

impl Field {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            error: None,
            required: false,
            optional: false,
            group: false,
            control: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the error message. The control becomes `aria-invalid="true"`.
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Set or clear the error message.
    pub fn error_opt(mut self, error: Option<impl Into<String>>) -> Self {
        self.error = error.map(Into::into);
        self
    }

    /// Mark the field required: a visual marker and `required` on the control.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Show an "(optional)" hint after the label.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Render as a `<fieldset>`/`<legend>` group instead of `<label for>`.
    pub fn group(mut self, group: bool) -> Self {
        self.group = group;
        self
    }

    /// The control rendered inside the field.
    pub fn control(mut self, control: impl Render) -> Self {
        self.control = Some(control.render());
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label_id(&self) -> String {
        format!("{}-label", self.id)
    }

    pub fn description_id(&self) -> String {
        format!("{}-description", self.id)
    }

    pub fn error_id(&self) -> String {
        format!("{}-error", self.id)
    }

    /// Wiring for the control: id, labelled-by (groups), described-by,
    /// invalid and required.
    pub fn aria(&self) -> FieldAria {
        let mut described = Vec::new();
        if self.description.is_some() {
            described.push(self.description_id());
        }
        if self.error.is_some() {
            described.push(self.error_id());
        }
        FieldAria {
            id: Some(self.id.clone()),
            labelled_by: self.group.then(|| self.label_id()),
            described_by: (!described.is_empty()).then(|| described.join(" ")),
            invalid: self.error.is_some(),
            required: self.required,
        }
    }

    fn label_text(&self) -> Markup {
        html! {
            (self.label)
            @if self.required {
                span.oa-field__required aria-hidden="true" { "*" }
            }
            @if self.optional {
                span.oa-field__optional { "(optional)" }
            }
        }
    }

    fn messages(&self) -> Markup {
        html! {
            @if let Some(error) = &self.error {
                p.oa-field__error id=(self.error_id()) { (error) }
            }
        }
    }

    fn description_markup(&self) -> Markup {
        html! {
            @if let Some(description) = &self.description {
                p.oa-field__description id=(self.description_id()) { (description) }
            }
        }
    }
}

impl Render for Field {
    fn render(&self) -> Markup {
        if self.group {
            html! {
                fieldset.oa-field data-invalid[self.error.is_some()] {
                    legend.oa-field__label id=(self.label_id()) { (self.label_text()) }
                    (self.description_markup())
                    @if let Some(control) = &self.control { (control) }
                    (self.messages())
                }
            }
        } else {
            html! {
                div.oa-field data-invalid[self.error.is_some()] {
                    label.oa-field__label id=(self.label_id()) for=(self.id) { (self.label_text()) }
                    (self.description_markup())
                    @if let Some(control) = &self.control { (control) }
                    (self.messages())
                }
            }
        }
    }
}
