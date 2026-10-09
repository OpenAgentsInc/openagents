//! The document, layout, left panel and main content surface.

use maud::{DOCTYPE, Markup, Render, html};

use super::{HxGet, Theme};
use crate::icons::{Icon, IconSize};

/// The `id` of the left panel; the sidebar toggle targets it.
const LEFT_PANEL_ID: &str = "oa-left-panel";

/// A whole HTML document: `<html>` carries the server-chosen `data-theme`
/// (none follows the system setting), and `color-scheme` allows both.
#[derive(Clone, Debug)]
pub struct Document {
    title: String,
    theme: Option<Theme>,
    head: Option<Markup>,
    body: Option<Markup>,
}

impl Document {
    /// A document whose tab reads `title · OpenAgents` (or `OpenAgents` alone
    /// when the title is `OpenAgents`).
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            theme: None,
            head: None,
            body: None,
        }
    }

    /// The explicit theme from the theme cookie, if any.
    #[must_use]
    pub fn theme(mut self, theme: Option<Theme>) -> Self {
        self.theme = theme;
        self
    }

    /// Extra `<head>` content: stylesheets, scripts, meta.
    #[must_use]
    pub fn head(mut self, head: impl Render) -> Self {
        self.head = Some(head.render());
        self
    }

    /// The `<body>` content, usually an [`AppShell`].
    #[must_use]
    pub fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }
}

impl Render for Document {
    fn render(&self) -> Markup {
        let title = if self.title == "OpenAgents" {
            self.title.clone()
        } else {
            format!("{} \u{b7} OpenAgents", self.title)
        };
        html! {
            (DOCTYPE)
            html lang="en" data-theme=[self.theme.map(Theme::as_str)] {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width, initial-scale=1, interactive-widget=resizes-content";
                    meta name="color-scheme" content="light dark";
                    title { (title) }
                    @if let Some(head) = &self.head { (head) }
                }
                body class="oa-body" {
                    @if let Some(body) = &self.body { (body) }
                }
            }
        }
    }
}

/// One navigation or conversation row in the left panel.
#[derive(Clone, Debug)]
pub struct NavItem {
    label: String,
    href: String,
    icon: Option<Markup>,
    current: bool,
    detail: Option<String>,
    trailing: Option<Markup>,
    shortcut: Option<(String, String)>,
    hx: Option<HxGet>,
    row_id: Option<String>,
    menu: Option<Markup>,
}

impl NavItem {
    /// A link row.
    #[must_use]
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
            icon: None,
            current: false,
            detail: None,
            trailing: None,
            shortcut: None,
            hx: None,
            row_id: None,
            menu: None,
        }
    }

    /// The row's `id` (on its `<li>`), so a response can replace the row,
    /// for example with a [`super::RowRename`] field.
    #[must_use]
    pub fn row_id(mut self, id: impl Into<String>) -> Self {
        self.row_id = Some(id.into());
        self
    }

    /// A [`super::RowMenu`] (the "…" button) after the link, shown on hover
    /// or focus and always on touch screens.
    #[must_use]
    pub fn menu(mut self, menu: impl Render) -> Self {
        self.menu = Some(menu.render());
        self
    }

    /// A keyboard shortcut that follows the row: `keys` in
    /// `aria-keyshortcuts` form (`Control+N`), shown as a quiet keycap
    /// `hint` (`⌃N`) at the row's end on wide screens. The shell script
    /// follows any `Control+<letter>` row it finds on the page.
    #[must_use]
    pub fn shortcut(mut self, keys: impl Into<String>, hint: impl Into<String>) -> Self {
        self.shortcut = Some((keys.into(), hint.into()));
        self
    }

    /// A leading icon.
    #[must_use]
    pub fn icon(mut self, icon: impl Render) -> Self {
        self.icon = Some(icon.render());
        self
    }

    /// Marks the row as the current page (`aria-current="page"`).
    #[must_use]
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// A quiet second line under the label, such as a chat's repository
    /// and branch. Like the label, it is cut with an ellipsis.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Trailing content such as a badge or a status indicator
    /// ([`super::ChatStatus`] for a chat row).
    #[must_use]
    pub fn trailing(mut self, trailing: impl Render) -> Self {
        self.trailing = Some(trailing.render());
        self
    }

    /// Also loads the row's view with HTMX (for example a conversation into
    /// the content area); the plain link still works without JavaScript.
    #[must_use]
    pub fn hx(mut self, hx: HxGet) -> Self {
        self.hx = Some(hx);
        self
    }
}

impl Render for NavItem {
    fn render(&self) -> Markup {
        let hx = self.hx.as_ref();
        html! {
            li class=(if self.menu.is_some() { "oa-nav-row oa-nav-row--menu" } else { "oa-nav-row" })
                id=[self.row_id.as_deref()] {
                a class="oa-nav-item" href=(self.href)
                    hx-get=[hx.map(|hx| hx.url.as_str())]
                    hx-target=[hx.and_then(|hx| hx.target.as_deref())]
                    hx-include=[hx.and_then(|hx| hx.include.as_deref())]
                    hx-swap=[hx.and_then(|hx| hx.swap.as_deref())]
                    hx-sync=[hx.and_then(|hx| hx.sync.as_deref())]
                    aria-current=[self.current.then_some("page")]
                    aria-keyshortcuts=[self.shortcut.as_ref().map(|(keys, _)| keys.as_str())]
                    title=[self.shortcut.as_ref().map(|(_, hint)| format!("{} ({hint})", self.label))] {
                    @if let Some(icon) = &self.icon {
                        span class="oa-nav-item-icon" aria-hidden="true" { (icon) }
                    }
                    @if let Some(detail) = &self.detail {
                        span class="oa-nav-item-label oa-nav-item-label--stacked" {
                            span class="oa-nav-item-title" { (self.label) }
                            span class="oa-nav-item-detail" { (detail) }
                        }
                    } @else {
                        span class="oa-nav-item-label" { (self.label) }
                    }
                    @if let Some(trailing) = &self.trailing {
                        span class="oa-nav-item-trailing" { (trailing) }
                    }
                    @if let Some((_, hint)) = &self.shortcut {
                        kbd class="oa-nav-shortcut" aria-hidden="true" { (hint) }
                    }
                }
                @if let Some(menu) = &self.menu { (menu) }
            }
        }
    }
}

/// A titled list in the conversation sidebar ("Chats", "Projects", ...).
#[derive(Clone, Debug)]
pub struct SidebarSection {
    title: String,
    items: Vec<NavItem>,
    empty: Option<String>,
    id: Option<String>,
    swap_oob: bool,
    after: Option<Markup>,
}

impl SidebarSection {
    /// A section with a visible heading.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
            empty: None,
            id: None,
            swap_oob: false,
            after: None,
        }
    }

    /// Adds a row.
    #[must_use]
    pub fn item(mut self, item: NavItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds rows.
    #[must_use]
    pub fn items(mut self, items: impl IntoIterator<Item = NavItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// The line shown when the section has no rows.
    #[must_use]
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// The section's id, so a response can replace it.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Marks this rendering as an HTMX out-of-band replacement
    /// (`hx-swap-oob="outerHTML"`) of the section with the same id.
    #[must_use]
    pub fn swap_oob(mut self, oob: bool) -> Self {
        self.swap_oob = oob;
        self
    }

    /// Content after the rows: a note, an error, a trailing link list.
    #[must_use]
    pub fn after(mut self, after: impl Render) -> Self {
        self.after = Some(after.render());
        self
    }
}

impl Render for SidebarSection {
    fn render(&self) -> Markup {
        html! {
            section class="oa-sidebar-section" id=[self.id.as_deref()]
                hx-swap-oob=[self.swap_oob.then_some("outerHTML")] aria-label=(self.title) {
                h2 class="oa-sidebar-section-title" { (self.title) }
                @if self.items.is_empty() {
                    @if let Some(empty) = &self.empty {
                        p class="oa-sidebar-empty" { (empty) }
                    }
                } @else {
                    ul class="oa-nav-list" role="list" {
                        @for item in &self.items { (item) }
                    }
                }
                @if let Some(after) = &self.after { (after) }
            }
        }
    }
}

/// The visitor's recent chats in the left panel, as ChatGPT lists them: one
/// row per conversation (newest first, in the order the caller adds them),
/// the open chat marked current, long titles cut with an ellipsis.
///
/// With no rows it renders an empty section with no heading and no text, so
/// a later response can still replace it out of band by its id. Any page
/// (home, chat, the demo) can build the same list:
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::ChatList;
/// let html = ChatList::new()
///     .id("chat-sidebar")
///     .chat("Fix the build", "/chat/1", true)
///     .chat("Plan the launch", "/chat/2", false)
///     .render()
///     .into_string();
/// assert!(html.contains(r#"href="/chat/1" aria-current="page""#));
/// ```
///
/// Organizing adds, all optional: a [`super::ChatSearch`] box on top
/// ([`ChatList::search`]), a short notice such as "Chat archived · Undo"
/// ([`ChatList::notice`]), a "Pinned" group above the chats
/// ([`ChatList::pinned`], hidden when empty), the line shown when nothing
/// matches ([`ChatList::empty`]), and content after the rows such as an
/// "Archived" link ([`ChatList::after`]). The groups sit in one
/// `{id}-rows` box, which a search replaces in place.
#[derive(Clone, Debug)]
pub struct ChatList {
    title: String,
    id: Option<String>,
    swap_oob: bool,
    items: Vec<NavItem>,
    pinned: Vec<NavItem>,
    search: Option<Markup>,
    notice: Option<Markup>,
    empty: Option<String>,
    after: Option<Markup>,
}

impl Default for ChatList {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatList {
    /// An empty list headed "Chats".
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: "Chats".to_owned(),
            id: None,
            swap_oob: false,
            items: Vec::new(),
            pinned: Vec::new(),
            search: None,
            notice: None,
            empty: None,
            after: None,
        }
    }

    /// Pinned rows, shown under a "Pinned" heading above the chats.
    #[must_use]
    pub fn pinned(mut self, items: impl IntoIterator<Item = NavItem>) -> Self {
        self.pinned.extend(items);
        self
    }

    /// The search box on top of the list (a [`super::ChatSearch`]).
    #[must_use]
    pub fn search(mut self, search: impl Render) -> Self {
        self.search = Some(search.render());
        self
    }

    /// A short notice above the rows, read aloud when it appears (such as
    /// "Chat archived" with an Undo button).
    #[must_use]
    pub fn notice(mut self, notice: impl Render) -> Self {
        self.notice = Some(notice.render());
        self
    }

    /// The line shown when there are no rows ("No chats found").
    #[must_use]
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// Content after the rows, such as an "Archived" link.
    #[must_use]
    pub fn after(mut self, after: impl Render) -> Self {
        self.after = Some(after.render());
        self
    }

    /// The heading, "Chats" by default.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// The section's id, so a response can replace it.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Marks this rendering as an HTMX out-of-band replacement
    /// (`hx-swap-oob="outerHTML"`) of the list with the same id.
    #[must_use]
    pub fn swap_oob(mut self, oob: bool) -> Self {
        self.swap_oob = oob;
        self
    }

    /// Adds a chat row: its title, link, and whether it is the open chat.
    #[must_use]
    pub fn chat(self, title: impl Into<String>, href: impl Into<String>, current: bool) -> Self {
        self.item(NavItem::new(title, href).current(current))
    }

    /// Adds a prepared row (for example one that also loads with HTMX).
    #[must_use]
    pub fn item(mut self, item: NavItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds prepared rows.
    #[must_use]
    pub fn items(mut self, items: impl IntoIterator<Item = NavItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Whether the list has no rows (pinned or not).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.pinned.is_empty()
    }
}

impl Render for ChatList {
    fn render(&self) -> Markup {
        let plain = self.search.is_none()
            && self.notice.is_none()
            && self.empty.is_none()
            && self.after.is_none()
            && self.pinned.is_empty();
        let rows_id = self.id.as_ref().map(|id| format!("{id}-rows"));
        html! {
            section class="oa-sidebar-section oa-chat-list" id=[self.id.as_deref()]
                hx-swap-oob=[self.swap_oob.then_some("outerHTML")] aria-label=(self.title) {
                @if plain {
                    @if !self.items.is_empty() {
                        h2 class="oa-sidebar-section-title" { (self.title) }
                        ul class="oa-nav-list" role="list" {
                            @for item in &self.items { (item) }
                        }
                    }
                } @else {
                    @if let Some(search) = &self.search { (search) }
                    div class="oa-chat-list-rows" id=[rows_id] data-oa-chat-rows {
                        div class="oa-chat-list-notice" role="status" {
                            @if let Some(notice) = &self.notice { (notice) }
                        }
                        @if !self.pinned.is_empty() {
                            div class="oa-chat-list-group" {
                                h2 class="oa-sidebar-section-title" { "Pinned" }
                                ul class="oa-nav-list" role="list" {
                                    @for item in &self.pinned { (item) }
                                }
                            }
                        }
                        @if !self.items.is_empty() {
                            div class="oa-chat-list-group" {
                                h2 class="oa-sidebar-section-title" { (self.title) }
                                ul class="oa-nav-list" role="list" {
                                    @for item in &self.items { (item) }
                                }
                            }
                        }
                        @if self.is_empty() {
                            @if let Some(empty) = &self.empty {
                                p class="oa-sidebar-empty" { (empty) }
                            }
                        }
                    }
                    @if let Some(after) = &self.after { (after) }
                }
            }
        }
    }
}

/// Small, quiet legal and project links (Terms, Privacy, source, social,
/// copyright). The home page centers them along the bottom of the main area
/// (`oa-home-legal`); no other page shows them.
#[derive(Clone, Debug, Default)]
pub struct LegalLinks {
    note: Option<String>,
    links: Vec<(String, String)>,
}

impl LegalLinks {
    /// No links yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A plain line after the links, such as the copyright.
    #[must_use]
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// A link. Absolute `http(s)` links get `rel="noopener"`.
    #[must_use]
    pub fn link(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.links.push((label.into(), href.into()));
        self
    }
}

impl Render for LegalLinks {
    fn render(&self) -> Markup {
        html! {
            nav class="oa-sidebar-legal" aria-label="Legal and links" {
                @for (label, href) in &self.links {
                    @let external = href.starts_with("http://") || href.starts_with("https://");
                    a href=(href) rel=[external.then_some("noopener")] { (label) }
                }
                @if let Some(note) = &self.note {
                    span class="oa-sidebar-legal-note" { (note) }
                }
            }
        }
    }
}

/// The left panel: brand, the toggle, top navigation ("New chat"),
/// conversation sections, bottom navigation ("Docs"), and the footer.
#[derive(Clone, Debug, Default)]
pub struct Sidebar {
    label: Option<String>,
    brand: Option<Markup>,
    nav: Vec<NavItem>,
    sections: Vec<Markup>,
    bottom: Vec<NavItem>,
    footer: Option<Markup>,
}

impl Sidebar {
    /// An empty sidebar labelled "Sidebar".
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The accessible name of the panel.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The brand row (the wordmark link).
    #[must_use]
    pub fn brand(mut self, brand: impl Render) -> Self {
        self.brand = Some(brand.render());
        self
    }

    /// A top navigation row, such as "New chat". A row with an icon keeps
    /// the icon visible when the panel is collapsed to its rail.
    #[must_use]
    pub fn nav(mut self, item: NavItem) -> Self {
        self.nav.push(item);
        self
    }

    /// A conversation-sidebar section: a [`SidebarSection`], a [`ChatList`],
    /// or other markup.
    #[must_use]
    pub fn section(mut self, section: impl Render) -> Self {
        self.sections.push(section.render());
        self
    }

    /// A row pinned to the bottom of the panel, above the footer ("Docs"
    /// for a visitor who is not signed in).
    #[must_use]
    pub fn bottom(mut self, item: NavItem) -> Self {
        self.bottom.push(item);
        self
    }

    /// The pinned footer, such as an [`super::AccountMenu`] or a sign-in
    /// link.
    #[must_use]
    pub fn footer(mut self, footer: impl Render) -> Self {
        self.footer = Some(footer.render());
        self
    }
}

/// How the main viewport behaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainMode {
    /// A document page: the viewport scrolls, and the footer slot sits at the
    /// end of the scrolled content.
    #[default]
    Scroll,
    /// An app page (chat, work views): the viewport fills the frame and does
    /// not scroll; the content places its own scrolling regions, and the
    /// composer docks at the bottom.
    App,
}

/// The cookie the shell script stores the wide-screen sidebar state in:
/// `collapsed` or `expanded`.
pub const SIDEBAR_COOKIE: &str = "oa_sidebar";

/// The data attribute the shell script binds the sidebar toggle by.
pub const SIDEBAR_TOGGLE_ATTR: &str = "data-oa-sidebar-toggle";

/// Whether a [`SIDEBAR_COOKIE`] value asks for the collapsed panel.
#[must_use]
pub fn sidebar_collapsed_from_cookie(value: &str) -> bool {
    value.trim() == "collapsed"
}

/// The whole app shell: layout, left panel, header, main frame.
///
/// The left panel has one toggle, in its header (ChatGPT's "Toggle
/// sidebar"). On wide screens the shell script ([`SIDEBAR_TOGGLE_ATTR`])
/// collapses the panel to a narrow rail and back, storing the choice in the
/// [`SIDEBAR_COOKIE`] cookie so the server renders the next page the same
/// way ([`AppShell::sidebar_collapsed`]). On narrow screens the panel rests
/// as that rail and the same toggle opens it as a native popover drawer
/// (`popovertarget`), which works without JavaScript, closes on Escape and
/// light-dismiss, and keeps focus order. Without the script, wide screens
/// hide the toggle rather than show a button that does nothing.
#[derive(Clone, Debug, Default)]
pub struct AppShell {
    sidebar: Option<Sidebar>,
    collapsed: bool,
    breadcrumb: Option<Markup>,
    header: Option<Markup>,
    actions: Option<Markup>,
    content: Option<Markup>,
    footer: Option<Markup>,
    composer: Option<Markup>,
    mode: MainMode,
    main_id: Option<String>,
}

impl AppShell {
    /// An empty shell.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The left panel. Without one, the shell has no sidebar toggle.
    #[must_use]
    pub fn sidebar(mut self, sidebar: Sidebar) -> Self {
        self.sidebar = Some(sidebar);
        self
    }

    /// Renders the left panel collapsed to its rail on wide screens (read
    /// the [`SIDEBAR_COOKIE`] with [`sidebar_collapsed_from_cookie`]).
    #[must_use]
    pub fn sidebar_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// The header row's breadcrumb (a [`super::Breadcrumb`]): the page's
    /// name, or a chat's title, on the same line as the actions. The left
    /// panel's toggle sits at the start of that line (in its rail when the
    /// panel is collapsed or the screen is narrow).
    #[must_use]
    pub fn breadcrumb(mut self, breadcrumb: impl Render) -> Self {
        self.breadcrumb = Some(breadcrumb.render());
        self
    }

    /// Extra leading header content after the breadcrumb.
    #[must_use]
    pub fn header(mut self, header: impl Render) -> Self {
        self.header = Some(header.render());
        self
    }

    /// The header's trailing actions, such as a [`super::ThemeToggle`].
    #[must_use]
    pub fn actions(mut self, actions: impl Render) -> Self {
        self.actions = Some(actions.render());
        self
    }

    /// The page content inside `<main>`.
    #[must_use]
    pub fn content(mut self, content: impl Render) -> Self {
        self.content = Some(content.render());
        self
    }

    /// The footer at the end of a [`MainMode::Scroll`] page.
    #[must_use]
    pub fn footer(mut self, footer: impl Render) -> Self {
        self.footer = Some(footer.render());
        self
    }

    /// A composer docked under the viewport (chat and work views).
    #[must_use]
    pub fn composer(mut self, composer: impl Render) -> Self {
        self.composer = Some(composer.render());
        self
    }

    /// Scroll (document pages) or app (fixed viewport) mode.
    #[must_use]
    pub fn mode(mut self, mode: MainMode) -> Self {
        self.mode = mode;
        self
    }

    /// The `<main>` id, `content` by default (the skip link's target).
    #[must_use]
    pub fn main_id(mut self, id: impl Into<String>) -> Self {
        self.main_id = Some(id.into());
        self
    }
}

impl Render for AppShell {
    fn render(&self) -> Markup {
        let main_id = self.main_id.as_deref().unwrap_or("content");
        let mode = match self.mode {
            MainMode::Scroll => "scroll",
            MainMode::App => "app",
        };
        let sidebar = match (&self.sidebar, self.collapsed) {
            (None, _) => "none",
            (Some(_), false) => "expanded",
            (Some(_), true) => "collapsed",
        };
        html! {
            div class="oa-layout" data-mode=(mode) data-sidebar=(sidebar) {
                a class="oa-skip-link" href=(format!("#{main_id}")) { "Skip to content" }
                @if let Some(sidebar) = &self.sidebar {
                    (render_sidebar(sidebar))
                }
                div class="oa-main-surface" {
                    header class="oa-main-header" {
                        div class="oa-main-header-content" {
                            @if let Some(breadcrumb) = &self.breadcrumb { (breadcrumb) }
                            @if let Some(header) = &self.header { (header) }
                        }
                        div class="oa-main-header-actions" {
                            @if let Some(actions) = &self.actions { (actions) }
                        }
                    }
                    div class="oa-main-frame" {
                        div class="oa-main-top-fade" aria-hidden="true" {}
                        div class="oa-main-viewport" {
                            main id=(main_id) class="oa-workspace" tabindex="-1" {
                                @if let Some(content) = &self.content { (content) }
                            }
                            @if self.mode == MainMode::Scroll {
                                @if let Some(footer) = &self.footer { (footer) }
                            }
                        }
                        @if let Some(composer) = &self.composer {
                            div class="oa-main-composer" { (composer) }
                        }
                    }
                }
            }
        }
    }
}

fn render_sidebar(sidebar: &Sidebar) -> Markup {
    let label = sidebar.label.as_deref().unwrap_or("Sidebar");
    html! {
        aside id=(LEFT_PANEL_ID) class="oa-left-panel" popover="auto" aria-label=(label) {
            div class="oa-sidebar-header" {
                div class="oa-sidebar-brand" {
                    @if let Some(brand) = &sidebar.brand { (brand) }
                }
                button type="button" class="oa-sidebar-toggle" data-oa-sidebar-toggle=""
                    popovertarget=(LEFT_PANEL_ID) aria-controls=(LEFT_PANEL_ID)
                    aria-label="Toggle sidebar" title="Toggle sidebar" {
                    (Icon::Sidebar.size(IconSize::Lg))
                }
            }
            div class="oa-conversation-sidebar" {
                @if !sidebar.nav.is_empty() {
                    nav class="oa-navigation" aria-label="Main" {
                        ul class="oa-nav-list" role="list" {
                            @for item in &sidebar.nav { (item) }
                        }
                    }
                }
                @for section in &sidebar.sections { (section) }
            }
            @if !sidebar.bottom.is_empty() || sidebar.footer.is_some() {
                div class="oa-sidebar-footer" {
                    @if !sidebar.bottom.is_empty() {
                        nav class="oa-sidebar-bottom" aria-label="More" {
                            ul class="oa-nav-list" role="list" {
                                @for item in &sidebar.bottom { (item) }
                            }
                        }
                    }
                    @if let Some(footer) = &sidebar.footer { (footer) }
                }
            }
        }
    }
}
