use maud::{Markup, Render, html};

use super::{Align, Side};

/// Padding of a non-compact tooltip (`data-gutter-size`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipGutter {
    #[default]
    Sm,
    Md,
    Lg,
}

impl TooltipGutter {
    fn as_str(self) -> &'static str {
        match self {
            TooltipGutter::Sm => "sm",
            TooltipGutter::Md => "md",
            TooltipGutter::Lg => "lg",
        }
    }
}

/// A `role="tooltip"` manual popover describing the element in the trigger
/// slot. The slot is rendered as given (a button, link, or text); `oaTooltip`
/// points the slot's first focusable element at the tooltip with
/// `aria-describedby`, shows it after `delay_ms` on mouse hover and at once
/// on keyboard focus, and hides it on leave, blur, press, and Escape.
///
/// A tooltip is an enhancement: put anything a reader must know in the
/// trigger's own label.
///
/// ```
/// use maud::Render;
/// use openagents_ui::overlays::Tooltip;
/// let html = Tooltip::new("copy-tip", maud::html! { button { "Copy" } }, "Copy to clipboard")
///     .render()
///     .into_string();
/// assert!(html.contains(r#"role="tooltip""#));
/// ```
#[derive(Clone, Debug)]
pub struct Tooltip {
    id: String,
    trigger: Markup,
    content: Markup,
    side: Side,
    align: Align,
    compact: bool,
    gutter: TooltipGutter,
    delay_ms: u32,
    decorated: bool,
}

impl Tooltip {
    /// `id` names the tooltip; `trigger` is the described element.
    pub fn new(id: impl Into<String>, trigger: impl Render, content: impl Render) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.render(),
            content: content.render(),
            side: Side::Top,
            align: Align::Center,
            compact: false,
            gutter: TooltipGutter::default(),
            delay_ms: 150,
            decorated: false,
        }
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// The small dark label style (`data-compact`).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn gutter(mut self, gutter: TooltipGutter) -> Self {
        self.gutter = gutter;
        self
    }

    /// Hover delay before showing, in milliseconds (default 150).
    pub fn delay_ms(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Dotted underline on a text trigger while the tooltip is open.
    pub fn decorated(mut self, decorated: bool) -> Self {
        self.decorated = decorated;
        self
    }
}

impl Render for Tooltip {
    fn render(&self) -> Markup {
        html! {
            span class="oa-overlay-root oa-tooltip-root" x-data="oaTooltip" data-state="closed"
                data-delay=(self.delay_ms) data-decorated=[self.decorated.then_some("")] {
                (self.trigger)
                span id=(self.id) class="oa-tooltip" popover="manual" role="tooltip"
                    data-oa-panel data-state="closed" data-side=(self.side.as_str())
                    data-align=(self.align.as_str())
                    data-compact=(if self.compact { "true" } else { "false" })
                    data-gutter-size=(self.gutter.as_str()) {
                    (self.content)
                }
            }
        }
    }
}
