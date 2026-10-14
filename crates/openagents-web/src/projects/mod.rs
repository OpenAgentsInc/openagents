//! Projects (#11034, `docs/web/sidebar.md` "Projects and repositories",
//! `docs/auth/github.md` "Repository access").
//!
//! A project is a name, optional instructions, and zero or more connected
//! GitHub repositories, one of them the primary when there are any.
//! `/projects` is the signed-in person's page: a form for a new project
//! (a name is enough), their projects, and a Connect GitHub step that
//! lists their repositories to start a project from. `/projects/{id}`
//! ([`page`]) is one project: its name and instructions, and its
//! repositories with add, remove, and make-primary. Connecting
//! starts a second trip to GitHub through the same OAuth App
//! ([`oa_auth::Purpose::Repos`]) that asks for `repo` and `read:org` only
//! then, or nothing more for public repositories only. GitHub returns to
//! `/auth/github/callback`; because the session cookie is `SameSite=Strict`
//! and that request comes from github.com, the callback continues with a
//! same-site step to `/auth/github/repos/finish`, which hands the code to
//! the account service under the session.
//!
//! The account service keeps the token encrypted and the projects; this
//! server keeps neither. The left panel groups chats by project through
//! [`sidebar`], read once per request by the [`scope`] middleware's
//! request-local cache.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use oa_auth::repos::{Access, Listing, Project, RepoError, Repository, Status};
use openagents_ui::actions::{
    Alert, Badge, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize,
};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::Input;
use serde::Deserialize;

use crate::App;
use crate::cloud::protect;
use crate::cloud::session::github::RepoCallError;
use crate::cloud::session::{CloudSession, SessionError, Viewer};
use crate::ui_page::UiPage;

/// The projects page.
pub(crate) const PAGE: &str = "/projects";
/// Where connecting repositories starts (`?access=private|public`).
pub(crate) const CONNECT: &str = "/auth/github/repos";
/// The same-site step after GitHub's callback.
pub(crate) const FINISH: &str = "/auth/github/repos/finish";
/// Installing the GitHub App on repositories (when this server has one).
pub(crate) const INSTALL: &str = "/auth/github/install";
/// Where GitHub's install page sends the person back (the App's setup URL).
pub(crate) const SETUP: &str = "/auth/github/setup";
/// Connecting GitHub again after it stopped accepting the access.
pub(crate) const RECONNECT: &str = "/auth/github/reconnect";
/// The cookie `shell.js` keeps the closed project groups in.
const CLOSED_COOKIE: &str = "oa_project_groups";
const CSRF_SCOPE: &str = "projects";
/// The most repositories the page lists at once.
pub(crate) mod api;
pub(crate) mod files;
pub(crate) mod members;
pub(crate) mod page;
pub(crate) mod shared_chats;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(api::routes())
        .merge(files::routes())
        .merge(members::routes())
        .route(PAGE, get(list_page).post(add))
        .route(REPOS, get(repositories))
        .merge(page::routes())
        .route("/projects/{id}/remove", post(remove))
        .route("/projects/disconnect", post(disconnect))
        .route(CONNECT, get(connect))
        .route(FINISH, get(finish))
        .route(INSTALL, get(install))
        .route(SETUP, get(setup))
        .route(RECONNECT, get(reconnect))
}

/// The signed-in person's projects for the left panel, and the groups they
/// closed in this browser.
#[derive(Clone, Debug)]
pub(crate) struct Sidebar {
    pub status: Status,
    pub closed: BTreeSet<String>,
}

impl Sidebar {
    /// The viewer's project with this id.
    pub(crate) fn project(&self, id: &str) -> Option<&Project> {
        self.status.projects.iter().find(|p| p.id == id)
    }

    /// Whether GitHub stopped accepting the stored access.
    pub(crate) fn reconnect(&self) -> bool {
        matches!(self.status.access, Access::Reconnect { .. })
    }
}

struct Context {
    headers: HeaderMap,
    sidebar: tokio::sync::OnceCell<Option<Arc<Sidebar>>>,
}

tokio::task_local! {
    static REQUEST: Arc<Context>;
}

/// Keeps a signed-in request's headers so the chat list can read the
/// person's projects ([`sidebar`]) without every caller passing them, and
/// reads them at most once per request. Requests without the session
/// cookie pass straight through.
pub(crate) async fn scope(request: Request, next: Next) -> Response {
    if !has_session(request.headers()) {
        return next.run(request).await;
    }
    let context = Arc::new(Context {
        headers: request.headers().clone(),
        sidebar: tokio::sync::OnceCell::new(),
    });
    REQUEST.scope(context, next.run(request)).await
}

/// Runs `work` as if inside a request with `headers` (the sidebar's live
/// stream draws the list after its request's scope has ended).
pub(crate) async fn with_headers<F: std::future::Future>(headers: HeaderMap, work: F) -> F::Output {
    if !has_session(&headers) {
        return work.await;
    }
    let context = Arc::new(Context {
        headers,
        sidebar: tokio::sync::OnceCell::new(),
    });
    REQUEST.scope(context, work).await
}

/// The signed-in person's projects for this request, or `None` when nobody
/// is signed in (or the account service can't be reached).
pub(crate) async fn sidebar(app: &App) -> Option<Arc<Sidebar>> {
    let context = REQUEST.try_with(Arc::clone).ok()?;
    context
        .sidebar
        .get_or_init(|| async {
            let service = app.config.cloud.as_deref()?;
            let status = service.github_status(&context.headers).await.ok()?;
            Some(Arc::new(Sidebar {
                status,
                closed: closed_groups(&context.headers),
            }))
        })
        .await
        .clone()
}

/// The signed-in request's headers, inside [`scope`] (who started each
/// chat in a shared project is read with them).
pub(crate) fn request_headers() -> Option<HeaderMap> {
    REQUEST.try_with(|context| context.headers.clone()).ok()
}

/// The project a new chat starts in: `value` when it is one of the
/// signed-in person's projects.
pub(crate) async fn chosen(app: &App, value: &str) -> Option<String> {
    let value = value.trim();
    if !oa_auth::repos::project_id(value) {
        return None;
    }
    sidebar(app)
        .await?
        .project(value)
        .filter(|project| api::role_of(project).edits())
        .map(|project| project.id.clone())
}

fn has_session(headers: &HeaderMap) -> bool {
    cookies(headers).any(|(name, value)| name == "oa_cloud_session" && !value.is_empty())
}

fn cookies(headers: &HeaderMap) -> impl Iterator<Item = (&str, &str)> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
}

/// The project groups this browser closed (`shell.js`).
fn closed_groups(headers: &HeaderMap) -> BTreeSet<String> {
    cookies(headers)
        .filter(|(name, _)| *name == CLOSED_COOKIE)
        .flat_map(|(_, value)| value.split('.'))
        .filter(|id| oa_auth::repos::project_id(id))
        .take(200)
        .map(str::to_string)
        .collect()
}

/// The signed-in viewer, or the answer to give instead (log in first).
async fn viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = crate::cloud::service(app)?;
    match service.authenticate(headers).await {
        Ok(viewer) => Ok((service, viewer)),
        Err(SessionError::Unauthenticated) => Err(protect(
            Redirect::to(&crate::auth::login_href(PAGE, false)).into_response(),
        )),
        Err(error) => Err(crate::cloud::refused(error)),
    }
}

#[derive(Default, Deserialize)]
struct PageQuery {
    #[serde(default)]
    q: String,
}

async fn list_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    render(
        service,
        &viewer,
        &headers,
        &query.q,
        None,
        StatusCode::OK,
        app.config.github_install.is_some(),
        crate::connections::access::permitted(&app, &viewer),
    )
    .await
}

/// The page, with `problem` shown on top when an action didn't work.
async fn render(
    service: &CloudSession,
    viewer: &Viewer,
    headers: &HeaderMap,
    q: &str,
    problem: Option<String>,
    status: StatusCode,
    install: bool,
    sources: bool,
) -> Response {
    let state = match service.github_status(headers).await {
        Ok(state) => state,
        Err(error) => return failed(headers, &error),
    };
    let token = service
        .csrf(headers, viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let body = view(&state, &token, q, problem.as_deref(), install, sources);
    page_for(headers, status, body)
}

/// The Projects page around `body`.
pub(crate) fn page_for(headers: &HeaderMap, status: StatusCode, body: Markup) -> Response {
    protect(
        UiPage::new("Projects")
            .path(PAGE)
            .section(PAGE)
            .status(status)
            .head(htmx_head())
            .content(PageColumn::new(body))
            .respond(headers),
    )
}

/// HTMX for the list that loads after the page shows.
fn htmx_head() -> Markup {
    html! {
        meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
        script src="/static/htmx.min.js" defer {}
    }
}

/// Where one page of repositories loads from.
const REPOS: &str = "/projects/repositories";

fn repos_href(page: u32, q: &str, project: Option<&str>) -> String {
    let mut href = format!("{REPOS}?page={page}");
    if let Some(project) = project {
        href.push_str("&project=");
        href.push_str(project);
    }
    if !q.is_empty() {
        href.push_str("&q=");
        href.push_str(&url::form_urlencoded::byte_serialize(q.as_bytes()).collect::<String>());
    }
    href
}

#[derive(Deserialize)]
struct ReposQuery {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    q: String,
    /// Adding to this project, not starting a new one.
    #[serde(default)]
    project: Option<String>,
}

/// One page of repositories to add, and a Show more button for the next.
async fn repositories(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ReposQuery>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let page = query.page.unwrap_or(1).clamp(1, oa_auth::repos::MAX_PAGE);
    let state = match service.github_status(&headers).await {
        Ok(state) => state,
        Err(error) => return fragment(html! { p role="alert" { (error.to_string()) } }),
    };
    let csrf = service
        .csrf(&headers, &viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let listed = service.github_repositories(&headers, page).await;
    let project = query
        .project
        .as_deref()
        .and_then(|id| state.projects.iter().find(|p| p.id == id));
    fragment(repos_page(listed, &csrf, query.q.trim(), page, project))
}

fn fragment(body: Markup) -> Response {
    protect(body.into_response())
}

/// The rows of one page (filtered by `q`), and the button that loads the
/// next page in its place. With `project`, Add puts the repository in that
/// project (those already in it are left out); without, Add starts a new
/// project from it (a repository may be in any number of projects).
pub(crate) fn repos_page(
    listed: Result<Listing, RepoCallError>,
    csrf: &str,
    q: &str,
    page: u32,
    project: Option<&Project>,
) -> Markup {
    let in_project = project.map(|p| p.id.as_str());
    let action = project.map_or_else(|| PAGE.to_owned(), |p| page::repositories_href(&p.id));
    let Listing {
        repositories,
        more,
        sso_hidden,
    } = match listed {
        Ok(found) => found,
        Err(RepoCallError::Repo(RepoError::Reconnect)) => {
            return html! {
                (Alert::new()
                    .color(Color::Warning)
                    .title("GitHub access ended")
                    .description("Your projects are still here. Reconnect GitHub to add repositories again.")
                    .actions(ButtonLink::new("Reconnect GitHub", RECONNECT)))
            };
        }
        Err(error) => {
            return html! {
                div #projects-repos {
                    p role="alert" { (error.to_string()) }
                    (ButtonLink::new("Try again", repos_href(page, q, in_project))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary)
                        .attr("hx-get", repos_href(page, q, in_project))
                        .attr("hx-target", "#projects-repos")
                        .attr("hx-swap", "outerHTML"))
                }
            };
        }
    };
    let have: BTreeSet<u64> = project
        .map(|p| p.repositories.iter().map(|r| r.repository_id).collect())
        .unwrap_or_default();
    let needle = q.to_lowercase();
    let shown: Vec<&Repository> = repositories
        .iter()
        .filter(|r| !have.contains(&r.id))
        .filter(|r| needle.is_empty() || r.full_name.to_lowercase().contains(&needle))
        .collect();
    // A repository not on these pages can still be added by owner/name.
    let by_name = page == 1
        && oa_auth::repos::full_name(q)
        && !repositories
            .iter()
            .any(|r| r.full_name.eq_ignore_ascii_case(q));
    let next = format!("projects-repos-{}", page + 1);
    html! {
        @if !shown.is_empty() {
            ul.oa-chat-archive-list role="list" {
                @for repository in shown {
                    li.oa-chat-archive-row {
                        span {
                            (repository.full_name)
                            @if repository.private { " " (Badge::new("Private")) }
                            @if repository.archived { " " (Badge::new("Archived")) }
                        }
                        form method="post" action=(action) {
                            input type="hidden" name="csrf" value=(csrf);
                            input type="hidden" name="repository" value=(repository.full_name);
                            (Button::new("Add")
                                .kind(ButtonType::Submit)
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Soft)
                                .color(Color::Secondary))
                        }
                    }
                }
            }
        } @else if !more && page == 1 && !by_name {
            p { @if q.is_empty() { "No more repositories to add." } @else { "No repositories match." } }
        }
        @if by_name {
            form method="post" action=(action) {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="repository" value=(q);
                (Button::new(format!("Add {q}"))
                    .kind(ButtonType::Submit)
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary))
            }
        }
        @if sso_hidden && page == 1 {
            p.oa-page-meta { "Some organization repositories are hidden until you authorize OpenAgents for that organization's single sign-on on GitHub." }
        }
        @if more {
            div id=(next) {
                (ButtonLink::new("Show more", repos_href(page + 1, q, in_project))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary)
                    .attr("hx-get", repos_href(page + 1, q, in_project))
                    .attr("hx-target", format!("#{next}"))
                    .attr("hx-swap", "outerHTML"))
            }
        }
    }
}

/// The page body (separate from I/O for tests). `install`: this server
/// adds repositories through its GitHub App.
fn view(
    state: &Status,
    csrf: &str,
    q: &str,
    problem: Option<&str>,
    install: bool,
    sources: bool,
) -> Markup {
    view_with(state, csrf, q, problem, install, None, sources)
}

/// The page body with `list`, the repositories list, in place of the one
/// that loads after the page shows.
pub(crate) fn view_with(
    state: &Status,
    csrf: &str,
    q: &str,
    problem: Option<&str>,
    install: bool,
    list: Option<Markup>,
    sources: bool,
) -> Markup {
    let q = q.trim();
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Projects" }
            p { "A project keeps chats, instructions, and files together. Add GitHub repositories to it if you like." }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        @if let Access::Reconnect { .. } = state.access {
            (Alert::new()
                .color(Color::Warning)
                .title("GitHub access ended")
                .description("Your projects are still here. Reconnect GitHub to add repositories again.")
                .actions(ButtonLink::new("Reconnect GitHub", RECONNECT)))
        }
        section aria-labelledby="projects-new" {
            (MarkdownRoot::new(html! { h2 #projects-new { "New project" } }))
            (page::new_form(csrf))
        }
        @if !state.projects.is_empty() {
            section aria-labelledby="projects-yours" {
                (MarkdownRoot::new(html! { h2 #projects-yours { "Your projects" } }))
                ul.oa-chat-archive-list role="list" {
                    @for project in &state.projects {
                        li.oa-chat-archive-row {
                            span {
                                a href=(page::href(&project.id)) { strong { (project.name) } }
                                " · " (page::repositories_line(project))
                                @if project.primary().is_some_and(|r| r.private) { " " (Badge::new("Private")) }
                            }
                            span.oa-page-actions {
                                (ButtonLink::new("New chat", format!("/?project={}", project.id))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                                (ButtonLink::new("Open", page::href(&project.id))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .color(Color::Secondary))
                                @if sources {
                                    (ButtonLink::new("Sources", crate::connections::sources::href(&project.id))
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .color(Color::Secondary))
                                }
                                (ButtonLink::new("Vault", crate::vault::project_href(&project.id))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .color(Color::Secondary))
                                form method="post" action=(format!("/projects/{}/remove", project.id)) {
                                    input type="hidden" name="csrf" value=(csrf);
                                    (Button::new("Remove")
                                        .kind(ButtonType::Submit)
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .color(Color::Secondary))
                                }
                            }
                        }
                    }
                }
            }
        }
        section aria-labelledby="projects-add" {
            (MarkdownRoot::new(html! { h2 #projects-add { "Start from a repository" } }))
            (github_section(state, csrf, q, install, list, None))
        }
    }
}

/// Connecting GitHub, and the repositories to pick from: each starts a new
/// project, or with `project` goes into it.
pub(crate) fn github_section(
    state: &Status,
    csrf: &str,
    q: &str,
    install: bool,
    list: Option<Markup>,
    project: Option<&Project>,
) -> Markup {
    let in_project = project.map(|p| p.id.as_str());
    let here = project.map_or_else(|| PAGE.to_owned(), |p| page::href(&p.id));
    html! {
        @match &state.access {
            Access::Installed { login, installations } => {
                p { "Connected to GitHub as " strong { (login) } "." }
                @if installations.is_empty() {
                    p { "Choose the repositories OpenAgents can use on GitHub." }
                    div.oa-page-actions {
                        (ButtonLink::new("Choose repositories", INSTALL))
                    }
                } @else {
                    form method="get" action=(here) role="search" {
                        (Input::new("q").value(q).placeholder("Filter repositories").aria_label("Filter repositories"))
                    }
                    @if let Some(list) = &list {
                        (list)
                    } @else {
                        div #projects-repos hx-get=(repos_href(1, q, in_project)) hx-trigger="load" hx-swap="outerHTML" {
                            p { (openagents_ui::actions::Busy::new("Loading your repositories")) }
                        }
                    }
                    p {
                        "Missing one? "
                        a href=(INSTALL) { "Choose repositories on GitHub" }
                    }
                }
                @if project.is_none() {
                    (disconnect_form(csrf))
                }
            }
            Access::None if install => {
                p { "Install OpenAgents on the GitHub repositories you want to use." }
                div.oa-page-actions {
                    (ButtonLink::new("Install on repositories", INSTALL))
                }
                p.oa-page-meta {
                    "OpenAgents can read and change code only in the repositories you pick, and you can change them on GitHub at any time."
                }
            }
            Access::Connected { login, private } => {
                p { "Connected to GitHub as " strong { (login) } "." }
                form method="get" action=(here) role="search" {
                    (Input::new("q").value(q).placeholder("Filter repositories").aria_label("Filter repositories"))
                }
                // The list loads after the page shows (one GitHub call
                // per page of repositories), most recently pushed first.
                @if let Some(list) = &list {
                    (list)
                } @else {
                    div #projects-repos hx-get=(repos_href(1, q, in_project)) hx-trigger="load" hx-swap="outerHTML" {
                        p { (openagents_ui::actions::Busy::new("Loading your repositories")) }
                    }
                }
                @if install {
                    p {
                        a href=(INSTALL) { "Install OpenAgents on your repositories" }
                        " to pick exactly which ones it can use."
                    }
                } @else if !private {
                    p {
                        "Only public repositories are shown. "
                        a href=(format!("{CONNECT}?access=private")) { "Include private repositories" }
                    }
                }
                @if project.is_none() {
                    (disconnect_form(csrf))
                }
            }
            _ => {
                p { "Connect GitHub to pick one of your repositories." }
                div.oa-page-actions {
                    (ButtonLink::new("Connect GitHub", format!("{CONNECT}?access=private")))
                    (crate::ui_page::action_link("Public repositories only", &format!("{CONNECT}?access=public")))
                }
                p.oa-page-meta {
                    "Including private repositories asks GitHub to let OpenAgents read and change their code. Public only asks for nothing new."
                }
            }
        }
    }
}

fn disconnect_form(csrf: &str) -> Markup {
    html! {
        form method="post" action="/projects/disconnect" {
            input type="hidden" name="csrf" value=(csrf);
            (Button::new("Disconnect GitHub")
                .kind(ButtonType::Submit)
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .color(Color::Secondary))
        }
    }
}

fn failed(headers: &HeaderMap, error: &RepoCallError) -> Response {
    match error {
        RepoCallError::Session(SessionError::Unauthenticated) => {
            protect(Redirect::to(&crate::auth::login_href(PAGE, false)).into_response())
        }
        RepoCallError::Session(error) => crate::cloud::refused(*error),
        RepoCallError::Repo(error) => protect(crate::ui_page::problem(
            headers,
            StatusCode::from_u16(error.status()).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
            "Projects",
            &error.to_string(),
            (PAGE, "Back to projects"),
        )),
    }
}

#[derive(Deserialize)]
struct AddForm {
    csrf: String,
    /// A new project by name (with `instructions`)…
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    /// …or one started from a repository, named after it.
    #[serde(default)]
    repository: Option<String>,
}

#[derive(Deserialize)]
struct Plain {
    csrf: String,
}

/// Checks the form's CSRF ticket for the signed-in viewer.
async fn checked<'a>(
    app: &'a App,
    headers: &HeaderMap,
    token: &str,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let (service, viewer) = viewer(app, headers).await?;
    service
        .verify_csrf(headers, Some(&viewer), CSRF_SCOPE, "", token)
        .map_err(crate::cloud::refused)?;
    Ok((service, viewer))
}

async fn add(State(app): State<App>, headers: HeaderMap, Form(form): Form<AddForm>) -> Response {
    let (service, viewer) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let repository = form
        .repository
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    let name = form
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty());
    let new = oa_auth::repos::NewProject {
        name: name.clone(),
        instructions: form.instructions.filter(|t| !t.trim().is_empty()),
        repositories: repository.into_iter().collect(),
    };
    let created = if new.name.is_none() && new.repositories.is_empty() {
        Err(RepoCallError::Repo(RepoError::Invalid))
    } else {
        service.create_project(&headers, &new).await
    };
    match created {
        Ok(project) => protect(Redirect::to(&page::href(&project.id)).into_response()),
        Err(RepoCallError::Repo(error)) => {
            let problem = if error == RepoError::Invalid && new.repositories.is_empty() {
                "Give the project a name of up to 100 characters.".to_owned()
            } else {
                error.to_string()
            };
            render(
                service,
                &viewer,
                &headers,
                "",
                Some(problem),
                StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
                app.config.github_install.is_some(),
                crate::connections::access::permitted(&app, &viewer),
            )
            .await
        }
        Err(error) => failed(&headers, &error),
    }
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Plain>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match service.remove_project(&headers, &id).await {
        Ok(()) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => failed(&headers, &error),
    }
}

async fn disconnect(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<Plain>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match service.github_disconnect(&headers).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => failed(&headers, &error),
    }
}

#[derive(Deserialize)]
struct ConnectQuery {
    #[serde(default)]
    access: String,
}

/// Start connecting repositories: signed in only, then off to GitHub.
async fn connect(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ConnectQuery>,
) -> Response {
    if let Err(response) = viewer(&app, &headers).await {
        return response;
    }
    let purpose = oa_auth::Purpose::Repos {
        private: query.access != "public",
    };
    crate::auth::begin(&app, &headers, Some(PAGE), purpose)
}

/// GitHub's callback for a repository trip: continue with a same-site
/// step, so the next request carries the `SameSite=Strict` session cookie.
/// The flow cookie stays until [`finish`].
pub(crate) fn continue_page(headers: &HeaderMap, code: &str, state: &str) -> Response {
    let next = format!("{FINISH}?code={}&state={}", encode(code), encode(state));
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Connecting GitHub" }
            p { a href=(next) { "Continue" } }
        }))
    });
    protect(
        UiPage::new("Connecting GitHub")
            .scriptless()
            .head(html! { meta http-equiv="refresh" content=(format!("0;url={next}")); })
            .content(content)
            .respond(headers),
    )
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[derive(Deserialize)]
struct FinishQuery {
    code: Option<String>,
    state: Option<String>,
}

/// Hand GitHub's code to the account service under the session, then back
/// to the projects page.
async fn finish(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<FinishQuery>,
) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return crate::cloud::refused(SessionError::Unavailable);
    };
    let clear = HeaderValue::from_str(&oa_auth::flow::clear_cookie(service.secure()))
        .expect("static cookie is valid");
    let mut response = finished(&app, &headers, query).await;
    response.headers_mut().append(header::SET_COOKIE, clear);
    response
}

async fn finished(app: &App, headers: &HeaderMap, query: FinishQuery) -> Response {
    let (service, viewer) = match viewer(app, headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let install = app.config.github_install.is_some();
    let flow = crate::auth::flow_cookie(headers).filter(|flow| {
        matches!(
            flow.purpose,
            oa_auth::Purpose::Repos { .. } | oa_auth::Purpose::Install | oa_auth::Purpose::Board
        ) && query
            .state
            .as_deref()
            .is_some_and(|state| flow.matches(state))
    });
    let (Some(flow), Some(code)) = (flow, query.code.as_deref()) else {
        return render(
            service,
            &viewer,
            headers,
            "",
            Some("That GitHub connection expired. Start again from this browser.".into()),
            StatusCode::BAD_REQUEST,
            install,
            crate::connections::access::permitted(&app, &viewer),
        )
        .await;
    };
    let granted = if flow.purpose == oa_auth::Purpose::Install {
        service
            .github_app_grant(headers, code, flow.verifier())
            .await
    } else {
        service.github_grant(headers, code, flow.verifier()).await
    };
    match granted {
        // Authorized but installed nowhere yet: on to GitHub's page for
        // picking repositories.
        Ok(Status {
            access: Access::Installed { installations, .. },
            ..
        }) if installations.is_empty() => match app.config.github_install.as_deref() {
            Some(found) => protect(Redirect::to(&found.install_url()).into_response()),
            None => protect(Redirect::to(&flow.return_to).into_response()),
        },
        Ok(_) => protect(Redirect::to(&flow.return_to).into_response()),
        Err(RepoCallError::Repo(error)) => {
            render(
                service,
                &viewer,
                headers,
                "",
                Some(error.to_string()),
                StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
                install,
                crate::connections::access::permitted(&app, &viewer),
            )
            .await
        }
        Err(error) => failed(headers, &error),
    }
}

/// "Install on repositories": a person who authorized the GitHub App goes
/// straight to GitHub's page for picking repositories; anyone else
/// authorizes it first (which comes back here through [`finish`]).
async fn install(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, _) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(found) = app.config.github_install.as_deref() else {
        return protect(Redirect::to(PAGE).into_response());
    };
    match service.github_status(&headers).await {
        Ok(Status {
            access: Access::Installed { .. },
            ..
        }) => protect(Redirect::to(&found.install_url()).into_response()),
        Ok(_) => crate::auth::begin(&app, &headers, Some(PAGE), oa_auth::Purpose::Install),
        Err(error) => failed(&headers, &error),
    }
}

/// GitHub's install page sends the person back here (the App's setup URL,
/// with an `installation_id` this server never trusts): the account
/// service finds the person's installations again with their own token.
async fn setup(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, _) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if app.config.github_install.is_none() {
        return protect(Redirect::to(PAGE).into_response());
    }
    match service.github_app_refresh(&headers).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        // Installed from GitHub without authorizing here yet, or the
        // authorization ended: authorize, then back to the projects page.
        Err(RepoCallError::Repo(RepoError::NotConnected | RepoError::Reconnect)) => {
            crate::auth::begin(&app, &headers, Some(PAGE), oa_auth::Purpose::Install)
        }
        Err(error) => failed(&headers, &error),
    }
}

/// Reconnect GitHub: through the GitHub App when this server has one,
/// else the OAuth App with private repositories.
async fn reconnect(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(response) = viewer(&app, &headers).await {
        return response;
    }
    let purpose = if app.config.github_install.is_some() {
        oa_auth::Purpose::Install
    } else {
        oa_auth::Purpose::Repos { private: true }
    };
    crate::auth::begin(&app, &headers, Some(PAGE), purpose)
}

#[cfg(test)]
pub(crate) mod tests;
