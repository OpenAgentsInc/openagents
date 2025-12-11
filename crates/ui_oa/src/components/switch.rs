//! Switch/toggle component

use gpui::prelude::*;
use gpui::*;
use theme_oa::ui::switch;

/// A toggle switch component
///
/// # Example
/// ```
/// Switch::new().on(true)
/// Switch::new().on_change(|on, _, _| println!("{}", on))
/// ```
#[derive(IntoElement)]
pub struct Switch {
    on: bool,
    disabled: bool,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Switch {
    /// Create a new switch (default off)
    pub fn new() -> Self {
        Self {
            on: false,
            disabled: false,
            on_change: None,
        }
    }

    /// Set the on/off state
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// Set whether the switch is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let track_color = if self.on {
            switch::TRACK_ON
        } else {
            switch::TRACK_OFF
        };

        let current_state = self.on;

        let mut el = div()
            .id("switch")
            .w(px(36.0))
            .h(px(20.0))
            .rounded_full()
            .bg(track_color)
            .flex()
            .items_center()
            .px(px(2.0));

        // Thumb - dark on white track, white on dark track
        let thumb_color = if self.on {
            switch::THUMB_ON  // dark thumb on white track
        } else {
            switch::THUMB     // white thumb on dark track
        };

        let thumb = div()
            .w(px(16.0))
            .h(px(16.0))
            .rounded_full()
            .bg(thumb_color);

        // Position thumb based on state
        if self.on {
            el = el.justify_end();
        } else {
            el = el.justify_start();
        }

        el = el.child(thumb);

        // Handle disabled state
        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el.cursor_pointer();

            // Handle click
            if let Some(handler) = self.on_change {
                el = el.on_click(move |_, window, cx| {
                    handler(!current_state, window, cx);
                });
            }
        }

        el
    }
}
