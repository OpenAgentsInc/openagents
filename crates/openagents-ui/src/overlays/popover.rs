use maud::{Markup, Render, html};

use super::{Align, Side, Trigger};

/// A trigger button and a native popover panel of arbitrary content
/// (`role="dialog"`). The panel opens through `popovertarget`, so it works
/// without JavaScript; `oaPopover` adds `aria-expanded`, `data-state`,
/// focus on open, and focus return.
///
/// ```
/// use maud::Render;
/// use openagents_ui::overlays::Popover;
/// let html = Popover::new("share", "Share", maud::html! { p { "Link copied" } })
///     .label("Share options")
///     .render()
///     .into_string();
/// assert!(html.contains(r#"popovertarget="share""#));
/// ```
#[derive(Clone, Debug)]
pub struct Popover {
    id: String,
    trigger: Trigger,
    content: Markup,
    label: Option<String>,
    side: Side,
    align: Align,
}

impl Popover {
    /// `id` names the panel; `trigger` is the trigger button's content.
    pub fn new(id: impl Into<String>, trigger: impl Render, content: impl Render) -> Self {
        Self {
            id: id.into(),
            trigger: Trigger::new(trigger),
            content: content.render(),
            label: None,
            side: Side::default(),
            align: Align::default(),
        }
    }

    /// Accessible name of the panel.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Extra classes on the trigger button (replaces nothing; the
    /// `oa-overlay-trigger` reset stays first).
    pub fn trigger_class(mut self, class: impl Into<String>) -> Self {
        self.trigger.class(class);
        self
    }

    /// Accessible name of the trigger, for icon-only content.
    pub fn trigger_label(mut self, label: impl Into<String>) -> Self {
        self.trigger.label(label);
        self
    }

    /// An extra attribute on the trigger button, such as `data-variant`.
    pub fn trigger_attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.trigger.attr(name, value);
        self
    }
}

impl Render for Popover {
    fn render(&self) -> Markup {
        let trigger = self.trigger.render(
            "oa-overlay-trigger",
            &[
                ("popovertarget", &self.id),
                ("aria-haspopup", "dialog"),
                ("aria-controls", &self.id),
                ("data-oa-trigger", ""),
                ("data-state", "closed"),
            ],
        );
        html! {
            div class="oa-overlay-root oa-popover-root" x-data="oaPopover" data-state="closed" {
                (trigger)
                div id=(self.id) class="oa-popover" popover="auto" role="dialog"
                    aria-label=[self.label.as_deref()] tabindex="-1" data-oa-panel
                    data-state="closed" data-side=(self.side.as_str())
                    data-align=(self.align.as_str()) {
                    (self.content)
                }
            }
        }
    }
}
