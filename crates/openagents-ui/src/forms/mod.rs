//! Form controls (UI-03): Field, Input, Textarea, Checkbox, RadioGroup,
//! Switch, SegmentedControl, Slider, Select, DatePicker, DateRangePicker and
//! TagInput.
//!
//! Every control renders a native form element, so form submission, labels,
//! keyboard behavior and validation work without JavaScript. Styles live in
//! `static/components/<component>.css` (ported from Apps SDK UI, MIT) and the
//! small Alpine CSP components (`oaTagInput`, `oaSlider`,
//! `oaTextareaAutogrow`) live in [`SCRIPT`] (`static/components/forms.js`).
//!
//! Labels, descriptions and errors are wired by [`Field`]: build the field
//! first, then pass [`Field::aria`] to the control so it gets the matching
//! `id`, `aria-describedby` and `aria-invalid`:
//!
//! ```
//! use openagents_ui::forms::{Field, Input, InputType};
//!
//! let field = Field::new("email", "Email").error("Enter an email address");
//! let input = Input::new("email").input_type(InputType::Email).aria(field.aria());
//! let html = maud::Render::render(&field.control(input)).into_string();
//! assert!(html.contains(r#"aria-describedby="email-error""#));
//! ```

mod checkbox;
mod date;
mod field;
mod input;
mod radio_group;
mod segmented_control;
mod select;
mod slider;
mod switch;
mod tag_input;
mod textarea;

pub use checkbox::Checkbox;
pub use date::{DatePicker, DateRangePicker};
pub use field::Field;
pub use input::{Input, InputType};
pub use radio_group::{Direction, RadioGroup};
pub use segmented_control::SegmentedControl;
pub use select::Select;
pub use slider::Slider;
pub use switch::{LabelPosition, Switch};
pub use tag_input::TagInput;
pub use textarea::Textarea;

/// `static/components/forms.js`: the Alpine CSP components used by the form
/// controls. Serve it before the Alpine CSP build.
pub const SCRIPT: &str = include_str!("../../static/components/forms.js");

/// Stylesheets owned by this module, as `(file name, contents)`.
pub const STYLESHEETS: &[(&str, &str)] = &[
    (
        "field.css",
        include_str!("../../static/components/field.css"),
    ),
    (
        "input.css",
        include_str!("../../static/components/input.css"),
    ),
    (
        "textarea.css",
        include_str!("../../static/components/textarea.css"),
    ),
    (
        "checkbox.css",
        include_str!("../../static/components/checkbox.css"),
    ),
    (
        "radio-group.css",
        include_str!("../../static/components/radio-group.css"),
    ),
    (
        "switch.css",
        include_str!("../../static/components/switch.css"),
    ),
    (
        "segmented-control.css",
        include_str!("../../static/components/segmented-control.css"),
    ),
    (
        "slider.css",
        include_str!("../../static/components/slider.css"),
    ),
    (
        "select.css",
        include_str!("../../static/components/select.css"),
    ),
    (
        "tag-input.css",
        include_str!("../../static/components/tag-input.css"),
    ),
];

/// Control size, mirroring Apps SDK UI `ControlSize` (`data-size`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    Xs3,
    Xs2,
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
    Xl2,
    Xl3,
}

impl ControlSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xs3 => "3xs",
            Self::Xs2 => "2xs",
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
            Self::Xl2 => "2xl",
            Self::Xl3 => "3xl",
        }
    }
}

/// Input surface variant (`data-variant`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    Outline,
    Soft,
}

impl Variant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Soft => "soft",
        }
    }
}

/// Accessibility wiring a [`Field`] hands to its control: the control id,
/// the label id (for groups), the `aria-describedby` list, and the invalid
/// and required flags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldAria {
    pub id: Option<String>,
    pub labelled_by: Option<String>,
    pub described_by: Option<String>,
    pub invalid: bool,
    pub required: bool,
}

impl FieldAria {
    /// Merge another set of wiring into this one; set values win.
    pub(crate) fn merge(&mut self, other: FieldAria) {
        if other.id.is_some() {
            self.id = other.id;
        }
        if other.labelled_by.is_some() {
            self.labelled_by = other.labelled_by;
        }
        if other.described_by.is_some() {
            self.described_by = other.described_by;
        }
        self.invalid |= other.invalid;
        self.required |= other.required;
    }

    /// `"true"` when invalid, for `aria-invalid=[...]`.
    pub(crate) fn invalid_attr(&self) -> Option<&'static str> {
        self.invalid.then_some("true")
    }
}

/// One `<option>` or radio choice: value, label, disabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub value: String,
    pub label: String,
    pub disabled: bool,
    pub description: Option<String>,
}

impl Choice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
            description: None,
        }
    }
}

#[cfg(test)]
mod tests;
