//! Overlays (UI-05): [`Popover`], [`Menu`], [`Tooltip`], [`SelectControl`],
//! and [`Dialog`] with its [`DialogTrigger`]. The plain native select is
//! [`crate::forms::Select`].
//!
//! Behavior comes from the platform first: the Popover API (`popover`,
//! `popovertarget`) shows, hides and light-dismisses panels, `<dialog>` with
//! `showModal()` handles modals, and CSS anchor positioning places panels.
//! [`SCRIPT`] (`static/components/overlays.js`, part of [`crate::script()`])
//! adds the rest through
//! Alpine.js CSP-build components registered with `Alpine.data`: roving
//! focus, arrow keys, type-ahead, Escape, focus return, `data-state`, and
//! `aria-expanded`. Markup only names a component (`x-data="oaMenu"`), never
//! an expression, so `script-src 'self'` holds.
//!
//! Without JavaScript every builder still works in its basic form: triggers
//! open panels through `popovertarget`, menu links navigate, a
//! [`SelectControl`] is a native `<select>`, and a dialog closes through its
//! `method="dialog"` form.
//!
//! Triggers are slots: pass any [`Render`] as the content, and use
//! `trigger_class` and `trigger_attr` to give the trigger button another
//! component's look (for example the Button classes) or extra attributes.

mod dialog;
mod menu;
mod popover;
mod select;
mod tooltip;

#[cfg(test)]
mod tests;

pub use dialog::{Dialog, DialogSize, DialogTrigger};
pub use menu::{Menu, MenuItem};
pub use popover::Popover;
pub use select::{SelectControl, SelectVariant};
pub use tooltip::{Tooltip, TooltipGutter};

use maud::{Markup, PreEscaped, Render};
use std::fmt::Write as _;

/// The shared Alpine CSP components for every overlay; already included in
/// [`crate::script()`], which loads before Alpine.
pub const SCRIPT: &str = include_str!("../../static/components/overlays.js");

/// The overlay stylesheets as `(file name, contents)`; already included in
/// [`crate::stylesheet()`]. `popover.css` holds the shared panel reset,
/// placement and transition, written so bundle order does not matter.
pub const STYLESHEETS: [(&str, &str); 5] = [
    (
        "popover.css",
        include_str!("../../static/components/popover.css"),
    ),
    ("menu.css", include_str!("../../static/components/menu.css")),
    (
        "tooltip.css",
        include_str!("../../static/components/tooltip.css"),
    ),
    (
        "select-control.css",
        include_str!("../../static/components/select-control.css"),
    ),
    (
        "dialog.css",
        include_str!("../../static/components/dialog.css"),
    ),
];

/// Which side of the trigger a panel opens on (`data-side`). It flips to
/// the opposite side when it does not fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Top => "top",
            Side::Bottom => "bottom",
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// How a panel lines up with its trigger along the side (`data-align`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

impl Align {
    fn as_str(self) -> &'static str {
        match self {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        }
    }
}

/// The trigger slot shared by the popover-based builders: caller content
/// inside a `<button>` the builder owns, so it can carry `popovertarget` and
/// the ARIA wiring.
#[derive(Clone, Debug)]
pub(crate) struct Trigger {
    content: Markup,
    class: Option<String>,
    label: Option<String>,
    attrs: Vec<(String, String)>,
}

impl Trigger {
    pub(crate) fn new(content: impl Render) -> Self {
        Self {
            content: content.render(),
            class: None,
            label: None,
            attrs: Vec::new(),
        }
    }

    pub(crate) fn class(&mut self, class: impl Into<String>) {
        self.class = Some(class.into());
    }

    pub(crate) fn label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub(crate) fn attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attrs.push((name.into(), value.into()));
    }

    /// `<button type="button" class=...>` with the builder's `fixed`
    /// attributes, then the caller's extra attributes (names already fixed by
    /// the builder are skipped), then the content.
    pub(crate) fn render(&self, base_class: &str, fixed: &[(&str, &str)]) -> Markup {
        let mut out = String::from("<button type=\"button\"");
        let class = match &self.class {
            Some(extra) => format!("{base_class} {extra}"),
            None => base_class.to_string(),
        };
        push_attr(&mut out, "class", &class);
        for (name, value) in fixed {
            push_attr(&mut out, name, value);
        }
        if let Some(label) = &self.label {
            push_attr(&mut out, "aria-label", label);
        }
        for (name, value) in &self.attrs {
            let taken = matches!(name.as_str(), "type" | "class" | "aria-label")
                || fixed.iter().any(|(fixed, _)| fixed == name);
            if !taken && valid_attr_name(name) {
                push_attr(&mut out, name, value);
            }
        }
        out.push('>');
        out.push_str(&self.content.0);
        out.push_str("</button>");
        PreEscaped(out)
    }
}

/// Attribute names a caller may add: letters, digits and `-_:.@`, starting
/// with a letter (covers `data-*`, `aria-*` and `hx-*`; rejects `on*`
/// handlers, which the CSP would block anyway).
fn valid_attr_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_:.@".contains(c))
        && !name.to_ascii_lowercase().starts_with("on")
}

fn push_attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    let _ = write!(maud::Escaper::new(out), "{value}");
    out.push('"');
}

/// The check mark used by checkable menu items and selected options.
pub(crate) fn check_icon() -> Markup {
    maud::html! {
        svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" {
            path d="M3.5 8.5l3 3 6-7" stroke="currentColor" stroke-width="1.75"
                stroke-linecap="round" stroke-linejoin="round" {}
        }
    }
}
