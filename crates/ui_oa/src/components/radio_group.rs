//! RadioGroup component for single selection from options

use gpui::prelude::*;
use gpui::*;
use theme_oa::text;
use theme_oa::ui::checkbox;

/// A single radio button (used within RadioGroup)
#[derive(IntoElement)]
pub struct Radio {
    label: SharedString,
    value: SharedString,
    checked: bool,
    disabled: bool,
    on_select: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Radio {
    /// Create a new radio button
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            checked: false,
            disabled: false,
            on_select: None,
        }
    }

    /// Set whether this radio is checked
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set whether this radio is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the selection handler
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Reuse checkbox colors for consistency
        let border_color = if self.checked {
            checkbox::CHECKED_BORDER
        } else {
            checkbox::UNCHECKED_BORDER
        };

        // Radio circle
        let mut circle = div()
            .w(px(16.0))
            .h(px(16.0))
            .rounded_full()
            .border_2()
            .border_color(border_color)
            .flex()
            .items_center()
            .justify_center();

        // Inner dot when checked
        if self.checked {
            circle = circle.child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(checkbox::CHECKED_BG)
            );
        }

        // Full row with label
        let mut row = div()
            .id(ElementId::Name(self.value.clone()))
            .flex()
            .items_center()
            .gap(px(8.0));

        if self.disabled {
            row = row.opacity(0.5).cursor_not_allowed();
        } else {
            row = row.cursor_pointer();

            if let Some(handler) = self.on_select {
                row = row.on_click(move |_, window, cx| {
                    handler(window, cx);
                });
            }
        }

        row.child(circle).child(
            div()
                .text_sm()
                .text_color(text::PRIMARY)
                .child(self.label)
        )
    }
}

/// A group of radio buttons for single selection
///
/// # Example
/// ```
/// RadioGroup::new("size")
///     .value(selected_value)
///     .option("Small", "sm")
///     .option("Medium", "md")
///     .option("Large", "lg")
///     .on_change(|value, _, _| set_selected(value))
/// ```
#[derive(IntoElement)]
pub struct RadioGroup {
    value: Option<SharedString>,
    options: Vec<(SharedString, SharedString)>, // (label, value)
    disabled: bool,
    on_change: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}

impl RadioGroup {
    /// Create a new radio group with a name
    pub fn new(_name: impl Into<SharedString>) -> Self {
        Self {
            value: None,
            options: Vec::new(),
            disabled: false,
            on_change: None,
        }
    }

    /// Set the currently selected value
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Add an option to the group
    pub fn option(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.options.push((label.into(), value.into()));
        self
    }

    /// Set disabled state for all options
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .flex()
            .flex_col()
            .gap(px(8.0));

        let current_value = self.value.clone();
        let on_change = self.on_change;

        for (label, opt_value) in self.options {
            let is_checked = current_value.as_ref().map(|v| v == &opt_value).unwrap_or(false);
            let value_clone = opt_value.clone();

            let mut radio = Radio::new(label, opt_value)
                .checked(is_checked)
                .disabled(self.disabled);

            if let Some(ref handler) = on_change {
                let handler = handler.as_ref() as *const _;
                let value_for_handler = value_clone.clone();
                radio = radio.on_select(move |window, cx| {
                    // Safety: handler lives as long as RadioGroup
                    unsafe {
                        let handler: &dyn Fn(SharedString, &mut Window, &mut App) = &*handler;
                        handler(value_for_handler.clone(), window, cx);
                    }
                });
            }

            container = container.child(radio);
        }

        container
    }
}
