//! `/environments`: set up a repository's environment by watching an agent
//! do it, save the checked result, and run Claude Code on it.
//!
//! The pages read and drive a [`Studio`] (`--environments PRIVATE_JSON`,
//! `coder_environment_operator::studio`): the setup agent's conversation
//! is its activity log, streamed here over SSE and rendered with the same
//! `openagents-ui` activity components as `/demo`. Without the flag, the
//! pages say environments aren't set up on this server and the left panel
//! shows no Environments entry.
//!
//! On the local address these pages are for anyone the local site lets
//! in; on a public host, only for a signed-in person allowed agent work
//! (a site admin, [`crate::agent_work`]), and every post must come from
//! this site. Each environment belongs to the account that made it: a
//! person reaches only their own ([`crate::agent_work::Scope`]).
//!
//! On a server that signs people in, every page here is for a signed-in
//! person only ([`signed_in_only`], the same sign-in check the header's
//! account reads, so the page and the header always agree), and
//! repositories come from that person's own GitHub access (connected on
//! `/projects`, [`crate::projects`]), or from a pasted public repository
//! address. Only a server with no sign-in at all uses the studio's own
//! GitHub token (`Studio::github`).
//!
//! `/environments/new` answers at once; the person's repositories load
//! after it ([`repositories`]), a page of 30 at a time, most recently
//! pushed first, from the account service's per-account cache.

mod view;

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Form;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Redirect, Response, Sse};
use axum::routing::{get, post};
use coder_environment_operator::studio::Studio;
use coder_environment_operator::studio::github::GitHub;
use maud::html;
use serde::Deserialize;

use crate::App;
use crate::agent_work::Scope;
use crate::cloud::session::SessionError;
use crate::cloud::session::github::RepoCallError;
use crate::ui_page::UiPage;

static SHOWN: AtomicBool = AtomicBool::new(false);

/// Whether the left panel offers Environments (a studio is configured).
pub(crate) fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// Offer Environments in the left panel: the studio opened after start.
pub(crate) fn mark_shown() {
    SHOWN.store(true, Ordering::Relaxed);
}

pub(crate) fn routes(app: &App) -> Router<App> {
    SHOWN.store(
        app.config.environments.is_some() || app.config.environments.configured(),
        Ordering::Relaxed,
    );
    Router::new()
        .route("/environments", get(index).post(create))
        .route("/environments/new", get(new))
        .route(REPOS, get(repositories))
        .route("/environments/{id}", get(show))
        .route("/environments/{id}/events", get(events))
        .route("/environments/{id}/message", post(message))
        .route("/environments/{id}/retry", post(retry))
        .route("/environments/{id}/save", post(save))
        .route("/environments/{id}/claude", post(claude))
        .route("/environments/{id}/runs/{run}", get(run))
        .route("/environments/{id}/runs/{run}/events", get(run_events))
        .route("/environments/{id}/runs/{run}/stop", post(stop))
        .route_layer(axum::middleware::from_fn_with_state(
            app.clone(),
            signed_in_only,
        ))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

/// Where one page of the person's repositories loads from.
pub(crate) const REPOS: &str = "/environments/repositories";

/// Whether this server signs people in (then these pages need a
/// signed-in person, and the studio's own GitHub token is never used).
fn sign_in(app: &App) -> bool {
    crate::account::sign_in_available(app)
}

/// On a server that signs people in, only a signed-in person reaches these
/// pages: a page asks to log in first and comes back here; a post goes to
/// the log in page; a fragment or stream answers 401 (HTMX follows its
/// `HX-Redirect`). The check is the request's shared sign-in
/// ([`crate::cloud::session::shared`]), the one the header shows.
async fn signed_in_only(State(app): State<App>, request: Request, next: Next) -> Response {
    if !sign_in(&app) {
        return next.run(request).await;
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return next.run(request).await;
    };
    match service.authenticate(request.headers()).await {
        Ok(_) => next.run(request).await,
        Err(SessionError::Unavailable) => crate::cloud::refused(SessionError::Unavailable),
        Err(_) => sign_in_first(&request),
    }
}

pub(crate) fn sign_in_first(request: &Request) -> Response {
    let headers = request.headers();
    let page = request
        .uri()
        .path_and_query()
        .map_or("/environments", |p| p.as_str());
    let streams = request.uri().path().ends_with("/events");
    if request.method() == axum::http::Method::GET && !htmx(headers) && !streams {
        return protect(Redirect::to(&crate::auth::login_href(page, false)).into_response());
    }
    let login = crate::auth::login_href("/environments", false);
    if htmx(headers) {
        let mut response = StatusCode::UNAUTHORIZED.into_response();
        if let Ok(value) = HeaderValue::from_str(&login) {
            response.headers_mut().insert("hx-redirect", value);
        }
        return protect(response);
    }
    if streams {
        return protect(StatusCode::UNAUTHORIZED.into_response());
    }
    protect(Redirect::to(&login).into_response())
}

fn protect(response: Response) -> Response {
    crate::chat_html::protect(response)
}

fn head() -> maud::Markup {
    html! {
        meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"historyRestoreAsHxRequest":false,"refreshOnHistoryMiss":true,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
        script src="/static/htmx.min.js" defer {}
        script src="/static/htmx-sse.js" defer {}
    }
}

/// The studio, while its machine backend answers its health probe.
fn studio(app: &App) -> Option<&Arc<Studio>> {
    if !app.config.environments.backend_up() {
        return None;
    }
    app.config.environments.studio()
}

/// Whose environments this request reaches ([`crate::agent_work::scope`]).
/// The site's gate and [`signed_in_only`] already turned away everyone
/// else, so `None` is answered as not found.
async fn mine(app: &App, headers: &HeaderMap) -> Option<Scope> {
    crate::agent_work::scope(app, headers).await
}

/// The request's scope when environment `id` is its own.
async fn owner_of(app: &App, studio: &Studio, headers: &HeaderMap, id: &str) -> Option<Scope> {
    let scope = mine(app, headers).await?;
    (valid_id(id) && scope.has(studio, id)).then_some(scope)
}

/// The GitHub client for this request. On a server that signs people in:
/// the signed-in person's own access when they connected GitHub, else no
/// token (public repositories only). On a server without sign-in: the
/// studio's. The person's token is used for this request only.
async fn github(app: &App, studio: &Studio, headers: &HeaderMap) -> GitHub {
    if !sign_in(app) {
        return studio.github().clone();
    }
    match app.config.cloud.as_deref() {
        Some(service) => GitHub::new(service.github_token(headers).await.ok()),
        None => GitHub::new(None),
    }
}

fn unavailable(app: &App, headers: &HeaderMap) -> Response {
    if app.config.environments.configured() {
        // Configured, but the machines can't be reached right now: the
        // studio retries and the probe watches, so this clears by itself
        // (#11256).
        return protect(crate::ui_page::problem(
            headers,
            StatusCode::SERVICE_UNAVAILABLE,
            "Environments",
            "Environments are temporarily unavailable. They come back on their own; try again in a minute.",
            ("/", "Home"),
        ));
    }
    protect(crate::ui_page::problem(
        headers,
        StatusCode::NOT_FOUND,
        "Environments",
        "Environments aren't set up on this server.",
        ("/", "Home"),
    ))
}

fn missing(headers: &HeaderMap) -> Response {
    protect(crate::ui_page::problem(
        headers,
        StatusCode::NOT_FOUND,
        "Environment not found",
        "There's no environment at this address.",
        ("/environments", "Environments"),
    ))
}

/// A post must come from a page of this site.
fn same_site(headers: &HeaderMap) -> bool {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    match text("sec-fetch-site") {
        Some(site) => site == "same-origin",
        None => match (text("origin"), text("host")) {
            (Some(origin), Some(host)) => {
                origin
                    .strip_prefix("http://")
                    .or_else(|| origin.strip_prefix("https://"))
                    == Some(host)
            }
            (None, _) => true,
            _ => false,
        },
    }
}

fn refused() -> Response {
    protect((StatusCode::FORBIDDEN, "Use this site's own pages.").into_response())
}

fn htmx(headers: &HeaderMap) -> bool {
    headers.get("hx-request").is_some_and(|h| h == "true")
}

fn valid_id(id: &str) -> bool {
    coder_environment::valid_id(id)
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    let Some(scope) = mine(&app, &headers).await else {
        return missing(&headers);
    };
    let rows = scope.rows(studio);
    protect(
        UiPage::new("Environments")
            .path("/environments")
            .section("/environments")
            .content(view::index(&rows))
            .respond(&headers),
    )
}

#[derive(Deserialize, Default)]
struct NewQuery {
    #[serde(default)]
    repo: String,
    #[serde(default)]
    pick: String,
}

async fn new(State(app): State<App>, headers: HeaderMap, Query(q): Query<NewQuery>) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    let chosen = if q.repo.trim().is_empty() {
        q.pick.trim()
    } else {
        q.repo.trim()
    };
    let list = sign_in(&app) || studio.github().signed_in();
    if chosen.is_empty() {
        return pick_page(list, &headers, "", None);
    }
    let Some(name) = coder_environment_operator::studio::github::RepoName::parse(chosen) else {
        return pick_page(
            list,
            &headers,
            chosen,
            Some("Enter a GitHub repository as owner/name or its github.com address."),
        );
    };
    let github = github(&app, studio, &headers).await;
    let repo = match github.repository(&name).await {
        Ok(r) => r,
        Err(e) => return pick_page(list, &headers, chosen, Some(e.as_str())),
    };
    let branches = github.branches(&name).await.unwrap_or_default();
    branch_page(
        &headers,
        &repo.full_name,
        &branches,
        &repo.default_branch,
        None,
    )
}

/// The repository step, answered at once: `list` loads the person's
/// repositories after the page shows ([`repositories`]).
fn pick_page(list: bool, headers: &HeaderMap, repo: &str, error: Option<&str>) -> Response {
    let content = view::pick(&view::Pick { list, repo, error });
    protect(
        UiPage::new("New environment")
            .path("/environments/new")
            .section("/environments")
            .head(view::htmx_head())
            .breadcrumb(view::breadcrumb("New environment"))
            .content(content)
            .status(if error.is_some() {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            })
            .respond(headers),
    )
}

#[derive(Deserialize, Default)]
struct ReposQuery {
    #[serde(default)]
    page: Option<u32>,
}

/// One page of the person's repositories to choose from, and a Show more
/// button for the next: from the account service on a server that signs
/// people in (30 a page, cached per account), else from the studio's
/// token (one read, up to 300).
async fn repositories(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ReposQuery>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let page = query.page.unwrap_or(1).clamp(1, oa_auth::repos::MAX_PAGE);
    let body = if sign_in(&app) {
        let Some(service) = app.config.cloud.as_deref() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        match service.github_repositories(&headers, page).await {
            Ok(listing) => view::repos(
                &listing
                    .repositories
                    .iter()
                    .map(|r| view::RepoRow {
                        full_name: &r.full_name,
                        private: r.private,
                    })
                    .collect::<Vec<_>>(),
                page,
                listing.more,
            ),
            Err(RepoCallError::Repo(
                oa_auth::repos::RepoError::NotConnected | oa_auth::repos::RepoError::OtherGithub,
            )) => view::repos_unconnected(false),
            Err(RepoCallError::Repo(oa_auth::repos::RepoError::Reconnect)) => {
                view::repos_unconnected(true)
            }
            Err(error) => view::repos_failed(&error.to_string(), page),
        }
    } else {
        match studio.github().repositories().await {
            Ok(found) if page == 1 => view::repos(
                &found
                    .iter()
                    .map(|r| view::RepoRow {
                        full_name: &r.full_name,
                        private: r.private,
                    })
                    .collect::<Vec<_>>(),
                1,
                false,
            ),
            Ok(_) => html! {},
            Err(error) => view::repos_failed(&error, page),
        }
    };
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    protect(response)
}

fn branch_page(
    headers: &HeaderMap,
    repo: &str,
    branches: &[String],
    default: &str,
    error: Option<&str>,
) -> Response {
    protect(
        UiPage::new("New environment")
            .path("/environments/new")
            .section("/environments")
            .breadcrumb(view::breadcrumb("New environment"))
            .content(view::branch(repo, branches, default, error))
            .status(if error.is_some() {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            })
            .respond(headers),
    )
}

#[derive(Deserialize)]
struct CreateForm {
    repo: String,
    #[serde(default)]
    branch: String,
}

async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<CreateForm>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    let branch = form.branch.trim();
    let github = github(&app, studio, &headers).await;
    let resolved = match coder_environment_operator::studio::github::RepoName::parse(&form.repo) {
        Some(name) => {
            github
                .resolve(&name, (!branch.is_empty()).then_some(branch))
                .await
        }
        None => Err("Enter a GitHub repository as owner/name or its github.com address.".into()),
    };
    let Some(scope) = mine(&app, &headers).await else {
        return missing(&headers);
    };
    match resolved {
        Ok(resolved) => match studio.create_for(&resolved, scope.account.as_deref()) {
            Ok(id) => protect(Redirect::to(&format!("/environments/{id}")).into_response()),
            Err(e) => branch_page(
                &headers,
                &resolved.repository.full(),
                &[],
                &resolved.branch,
                Some(e.as_str()),
            ),
        },
        Err(e) => branch_page(&headers, form.repo.trim(), &[], branch, Some(e.as_str())),
    }
}

/// Whether Claude Code can run for this request: the signed-in person
/// saved their own Claude key in Settings, or this server names one.
pub(crate) async fn claude_ready(app: &App, studio: &Studio, headers: &HeaderMap) -> bool {
    claude_offer(app, studio, headers).await == view::Claude::Ready
}

/// What the Claude Code card offers this request.
async fn claude_offer(app: &App, studio: &Studio, headers: &HeaderMap) -> view::Claude {
    if crate::cloud::byo::saved(app, headers).await || studio.claude_ready() {
        view::Claude::Ready
    } else if app.config.cloud_byo.is_some() {
        view::Claude::AddKey
    } else {
        view::Claude::Unavailable
    }
}

/// The environment's page, with an optional notice under the transcript.
async fn page(
    app: &App,
    studio: &Studio,
    headers: &HeaderMap,
    id: &str,
    notice: Option<&str>,
) -> Response {
    let Some(scope) = owner_of(app, studio, headers, id).await else {
        return missing(headers);
    };
    let Some(v) = studio.view(id) else {
        return missing(headers);
    };
    let rows = scope.rows(studio);
    let stream = format!("/environments/{id}/events?after={}", v.records.len());
    let ready = claude_offer(app, studio, headers).await;
    let body = view::transcript(&v, ready, notice);
    protect(
        UiPage::new(v.summary.repository.clone())
            .path(format!("/environments/{id}"))
            .section("/environments")
            .app()
            .head(head())
            .breadcrumb(view::breadcrumb(&v.summary.repository))
            .sidebar_section(view::sidebar(&rows, Some(id)))
            .content(html! {
                div class="oa-thread-view" { (view::thread(&stream, body)) }
            })
            .composer(view::dock(id, None, false))
            .respond(headers),
    )
}

async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !valid_id(&id) {
        return missing(&headers);
    }
    page(&app, studio, &headers, &id, None).await
}

#[derive(Deserialize, Default)]
struct After {
    #[serde(default)]
    after: usize,
}

/// The transcript, again whenever the conversation grows.
async fn events(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(after): Query<After>,
) -> Response {
    let Some(studio) = studio(&app).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if owner_of(&app, &studio, &headers, &id).await.is_none() || studio.view(&id).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let ready = claude_offer(&app, &studio, &headers).await;
    let shutdown = app.config.shutdown.clone();
    let stream = futures_util::stream::unfold(
        (studio, id, after.after, 0u16),
        move |(studio, id, mut cursor, ticks)| async move {
            if ticks >= 600 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let revision = studio.revision(&id);
            let event = if revision != cursor {
                cursor = revision;
                match studio.view(&id) {
                    Some(v) => Event::default()
                        .id(format!("{id}:{revision}"))
                        .event("transcript")
                        .data(view::transcript(&v, ready, None).into_string()),
                    None => Event::default().comment("gone"),
                }
            } else {
                Event::default().comment("current")
            };
            Some((Ok::<_, Infallible>(event), (studio, id, cursor, ticks + 1)))
        },
    );
    protect(
        Sse::new(shutdown.until(stream))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

#[derive(Deserialize)]
struct MessageForm {
    q: String,
}

async fn message(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<MessageForm>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if owner_of(&app, studio, &headers, &id).await.is_none() {
        return missing(&headers);
    }
    let result = studio.steer(&id, &form.q);
    if htmx(&headers) {
        let status = result.err();
        return protect(view::dock(&id, status.as_deref(), true).into_response());
    }
    match result {
        Ok(()) => protect(Redirect::to(&format!("/environments/{id}")).into_response()),
        Err(e) => page(&app, studio, &headers, &id, Some(e.as_str())).await,
    }
}

async fn retry(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if owner_of(&app, studio, &headers, &id).await.is_none() {
        return missing(&headers);
    }
    match studio.retry(&id) {
        Ok(()) => protect(Redirect::to(&format!("/environments/{id}")).into_response()),
        Err(e) => page(&app, studio, &headers, &id, Some(e.as_str())).await,
    }
}

#[derive(Deserialize)]
struct SaveForm {
    candidate: String,
}

async fn save(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<SaveForm>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if owner_of(&app, studio, &headers, &id).await.is_none() {
        return missing(&headers);
    }
    match studio.save(&id, &form.candidate).await {
        Ok(_) => protect(Redirect::to(&format!("/environments/{id}")).into_response()),
        Err(e) => page(&app, studio, &headers, &id, Some(e.as_str())).await,
    }
}

#[derive(Deserialize)]
struct ClaudeForm {
    prompt: String,
}

async fn claude(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<ClaudeForm>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if owner_of(&app, studio, &headers, &id).await.is_none() {
        return missing(&headers);
    }
    let own = crate::cloud::byo::run_key(&app, &headers).await;
    match studio.run_claude(&id, &form.prompt, own) {
        Ok(run) => protect(Redirect::to(&format!("/environments/{id}/runs/{run}")).into_response()),
        Err(e) => page(&app, studio, &headers, &id, Some(e.as_str())).await,
    }
}

async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    let Some(scope) = owner_of(&app, studio, &headers, &id).await else {
        return missing(&headers);
    };
    if !valid_id(&run) {
        return missing(&headers);
    }
    let (Some(v), Some(r)) = (studio.view(&id), studio.claude_run(&id, &run)) else {
        return missing(&headers);
    };
    let stream = format!("/environments/{id}/runs/{run}/events");
    let rows = scope.rows(studio);
    protect(
        UiPage::new("Claude Code")
            .path(format!("/environments/{id}/runs/{run}"))
            .section("/environments")
            .app()
            .head(head())
            .breadcrumb(
                openagents_ui::shell::Breadcrumb::new("Claude Code")
                    .crumb("Environments", "/environments")
                    .crumb(v.summary.repository.clone(), format!("/environments/{id}")),
            )
            .sidebar_section(view::sidebar(&rows, Some(&id)))
            .content(html! {
                div class="oa-thread-view" { (view::thread(&stream, view::run_transcript(&r))) }
            })
            .respond(&headers),
    )
}

/// The run's transcript, again every few seconds while it changes.
async fn run_events(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Response {
    let Some(studio) = studio(&app).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if owner_of(&app, &studio, &headers, &id).await.is_none()
        || !valid_id(&run)
        || studio.claude_run(&id, &run).is_none()
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let shutdown = app.config.shutdown.clone();
    let stream = futures_util::stream::unfold(
        (studio, id, run, String::new(), 0u16),
        |(studio, id, run, mut last, ticks)| async move {
            if ticks >= 600 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            let event = match studio.claude_run(&id, &run) {
                Some(r) => {
                    let body = view::run_transcript(&r).into_string();
                    if body != last {
                        last = body.clone();
                        Event::default().event("transcript").data(body)
                    } else {
                        Event::default().comment("current")
                    }
                }
                None => Event::default().comment("gone"),
            };
            Some((
                Ok::<_, Infallible>(event),
                (studio, id, run, last, ticks + 1),
            ))
        },
    );
    protect(
        Sse::new(shutdown.until(stream))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

async fn stop(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return unavailable(&app, &headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if owner_of(&app, studio, &headers, &id).await.is_none() || !valid_id(&run) {
        return missing(&headers);
    }
    let _ = studio.stop_claude(&id, &run);
    let mut response = Redirect::to(&format!("/environments/{id}/runs/{run}")).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    protect(response)
}

#[cfg(test)]
mod tests;
