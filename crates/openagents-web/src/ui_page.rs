//! Pages in the Coder Light / Coder Noir design language.
//!
//! Every page that moves off [`crate::layout`] renders through [`UiPage`]:
//! the `openagents-ui` document (theme from the cookie, else the system
//! setting), the shared app shell with the site navigation, the theme
//! toggle, and the design-language assets. Pages supply only their content,
//! and optionally a header, actions, a composer, or extra sidebar sections.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{ButtonLink, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::shell::{
    AppShell, Document, MainMode, NavItem, Sidebar, SidebarSection, ThemeToggle,
};

use crate::layout::{COPYRIGHT, GITHUB, SECTIONS, X};
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
    head: Option<Markup>,
    scripts: bool,
    toggle: bool,
    status: StatusCode,
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
            head: None,
            scripts: true,
            toggle: true,
            status: StatusCode::OK,
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

    /// Extra `<head>` content after the design-language assets, such as an
    /// area's own stylesheet or deferred script (never inline script).
    pub fn head(mut self, head: impl Render) -> Self {
        self.head = Some(head.render());
        self
    }

    /// A page that must run no script (its CSP has no `script-src`): no
    /// component script or Alpine; the theme toggle uses its fallback form.
    pub fn scriptless(mut self) -> Self {
        self.scripts = false;
        self
    }

    /// A page whose policy allows no form (`form-action 'none'`): no theme
    /// toggle, since without script its fallback is a form. The theme still
    /// follows the cookie, else the system setting.
    pub fn without_toggle(mut self) -> Self {
        self.toggle = false;
        self
    }

    /// The status [`UiPage::respond`] answers with (default `200`).
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
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
        let toggle = self.toggle.then(|| {
            ThemeToggle::new()
                .fallback_action(theme::TOGGLE_PATH)
                .return_to(self.return_to)
        });
        let actions = html! {
            @if let Some(actions) = &self.actions { (actions) }
            @if let Some(toggle) = toggle { (toggle) }
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
                @if let Some(head) = &self.head { (head) }
            })
            .body(shell)
            .render()
    }

    /// The page answered with its status (`200` unless [`UiPage::status`]).
    pub fn respond(self, headers: &HeaderMap) -> Response {
        let status = self.status;
        (status, Html(self.render(headers).into_string())).into_response()
    }
}

/// Trusted, already-escaped markup (such as `markdown::render` output or a
/// page body built with `layout::escape`) as prose in the reading column.
pub fn prose(content: impl Render) -> Markup {
    PageColumn::new(MarkdownRoot::new(content)).render()
}

/// [`prose`] in the wide column, for pages of tables.
pub fn wide_prose(content: impl Render) -> Markup {
    PageColumn::new(MarkdownRoot::new(content)).wide().render()
}

/// A secondary button link, for a page's "way back" and other actions.
pub fn action_link(label: &str, href: &str) -> ButtonLink {
    ButtonLink::new(label, href)
        .color(Color::Secondary)
        .variant(ButtonVariant::Outline)
}

/// A page answered with `status`: a heading, a sentence, and a way back.
/// It runs no script, so it is safe under any page's policy.
pub fn problem(
    headers: &HeaderMap,
    status: StatusCode,
    title: &str,
    text: &str,
    back: (&str, &str),
) -> Response {
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! { h1 { (title) } p { (text) } }))
        div.oa-page-actions { (action_link(back.1, back.0)) }
    });
    UiPage::new(title)
        .status(status)
        .scriptless()
        .content(content)
        .respond(headers)
}

fn legal_footer() -> Markup {
    html! {
        nav class="oa-legal" aria-label="Legal and links" {
            span { (COPYRIGHT) } " \u{b7} "
            a href="/terms" { "Terms" } " \u{b7} "
            a href="/privacy" { "Privacy" } " \u{b7} "
            a href=(GITHUB) rel="noopener" { "GitHub" } " \u{b7} "
            a href=(X) rel="noopener" { "X" }
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
    fn every_scrolling_page_links_the_terms_privacy_github_and_x_once() {
        let html = UiPage::new("Download OpenAgents")
            .section("/download")
            .scriptless()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string();
        assert_eq!(html.matches("href=\"/terms\"").count(), 1);
        assert_eq!(html.matches("href=\"/privacy\"").count(), 1);
        assert!(html.contains(
            "<a href=\"https://github.com/OpenAgentsInc/openagents\" rel=\"noopener\">GitHub</a>"
        ));
        assert!(html.contains("<a href=\"https://x.com/OpenAgentsInc\" rel=\"noopener\">X</a>"));
        assert!(html.contains(COPYRIGHT));
        assert!(html.contains("href=\"/download\" aria-current=\"page\""));
        assert!(!html.contains("href=\"/docs\" aria-current"));
        assert!(html.contains("width=device-width"));
        assert!(!html.to_ascii_lowercase().contains("<script"));
    }

    #[test]
    fn a_page_without_the_toggle_has_no_form() {
        let html = UiPage::new("Connect")
            .scriptless()
            .without_toggle()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string()
            .to_ascii_lowercase();
        assert!(
            !html.contains("<form") && !html.contains("<script"),
            "{html}"
        );
        assert!(!html.contains("data-oa-theme-toggle"), "{html}");
    }

    #[tokio::test]
    async fn problem_pages_answer_their_status_with_a_way_back_and_no_script() {
        let response = problem(
            &HeaderMap::new(),
            StatusCode::NOT_FOUND,
            "Not found",
            "Nothing <here>.",
            ("/", "Home"),
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("<h1>Not found</h1>"), "{html}");
        assert!(html.contains("<p>Nothing &lt;here&gt;.</p>"), "{html}");
        assert!(html.contains("href=\"/\""), "{html}");
        assert!(!html.to_ascii_lowercase().contains("<script"), "{html}");
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
