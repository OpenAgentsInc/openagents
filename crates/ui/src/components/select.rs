//! Select component for dropdown selection

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// A select option
pub struct SelectOption {
    pub value: SharedString,
    pub label: SharedString,
}

impl SelectOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// Select dropdown component
///
/// Note: This is a simplified select that shows options on click.
/// Full dropdown positioning would require overlay support.
///
/// # Example
/// ```
/// Select::new()
///     .placeholder("Select an option")
///     .option(SelectOption::new("opt1", "Option 1"))
///     .option(SelectOption::new("opt2", "Option 2"))
///     .value("opt1")
///     .on_change(|value, _, _| set_value(value))
/// ```
#[derive(IntoElement)]
pub struct Select {
    options: Vec<SelectOption>,
    value: Option<SharedString>,
    placeholder: SharedString,
    disabled: bool,
    open: bool,
    on_change: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}

impl Select {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            value: None,
            placeholder: "Select...".into(),
            disabled: false,
            open: false,
            on_change: None,
        }
    }

    pub fn option(mut self, opt: SelectOption) -> Self {
        self.options.push(opt);
        self
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Find selected label
        let selected_label = self.value.as_ref().and_then(|v| {
            self.options.iter().find(|o| &o.value == v).map(|o| o.label.clone())
        });

        let display_text = selected_label.unwrap_or_else(|| self.placeholder.clone());
        let is_placeholder = self.value.is_none();

        // Trigger button
        let mut trigger = div()
            .id("select-trigger")
            .w(px(200.0))
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer();

        let text_color = if is_placeholder { text::MUTED } else { text::PRIMARY };

        trigger = trigger
            .child(
                div()
                    .text_sm()
                    .text_color(text_color)
                    .child(display_text)
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .child("▼")
            );

        if self.disabled {
            trigger = trigger.opacity(0.5).cursor_not_allowed();
        }

        let mut container = div().relative().child(trigger);

        // Dropdown (shown when open)
        if self.open && !self.disabled {
            let mut dropdown = div()
                .absolute()
                .top(px(42.0))
                .left_0()
                .w(px(200.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(border::DEFAULT)
                .bg(bg::ELEVATED);

            for opt in self.options {
                let is_selected = self.value.as_ref().map(|v| v == &opt.value).unwrap_or(false);
                let opt_value = opt.value.clone();

                let mut option_el = div()
                    .id(ElementId::Name(opt.value))
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(bg::HOVER));

                if is_selected {
                    option_el = option_el
                        .text_color(text::PRIMARY)
                        .font_weight(FontWeight::MEDIUM);
                } else {
                    option_el = option_el.text_color(text::SECONDARY);
                }

                if let Some(ref handler) = self.on_change {
                    let handler_ptr = handler.as_ref() as *const _;
                    option_el = option_el.on_click(move |_, window, cx| {
                        unsafe {
                            let handler: &dyn Fn(SharedString, &mut Window, &mut App) = &*handler_ptr;
                            handler(opt_value.clone(), window, cx);
                        }
                    });
                }

                option_el = option_el.child(opt.label);
                dropdown = dropdown.child(option_el);
            }

            container = container.child(dropdown);
        }

        container
    }
}
