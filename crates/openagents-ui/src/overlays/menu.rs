use maud::{Markup, Render, html};

use super::{Align, Side, Trigger, check_icon};

/// A trigger button and a `role="menu"` popover of [`MenuItem`]s.
///
/// Without JavaScript the trigger opens the list through `popovertarget`,
/// links navigate and buttons submit their form. `oaMenu` adds arrow keys,
/// Home/End, type-ahead, hover highlight, Tab to close, focus return,
/// `aria-expanded` and `data-state`, toggles checkable items, closes after a
/// choice, and fires a bubbling `oa-menu-select` event with
/// `{ value, checked }`.
///
/// ```
/// use maud::Render;
/// use openagents_ui::overlays::{Menu, MenuItem};
/// let html = Menu::new("account", "Account")
///     .item(MenuItem::link("Settings", "/settings"))
///     .item(MenuItem::separator())
///     .item(MenuItem::button("Sign out").submit().form("logout"))
///     .render()
///     .into_string();
/// assert!(html.contains(r#"role="menu""#));
/// ```
#[derive(Clone, Debug)]
pub struct Menu {
    id: String,
    trigger: Trigger,
    items: Vec<MenuItem>,
    label: Option<String>,
    side: Side,
    align: Align,
}

impl Menu {
    /// `id` names the list; `trigger` is the trigger button's content.
    pub fn new(id: impl Into<String>, trigger: impl Render) -> Self {
        Self {
            id: id.into(),
            trigger: Trigger::new(trigger),
            items: Vec::new(),
            label: None,
            side: Side::default(),
            align: Align::default(),
        }
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Accessible name of the menu.
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

    /// Extra classes on the trigger button.
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

impl Render for Menu {
    fn render(&self) -> Markup {
        let trigger = self.trigger.render(
            "oa-overlay-trigger",
            &[
                ("popovertarget", &self.id),
                ("aria-haspopup", "menu"),
                ("aria-controls", &self.id),
                ("data-oa-trigger", ""),
                ("data-state", "closed"),
            ],
        );
        html! {
            div class="oa-overlay-root oa-menu" x-data="oaMenu" data-state="closed" {
                (trigger)
                div id=(self.id) class="oa-menu-list" popover="auto" role="menu"
                    aria-label=[self.label.as_deref()] tabindex="-1" data-oa-panel
                    data-state="closed" data-side=(self.side.as_str())
                    data-align=(self.align.as_str()) {
                    @for item in &self.items { (item) }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Link(String),
    Button,
    Separator,
    Heading,
}

/// One entry of a [`Menu`]: a link, a button, a separator, or a heading.
#[derive(Clone, Debug)]
pub struct MenuItem {
    kind: Kind,
    label: String,
    icon: Option<Markup>,
    disabled: bool,
    checked: Option<bool>,
    value: Option<String>,
    name: Option<String>,
    form: Option<String>,
    submit: bool,
    keep_open: bool,
}

impl MenuItem {
    fn with(kind: Kind, label: String) -> Self {
        Self {
            kind,
            label,
            icon: None,
            disabled: false,
            checked: None,
            value: None,
            name: None,
            form: None,
            submit: false,
            keep_open: false,
        }
    }

    /// `<a role="menuitem" href>`: navigates with or without JavaScript.
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self::with(Kind::Link(href.into()), label.into())
    }

    /// `<button role="menuitem">`, `type="button"` unless [`Self::submit`].
    pub fn button(label: impl Into<String>) -> Self {
        Self::with(Kind::Button, label.into())
    }

    /// A `role="separator"` rule between groups.
    pub fn separator() -> Self {
        Self::with(Kind::Separator, String::new())
    }

    /// A non-interactive group heading.
    pub fn heading(label: impl Into<String>) -> Self {
        Self::with(Kind::Heading, label.into())
    }

    /// Leading icon; any [`Render`].
    pub fn icon(mut self, icon: impl Render) -> Self {
        self.icon = Some(icon.render());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes a button `role="menuitemcheckbox"` with this `aria-checked`;
    /// `oaMenu` toggles it on choice.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// `data-value` reported by `oa-menu-select`, and the button's `value`.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// The button's form field `name`.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The `form` id a submit button belongs to (menus usually sit outside
    /// the form they act on).
    pub fn form(mut self, form: impl Into<String>) -> Self {
        self.form = Some(form.into());
        self
    }

    /// Render the button as `type="submit"`.
    pub fn submit(mut self) -> Self {
        self.submit = true;
        self
    }

    /// Keep the menu open after this item is chosen.
    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }

    fn inner(&self) -> Markup {
        html! {
            span class="oa-menu-item-inner" {
                @if let Some(icon) = &self.icon {
                    span class="oa-menu-item-icon" aria-hidden="true" { (icon) }
                }
                span class="oa-menu-item-label" { (self.label) }
                @if self.checked.is_some() {
                    span class="oa-menu-check" aria-hidden="true" { (check_icon()) }
                }
            }
        }
    }
}

impl Render for MenuItem {
    fn render(&self) -> Markup {
        let disabled = self.disabled.then_some("true");
        let disabled_flag = self.disabled.then_some("");
        let keep_open = self.keep_open.then_some("");
        match &self.kind {
            Kind::Separator => html! { div class="oa-menu-separator" role="separator" {} },
            Kind::Heading => html! {
                div class="oa-menu-heading" role="presentation" { (self.label) }
            },
            Kind::Link(href) => html! {
                // A disabled link drops its href so it cannot navigate.
                a class="oa-menu-item" role="menuitem" tabindex="-1"
                    href=[(!self.disabled).then_some(href.as_str())]
                    aria-disabled=[disabled] data-disabled=[disabled_flag]
                    data-value=[self.value.as_deref()] data-keep-open=[keep_open] {
                    (self.inner())
                }
            },
            Kind::Button => {
                let role = if self.checked.is_some() {
                    "menuitemcheckbox"
                } else {
                    "menuitem"
                };
                let checked = self
                    .checked
                    .map(|checked| if checked { "true" } else { "false" });
                html! {
                    button class="oa-menu-item" type=(if self.submit { "submit" } else { "button" })
                        role=(role) tabindex="-1" aria-checked=[checked]
                        disabled[self.disabled] aria-disabled=[disabled]
                        data-disabled=[disabled_flag] name=[self.name.as_deref()]
                        value=[self.value.as_deref()] form=[self.form.as_deref()]
                        data-value=[self.value.as_deref()] data-keep-open=[keep_open] {
                        (self.inner())
                    }
                }
            }
        }
    }
}
