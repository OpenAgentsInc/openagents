//! Pages in the Coder Light / Coder Noir design language.
//!
//! Every page except `/demo`, the `/components` catalog, and the full-screen
//! canvas pages renders through [`UiPage`]: the `openagents-ui` document (theme from the cookie, else the system
//! setting), the shared app shell with the site navigation, the theme
//! toggle, and the design-language assets. Pages supply only their content,
//! and optionally a breadcrumb, actions, a composer, or extra sidebar
//! sections. The header row shows the page's name as its breadcrumb unless
//! the page gives one (a chat's title) or is the home page. The bottom of
//! the left panel holds the account ([`crate::account`]): the signed-in
//! account's menu, or Docs and a sign-in link.

use axum::http::header;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{ButtonLink, ButtonVariant, Color, ControlSize};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::icons::{Icon, IconSize};
use openagents_ui::overlays::MenuItem;
use openagents_ui::shell::{
    AccountMenu, AppShell, Breadcrumb, Document, LegalLinks, MainMode, NavItem, SIDEBAR_COOKIE,
    Sidebar, ThemeToggle, sidebar_collapsed_from_cookie,
};

use crate::account::Account;
use crate::layout::{COPYRIGHT, DOCS, DOWNLOAD, GITHUB, X};
use crate::theme;

/// Where the account menu's entries go. Billing joins them once a real
/// payment flow exists (docs/web/cloud-reset.md).
const SETTINGS: &str = crate::settings::PAGE;
const SIGN_OUT: &str = crate::cloud::SIGN_OUT;
const ROADMAP: &str = "/roadmap";
const PROMISES: &str = "/promises";

/// A top-level destination in the left panel, under "New chat".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    /// `/environments`: set up a repository and run Claude Code in it.
    Environments,
}

impl Nav {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Environments => "Environments",
        }
    }

    #[must_use]
    pub const fn href(self) -> &'static str {
        match self {
            Self::Environments => "/environments",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Environments => Icon::Cube,
        }
    }
}

/// The destinations the left panel shows, in order. A destination is added
/// here when its page exists (`Nav::Environments` with `/environments`,
/// shown on the local address only).
pub const NAV: &[Nav] = &[Nav::Environments];

/// Whether the left panel offers `nav`: Environments only when a studio is
/// configured and the request came to the local address.
fn nav_shown(nav: Nav, studio: bool, local: bool) -> bool {
    match nav {
        Nav::Environments => studio && local,
    }
}

/// One page: title, current section, content, and optional shell slots.
#[must_use]
pub struct UiPage {
    title: String,
    section: Option<String>,
    return_to: String,
    mode: MainMode,
    breadcrumb: Option<Option<Markup>>,
    account: Option<Account>,
    header: Option<Markup>,
    actions: Option<Markup>,
    content: Option<Markup>,
    composer: Option<Markup>,
    sections: Vec<Markup>,
    head: Option<Markup>,
    description: Option<String>,
    canonical: Option<String>,
    scripts: bool,
    toggle: bool,
    boost: bool,
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
            breadcrumb: None,
            account: None,
            header: None,
            actions: None,
            content: None,
            composer: None,
            sections: Vec::new(),
            head: None,
            description: None,
            canonical: None,
            scripts: true,
            toggle: true,
            boost: false,
            status: StatusCode::OK,
        }
    }

    /// The navigation entry to mark current, by its href (`/` marks "New
    /// chat", `/download` the Download pill, `/docs` the Docs row).
    pub fn section(mut self, href: impl Into<String>) -> Self {
        self.section = Some(href.into());
        self
    }

    /// Where the no-JavaScript theme toggle returns to: this page's path.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.return_to = path.into();
        self.canonical = Some(self.return_to.clone());
        self
    }

    /// An app page (chat, work views): the content area does not scroll and
    /// the composer docks under it.
    pub fn app(mut self) -> Self {
        self.mode = MainMode::App;
        self
    }

    /// The header row's breadcrumb, in place of the page's name (a chat's
    /// title, see [`Breadcrumb`]).
    pub fn breadcrumb(mut self, breadcrumb: impl Render) -> Self {
        self.breadcrumb = Some(Some(breadcrumb.render()));
        self
    }

    /// No breadcrumb (the home page).
    pub fn without_breadcrumb(mut self) -> Self {
        self.breadcrumb = Some(None);
        self
    }

    /// The account to show, in place of the one [`crate::account::current`]
    /// resolved for this request.
    pub fn account(mut self, account: Account) -> Self {
        self.account = Some(account);
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

    /// The page's one-line summary for search engines and agents
    /// (`<meta name="description">`); the site's own line without it.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
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

    /// A page in the chat family (the new chat and a chat's page, which
    /// share one `<head>`, [`crate::chat_html::head`]): its body is boosted,
    /// so moving between them swaps the body instead of loading a new
    /// document; `/static/chat-start.js` sends every other address to a
    /// full load.
    pub fn boosted(mut self) -> Self {
        self.boost = true;
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

    /// An extra left-panel section, such as the recent-chat list
    /// ([`openagents_ui::shell::ChatList`]).
    pub fn sidebar_section(mut self, section: impl Render) -> Self {
        self.sections.push(section.render());
        self
    }

    /// The whole HTML document for a request with these headers.
    pub fn render(self, headers: &HeaderMap) -> Markup {
        let current = self.section.as_deref();
        let scripts = self.scripts;
        let mut sidebar = Sidebar::new()
            .label("Main")
            .brand(html! { a class="oa-wordmark" href="/" { "OpenAgents" } })
            .nav(
                NavItem::new("New chat", "/")
                    .icon(Icon::ComposeEditSquare.size(IconSize::Md))
                    .current(current == Some("/"))
                    .shortcut("Control+N", "⌃N"),
            );
        for nav in NAV {
            // Environments answer only on the local address (the host
            // guard refuses them on public hosts), so a public page never
            // links them: no control that leads to a refusal.
            if !nav_shown(
                *nav,
                crate::environments::shown(),
                crate::local_request(headers),
            ) {
                continue;
            }
            sidebar = sidebar.nav(
                NavItem::new(nav.label(), nav.href())
                    .icon(nav.icon().size(IconSize::Md))
                    .current(current == Some(nav.href())),
            );
        }
        for section in self.sections {
            sidebar = sidebar.section(section);
        }
        let docs = NavItem::new("Docs", DOCS)
            .icon(Icon::Book.size(IconSize::Md))
            .current(current == Some(DOCS));
        // A page whose policy allows no form gets no sign-out form.
        let forms = self.toggle;
        let account = self.account.unwrap_or_else(crate::account::current);
        // Signed out on a server that signs people in: Log in and Sign up
        // in the header, returning to this page (docs/auth).
        let signed_out = account == Account::SignedOut;
        let log_in = crate::auth::login_href(&self.return_to, false);
        let sign_up = crate::auth::login_href(&self.return_to, true);
        let toggle = self.toggle.then(|| {
            ThemeToggle::new()
                .fallback_action(theme::TOGGLE_PATH)
                .return_to(self.return_to.clone())
        });
        // Roadmap and Promises sit with Docs at the bottom of the panel for
        // everyone (a signed-in person's Docs is in the account menu).
        let roadmap = NavItem::new("Roadmap", ROADMAP)
            .icon(Icon::MapsDirections.size(IconSize::Md))
            .current(current == Some(ROADMAP));
        let promises = NavItem::new("Promises", PROMISES)
            .icon(Icon::NotebookCheck.size(IconSize::Md))
            .current(current == Some(PROMISES));
        let sidebar = match account {
            Account::SignedIn {
                name,
                sign_out,
                picture,
            } => {
                let mut menu = AccountMenu::new(name)
                    .item(MenuItem::link("Settings", SETTINGS).icon(Icon::Settings))
                    .item(MenuItem::separator())
                    // Download lives in the header's pill only, not twice.
                    .item(MenuItem::link("Docs", DOCS).icon(Icon::Book));
                if picture {
                    menu = menu.picture(crate::account::AVATAR);
                }
                let sign_out = sign_out.filter(|_| forms);
                if sign_out.is_some() {
                    menu = menu.item(MenuItem::separator()).item(
                        MenuItem::button("Sign out")
                            .icon(Icon::ExitLogout)
                            .submit()
                            .form("oa-sign-out"),
                    );
                }
                sidebar.bottom(roadmap).bottom(promises).footer(html! {
                    (menu)
                    @if let Some(csrf) = sign_out {
                        form id="oa-sign-out" method="post" action=(SIGN_OUT) hidden {
                            input type="hidden" name="csrf" value=(csrf);
                        }
                    }
                })
            }
            Account::SignedOut => sidebar
                .bottom(docs)
                .bottom(roadmap)
                .bottom(promises)
                .bottom(NavItem::new("Log in", &log_in).icon(Icon::EnterLogin.size(IconSize::Md))),
            Account::Unknown => sidebar.bottom(docs).bottom(roadmap).bottom(promises),
        };
        // The theme toggle sits in the sidebar's bottom-right corner.
        let sidebar = match toggle {
            Some(toggle) => sidebar.corner(toggle),
            None => sidebar,
        };
        let on_download = current == Some(DOWNLOAD);
        let mut download = ButtonLink::new("Download", DOWNLOAD)
            .color(Color::Secondary)
            .variant(ButtonVariant::Outline)
            .size(ControlSize::Sm)
            .pill(true)
            .icon_start(Icon::Download)
            .selected(on_download);
        if on_download {
            download = download.attr("aria-current", "page");
        }
        let actions = html! {
            @if let Some(actions) = &self.actions { (actions) }
            (download)
            @if signed_out {
                (ButtonLink::new("Log in", &log_in)
                    .size(ControlSize::Sm)
                    .pill(true))
                (ButtonLink::new("Sign up", &sign_up)
                    .color(Color::Secondary)
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Sm)
                    .pill(true))
            }
        };
        let mut shell = AppShell::new()
            .mode(self.mode)
            .sidebar(sidebar)
            .sidebar_collapsed(sidebar_collapsed(headers))
            .actions(actions);
        let breadcrumb = match self.breadcrumb {
            Some(breadcrumb) => breadcrumb,
            None if current == Some("/") => None,
            None => Some(Breadcrumb::new(self.title.clone()).render()),
        };
        if let Some(breadcrumb) = breadcrumb {
            shell = shell.breadcrumb(breadcrumb);
        }
        if let Some(header) = self.header {
            shell = shell.header(header);
        }
        if let Some(content) = self.content {
            shell = shell.content(content);
        }
        if let Some(composer) = self.composer {
            shell = shell.composer(composer);
        }
        let canonical = self
            .canonical
            .as_deref()
            .map(|path| path.split('?').next().unwrap_or(path));
        let metadata =
            crate::agent_ready::head(&self.title, self.description.as_deref(), canonical);
        Document::new(self.title)
            .theme(theme::from_headers(headers))
            .boost(self.boost && scripts)
            .head(html! {
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                (PreEscaped(metadata))
                (PreEscaped(theme::style_tag()))
                @if scripts {
                    (PreEscaped(theme::script_tags()))
                    (PreEscaped(crate::agent_ready::webmcp_tag()))
                }
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

/// The quiet legal and project links the home page centers under its
/// docked composer (no other page shows them).
pub fn legal_links() -> Markup {
    html! {
        div.oa-home-legal {
            (LegalLinks::new()
                .link("Terms", "/terms")
                .link("Privacy", "/privacy")
                .link("GitHub", GITHUB)
                .link("X", X)
                .note(COPYRIGHT))
        }
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

/// Whether the visitor collapsed the left panel (the shell script's
/// [`SIDEBAR_COOKIE`]).
fn sidebar_collapsed(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| name == SIDEBAR_COOKIE && sidebar_collapsed_from_cookie(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, header};

    #[test]
    fn environments_is_offered_on_the_local_address_only() {
        assert!(nav_shown(Nav::Environments, true, true));
        assert!(!nav_shown(Nav::Environments, true, false), "public host");
        assert!(!nav_shown(Nav::Environments, false, true), "no studio");
        // A public page carries no link to it.
        let html = UiPage::new("OpenAgents")
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string();
        assert!(!html.contains("href=\"/environments\""));
    }

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
        let html = UiPage::new("OpenAgents")
            .section("/")
            .path("/")
            .app()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string();
        for needle in [
            theme::STYLESHEET_PATH,
            theme::SCRIPT_PATH,
            theme::ALPINE_PATH,
            "data-oa-theme-toggle",
            "data-oa-sidebar-toggle",
            "href=\"/\" aria-current=\"page\"",
            ">New chat</span>",
            "aria-keyshortcuts=\"Control+N\"",
            "<kbd class=\"oa-nav-shortcut\" aria-hidden=\"true\">⌃N</kbd>",
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        // No Chat or Cloud entries, no main-header menu button.
        assert!(!html.contains(">Chat</span>") && !html.contains(">Cloud</span>"));
        assert!(!html.contains("href=\"/chat\"") && !html.contains("href=\"/cloud/app\""));
        assert_eq!(html.matches("data-oa-sidebar-toggle").count(), 1);
        assert!(!html.contains("Open sidebar") && !html.contains("Close sidebar"));
        assert!(html.contains("data-sidebar=\"expanded\""));
        assert!(!html.contains("<footer"), "no page footer");
        // The home page has no breadcrumb.
        assert!(!html.contains("oa-breadcrumb"));
    }

    #[test]
    fn the_collapsed_cookie_renders_the_rail() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_theme=dark; oa_sidebar=collapsed"),
        );
        let html = UiPage::new("x").render(&h).into_string();
        assert!(html.contains("data-sidebar=\"collapsed\""), "{html}");
    }

    #[test]
    fn download_is_a_pill_beside_the_toggle_and_docs_sits_at_the_sidebar_bottom() {
        let html = UiPage::new("Download OpenAgents")
            .section("/download")
            .scriptless()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string();
        // Legal links are only on the home page.
        assert!(!html.contains("href=\"/terms\"") && !html.contains(COPYRIGHT));
        assert!(!html.contains("<footer") && !html.contains("oa-legal\""));
        // Docs lives in the left panel's footer.
        let aside_end = html.find("</aside>").unwrap();
        let sidebar_footer = html.find("class=\"oa-sidebar-footer\"").unwrap();
        let docs = html.find("href=\"/docs\"").unwrap();
        assert!(sidebar_footer < docs && docs < aside_end);
        // The page's name is the header row's breadcrumb.
        assert!(html.contains(
            "<span class=\"oa-breadcrumb-current\" aria-current=\"page\" title=\"Download OpenAgents\">"
        ));
        // Download is a pill link in the header actions; the theme toggle
        // sits in the sidebar's bottom-right corner, not the header.
        let actions = html.find("class=\"oa-main-header-actions\"").unwrap();
        let download = html.find("href=\"/download\"").unwrap();
        let toggle = html.find("data-oa-theme-toggle").unwrap();
        assert!(actions < download);
        assert!(sidebar_footer < toggle && toggle < aside_end);
        assert!(html.contains("class=\"oa-sidebar-corner\""));
        assert!(html[download.saturating_sub(200)..download].contains("oa-button"));
        assert!(html.contains("data-pill"), "{html}");
        assert!(
            html.contains("data-selected"),
            "the current page's pill is selected"
        );
        assert!(html.contains("width=device-width"));
        assert!(!html.contains("<script src"));
    }

    #[test]
    fn legal_links_list_terms_privacy_and_the_project() {
        let html = legal_links().into_string();
        assert!(html.starts_with("<div class=\"oa-home-legal\">"));
        assert_eq!(html.matches("href=\"/terms\"").count(), 1);
        assert_eq!(html.matches("href=\"/privacy\"").count(), 1);
        assert!(html.contains(
            "<a href=\"https://github.com/OpenAgentsInc/openagents\" rel=\"noopener\">GitHub</a>"
        ));
        assert!(html.contains("<a href=\"https://x.com/OpenAgentsInc\" rel=\"noopener\">X</a>"));
        assert!(html.contains(COPYRIGHT));
    }

    #[test]
    fn the_account_sits_at_the_bottom_left() {
        let page = |account: Account| {
            UiPage::new("Docs")
                .account(account)
                .render(&HeaderMap::new())
                .into_string()
        };
        // Signed in: an account menu above the button with settings,
        // docs and a sign-out form; Download is the header pill only.
        let html = page(Account::SignedIn {
            name: "Ada <Lovelace>".into(),
            sign_out: Some("token".into()),
            picture: false,
        });
        let footer = html.find("class=\"oa-sidebar-footer\"").unwrap();
        let account = html.find("<div class=\"oa-account\">").expect("account");
        assert!(footer < account && account < html.find("</aside>").unwrap());
        assert!(html.contains("<span class=\"oa-account-name\">Ada &lt;Lovelace&gt;</span>"));
        assert!(html.contains("data-side=\"top\""));
        assert!(!html.contains(">Billing<"));
        // Download is the header pill, not a second menu entry.
        let aside_end = html.find("</aside>").unwrap();
        assert!(!html[account..aside_end].contains(&format!("href=\"{DOWNLOAD}\"")));
        for href in [SETTINGS, DOCS] {
            assert!(
                html[account..].contains(&format!("href=\"{href}\"")),
                "{href}"
            );
        }
        assert!(
            html.contains("<form id=\"oa-sign-out\" method=\"post\" action=\"/sign-out\" hidden>")
        );
        assert!(html.contains("form=\"oa-sign-out\""));
        assert!(html.contains("name=\"csrf\" value=\"token\""));
        assert!(!html.contains(">Sign in<"));
        // A page that allows no form gets no sign-out button.
        let strict = UiPage::new("x")
            .without_toggle()
            .account(Account::SignedIn {
                name: "a".into(),
                sign_out: Some("t".into()),
                picture: true,
            })
            .render(&HeaderMap::new())
            .into_string();
        assert!(!strict.contains("<form") && !strict.contains(">Sign out<"));
        // Signed out: Docs and a sign-in link.
        let html = page(Account::SignedOut);
        assert!(html.contains("href=\"/login\"") && html.contains(">Log in</span>"));
        // ...and Log in and Sign up beside Download in the header.
        let header = &html[html.find("Download").unwrap()..];
        assert!(header.contains(">Log in<") && header.contains("href=\"/signup\""));
        assert!(html.contains("href=\"/docs\"") && !html.contains("oa-account"));
        // No sign-in on this server: Docs only.
        let html = page(Account::Unknown);
        assert!(
            !html.contains("/login")
                && !html.contains("/signup")
                && html.contains("href=\"/docs\"")
        );
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
            !html.contains("<form") && !html.contains("<script src"),
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
        assert!(!html.to_ascii_lowercase().contains("<script src"), "{html}");
    }

    #[test]
    fn scriptless_pages_load_no_script_but_keep_the_toggle_form() {
        let html = UiPage::new("Connect")
            .scriptless()
            .content(html! { p { "x" } })
            .render(&HeaderMap::new())
            .into_string()
            .to_ascii_lowercase();
        // Only the JSON-LD data block, which never runs.
        assert_eq!(
            html.matches("<script").count(),
            html.matches("<script type=\"application/ld+json\">")
                .count(),
            "{html}"
        );
        assert!(!html.contains("<script src"), "{html}");
        assert!(html.contains("action=\"/theme\""), "{html}");
    }
}
