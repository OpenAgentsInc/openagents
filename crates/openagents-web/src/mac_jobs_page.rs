//! `/settings/mac-jobs` (#11223): the account's linked Macs and the jobs
//! sent to them, with each job's log, its result, its files, and, when a
//! job waits for the owner (a TestFlight upload), Approve and Deny.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /settings/mac-jobs` | The Macs and the jobs, newest first |
//! | `GET /settings/mac-jobs/{id}` | One job: its question, result, files, and log |
//! | `POST /settings/mac-jobs/{id}/answer` `{csrf, question, decision}` | Approve or deny the job's question |
//! | `POST /settings/mac-jobs/{id}/stop` `{csrf}` | Stop the job |
//! | `GET /settings/mac-jobs/{id}/files/{file}` | A file the job made |
//!
//! The pages read themselves again every few seconds while a job is going.
//! Only the signed-in account's own jobs are shown.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::PageColumn;
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::chat_store::{Store, account_owner, now_unix};
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, Viewer};
use crate::mac_jobs::{self, Answered, Job, JobState};
use crate::settings::viewer;
use crate::ui_page::UiPage;

pub(crate) const PAGE: &str = "/settings/mac-jobs";
/// How often a page with a job going reads itself again, in seconds.
const REFRESH_SECONDS: u32 = 5;
/// The most log lines a job's page shows (the newest).
const SHOWN_LINES: usize = 400;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(list_page))
        .route("/settings/mac-jobs/{id}", get(job_page))
        .route("/settings/mac-jobs/{id}/answer", post(answer_route))
        .route("/settings/mac-jobs/{id}/stop", post(stop_route))
        .route("/settings/mac-jobs/{id}/files/{file}", get(file_route))
}

/// A job's state as the owner reads it.
pub(crate) fn state_words(state: JobState) -> &'static str {
    match state {
        JobState::Waiting => "Waiting for the Mac",
        JobState::Running => "Running",
        JobState::Asking => "Waiting for your approval",
        JobState::Done => "Done",
        JobState::Failed => "Failed",
        JobState::Cancelled => "Stopped",
    }
}

fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        0..=1_023 => format!("{bytes} B"),
        1_024..=1_048_575 => format!("{:.0} KB", bytes as f64 / 1_024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// The page with the account menu, reading itself again while
/// `refresh`.
fn page(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    title: &str,
    path: &str,
    refresh: bool,
    body: Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    let mut page = UiPage::new(title)
        .path(path)
        .section(crate::settings::PAGE)
        .account(account)
        .content(PageColumn::new(body));
    if refresh {
        page = page.head(html! {
            meta http-equiv="refresh" content=(REFRESH_SECONDS.to_string());
        });
    }
    protect(page.respond(headers))
}

/// The Macs section of a page.
fn macs_markup(found: &[(String, mac_jobs::Reported, bool)], now: u64) -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby="mac-jobs-macs" {
            h2 #mac-jobs-macs { "Your Macs" }
            @if found.is_empty() {
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { "No Mac is linked yet" }
                        span class="oa-settings-hint" {
                            "On your Mac, sign in to Coder, then run: openagents mac serve"
                        }
                    }
                }
            }
            @for (name, reported, online) in found {
                @let caps = &reported.capabilities;
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" {
                            (name) " · "
                            @if *online { @if caps.busy { "Running a job" } @else { "Online" } }
                            @else { "Last seen " (ago(reported.reported_unix, now)) }
                        }
                        span class="oa-settings-hint" {
                            "macOS " (caps.macos.as_deref().unwrap_or("unknown"))
                            " · Xcode " (caps.xcode.as_deref().unwrap_or("not installed"))
                            " · " (caps.signing_identities.len()) " signing identities"
                            " · " (caps.simulators.len()) " simulators"
                            " · App Store Connect key "
                            @if caps.asc_key { "on this Mac" } @else { "not set up" }
                            @if let Some(free) = caps.free_disk_gb { " · " (free) " GB free" }
                        }
                    }
                }
            }
        }
    }
}

fn job_row(job: &Job, now: u64) -> Markup {
    let href = format!("{PAGE}/{}", job.id);
    html! {
        div class="oa-settings-row" {
            div class="oa-settings-text" {
                span class="oa-settings-label" { a href=(href) { (job.spec.title()) } }
                span class="oa-settings-hint" {
                    (state_words(job.state)) " · " (job.computer) " · "
                    (ago(job.created_unix, now))
                    @if let Some(line) = job.last_line() { " · " (line) }
                }
            }
        }
    }
}

async fn list_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let store = &app.config.chat_store;
    let found = mac_jobs::macs(store, &owner).await.unwrap_or_default();
    let jobs = mac_jobs::list(store, &owner).await.unwrap_or_default();
    let now = now_unix();
    let going = jobs.iter().any(|job| !job.state.finished());
    let body = html! {
        div class="oa-settings" {
            p {
                "Steps that need a Mac, such as iOS builds, the release gate, and TestFlight "
                "uploads, run on your own Mac. Signing and App Store keys stay on it."
            }
            (macs_markup(&found, now))
            section class="oa-settings-group" aria-labelledby="mac-jobs-jobs" {
                h2 #mac-jobs-jobs { "Jobs" }
                @if jobs.is_empty() {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "No jobs yet" }
                            span class="oa-settings-hint" {
                                "Send one with: openagents mac run ios-release-gate --ref main"
                            }
                        }
                    }
                }
                @for job in &jobs { (job_row(job, now)) }
            }
        }
    };
    page(&headers, service, &viewer, "Mac jobs", PAGE, going, body)
}

/// The question card: what the job will do, then Approve and Deny.
fn question_markup(app: &App, owner: &str, job: &Job) -> Markup {
    let Some(question) = job
        .question
        .as_ref()
        .filter(|_| job.state == JobState::Asking && job.approval.is_none())
    else {
        return html! {};
    };
    let action = format!("{PAGE}/{}/answer", job.id);
    html! {
        div class="oa-thread-notice" role="alert" {
            p { strong { "This job is waiting for your approval" } }
            @for line in question.text.lines().filter(|line| !line.trim().is_empty()) {
                p { (line) }
            }
            p { "This answers this one job only. Your Mac records where you answered." }
            form method="post" action=(action) {
                input type="hidden" name="csrf" value=(crate::pages::chat::csrf(app, owner));
                input type="hidden" name="question" value=(question.id);
                (Button::new("Approve").kind(ButtonType::Submit).name("decision").value("approve"))
                " "
                (Button::new("Deny")
                    .kind(ButtonType::Submit)
                    .name("decision")
                    .value("deny")
                    .color(Color::Secondary))
            }
        }
    }
}

async fn job_page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let path = format!("{PAGE}/{id}");
    let (service, viewer) = match viewer(&app, &headers, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let job = match mac_jobs::load(&app.config.chat_store, &owner, &id).await {
        Ok(Some(job)) => job,
        _ => return protect(Redirect::to(PAGE).into_response()),
    };
    let now = now_unix();
    let shown = job.lines.len().saturating_sub(SHOWN_LINES);
    let stop = format!("{PAGE}/{}/stop", job.id);
    let body = html! {
        div class="oa-settings" {
            p { a href=(PAGE) { "All Mac jobs" } }
            (question_markup(&app, &owner, &job))
            section class="oa-settings-group" aria-labelledby="mac-job" {
                h2 #mac-job { (job.spec.title()) }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (state_words(job.state)) }
                        span class="oa-settings-hint" {
                            "On " (job.computer) " · " (job.spec.repo) " at " (job.spec.git_ref)
                            @if let Some(commit) = &job.commit { " (" (commit.chars().take(12).collect::<String>()) ")" }
                            " · sent " (ago(job.created_unix, now))
                        }
                        @if let Some(summary) = &job.summary { span class="oa-settings-hint" { (summary) } }
                        @if let Some(why) = &job.why { span class="oa-settings-hint" { (why) } }
                        @if let Some(approval) = &job.approval {
                            span class="oa-settings-hint" {
                                @if approval.decision == "approved" { "Approved" } @else { "Denied" }
                                " on the " (approval.via)
                            }
                        }
                    }
                    @if !job.state.finished() && !job.cancel {
                        div class="oa-settings-control" {
                            form method="post" action=(stop) {
                                input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app, &owner));
                                (Button::new("Stop")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
            @let files: Vec<_> = job.artifacts.iter().filter(|a| a.done).collect();
            @if !files.is_empty() {
                section class="oa-settings-group" aria-labelledby="mac-job-files" {
                    h2 #mac-job-files { "Files" }
                    @for file in files {
                        div class="oa-settings-row" {
                            div class="oa-settings-text" {
                                span class="oa-settings-label" {
                                    a href=(format!("{PAGE}/{}/files/{}", job.id, file.name)) { (file.name) }
                                }
                                span class="oa-settings-hint" { (size(file.size)) }
                            }
                        }
                    }
                }
            }
            section class="oa-settings-group" aria-labelledby="mac-job-log" {
                h2 #mac-job-log { "Log" }
                @if job.lines.is_empty() {
                    p { "Nothing yet." }
                } @else {
                    pre class="oa-mac-job-log" style="white-space: pre-wrap; overflow-x: auto; font-size: 0.8rem" {
                        @for line in &job.lines[shown..] { (line) "\n" }
                    }
                }
            }
        }
    };
    page(
        &headers,
        service,
        &viewer,
        "Mac job",
        &path,
        !job.state.finished(),
        body,
    )
}

/// Whether `supplied` is this owner's form token, compared in constant time.
fn token_fits(app: &App, owner: &str, supplied: &str) -> bool {
    let expected = crate::pages::chat::csrf(app, owner);
    expected.len() == supplied.len()
        && expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(
        crate::layout::problem(status, "Mac jobs", text, (PAGE, "Back to Mac jobs"))
            .into_response(),
    )
}

#[derive(Deserialize)]
struct AnswerForm {
    csrf: String,
    question: String,
    decision: String,
}

async fn answer_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<AnswerForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem(StatusCode::BAD_REQUEST, "Choose Approve or Deny.");
    };
    let path = format!("{PAGE}/{id}");
    let (_, viewer) = match viewer(&app, &headers, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    if !token_fits(&app, &owner, &form.csrf) {
        return problem(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload the page.",
        );
    }
    let approve = match form.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return problem(StatusCode::BAD_REQUEST, "Choose Approve or Deny."),
    };
    match mac_jobs::answer_question(
        &app.config.chat_store,
        &owner,
        &id,
        &form.question,
        approve,
        "web",
    )
    .await
    {
        Ok(Answered::Recorded) => protect(Redirect::to(&path).into_response()),
        Ok(Answered::NotAsking | Answered::Unknown) => problem(
            StatusCode::CONFLICT,
            "That job isn't waiting for an answer anymore.",
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Your answer couldn't be saved. Try again in a minute.",
        ),
    }
}

#[derive(Deserialize)]
struct StopForm {
    csrf: String,
}

async fn stop_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<StopForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem(StatusCode::BAD_REQUEST, "Reload the page and try again.");
    };
    let path = format!("{PAGE}/{id}");
    let (_, viewer) = match viewer(&app, &headers, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    if !token_fits(&app, &owner, &form.csrf) {
        return problem(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload the page.",
        );
    }
    match mac_jobs::cancel(&app.config.chat_store, &owner, &id).await {
        Ok(_) => protect(Redirect::to(&path).into_response()),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "The job couldn't be stopped. Try again in a minute.",
        ),
    }
}

async fn file_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    let (_, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    match mac_jobs::artifact_body(&app.config.chat_store, &owner, &id, &file).await {
        Ok(Some((size, body))) => protect(mac_jobs::download(&file, size, body)),
        _ => problem(StatusCode::NOT_FOUND, "That file isn't there."),
    }
}

/// The Settings row that leads here, once the account has a Mac or a job.
pub(crate) async fn settings_row(store: &Store, owner: &str) -> Markup {
    let macs = mac_jobs::macs(store, owner).await.unwrap_or_default();
    if macs.is_empty() {
        return html! {};
    }
    let online = macs.iter().filter(|(_, _, online)| *online).count();
    html! {
        div class="oa-settings" {
            section class="oa-settings-group" aria-labelledby="settings-mac-jobs" {
                h2 #settings-mac-jobs { "Mac jobs" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { a href=(PAGE) { "Your Macs and their jobs" } }
                        span class="oa-settings-hint" {
                            (macs.len()) " linked, " (online) " online"
                        }
                    }
                }
            }
        }
    }
}
