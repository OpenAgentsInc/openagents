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
//! These pages are local only (the site guard answers them on the local
//! address, like `/app`), and every post must come from this site.
//! Repositories come from the signed-in person's own GitHub access when
//! they connected GitHub on `/projects` ([`crate::projects`]), else from
//! the studio's GitHub token (`Studio::github`), else from a pasted public
//! repository address.

mod view;

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Form;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Redirect, Response, Sse};
use axum::routing::{get, post};
use coder_environment_operator::studio::Studio;
use coder_environment_operator::studio::github::GitHub;
use maud::html;
use serde::Deserialize;

use crate::App;
use crate::ui_page::UiPage;

static SHOWN: AtomicBool = AtomicBool::new(false);

/// Whether the left panel offers Environments (a studio is configured).
pub(crate) fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

pub(crate) fn routes(app: &App) -> Router<App> {
    SHOWN.store(app.config.environments.is_some(), Ordering::Relaxed);
    Router::new()
        .route("/environments", get(index).post(create))
        .route("/environments/new", get(new))
        .route("/environments/{id}", get(show))
        .route("/environments/{id}/events", get(events))
        .route("/environments/{id}/message", post(message))
        .route("/environments/{id}/retry", post(retry))
        .route("/environments/{id}/save", post(save))
        .route("/environments/{id}/claude", post(claude))
        .route("/environments/{id}/runs/{run}", get(run))
        .route("/environments/{id}/runs/{run}/events", get(run_events))
        .route("/environments/{id}/runs/{run}/stop", post(stop))
        .layer(DefaultBodyLimit::max(64 * 1024))
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

fn studio(app: &App) -> Option<&Arc<Studio>> {
    app.config.environments.as_ref()
}

/// The GitHub client for this request: the signed-in person's own access
/// when they connected GitHub, else the studio's. The person's token is
/// used for this request only.
async fn github(app: &App, studio: &Studio, headers: &HeaderMap) -> GitHub {
    if let Some(service) = app.config.cloud.as_deref()
        && let Ok(token) = service.github_token(headers).await
    {
        return GitHub::new(Some(token));
    }
    studio.github().clone()
}

fn unavailable(headers: &HeaderMap) -> Response {
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
        return unavailable(&headers);
    };
    let rows = studio.list();
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
        return unavailable(&headers);
    };
    let chosen = if q.repo.trim().is_empty() {
        q.pick.trim()
    } else {
        q.repo.trim()
    };
    let github = github(&app, studio, &headers).await;
    if chosen.is_empty() {
        return pick_page(&github, &headers, "", None).await;
    }
    let Some(name) = coder_environment_operator::studio::github::RepoName::parse(chosen) else {
        return pick_page(
            &github,
            &headers,
            chosen,
            Some("Enter a GitHub repository as owner/name or its github.com address."),
        )
        .await;
    };
    let repo = match github.repository(&name).await {
        Ok(r) => r,
        Err(e) => return pick_page(&github, &headers, chosen, Some(e.as_str())).await,
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

async fn pick_page(
    github: &GitHub,
    headers: &HeaderMap,
    repo: &str,
    error: Option<&str>,
) -> Response {
    let mine = if github.signed_in() {
        github.repositories().await.ok()
    } else {
        None
    };
    let content = view::pick(&view::Pick {
        mine: mine.as_deref(),
        repo,
        error,
    });
    protect(
        UiPage::new("New environment")
            .path("/environments/new")
            .section("/environments")
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
        return unavailable(&headers);
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
    match resolved {
        Ok(resolved) => match studio.create(&resolved) {
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
    let Some(v) = studio.view(id) else {
        return missing(headers);
    };
    let rows = studio.list();
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
        return unavailable(&headers);
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
    if !valid_id(&id) || studio.view(&id).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let ready = claude_offer(&app, &studio, &headers).await;
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
        Sse::new(stream)
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
        return unavailable(&headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if !valid_id(&id) {
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
        return unavailable(&headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if !valid_id(&id) {
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
        return unavailable(&headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if !valid_id(&id) {
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
        return unavailable(&headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if !valid_id(&id) {
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
        return unavailable(&headers);
    };
    if !valid_id(&id) || !valid_id(&run) {
        return missing(&headers);
    }
    let (Some(v), Some(r)) = (studio.view(&id), studio.claude_run(&id, &run)) else {
        return missing(&headers);
    };
    let stream = format!("/environments/{id}/runs/{run}/events");
    let rows = studio.list();
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
async fn run_events(State(app): State<App>, Path((id, run)): Path<(String, String)>) -> Response {
    let Some(studio) = studio(&app).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !valid_id(&id) || !valid_id(&run) || studio.claude_run(&id, &run).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
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
        Sse::new(stream)
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
        return unavailable(&headers);
    };
    if !same_site(&headers) {
        return refused();
    }
    if !valid_id(&id) || !valid_id(&run) {
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
