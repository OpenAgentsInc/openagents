//! "Connect your terminal" (#11089): Settings' guided page that takes a
//! person from nothing to their terminal's chats in the sidebar.
//!
//! 1. **Install Coder**: the install command for their system (from the
//!    user agent) first, with a copy button.
//! 2. **Sign in from your terminal**: `coder login --pair <code>`. The pair
//!    code is the account's own ([`crate::cloud::CloudSession::pair_code`]);
//!    Coder sends it with its sign-in, and this page lists the sign-ins
//!    waiting under it, live, with Approve and Deny right here. A plain
//!    `coder login` still works by typing its code.
//! 3. **Choose where your chats live**: "Sync all my chats" or "Keep chats
//!    on this computer", kept per computer
//!    ([`crate::chat_store::SyncChoice`]). Coder asks the same question in
//!    the terminal; whichever answers first wins and the other shows it.
//! 4. **Your terminal is connected**, with the computer's name and, when it
//!    syncs, how many of its chats are in the sidebar. New chats appear in
//!    the sidebar by themselves (the sidebar's live stream draws rows it
//!    hasn't seen).
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /settings/terminal[?computer=][&problem=]` | The page |
//! | `GET /settings/terminal/live?since=[&computer=]` | Steps 2 to 4's live part, polled by the page every two seconds |
//! | `POST /settings/terminal/approve` `{csrf, code, decision}` | Approve or Deny a waiting sign-in |
//! | `POST /settings/terminal/sync` `{csrf, computer, choice, back?}` | Choose for a computer (also Settings > Computers) |
//!
//! On a local server (`127.0.0.1`, `localhost`) the sign-in command names
//! the server (`OPENAGENTS_ORIGIN=…`) and the local stack's Coder.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, CopyButton};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::chat_store::{SyncChoice, account_owner};
use crate::cloud::protect;
use crate::cloud::session::device::PairedRequest;
use crate::cloud::session::{CloudSession, SessionError, Viewer};
use crate::cloud::{refused, service};
use crate::coder_sync::line;
use crate::pages::download::{CODER_PS1, CODER_SH};
use crate::ui_page::{UiPage, action_link};

pub(crate) const PAGE: &str = "/settings/terminal";
const LIVE: &str = "/settings/terminal/live";
const APPROVE: &str = "/settings/terminal/approve";
pub(crate) const SYNC: &str = "/settings/terminal/sync";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page))
        .route(LIVE, get(live))
        .route(APPROVE, post(approve))
        .route(SYNC, post(choose))
}

/// Where the person stands, from what the page knows.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Waiting for `coder login`; the sign-ins waiting under the pair code.
    SignIn(Vec<Waiting>),
    /// Signed in on `computer`, not chosen yet.
    Choose { computer: String },
    /// Connected and chosen; with sync on, its chats in the sidebar.
    Connected {
        computer: String,
        choice: SyncChoice,
        chats: usize,
    },
}

/// A sign-in waiting under the pair code, with its Approve ticket.
#[derive(Debug, PartialEq, Eq)]
struct Waiting {
    code: String,
    app: String,
    computer: String,
    csrf: String,
}

/// The two commands a system can install with, its own first.
fn install_commands(headers: &HeaderMap) -> [(&'static str, &'static str); 2] {
    let windows = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|agent| agent.contains("Windows"));
    let unix = ("macOS and Linux", CODER_SH);
    let powershell = ("Windows, in PowerShell", CODER_PS1);
    if windows {
        [powershell, unix]
    } else {
        [unix, powershell]
    }
}

/// The local server's address, when the page is served from this
/// computer (`http://127.0.0.1:4301`).
fn local_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(header::HOST)?.to_str().ok()?;
    let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
    (matches!(name, "127.0.0.1" | "localhost")
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':')))
    .then(|| format!("http://{host}"))
}

/// The sign-in command for this server.
fn login_command(pair: &str, local: Option<&str>) -> String {
    match local {
        Some(origin) => format!(
            "OPENAGENTS_ORIGIN={origin} ~/.openagents/full-local/bin/coder login --pair {pair}"
        ),
        None => format!("coder login --pair {pair}"),
    }
}

/// A command with its copy button.
fn command(id: &str, text: &str) -> Markup {
    html! {
        div class="oa-terminal-command" {
            pre id=(id) { code { (text) } }
            (CopyButton::new(text)
                .label("Copy")
                .copied_label("Copied")
                .variant(ButtonVariant::Soft)
                .color(Color::Secondary))
        }
    }
}

/// Steps 1 and 2's commands, and the live part below them.
fn page_body(headers: &HeaderMap, pair: &str, live: Markup) -> Markup {
    let local = local_origin(headers);
    let [first, second] = install_commands(headers);
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Connect your terminal" }
            p { "Use Coder in your terminal and keep its chats with your account." }
            h2 { "1. Install Coder" }
            p { (first.0) ":" }
        }))
        (command("terminal-install", first.1))
        (MarkdownRoot::new(html! {
            p { (second.0) ":" }
        }))
        (command("terminal-install-other", second.1))
        (MarkdownRoot::new(html! {
            @if local.is_some() {
                p.oa-page-meta {
                    "On this local server, " code { "scripts/dev/full-local.sh start" }
                    " already built Coder at " code { "~/.openagents/full-local/bin/coder" } "."
                }
            } @else {
                p.oa-page-meta { "Already installed? Go on to step 2." }
            }
            h2 { "2. Sign in from your terminal" }
            p { "Run this in a terminal:" }
        }))
        (command("terminal-login", &login_command(pair, local.as_deref())))
        (live)
    }
}

/// The live part's address for the page's state.
fn live_href(since: u64, computer: Option<&str>) -> String {
    let mut href = format!("{LIVE}?since={since}");
    if let Some(computer) = computer {
        href.push_str("&computer=");
        href.push_str(
            &url::form_urlencoded::byte_serialize(computer.as_bytes()).collect::<String>(),
        );
    }
    href
}

/// Plain words for a code that couldn't be approved.
fn problem_text(code: &str) -> &'static str {
    match code {
        "expired_token" => "That sign-in expired. Run the command again.",
        "denied" => "Sign-in denied. Run the command again to start over.",
        _ => "That sign-in didn't work. Run the command again.",
    }
}

/// Steps 2 to 4 for `step`, refreshing itself every two seconds while
/// anything can still change.
fn live_markup(step: &Step, since: u64, problem: Option<&str>, choose_csrf: &str) -> Markup {
    let computer = match step {
        Step::SignIn(_) => None,
        Step::Choose { computer } | Step::Connected { computer, .. } => Some(computer.as_str()),
    };
    html! {
        div #terminal-live hx-get=(live_href(since, computer)) hx-trigger="every 2s" hx-swap="outerHTML" {
            @match step {
                Step::SignIn(waiting) => {
                    @if let Some(problem) = problem {
                        (MarkdownRoot::new(html! { p role="alert" { (problem_text(problem)) } }))
                    }
                    @if waiting.is_empty() {
                        (MarkdownRoot::new(html! {
                            p role="status" { "Waiting for your terminal. This page updates by itself." }
                        }))
                        form method="get" action=(crate::device::PAGE) class="oa-terminal-code" {
                            label for="terminal-code" { "Ran plain " code { "coder login" } "? Enter the code it shows:" }
                            (openagents_ui::forms::Input::new("code").id("terminal-code").autocomplete("off").spellcheck(false).placeholder("BCDF-GHJK"))
                            (Button::new("Continue")
                                .kind(ButtonType::Submit)
                                .variant(ButtonVariant::Soft)
                                .color(Color::Secondary))
                        }
                    }
                    @for request in waiting {
                        (MarkdownRoot::new(html! {
                            p { strong { (request.app) " on " (request.computer) } " wants to sign in. Your terminal shows " strong { (request.code) } "." }
                            p.oa-page-meta { "Approve only if the code matches." }
                        }))
                        form method="post" action=(APPROVE) {
                            input type="hidden" name="csrf" value=(request.csrf);
                            input type="hidden" name="code" value=(request.code);
                            div.oa-page-actions {
                                (Button::new("Approve").kind(ButtonType::Submit).name("decision").value("approve"))
                                (Button::new("Deny")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary)
                                    .name("decision")
                                    .value("deny"))
                            }
                        }
                    }
                }
                Step::Choose { computer } => {
                    (MarkdownRoot::new(html! {
                        p role="status" { "Coder on " strong { (computer) } " is signed in." }
                        h2 { "3. Choose where your chats live" }
                        p { "Your terminal asks the same question. Answer in either place." }
                    }))
                    (choice_forms(computer, choose_csrf, None, false))
                }
                Step::Connected { computer, choice, chats } => {
                    (MarkdownRoot::new(html! {
                        h2 { "4. Your terminal is connected" }
                        p role="status" { "Coder on " strong { (computer) } " is signed in to your account." }
                        @match choice {
                            SyncChoice::All => {
                                @if *chats == 0 {
                                    p { "Its chats sync here. Run " code { "coder" } " and start a chat: it shows in the sidebar." }
                                } @else if *chats == 1 {
                                    p { "Its chats sync here. 1 chat from " (computer) " is in your sidebar." }
                                } @else {
                                    p { "Its chats sync here. " (chats) " chats from " (computer) " are in your sidebar." }
                                }
                            }
                            SyncChoice::Local => {
                                p { "Its chats stay on " (computer) ". Nothing is sent." }
                            }
                        }
                    }))
                    (choice_forms(computer, choose_csrf, Some(*choice), false))
                    div.oa-page-actions {
                        (action_link("Settings", crate::settings::PAGE))
                        (action_link("Connect another computer", PAGE))
                    }
                }
            }
        }
    }
}

/// The two choices for `computer` as buttons; the current one (if any)
/// is marked and the other offered. `back_to_settings` returns to
/// Settings after choosing.
pub(crate) fn choice_forms(
    computer: &str,
    csrf: &str,
    current: Option<SyncChoice>,
    back_to_settings: bool,
) -> Markup {
    let option = |choice: SyncChoice, label: &str, hint: &str| {
        let chosen = current == Some(choice);
        html! {
            form method="post" action=(SYNC) class="oa-settings-row" {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="computer" value=(computer);
                input type="hidden" name="choice" value=(choice.as_str());
                @if back_to_settings { input type="hidden" name="back" value="settings"; }
                div class="oa-settings-text" {
                    span class="oa-settings-label" { (label) @if chosen { " (chosen)" } }
                    span class="oa-settings-hint" { (hint) }
                }
                div class="oa-settings-control" {
                    @if !chosen {
                        @let button = Button::new(if current.is_some() { "Switch" } else { "Choose" })
                            .kind(ButtonType::Submit);
                        @if choice == SyncChoice::All && current.is_none() {
                            (button)
                        } @else {
                            (button.variant(ButtonVariant::Soft).color(Color::Secondary))
                        }
                    }
                }
            }
        }
    };
    html! {
        div class="oa-settings" {
            (option(
                SyncChoice::All,
                "Sync all my chats",
                "This computer's chats, earlier ones too, show here on openagents.com.",
            ))
            (option(
                SyncChoice::Local,
                "Keep chats on this computer",
                "Nothing is sent. You can change this later in Settings.",
            ))
        }
    }
}

/// The signed-in viewer, or a trip through sign-in back to `back`.
async fn viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
    back: &str,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = service(app)?;
    match service.authenticate(headers).await {
        Ok(viewer) => Ok((service, viewer)),
        Err(SessionError::Unauthenticated) => Err(protect(
            Redirect::to(&crate::auth::login_href(back, false)).into_response(),
        )),
        Err(error) => Err(refused(error)),
    }
}

#[derive(Default, Deserialize)]
struct PageQuery {
    computer: Option<String>,
    problem: Option<String>,
    since: Option<u64>,
}

/// Where the person stands: the computer the page knows (or one signed
/// in since `since`), its choice, or the sign-ins waiting.
async fn step(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
    computer: Option<String>,
    since: u64,
) -> Result<Step, SessionError> {
    let computer = match computer.map(|c| line(&c, 64)).filter(|c| !c.is_empty()) {
        Some(computer) => Some(computer),
        // Approved somewhere else (the /device page): a computer signed in
        // since the page opened.
        None => service
            .app_sessions(headers)
            .await
            .ok()
            .and_then(|sessions| {
                sessions
                    .into_iter()
                    .filter(|s| s.created_at + 2 >= since)
                    .max_by_key(|s| s.created_at)
                    .map(|s| line(&s.computer, 64))
            })
            .filter(|c| !c.is_empty()),
    };
    let Some(computer) = computer else {
        let pair = service.pair_code(viewer);
        let waiting = service
            .device_paired(headers, &pair)
            .await?
            .into_iter()
            .filter_map(|request: PairedRequest| {
                let code = oa_auth::device::normalize_user_code(&request.user_code)?;
                let csrf = service
                    .csrf(headers, viewer, "terminal-approve", &code)
                    .ok()?;
                Some(Waiting {
                    code: request.user_code,
                    app: request.app,
                    computer: request.computer,
                    csrf,
                })
            })
            .collect();
        return Ok(Step::SignIn(waiting));
    };
    let owner = account_owner(&viewer.account_id);
    let store = &app.config.chat_store;
    let choice = crate::coder_sync::choice(store, &owner, &computer)
        .await
        .ok()
        .flatten();
    Ok(match choice {
        None => Step::Choose { computer },
        Some(choice) => {
            let chats = store.list(&owner).await.map_or(0, |rows| {
                rows.iter()
                    .filter(|chat| {
                        chat.archived_unix.is_none()
                            && chat
                                .terminal
                                .as_ref()
                                .is_some_and(|t| t.computer == computer && t.deleted_unix.is_none())
                    })
                    .count()
            });
            Step::Connected {
                computer,
                choice,
                chats,
            }
        }
    })
}

fn choose_csrf(
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
    step: &Step,
) -> String {
    match step {
        Step::Choose { computer } | Step::Connected { computer, .. } => service
            .csrf(headers, viewer, "terminal-sync", computer)
            .unwrap_or_default(),
        Step::SignIn(_) => String::new(),
    }
}

/// `GET /settings/terminal`.
async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = query.map(|q| q.0).unwrap_or_default();
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let since = query.since.unwrap_or_else(crate::cloud::session::now);
    let step = match step(&app, service, &headers, &viewer, query.computer, since).await {
        Ok(step) => step,
        Err(error) => return refused(error),
    };
    let csrf = choose_csrf(service, &headers, &viewer, &step);
    let live = live_markup(&step, since, query.problem.as_deref(), &csrf);
    let body = page_body(&headers, &service.pair_code(&viewer), live);
    let owner = account_owner(&viewer.account_id);
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(&headers, &viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    // The chat pages' policy: HTMX and its live stream, no inline anything.
    crate::chat_html::protect(
        UiPage::new("Connect your terminal")
            .path(PAGE)
            .section(crate::settings::PAGE)
            .account(account)
            .head(crate::chat_html::head())
            .sidebar_section(crate::pages::chat::chat_list(&app, &owner, None, false, false).await)
            .content(PageColumn::new(body))
            .respond(&headers),
    )
}

/// `GET /settings/terminal/live`: the live part alone.
async fn live(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = query.map(|q| q.0).unwrap_or_default();
    let Ok(service) = service(&app) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(viewer) = service.authenticate(&headers).await else {
        // Signed out meanwhile: stop polling (HTMX leaves the page as is).
        return protect(StatusCode::NO_CONTENT.into_response());
    };
    let since = query.since.unwrap_or_else(crate::cloud::session::now);
    match step(&app, service, &headers, &viewer, query.computer, since).await {
        Ok(step) => {
            let csrf = choose_csrf(service, &headers, &viewer, &step);
            protect(live_markup(&step, since, None, &csrf).into_response())
        }
        Err(_) => protect(StatusCode::NO_CONTENT.into_response()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApproveForm {
    csrf: String,
    code: String,
    decision: String,
}

/// `POST /settings/terminal/approve`.
async fn approve(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ApproveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(code) = oa_auth::device::normalize_user_code(&form.code) else {
        return refused(SessionError::InvalidRequest);
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "terminal-approve",
        &code,
        &form.csrf,
    ) {
        return refused(error);
    }
    let approve = match form.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return refused(SessionError::InvalidRequest),
    };
    let to = match service.device_decide(&headers, &code, approve).await {
        Ok(Ok(request)) if approve => format!(
            "{PAGE}?computer={}",
            url::form_urlencoded::byte_serialize(request.computer.as_bytes()).collect::<String>()
        ),
        Ok(Ok(_)) => format!("{PAGE}?problem=denied"),
        Ok(Err(problem)) => format!(
            "{PAGE}?problem={}",
            if problem == "expired_token" {
                "expired_token"
            } else {
                "invalid"
            }
        ),
        Err(error) => return refused(error),
    };
    protect(Redirect::to(&to).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChooseForm {
    csrf: String,
    computer: String,
    choice: String,
    #[serde(default)]
    back: String,
}

/// `POST /settings/terminal/sync`: the person's choice for a computer.
async fn choose(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ChooseForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let computer = line(&form.computer, 64);
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "terminal-sync",
        &computer,
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(choice) = SyncChoice::parse(&form.choice) else {
        return refused(SessionError::InvalidRequest);
    };
    let owner = account_owner(&viewer.account_id);
    match crate::coder_sync::choose(&app.config.chat_store, &owner, &computer, choice).await {
        Ok(Ok(_)) => {}
        Ok(Err(_)) => return refused(SessionError::InvalidRequest),
        Err(_) => return refused(SessionError::Unavailable),
    }
    let to = if form.back == "settings" {
        crate::settings::PAGE.to_owned()
    } else {
        format!(
            "{PAGE}?computer={}",
            url::form_urlencoded::byte_serialize(computer.as_bytes()).collect::<String>()
        )
    };
    protect(Redirect::to(&to).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(host: &str, agent: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, host.parse().unwrap());
        headers.insert(header::USER_AGENT, agent.parse().unwrap());
        headers
    }

    #[test]
    fn the_install_command_for_the_persons_system_comes_first() {
        let mac = headers(
            "openagents.com",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)",
        );
        assert_eq!(install_commands(&mac)[0].1, CODER_SH);
        let windows = headers(
            "openagents.com",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
        );
        assert_eq!(install_commands(&windows)[0].1, CODER_PS1);
    }

    #[test]
    fn a_local_server_names_itself_in_the_sign_in_command() {
        let local = headers("127.0.0.1:4301", "x");
        assert_eq!(
            local_origin(&local).as_deref(),
            Some("http://127.0.0.1:4301")
        );
        assert_eq!(
            login_command("abcd", local_origin(&local).as_deref()),
            "OPENAGENTS_ORIGIN=http://127.0.0.1:4301 ~/.openagents/full-local/bin/coder login --pair abcd"
        );
        let site = headers("openagents.com", "x");
        assert_eq!(local_origin(&site), None);
        assert_eq!(login_command("abcd", None), "coder login --pair abcd");
        assert_eq!(local_origin(&headers("127.0.0.1:1;rm", "x")), None);
    }

    #[test]
    fn every_step_has_its_command_or_button_and_plain_words() {
        let body = page_body(
            &headers("openagents.com", "Macintosh"),
            "pairpairpairpair",
            live_markup(&Step::SignIn(Vec::new()), 1, None, ""),
        )
        .into_string();
        assert!(body.contains("1. Install Coder"));
        assert!(body.contains(CODER_SH) && body.contains(CODER_PS1));
        assert!(body.contains("coder login --pair pairpairpairpair"));
        assert!(body.contains("Waiting for your terminal"));
        assert!(body.contains(r#"hx-trigger="every 2s""#));
        assert!(body.contains("/settings/terminal/live?since=1"));
        crate::copy_guard::assert_plain(PAGE, &body);

        let waiting = live_markup(
            &Step::SignIn(vec![Waiting {
                code: "BCDF-GHJK".into(),
                app: "Coder".into(),
                computer: "<b>studio</b>".into(),
                csrf: "ticket".into(),
            }]),
            1,
            None,
            "",
        )
        .into_string();
        assert!(waiting.contains("BCDF-GHJK"));
        assert!(waiting.contains(r#"name="decision" value="approve""#));
        assert!(waiting.contains(r#"name="decision" value="deny""#));
        assert!(!waiting.contains("<b>studio</b>"));
        crate::copy_guard::assert_plain(PAGE, &waiting);

        let choose = live_markup(
            &Step::Choose {
                computer: "Studio".into(),
            },
            1,
            None,
            "t",
        )
        .into_string();
        assert!(choose.contains("3. Choose where your chats live"));
        assert!(
            choose.contains("Sync all my chats") && choose.contains("Keep chats on this computer")
        );
        assert!(!choose.contains("(chosen)"), "no silent default");
        assert!(choose.contains("computer=Studio"));
        crate::copy_guard::assert_plain(PAGE, &choose);

        let done = live_markup(
            &Step::Connected {
                computer: "Studio".into(),
                choice: SyncChoice::All,
                chats: 3,
            },
            1,
            None,
            "t",
        )
        .into_string();
        assert!(done.contains("4. Your terminal is connected"));
        assert!(done.contains("3 chats from Studio are in your sidebar"));
        assert!(done.contains("Sync all my chats (chosen)"));
        crate::copy_guard::assert_plain(PAGE, &done);
        let kept = live_markup(
            &Step::Connected {
                computer: "Studio".into(),
                choice: SyncChoice::Local,
                chats: 0,
            },
            1,
            Some("denied"),
            "t",
        )
        .into_string();
        assert!(kept.contains("stay on Studio"));
        crate::copy_guard::assert_plain(PAGE, &kept);
    }
}
