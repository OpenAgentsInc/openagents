//! Projects from connected GitHub repositories (#11034,
//! `docs/web/sidebar.md` "Projects and repositories", `docs/auth/github.md`
//! "Repository access").
//!
//! `/projects` is the signed-in person's page: their projects, and a
//! Connect GitHub step that lists their repositories to add. Connecting
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
use oa_auth::repos::{Access, Project, RepoError, Repository, Status};
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
/// The cookie `shell.js` keeps the closed project groups in.
const CLOSED_COOKIE: &str = "oa_project_groups";
const CSRF_SCOPE: &str = "projects";
/// The most repositories the page lists at once.
const SHOWN: usize = 100;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page).post(add))
        .route("/projects/{id}/remove", post(remove))
        .route("/projects/disconnect", post(disconnect))
        .route(CONNECT, get(connect))
        .route(FINISH, get(finish))
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
        .map(|project| project.id.clone())
}

/// The new-chat composer's project picker, for a signed-in person with
/// projects; `preferred` (from `/?project=`) starts selected.
pub(crate) async fn picker(app: &App, preferred: Option<&str>) -> Option<Markup> {
    let sidebar = sidebar(app).await?;
    if sidebar.status.projects.is_empty() {
        return None;
    }
    let mut select = openagents_ui::forms::Select::new("project")
        .id("chat-project")
        .form("chat-form")
        .aria_label("Project")
        .block(false)
        .size(openagents_ui::forms::ControlSize::Sm)
        .option("", "No project");
    for project in &sidebar.status.projects {
        select = select.option(project.id.clone(), project.name.clone());
    }
    if let Some(project) = preferred.and_then(|id| sidebar.project(id)) {
        select = select.selected(project.id.clone());
    }
    Some(html! { div.oa-composer-selector-group { (select) } })
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

async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    render(service, &viewer, &headers, &query.q, None, StatusCode::OK).await
}

/// The page, with `problem` shown on top when an action didn't work.
async fn render(
    service: &CloudSession,
    viewer: &Viewer,
    headers: &HeaderMap,
    q: &str,
    problem: Option<String>,
    status: StatusCode,
) -> Response {
    let state = match service.github_status(headers).await {
        Ok(state) => state,
        Err(error) => return failed(headers, &error),
    };
    let token = service
        .csrf(headers, viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let listed = match state.access {
        Access::Connected { .. } => Some(service.github_repositories(headers).await),
        _ => None,
    };
    // A token GitHub stopped accepting shows up while listing.
    let state = match &listed {
        Some(Err(RepoCallError::Repo(RepoError::Reconnect))) => {
            service.github_status(headers).await.unwrap_or(state)
        }
        _ => state,
    };
    let body = view(&state, listed.as_ref(), &token, q, problem.as_deref());
    protect(
        UiPage::new("Projects")
            .path(PAGE)
            .section(PAGE)
            .status(status)
            .content(PageColumn::new(body))
            .respond(headers),
    )
}

/// The page body (separate from I/O for tests).
fn view(
    state: &Status,
    listed: Option<&Result<Vec<Repository>, RepoCallError>>,
    csrf: &str,
    q: &str,
    problem: Option<&str>,
) -> Markup {
    let have: BTreeSet<u64> = state.projects.iter().map(|p| p.repository_id).collect();
    let q = q.trim();
    let needle = q.to_lowercase();
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Projects" }
            p { "A project is a GitHub repository. Chats you start in a project are grouped under it." }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        @if let Access::Reconnect { .. } = state.access {
            (Alert::new()
                .color(Color::Warning)
                .title("GitHub access ended")
                .description("Your projects are still here. Reconnect GitHub to add repositories again.")
                .actions(ButtonLink::new("Reconnect GitHub", format!("{CONNECT}?access=private"))))
        }
        @if !state.projects.is_empty() {
            section aria-labelledby="projects-yours" {
                (MarkdownRoot::new(html! { h2 #projects-yours { "Your projects" } }))
                ul.oa-chat-archive-list role="list" {
                    @for project in &state.projects {
                        li.oa-chat-archive-row {
                            span {
                                strong { (project.name) } " · " (project.repository)
                                @if project.private { " " (Badge::new("Private")) }
                            }
                            span.oa-page-actions {
                                (ButtonLink::new("New chat", format!("/?project={}", project.id))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Soft)
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
            (MarkdownRoot::new(html! { h2 #projects-add { "Add a repository" } }))
            @match &state.access {
                Access::Connected { login, private } => {
                    p { "Connected to GitHub as " strong { (login) } "." }
                    @match listed {
                        Some(Ok(repositories)) => {
                            form method="get" action=(PAGE) role="search" {
                                (Input::new("q").value(q).placeholder("Filter repositories").aria_label("Filter repositories"))
                            }
                            @let shown: Vec<&Repository> = repositories
                                .iter()
                                .filter(|r| !have.contains(&r.id))
                                .filter(|r| needle.is_empty() || r.full_name.to_lowercase().contains(&needle))
                                .take(SHOWN)
                                .collect();
                            @if shown.is_empty() {
                                p { @if q.is_empty() { "No more repositories to add." } @else { "No repositories match." } }
                            } @else {
                                ul.oa-chat-archive-list role="list" {
                                    @for repository in shown {
                                        li.oa-chat-archive-row {
                                            span {
                                                (repository.full_name)
                                                @if repository.private { " " (Badge::new("Private")) }
                                            }
                                            form method="post" action=(PAGE) {
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
                            }
                        }
                        Some(Err(error)) => { p role="alert" { (error.to_string()) } }
                        None => {}
                    }
                    @if !private {
                        p {
                            "Only public repositories are shown. "
                            a href=(format!("{CONNECT}?access=private")) { "Include private repositories" }
                        }
                    }
                    form method="post" action="/projects/disconnect" {
                        input type="hidden" name="csrf" value=(csrf);
                        (Button::new("Disconnect GitHub")
                            .kind(ButtonType::Submit)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .color(Color::Secondary))
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
    repository: String,
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
    match service.add_project(&headers, form.repository.trim()).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(RepoCallError::Repo(error)) => {
            render(
                service,
                &viewer,
                &headers,
                "",
                Some(error.to_string()),
                StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
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
    let flow = crate::auth::flow_cookie(headers).filter(|flow| {
        matches!(flow.purpose, oa_auth::Purpose::Repos { .. })
            && query
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
        )
        .await;
    };
    match service.github_grant(headers, code, flow.verifier()).await {
        Ok(_) => protect(Redirect::to(&flow.return_to).into_response()),
        Err(RepoCallError::Repo(error)) => {
            render(
                service,
                &viewer,
                headers,
                "",
                Some(error.to_string()),
                StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
            )
            .await
        }
        Err(error) => failed(headers, &error),
    }
}

#[cfg(test)]
mod tests;
