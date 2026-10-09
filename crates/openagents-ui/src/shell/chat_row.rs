//! Organizing chats in the left panel: a row's "…" menu ([`RowMenu`]), the
//! rename field that replaces a row ([`RowRename`]), and the search box over
//! the chat list ([`ChatSearch`]).
//!
//! Every control is a plain form first: menu entries submit hidden forms
//! next to the menu, rename is a POST form, and search is a GET form. HTMX
//! attributes swap the list in place when the page loads HTMX; the shell
//! script (`static/components/shell.js`) adds the keys (⌘K / ⌃K, arrows,
//! Enter, Escape, ⌃⇧[ and ⌃⇧]).

use maud::{Markup, Render, html};

use crate::icons::{Icon, IconSize};
use crate::overlays::{Align, Menu, MenuItem};

/// One entry of a [`RowMenu`]: a labelled form that posts (or gets) to
/// `action` with its hidden fields.
#[derive(Clone, Debug)]
pub struct RowAction {
    label: String,
    icon: Option<Icon>,
    post: bool,
    action: String,
    fields: Vec<(String, String)>,
    target: Option<String>,
    swap: Option<String>,
    confirm: Option<String>,
    navigate: bool,
}

impl RowAction {
    fn with(label: String, action: String, post: bool) -> Self {
        Self {
            label,
            icon: None,
            post,
            action,
            fields: Vec::new(),
            target: None,
            swap: None,
            confirm: None,
            navigate: false,
        }
    }

    /// A page to open, such as Delete's confirm step: a plain `GET` link
    /// the browser follows, without HTMX.
    #[must_use]
    pub fn open(label: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            navigate: true,
            ..Self::with(label.into(), action.into(), false)
        }
    }

    /// A change, such as Pin or Archive: a `POST` form.
    #[must_use]
    pub fn post(label: impl Into<String>, action: impl Into<String>) -> Self {
        Self::with(label.into(), action.into(), true)
    }

    /// A view, such as Rename's field: a `GET` form.
    #[must_use]
    pub fn get(label: impl Into<String>, action: impl Into<String>) -> Self {
        Self::with(label.into(), action.into(), false)
    }

    /// A leading icon.
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A hidden field sent with the form.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// The element HTMX swaps with the answer (`hx-target`).
    #[must_use]
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// How HTMX swaps it (`hx-swap`).
    #[must_use]
    pub fn swap(mut self, swap: impl Into<String>) -> Self {
        self.swap = Some(swap.into());
        self
    }

    /// A question to confirm first (`hx-confirm`), such as archiving a chat
    /// that is still working.
    #[must_use]
    pub fn confirm(mut self, question: impl Into<String>) -> Self {
        self.confirm = Some(question.into());
        self
    }
}

/// A chat row's "…" menu: a quiet icon button that shows on hover or focus
/// (always on touch screens) and opens a [`Menu`] of [`RowAction`]s.
///
/// Pass it to [`super::NavItem::menu`]. `id` must be unique on the page
/// (the forms are `{id}-0`, `{id}-1`, ...; the list is `{id}-menu`).
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::{RowAction, RowMenu};
/// let html = RowMenu::new("chat-1", "Fix the build")
///     .action(RowAction::post("Pin", "/chat/1/pin").field("pinned", "1"))
///     .render()
///     .into_string();
/// assert!(html.contains(r#"form="chat-1-0""#));
/// assert!(html.contains(r#"method="post" action="/chat/1/pin""#));
/// ```
#[derive(Clone, Debug)]
pub struct RowMenu {
    id: String,
    label: String,
    actions: Vec<RowAction>,
}

impl RowMenu {
    /// A menu for the row named `label` (the chat's title).
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            actions: Vec::new(),
        }
    }

    /// Adds an entry.
    #[must_use]
    pub fn action(mut self, action: RowAction) -> Self {
        self.actions.push(action);
        self
    }
}

impl Render for RowMenu {
    fn render(&self) -> Markup {
        let form_id = |index: usize| format!("{}-{index}", self.id);
        let items = self.actions.iter().enumerate().map(|(index, action)| {
            let item = MenuItem::button(action.label.clone())
                .submit()
                .form(form_id(index));
            match action.icon {
                Some(icon) => item.icon(icon.size(IconSize::Sm)),
                None => item,
            }
        });
        html! {
            span class="oa-row-menu" {
                (Menu::new(format!("{}-menu", self.id), Icon::DotsHorizontal.size(IconSize::Sm))
                    .label(format!("Options for {}", self.label))
                    .align(Align::End)
                    .trigger_class("oa-row-menu-trigger")
                    .trigger_label(format!("Options for {}", self.label))
                    .items(items))
                @for (index, action) in self.actions.iter().enumerate() {
                    form id=(form_id(index)) hidden
                        method=(if action.post { "post" } else { "get" })
                        action=(action.action)
                        hx-post=[action.post.then_some(action.action.as_str())]
                        hx-get=[(!action.post && !action.navigate).then_some(action.action.as_str())]
                        hx-target=[action.target.as_deref()]
                        hx-swap=[action.swap.as_deref()]
                        hx-confirm=[action.confirm.as_deref()] {
                        @for (name, value) in &action.fields {
                            input type="hidden" name=(name) value=(value);
                        }
                    }
                }
            }
        }
    }
}

/// The field that replaces a row while its chat is renamed: Enter (or
/// Save) posts the new title; Escape (or Cancel) puts the row back.
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::RowRename;
/// let html = RowRename::new("chat-row-1", "/chat/1/rename", "Old <title>", "/chat/1")
///     .render()
///     .into_string();
/// assert!(html.contains(r#"value="Old &lt;title&gt;""#));
/// assert!(html.contains("data-oa-rename-cancel"));
/// ```
#[derive(Clone, Debug)]
pub struct RowRename {
    row_id: String,
    action: String,
    title: String,
    cancel: String,
    cancel_hx: Option<String>,
    max_chars: usize,
    fields: Vec<(String, String)>,
    target: Option<String>,
    swap: Option<String>,
}

impl RowRename {
    /// The row `row_id` (the `<li>` it replaces) renaming to `action`, with
    /// the current `title`; Cancel follows `cancel` without JavaScript.
    #[must_use]
    pub fn new(
        row_id: impl Into<String>,
        action: impl Into<String>,
        title: impl Into<String>,
        cancel: impl Into<String>,
    ) -> Self {
        Self {
            row_id: row_id.into(),
            action: action.into(),
            title: title.into(),
            cancel: cancel.into(),
            cancel_hx: None,
            max_chars: 120,
            fields: Vec::new(),
            target: None,
            swap: None,
        }
    }

    /// Where Cancel loads the list from with HTMX (it swaps like Save).
    #[must_use]
    pub fn cancel_hx(mut self, url: impl Into<String>) -> Self {
        self.cancel_hx = Some(url.into());
        self
    }

    /// The longest title, 120 characters by default.
    #[must_use]
    pub fn max_chars(mut self, max: usize) -> Self {
        self.max_chars = max;
        self
    }

    /// A hidden field sent with the form.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// The element HTMX swaps with the answer (`hx-target`).
    #[must_use]
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// How HTMX swaps it (`hx-swap`).
    #[must_use]
    pub fn swap(mut self, swap: impl Into<String>) -> Self {
        self.swap = Some(swap.into());
        self
    }
}

impl Render for RowRename {
    fn render(&self) -> Markup {
        let target = self.target.as_deref();
        let swap = self.swap.as_deref();
        html! {
            li id=(self.row_id) class="oa-nav-row oa-nav-row--editing" {
                form class="oa-row-rename" method="post" action=(self.action)
                    hx-post=(self.action) hx-target=[target] hx-swap=[swap] data-oa-rename {
                    input class="oa-row-rename-input" type="text" name="title" value=(self.title)
                        required maxlength=(self.max_chars) aria-label="Chat name"
                        autocomplete="off" autofocus;
                    @for (name, value) in &self.fields {
                        input type="hidden" name=(name) value=(value);
                    }
                    span class="oa-row-rename-actions" {
                        button class="oa-row-rename-button" type="submit" { "Save" }
                        a class="oa-row-rename-button" href=(self.cancel)
                            hx-get=[self.cancel_hx.as_deref()] hx-target=[target] hx-swap=[swap]
                            data-oa-rename-cancel { "Cancel" }
                    }
                }
            }
        }
    }
}

/// The search box at the top of the chat list. Without JavaScript it is a
/// `GET` form to `action`; with HTMX each change loads `action` and swaps
/// `target` from the answer (`hx-select`), so the box keeps focus.
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::ChatSearch;
/// let html = ChatSearch::new("/chat/list", "#chat-sidebar-rows").value("fix").render().into_string();
/// assert!(html.contains(r#"role="search""#) && html.contains(r#"value="fix""#));
/// ```
#[derive(Clone, Debug)]
pub struct ChatSearch {
    action: String,
    target: String,
    value: String,
    fields: Vec<(String, String)>,
}

impl ChatSearch {
    /// Searches through `action`, replacing `target` (a selector) in place.
    #[must_use]
    pub fn new(action: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            target: target.into(),
            value: String::new(),
            fields: Vec::new(),
        }
    }

    /// The current search text.
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// A hidden field sent with each search (such as the open chat).
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }
}

impl Render for ChatSearch {
    fn render(&self) -> Markup {
        html! {
            form class="oa-chat-search" role="search" method="get" action=(self.action)
                data-oa-chat-search {
                span class="oa-chat-search-icon" aria-hidden="true" {
                    (Icon::MagnifyingGlassSmSearch.size(IconSize::Sm))
                }
                input class="oa-chat-search-input" type="search" name="q" value=(self.value)
                    placeholder="Search chats" aria-label="Search chats" autocomplete="off"
                    aria-keyshortcuts="Control+K Meta+K"
                    hx-get=(self.action) hx-trigger="input changed delay:150ms, search"
                    hx-target=(self.target) hx-select=(self.target) hx-swap="outerHTML"
                    hx-include="closest form" hx-sync="this:replace";
                @for (name, value) in &self.fields {
                    input type="hidden" name=(name) value=(value);
                }
            }
        }
    }
}
