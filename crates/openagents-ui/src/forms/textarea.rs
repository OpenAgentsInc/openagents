use maud::{Markup, Render, html};

use super::{ControlSize, FieldAria, Variant};

/// Largest `data-min-rows` value textarea.css has a preset for.
const MAX_MIN_ROWS_PRESET: u32 = 12;
/// Largest `data-max-rows` value textarea.css has a preset for.
const MAX_MAX_ROWS_PRESET: u32 = 24;

/// A native `<textarea>` in the Apps SDK UI Textarea container.
///
/// With [`Textarea::auto_grow`] the field grows with its content between
/// `min_rows` and `max_rows`: natively through `field-sizing: content`, and
/// through the `oaTextareaAutogrow` Alpine component elsewhere.
#[derive(Clone, Debug)]
pub struct Textarea {
    name: String,
    value: Option<String>,
    placeholder: Option<String>,
    rows: u32,
    min_rows: Option<u32>,
    max_rows: Option<u32>,
    maxlength: Option<u32>,
    autocomplete: Option<String>,
    spellcheck: Option<bool>,
    aria: FieldAria,
    aria_label: Option<String>,
    variant: Variant,
    size: ControlSize,
    auto_grow: bool,
    disabled: bool,
    readonly: bool,
}

impl Textarea {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
            placeholder: None,
            rows: 3,
            min_rows: None,
            max_rows: None,
            maxlength: None,
            autocomplete: None,
            spellcheck: None,
            aria: FieldAria::default(),
            aria_label: None,
            variant: Variant::Outline,
            size: ControlSize::Md,
            auto_grow: false,
            disabled: false,
            readonly: false,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Native `rows`; also the default minimum height.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    pub fn min_rows(mut self, rows: u32) -> Self {
        self.min_rows = Some(rows.max(1));
        self
    }

    pub fn max_rows(mut self, rows: u32) -> Self {
        self.max_rows = Some(rows.max(1));
        self
    }

    pub fn maxlength(mut self, maxlength: u32) -> Self {
        self.maxlength = Some(maxlength);
        self
    }

    /// The native `autocomplete` hint, such as `"off"` for a secret.
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

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn auto_grow(mut self, auto_grow: bool) -> Self {
        self.auto_grow = auto_grow;
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
}

impl Render for Textarea {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        let min_rows = self.min_rows.unwrap_or(self.rows);
        let max_rows = self.max_rows.unwrap_or(min_rows.max(12)).max(min_rows);
        // CSP-safe sizing: textarea.css maps these onto presets (min 1-12,
        // max 1-24); the native `rows` keeps the exact minimum.
        let min_preset = min_rows.clamp(1, MAX_MIN_ROWS_PRESET);
        let max_preset = max_rows.clamp(min_preset, MAX_MAX_ROWS_PRESET);
        html! {
            span.oa-textarea
                data-variant=(self.variant.as_str())
                data-size=(self.size.as_str())
                data-disabled[self.disabled]
                data-invalid[self.aria.invalid]
                data-auto-grow[self.auto_grow]
                x-data=[self.auto_grow.then_some("oaTextareaAutogrow")]
                data-min-rows=(min_preset)
                data-max-rows=(max_preset)
            {
                textarea.oa-textarea__control
                    id=(id)
                    name=(self.name)
                    rows=(min_rows)
                    placeholder=[self.placeholder.as_deref()]
                    maxlength=[self.maxlength]
                    autocomplete=[self.autocomplete.as_deref()]
                    spellcheck=[self.spellcheck.map(|on| if on { "true" } else { "false" })]
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    required[self.aria.required]
                    disabled[self.disabled]
                    readonly[self.readonly]
                {
                    @if let Some(value) = &self.value { (value) }
                }
            }
        }
    }
}
