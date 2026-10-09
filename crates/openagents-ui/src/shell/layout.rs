//! The document, layout, left panel and main content surface.

use maud::{DOCTYPE, Markup, Render, html};

use super::{HxGet, Theme, glyph};

/// The `id` of the left panel; the header toggle targets it.
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
                    meta name="viewport" content="width=device-width, initial-scale=1";
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
    trailing: Option<Markup>,
    hx: Option<HxGet>,
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
            trailing: None,
            hx: None,
        }
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

    /// Trailing content such as a badge or a status indicator.
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
            li class="oa-nav-row" {
                a class="oa-nav-item" href=(self.href)
                    hx-get=[hx.map(|hx| hx.url.as_str())]
                    hx-target=[hx.and_then(|hx| hx.target.as_deref())]
                    hx-include=[hx.and_then(|hx| hx.include.as_deref())]
                    hx-swap=[hx.and_then(|hx| hx.swap.as_deref())]
                    hx-sync=[hx.and_then(|hx| hx.sync.as_deref())]
                    aria-current=[self.current.then_some("page")] {
                    @if let Some(icon) = &self.icon {
                        span class="oa-nav-item-icon" aria-hidden="true" { (icon) }
                    }
                    span class="oa-nav-item-label" { (self.label) }
                    @if let Some(trailing) = &self.trailing {
                        span class="oa-nav-item-trailing" { (trailing) }
                    }
                }
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

/// The left panel: brand, main navigation, conversation sections, footer.
#[derive(Clone, Debug, Default)]
pub struct Sidebar {
    label: Option<String>,
    brand: Option<Markup>,
    nav: Vec<NavItem>,
    sections: Vec<SidebarSection>,
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

    /// A main navigation row.
    #[must_use]
    pub fn nav(mut self, item: NavItem) -> Self {
        self.nav.push(item);
        self
    }

    /// A conversation-sidebar section.
    #[must_use]
    pub fn section(mut self, section: SidebarSection) -> Self {
        self.sections.push(section);
        self
    }

    /// The pinned footer (account, theme toggle, legal links).
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

/// The whole app shell: layout, left panel, header, main frame.
///
/// On narrow screens the left panel collapses into a native popover opened
/// by the header's menu button (`popovertarget`), so it works without
/// JavaScript, closes on Escape and light-dismiss, and keeps focus order.
#[derive(Clone, Debug, Default)]
pub struct AppShell {
    sidebar: Option<Sidebar>,
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

    /// The left panel. Without one, the shell has no menu button.
    #[must_use]
    pub fn sidebar(mut self, sidebar: Sidebar) -> Self {
        self.sidebar = Some(sidebar);
        self
    }

    /// The header's leading content: page title, breadcrumbs, wordmark.
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
        html! {
            div class="oa-layout" data-mode=(mode)
                data-sidebar=(if self.sidebar.is_some() { "present" } else { "none" }) {
                a class="oa-skip-link" href=(format!("#{main_id}")) { "Skip to content" }
                @if let Some(sidebar) = &self.sidebar {
                    (render_sidebar(sidebar))
                }
                div class="oa-main-surface" {
                    header class="oa-main-header" {
                        @if self.sidebar.is_some() {
                            button type="button" class="oa-sidebar-toggle"
                                popovertarget=(LEFT_PANEL_ID) popovertargetaction="show"
                                aria-controls=(LEFT_PANEL_ID) aria-label="Open sidebar" {
                                (glyph::menu())
                            }
                        }
                        div class="oa-main-header-content" {
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
                button type="button" class="oa-sidebar-close"
                    popovertarget=(LEFT_PANEL_ID) popovertargetaction="hide"
                    aria-controls=(LEFT_PANEL_ID) aria-label="Close sidebar" {
                    (glyph::close())
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
            @if let Some(footer) = &sidebar.footer {
                div class="oa-sidebar-footer" { (footer) }
            }
        }
    }
}
