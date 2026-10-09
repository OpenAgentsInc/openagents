//! Pages in the Coder Light / Coder Noir design language.
//!
//! Every page that moves off [`crate::layout`] renders through [`UiPage`]:
//! the `openagents-ui` document (theme from the cookie, else the system
//! setting), the shared app shell with the site navigation, the theme
//! toggle, and the design-language assets. Pages supply only their content,
//! and optionally a header, actions, a composer, or extra sidebar sections.

use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::shell::{
    AppShell, Document, MainMode, NavItem, Sidebar, SidebarSection, ThemeToggle,
};

use crate::layout::SECTIONS;
use crate::theme;

/// The navigation every page shares, before [`SECTIONS`].
const PRIMARY: [(&str, &str); 3] = [("Home", "/"), ("Chat", "/chat"), ("Cloud", "/cloud/app")];

/// One page: title, current section, content, and optional shell slots.
#[must_use]
pub struct UiPage {
    title: String,
    section: Option<String>,
    return_to: String,
    mode: MainMode,
    header: Option<Markup>,
    actions: Option<Markup>,
    content: Option<Markup>,
    composer: Option<Markup>,
    footer: Option<Markup>,
    sections: Vec<SidebarSection>,
    scripts: bool,
}

impl UiPage {
    /// A scrolling document page titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            section: None,
            return_to: "/".to_owned(),
            mode: MainMode::Scroll,
            header: None,
            actions: None,
            content: None,
            composer: None,
            footer: None,
            sections: Vec::new(),
            scripts: true,
        }
    }

    /// The navigation entry to mark current, by its href (for example `/chat`).
    pub fn section(mut self, href: impl Into<String>) -> Self {
        self.section = Some(href.into());
        self
    }

    /// Where the no-JavaScript theme toggle returns to: this page's path.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.return_to = path.into();
        self
    }

    /// An app page (chat, work views): the content area does not scroll and
    /// the composer docks under it.
    pub fn app(mut self) -> Self {
        self.mode = MainMode::App;
        self
    }

    pub fn header(mut self, header: impl Render) -> Self {
        self.header = Some(header.render());
        self
    }

    pub fn actions(mut self, actions: impl Render) -> Self {
        self.actions = Some(actions.render());
        self
    }

    pub fn content(mut self, content: impl Render) -> Self {
        self.content = Some(content.render());
        self
    }

    pub fn composer(mut self, composer: impl Render) -> Self {
        self.composer = Some(composer.render());
        self
    }

    /// Replaces the default legal footer on scrolling pages.
    pub fn footer(mut self, footer: impl Render) -> Self {
        self.footer = Some(footer.render());
        self
    }

    /// A page that must run no script (its CSP has no `script-src`): no
    /// component script or Alpine; the theme toggle uses its fallback form.
    pub fn scriptless(mut self) -> Self {
        self.scripts = false;
        self
    }

    /// An extra left-panel section, such as a conversation list.
    pub fn sidebar_section(mut self, section: SidebarSection) -> Self {
        self.sections.push(section);
        self
    }

    /// The whole HTML document for a request with these headers.
    pub fn render(self, headers: &HeaderMap) -> Markup {
        let current = self.section.as_deref();
        let scripts = self.scripts;
        let mut sidebar = Sidebar::new()
            .label("Main")
            .brand(html! { a class="oa-wordmark" href="/" { "OpenAgents" } });
        for (name, href) in PRIMARY.iter().chain(SECTIONS.iter()) {
            sidebar = sidebar.nav(NavItem::new(*name, *href).current(current == Some(*href)));
        }
        for section in self.sections {
            sidebar = sidebar.section(section);
        }
        let toggle = ThemeToggle::new()
            .fallback_action(theme::TOGGLE_PATH)
            .return_to(self.return_to);
        let actions = html! {
            @if let Some(actions) = &self.actions { (actions) }
            (toggle)
        };
        let mut shell = AppShell::new()
            .mode(self.mode)
            .sidebar(sidebar)
            .actions(actions);
        if let Some(header) = self.header {
            shell = shell.header(header);
        }
        if let Some(content) = self.content {
            shell = shell.content(content);
        }
        if let Some(composer) = self.composer {
            shell = shell.composer(composer);
        }
        if matches!(self.mode, MainMode::Scroll) {
            shell = shell.footer(self.footer.unwrap_or_else(legal_footer));
        }
        Document::new(self.title)
            .theme(theme::from_headers(headers))
            .head(html! {
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                (PreEscaped(theme::style_tag()))
                @if scripts { (PreEscaped(theme::script_tags())) }
            })
            .body(shell)
            .render()
    }

    /// The page answered with `200`.
    pub fn respond(self, headers: &HeaderMap) -> Response {
        Html(self.render(headers).into_string()).into_response()
    }
}

fn legal_footer() -> Markup {
    html! {
        nav class="oa-legal" aria-label="Legal and links" {
            a href="/terms" { "Terms" } " \u{b7} "
            a href="/privacy" { "Privacy" } " \u{b7} "
            a href="https://github.com/OpenAgentsInc/openagents" rel="noopener" { "GitHub" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, header};

    #[test]
    fn follows_the_system_without_a_cookie_and_the_cookie_with_one() {
        let page = |h: &HeaderMap| UiPage::new("Test").content(html! { p { "hi" } }).render(h);
        let none = page(&HeaderMap::new()).into_string();
        assert!(none.contains("<html lang=\"en\">"), "{none}");
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("oa_theme=light"));
        assert!(page(&h).into_string().contains("data-theme=\"light\""));
    }

    #[test]
    fn carries_assets_navigation_toggle_and_marks_the_section() {
        let html = UiPage::new("Chat")
            .section("/chat")
            .path("/chat")
            .app()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string();
        for needle in [
            theme::STYLESHEET_PATH,
            theme::SCRIPT_PATH,
            theme::ALPINE_PATH,
            "data-oa-theme-toggle",
            "href=\"/chat\" aria-current=\"page\"",
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        assert!(
            !html.contains("class=\"oa-legal\""),
            "app pages have no footer"
        );
    }

    #[test]
    fn scriptless_pages_load_no_script_but_keep_the_toggle_form() {
        let html = UiPage::new("Connect")
            .scriptless()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string()
            .to_ascii_lowercase();
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("action=\"/theme\""), "{html}");
    }
}
