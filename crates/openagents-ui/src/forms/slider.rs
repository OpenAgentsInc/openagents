use maud::{Markup, Render, html};

use super::FieldAria;

/// A native `<input type="range">` with an optional label row and value
/// readout. The `oaSlider` Alpine component keeps the filled track and the
/// readout current as the value changes; without JS the native thumb moves
/// and the form submits the value.
#[derive(Clone, Debug)]
pub struct Slider {
    name: String,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    label: Option<String>,
    unit: Option<String>,
    marks: Vec<String>,
    disabled: bool,
    aria: FieldAria,
    aria_label: Option<String>,
}

impl Slider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: 0.0,
            label: None,
            unit: None,
            marks: Vec::new(),
            disabled: false,
            aria: FieldAria::default(),
            aria_label: None,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    /// Label shown in the header row, tied to the input with `for`.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Unit shown after the value readout, such as `"%"`.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Evenly spaced labels under the track.
    pub fn marks<I, S>(mut self, marks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.marks = marks.into_iter().map(Into::into).collect();
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

    fn fill_percent(&self) -> f64 {
        if self.max > self.min {
            ((self.value - self.min) / (self.max - self.min) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

impl Render for Slider {
    fn render(&self) -> Markup {
        let id = self.aria.id.clone().unwrap_or_else(|| self.name.clone());
        // CSP-safe fill: slider.css has a preset per 5%; oaSlider sets the
        // exact value through the CSSOM once Alpine starts.
        let fill = ((self.fill_percent() / 5.0).round() as u32) * 5;
        html! {
            div.oa-slider x-data="oaSlider" {
                @if self.label.is_some() || self.unit.is_some() {
                    div.oa-slider__header {
                        @if let Some(label) = &self.label {
                            label for=(id) { (label) }
                        } @else {
                            span {}
                        }
                        output.oa-slider__value for=(id) {
                            span.oa-slider__value-text { (number(self.value)) }
                            @if let Some(unit) = &self.unit {
                                span.oa-slider__unit { (unit) }
                            }
                        }
                    }
                }
                input.oa-slider__input
                    type="range"
                    id=(id)
                    name=(self.name)
                    min=(number(self.min))
                    max=(number(self.max))
                    step=(number(self.step))
                    value=(number(self.value))
                    data-fill=(fill)
                    aria-label=[self.aria_label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid_attr()]
                    disabled[self.disabled];
                @if !self.marks.is_empty() {
                    div.oa-slider__marks aria-hidden="true" {
                        @for mark in &self.marks {
                            span.oa-slider__mark { (mark) }
                        }
                    }
                }
            }
        }
    }
}
