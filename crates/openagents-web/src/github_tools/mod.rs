//! The chat's GitHub tools with the signed-in person's GitHub connection
//! (#11167; `docs/web/github-tools.md`).
//!
//! `/chat/{id}/github` lists the tools: open an issue (and put it on a
//! board), comment on an issue or pull request, close an issue, move an
//! issue on a board, and open a pull request. Each is a typed form
//! ([`github_actions::Action`], shared with the CLI's `issue` and
//! `project` verbs); sending it shows a confirm card that says the
//! repository and every change before anything happens. The card carries
//! the action sealed with this server's key, bound to the person and the
//! chat, so Confirm runs exactly what it showed, once.
//!
//! Running reads the person's GitHub connection from the account service
//! for that request only ([`crate::cloud::session::CloudSession::github_token`])
//! and calls GitHub's REST API as them ([`run`]). When the connection
//! lacks the access a change needs (boards need GitHub's `project`
//! access, asked for only when a board tool is first used), nothing
//! changes: a consent page says what GitHub will be asked, one button
//! goes there ([`GRANT`], [`oa_auth::Purpose::Board`]), and the person
//! comes back to the same card. What ran is noted in the chat.
//!
//! The chat can propose a change too. The worker's router picks an
//! `openagents issue|project` command for the message through its command
//! tree (Jev chooses the command, the model fills its text, the command's
//! own parser checks it; `coder::cli_route`), and the reply carries the
//! command ([`openagents_chat::router::Meta::command`]). The thread shows
//! that reply's card ([`thread_entry`], loaded from [`proposal`]): the
//! command read into the same action ([`github_actions::Action::from_argv`],
//! exact parsing of the chosen command, never of the message) and sealed
//! the same way, so Confirm runs exactly what it shows.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use maud::{Markup, html};
use oa_auth::repos::{Access, RepoError};
use openagents_ui::actions::{Button, ButtonLink, ButtonType};
use openagents_ui::forms::{Checkbox, Field, Input, Textarea};
use openagents_ui::shell::Breadcrumb;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::App;
use crate::chat_store::{Conversation, Message, Role};
use crate::cloud::session::SessionError;
use crate::cloud::session::github::RepoCallError;
use crate::ui_page::UiPage;
use github_actions::action::MAX_BODY;
use github_actions::rest::{self as run, Failure};
use github_actions::{Action, ArgvError, Fields, Tool};

/// What a tool form sends: the page's CSRF token and the tool's fields.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct ToolForm {
    #[serde(default)]
    pub csrf: String,
    #[serde(flatten)]
    pub fields: Fields,
}

/// Where asking GitHub for boards access starts (`?return_to=`).
pub(crate) const GRANT: &str = "/auth/github/board";
/// The cookie that keeps a card across the trip to GitHub for more
/// access, so the person comes back to the same card.
const COOKIE: &str = "oa_github_tool";
/// The longest cookie value kept; a larger card is filled in again.
const COOKIE_MAX: usize = 3_800;
/// How long a confirm card stays good, in seconds.
const CARD_SECONDS: u64 = 1_800;
/// A chat with this many messages gets no more notes.
const NOTES_UNTIL: usize = 96;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/github", get(tools).post(review))
        .route("/chat/{id}/github/confirm", get(confirm_again))
        .route("/chat/{id}/github/run", axum::routing::post(confirmed))
        .route("/chat/{id}/github/proposed/{index}", get(proposal))
        .route(GRANT, get(grant))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A confirm card's action, bound to the person and the chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sealed {
    pub owner: String,
    pub chat: String,
    pub issued: u64,
    /// Random; a card runs once.
    pub id: String,
    pub action: Action,
}

fn mac(app: &App, payload: &[u8]) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&app.config.ask_salt).expect("HMAC accepts 32 bytes");
    mac.update(b"openagents.web.github-tools.v1:");
    mac.update(payload);
    mac
}

/// Seal `action` for `owner`'s chat `chat`.
pub(crate) fn seal(app: &App, owner: &str, chat: &str, action: Action) -> String {
    let sealed = Sealed {
        owner: owner.to_string(),
        chat: chat.to_string(),
        issued: now(),
        id: secp256k1::rand::random::<[u8; 16]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        action,
    };
    let payload = serde_json::to_vec(&sealed).expect("a card serializes");
    let tag = mac(app, &payload).finalize().into_bytes();
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&payload),
        URL_SAFE_NO_PAD.encode(tag)
    )
}

/// Open a sealed card: this server's seal, this person and chat, still
/// fresh, and an action within bounds.
pub(crate) fn open(app: &App, owner: &str, chat: &str, value: &str) -> Option<Sealed> {
    if value.len() > 64 * 1024 {
        return None;
    }
    let (payload, tag) = value.trim().split_once('.')?;
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let tag = URL_SAFE_NO_PAD.decode(tag).ok()?;
    mac(app, &payload).verify_slice(&tag).ok()?;
    let sealed: Sealed = serde_json::from_slice(&payload).ok()?;
    (sealed.owner == owner
        && sealed.chat == chat
        && now().saturating_sub(sealed.issued) <= CARD_SECONDS
        && sealed.issued <= now().saturating_add(60)
        && sealed.action.valid())
    .then_some(sealed)
}

/// Cards that ran (or are running) on this server, with when.
static RAN: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Mark card `id` as running; `false` when it already ran.
fn claim(id: &str) -> bool {
    let mut ran = RAN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let ran = ran.get_or_insert_with(HashMap::new);
    let cutoff = now().saturating_sub(CARD_SECONDS * 2);
    ran.retain(|_, at| *at >= cutoff);
    if ran.contains_key(id) {
        return false;
    }
    ran.insert(id.to_string(), now());
    true
}

/// Let card `id` run again (nothing changed: it waited for more access).
fn unclaim(id: &str) {
    let mut ran = RAN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(ran) = ran.as_mut() {
        ran.remove(id);
    }
}

/// The chat and the person's GitHub, ready for a tool.
struct Ready {
    owner: String,
    chat: Conversation,
    /// The repository a form starts with: the chat's project's, or the
    /// person's only project's.
    repository: String,
}

/// The chat, when the signed-in person can use GitHub tools in it;
/// otherwise the page that says what to do first.
async fn ready(app: &App, headers: &HeaderMap, id: &str) -> Result<Ready, Response> {
    let loaded = crate::pages::chat::load(app, headers, id).await?;
    let chat = loaded.conversation;
    let back = format!("/chat/{id}");
    if chat.terminal.is_some() {
        return Err(crate::ui_page::problem(
            headers,
            StatusCode::NOT_FOUND,
            "GitHub tools",
            "GitHub tools work in chats started on this site.",
            (&back, "Back to the chat"),
        ));
    }
    let Some(sidebar) = crate::projects::sidebar(app).await else {
        let login = crate::auth::login_href(&format!("/chat/{id}/github"), false);
        return Err(crate::ui_page::problem(
            headers,
            StatusCode::OK,
            "Sign in to use GitHub tools",
            "GitHub tools change issues and boards as you, so sign in with GitHub first.",
            (&login, "Sign in"),
        ));
    };
    match &sidebar.status.access {
        Access::None => {
            return Err(crate::ui_page::problem(
                headers,
                StatusCode::OK,
                "Connect GitHub first",
                "GitHub tools use your GitHub connection. Connect GitHub on Projects, then come back to this chat.",
                (crate::projects::PAGE, "Connect GitHub"),
            ));
        }
        Access::Reconnect { .. } => {
            return Err(crate::ui_page::problem(
                headers,
                StatusCode::OK,
                "Connect GitHub again",
                "GitHub stopped accepting your connection. Connect it again, then come back to this chat.",
                (crate::projects::RECONNECT, "Connect GitHub again"),
            ));
        }
        Access::Connected { .. } | Access::Installed { .. } => {}
    }
    let projects = &sidebar.status.projects;
    let repository = chat
        .project
        .as_deref()
        .and_then(|project| sidebar.project(project))
        .or_else(|| (projects.len() == 1).then(|| &projects[0]))
        .map(|project| project.repository.clone())
        .unwrap_or_default();
    Ok(Ready {
        owner: chat.owner.clone(),
        chat,
        repository,
    })
}

async fn tools(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    match ready(&app, &headers, &id).await {
        Ok(ready) => tools_page(&app, &headers, &ready, &ToolForm::default(), None),
        Err(response) => response,
    }
}

async fn review(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<ToolForm>,
) -> Response {
    let owner = match crate::pages::chat::validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let ready = match ready(&app, &headers, &id).await {
        Ok(ready) if ready.owner == owner => ready,
        Ok(_) => return gone(&headers, &id),
        Err(response) => return response,
    };
    match Action::from_fields(&form.fields) {
        Ok(action) => {
            let sealed = seal(&app, &owner, &id, action.clone());
            card_page(&app, &headers, &ready, &action, &sealed)
        }
        Err(problem) => tools_page(&app, &headers, &ready, &form, Some(problem)),
    }
}

/// Back from GitHub with more access: the same card again.
async fn confirm_again(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ready = match ready(&app, &headers, &id).await {
        Ok(ready) => ready,
        Err(response) => return response,
    };
    let kept = cookie(&headers).and_then(|value| open(&app, &ready.owner, &id, &value));
    match kept {
        Some(sealed) => {
            let value = seal(&app, &ready.owner, &id, sealed.action.clone());
            card_page(&app, &headers, &ready, &sealed.action, &value)
        }
        None => {
            crate::chat_html::protect(Redirect::to(&format!("/chat/{id}/github")).into_response())
        }
    }
}

#[derive(Deserialize)]
struct Confirmed {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    card: String,
}

async fn confirmed(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Confirmed>,
) -> Response {
    let owner = match crate::pages::chat::validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let ready = match ready(&app, &headers, &id).await {
        Ok(ready) if ready.owner == owner => ready,
        Ok(_) => return gone(&headers, &id),
        Err(response) => return response,
    };
    let Some(sealed) = open(&app, &owner, &id, &form.card) else {
        return crate::ui_page::problem(
            &headers,
            StatusCode::BAD_REQUEST,
            "This card is out of date",
            "Nothing changed on GitHub. Fill in the tool again to see a new card.",
            (&format!("/chat/{id}/github"), "Back to GitHub tools"),
        );
    };
    if !claim(&sealed.id) {
        return crate::ui_page::problem(
            &headers,
            StatusCode::CONFLICT,
            "This change already ran",
            "Each card runs once. See the chat for what it did.",
            (&format!("/chat/{id}"), "Back to the chat"),
        );
    }
    let Some(service) = app.config.cloud.as_deref() else {
        unclaim(&sealed.id);
        return crate::cloud::refused(SessionError::Unavailable);
    };
    let token = match service.github_token(&headers).await {
        Ok(token) => token,
        Err(error) => {
            unclaim(&sealed.id);
            return match error {
                RepoCallError::Repo(RepoError::NotConnected) => crate::ui_page::problem(
                    &headers,
                    StatusCode::OK,
                    "Connect GitHub first",
                    "Nothing changed. GitHub tools use your GitHub connection; connect GitHub on Projects.",
                    (crate::projects::PAGE, "Connect GitHub"),
                ),
                RepoCallError::Repo(RepoError::Reconnect) => crate::ui_page::problem(
                    &headers,
                    StatusCode::OK,
                    "Connect GitHub again",
                    "Nothing changed. GitHub stopped accepting your connection.",
                    (crate::projects::RECONNECT, "Connect GitHub again"),
                ),
                RepoCallError::Session(error) => crate::cloud::refused(error),
                RepoCallError::Repo(error) => crate::ui_page::problem(
                    &headers,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Nothing changed",
                    &error.to_string(),
                    (&format!("/chat/{id}/github"), "Back to GitHub tools"),
                ),
            };
        }
    };
    let api = run::Http::new(&api_base(&app), token);
    match run::run(&api, &sealed.action).await {
        Ok(done) => {
            let mut text = format!("GitHub: {}", done.summary);
            if let Some(problem) = &done.problem {
                text.push(' ');
                text.push_str(problem);
            }
            if let Some(link) = &done.link {
                text.push(' ');
                text.push_str(link);
            }
            note(&app, &owner, &id, text).await;
            done_page(&headers, &ready, &done)
        }
        Err(Failure::NeedsAccess { board }) => {
            unclaim(&sealed.id);
            consent_page(&app, &headers, &ready, board, &form.card)
        }
        Err(failure) => crate::ui_page::problem(
            &headers,
            StatusCode::BAD_GATEWAY,
            "That didn't finish",
            &failure.text(),
            (&format!("/chat/{id}/github"), "Back to GitHub tools"),
        ),
    }
}

#[derive(Deserialize)]
struct Back {
    return_to: Option<String>,
}

/// Ask GitHub for the access GitHub tools need (boards included), signed
/// in only, then back to `return_to`.
async fn grant(State(app): State<App>, headers: HeaderMap, Query(back): Query<Back>) -> Response {
    let back = oa_auth::return_to(back.return_to.as_deref());
    let service = match crate::cloud::service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    match service.authenticate(&headers).await {
        Ok(_) => crate::auth::begin(&app, &headers, Some(&back), oa_auth::Purpose::Board),
        Err(SessionError::Unauthenticated) => crate::cloud::protect(
            Redirect::to(&crate::auth::login_href(&back, false)).into_response(),
        ),
        Err(error) => crate::cloud::refused(error),
    }
}

/// GitHub's API origin: the configured one, else GitHub's.
fn api_base(app: &App) -> String {
    app.config.github.as_ref().map_or_else(
        || "https://api.github.com".to_string(),
        |github| github.endpoints.api_url.trim_end_matches('/').to_string(),
    )
}

/// Note what ran in the chat, unless an answer is being written there
/// (the note would land in its place) or the chat is full.
async fn note(app: &App, owner: &str, id: &str, text: String) {
    let Ok(Some(loaded)) = app.config.chat_store.load(owner, id).await else {
        return;
    };
    let chat = &loaded.conversation;
    if chat.pending.is_some() || chat.messages.len() >= NOTES_UNTIL || chat.deleted() {
        return;
    }
    let mut next = chat.clone();
    next.messages.push(Message {
        role: Role::Tool,
        text,
        request_id: None,
    });
    next.revision += 1;
    next.updated_unix = now();
    let _ = app.config.chat_store.compare_and_swap(&loaded, &next).await;
}

fn gone(headers: &HeaderMap, id: &str) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::FORBIDDEN,
        "Reload this page",
        "This page is out of date.",
        (&format!("/chat/{id}"), "Back to the chat"),
    )
}

/// The card kept across the trip to GitHub, if any.
fn cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.is_empty())
}

fn page(headers: &HeaderMap, ready: &Ready, path: &str, title: &str, body: Markup) -> Response {
    let id = &ready.chat.id;
    let page = UiPage::new(title.to_string())
        .path(path.to_string())
        .head(crate::chat_html::head())
        .breadcrumb(
            Breadcrumb::new(title.to_string())
                .crumb(ready.chat.title.clone(), format!("/chat/{id}")),
        )
        .content(crate::ui_page::prose(body));
    crate::chat_html::protect(page.respond(headers))
}

fn text_field(
    form: &str,
    name: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    required: bool,
) -> Markup {
    let id = format!("gh-{form}-{name}");
    let field = Field::new(id.clone(), label).required(required);
    let aria = field.aria();
    let input = Input::new(name)
        .id(id)
        .value(value)
        .placeholder(placeholder)
        .required(required)
        .aria(aria);
    html! { (field.control(input)) }
}

fn area_field(form: &str, name: &str, label: &str, value: &str, required: bool) -> Markup {
    let id = format!("gh-{form}-{name}");
    let field = Field::new(id.clone(), label).required(required);
    let aria = field.aria();
    let area = Textarea::new(name)
        .id(id)
        .rows(4)
        .maxlength(MAX_BODY as u32)
        .value(value)
        .required(required)
        .aria(aria);
    html! { (field.control(area)) }
}

/// A form's starting values: what was sent, on the form it was sent from.
struct Prefill {
    mine: bool,
}

impl Prefill {
    fn pick(&self, sent: &str, default: &str) -> String {
        (if self.mine { sent } else { default }).to_string()
    }
}

/// The tools, one form each; `form` fills the one it was sent from, with
/// `problem` beside it.
pub(crate) fn tools_markup(
    csrf: &str,
    chat: &str,
    repository: &str,
    form: &ToolForm,
    problem: Option<&str>,
) -> Markup {
    let sent = Tool::from_key(&form.fields.tool);
    let post = format!("/chat/{chat}/github");
    html! {
        h1 { "GitHub tools" }
        p {
            "Change issues, boards and pull requests as you, with your GitHub connection. "
            "Each change shows what it will do first, and runs only when you confirm."
        }
        @for tool in Tool::ALL {
            @let mine = sent == Some(tool);
            @let value = Prefill { mine };
            @let key = tool.key();
            section.oa-github-tool id=(format!("gh-{key}")) {
                h2 { (tool.name()) }
                @if mine {
                    @if let Some(problem) = problem {
                        p.oa-thread-error role="alert" { (problem) }
                    }
                }
                form method="post" action=(post) {
                    input type="hidden" name="csrf" value=(csrf);
                    input type="hidden" name="tool" value=(key);
                    (text_field(key, "repository", "Repository", &value.pick(&form.fields.repository, repository), "owner/name", true))
                    @match tool {
                        Tool::CreateIssue => {
                            (text_field(key, "title", "Title", &value.pick(&form.fields.title, ""), "", true))
                            (area_field(key, "body", "Description", &value.pick(&form.fields.body, ""), false))
                            (text_field(key, "board", "Board number (optional)", &value.pick(&form.fields.board, ""), "22", false))
                            (text_field(key, "status", "Status on the board", &value.pick(&form.fields.status, "Todo"), "Todo", false))
                        }
                        Tool::Comment => {
                            (text_field(key, "number", "Issue or pull request number", &value.pick(&form.fields.number, ""), "11167", true))
                            (area_field(key, "body", "Comment", &value.pick(&form.fields.body, ""), true))
                        }
                        Tool::CloseIssue => {
                            (text_field(key, "number", "Issue number", &value.pick(&form.fields.number, ""), "11167", true))
                            (area_field(key, "comment", "Comment (optional)", &value.pick(&form.fields.comment, ""), false))
                        }
                        Tool::MoveOnBoard => {
                            (text_field(key, "number", "Issue or pull request number", &value.pick(&form.fields.number, ""), "11167", true))
                            (text_field(key, "board", "Board number", &value.pick(&form.fields.board, ""), "22", true))
                            (text_field(key, "status", "Status", &value.pick(&form.fields.status, "Todo"), "In Progress", true))
                        }
                        Tool::OpenPullRequest => {
                            (text_field(key, "head", "Branch with your changes", &value.pick(&form.fields.head, ""), "fix-login", true))
                            (text_field(key, "base", "Into branch (optional; the default branch if empty)", &value.pick(&form.fields.base, ""), "main", false))
                            (text_field(key, "title", "Title", &value.pick(&form.fields.title, ""), "", true))
                            (area_field(key, "body", "Description", &value.pick(&form.fields.body, ""), false))
                            (Checkbox::new("draft", "Open as a draft").id(format!("gh-{key}-draft")).value("1").checked(mine && form.fields.draft.is_some()))
                        }
                    }
                    div.oa-page-actions {
                        (Button::new("Review").kind(ButtonType::Submit))
                    }
                }
            }
        }
    }
}

fn tools_page(
    app: &App,
    headers: &HeaderMap,
    ready: &Ready,
    form: &ToolForm,
    problem: Option<&str>,
) -> Response {
    let id = &ready.chat.id;
    let body = tools_markup(
        &crate::pages::chat::csrf(app, &ready.owner),
        id,
        &ready.repository,
        form,
        problem,
    );
    let mut response = page(
        headers,
        ready,
        &format!("/chat/{id}/github"),
        "GitHub tools",
        body,
    );
    if problem.is_some() {
        *response.status_mut() = StatusCode::BAD_REQUEST;
    }
    response
}

/// The confirm card: the repository, every change, the text to post, and
/// Confirm.
pub(crate) fn card_markup(csrf: &str, chat: &str, action: &Action, sealed: &str) -> Markup {
    let card = action.card();
    html! {
        h1 { (card.heading) }
        (card_body(csrf, chat, action, sealed))
    }
}

/// Everything on the card under its heading.
fn card_body(csrf: &str, chat: &str, action: &Action, sealed: &str) -> Markup {
    let card = action.card();
    html! {
        p { "In " a href=(format!("https://github.com/{}", card.repository)) { (card.repository) } ", as you:" }
        ul {
            @for change in &card.changes { li { (change) } }
        }
        @if let Some(text) = &card.text {
            pre.oa-github-text { (text) }
        }
        p { "Nothing changes on GitHub until you confirm." }
        form method="post" action=(format!("/chat/{chat}/github/run")) {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="card" value=(sealed);
            div.oa-page-actions {
                (Button::new("Confirm").kind(ButtonType::Submit))
                (crate::ui_page::action_link("Cancel", &format!("/chat/{chat}/github")))
            }
        }
    }
}

fn card_page(
    app: &App,
    headers: &HeaderMap,
    ready: &Ready,
    action: &Action,
    sealed: &str,
) -> Response {
    let id = &ready.chat.id;
    let body = card_markup(
        &crate::pages::chat::csrf(app, &ready.owner),
        id,
        action,
        sealed,
    );
    let mut response = page(
        headers,
        ready,
        &format!("/chat/{id}/github"),
        action.tool().name(),
        body,
    );
    // The card no longer needs keeping once it shows again.
    if let Ok(value) = HeaderValue::from_str(&set_cookie(app, "", 0)) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

fn set_cookie(app: &App, value: &str, seconds: u64) -> String {
    format!(
        "{COOKIE}={value}; Path=/chat/; Max-Age={seconds}; HttpOnly; SameSite=Lax{}",
        if app.config.secure_cookies {
            "; Secure"
        } else {
            ""
        }
    )
}

/// What GitHub will be asked, one button to go there, and the card kept
/// for when the person comes back.
pub(crate) fn consent_markup(chat: &str, board: bool, kept: bool) -> Markup {
    let back = format!("/chat/{chat}/github/confirm");
    let href = format!(
        "{GRANT}?return_to={}",
        url::form_urlencoded::byte_serialize(back.as_bytes()).collect::<String>()
    );
    html! {
        h1 { @if board { "Allow changes to your boards" } @else { "Allow changes on GitHub" } }
        p {
            "Nothing changed yet. To do this, GitHub needs to let OpenAgents "
            @if board {
                "read and change your GitHub Projects boards, and change issues and pull requests in your repositories."
            } @else {
                "change issues and pull requests in your repositories."
            }
        }
        p {
            "GitHub shows exactly what it allows on the next page. You can take it back any time in "
            a href="https://github.com/settings/applications" { "GitHub's settings" } "."
        }
        @if kept {
            p { "Afterwards you come back to this change to confirm it." }
        } @else {
            p { "Afterwards, fill in the tool again." }
        }
        div.oa-page-actions {
            (ButtonLink::new("Allow on GitHub", href))
            (crate::ui_page::action_link("Not now", &format!("/chat/{chat}/github")))
        }
    }
}

fn consent_page(
    app: &App,
    headers: &HeaderMap,
    ready: &Ready,
    board: bool,
    sealed: &str,
) -> Response {
    let id = &ready.chat.id;
    let kept = sealed.len() <= COOKIE_MAX;
    let title = if board {
        "Allow changes to your boards"
    } else {
        "Allow changes on GitHub"
    };
    let mut response = page(
        headers,
        ready,
        &format!("/chat/{id}/github"),
        title,
        consent_markup(id, board, kept),
    );
    if kept && let Ok(value) = HeaderValue::from_str(&set_cookie(app, sealed, CARD_SECONDS)) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

/// What ran, with the link to it on GitHub.
pub(crate) fn done_markup(chat: &str, done: &run::Done) -> Markup {
    html! {
        h1 { "Done" }
        p { (done.summary) }
        @if let Some(problem) = &done.problem {
            p role="alert" { (problem) }
        }
        div.oa-page-actions {
            @if let Some(link) = &done.link {
                (ButtonLink::new("Open on GitHub", link.clone()))
            }
            (crate::ui_page::action_link("Back to the chat", &format!("/chat/{chat}")))
        }
    }
}

fn done_page(headers: &HeaderMap, ready: &Ready, done: &run::Done) -> Response {
    let id = &ready.chat.id;
    page(
        headers,
        ready,
        &format!("/chat/{id}/github"),
        "Done",
        done_markup(id, done),
    )
}

/// The change a reply's proposed command makes: `None` when the command
/// isn't a GitHub one, else the action (with `repository`, the chat's
/// project's, when the command names none) or why there is none.
pub(crate) fn proposed(
    argv: &[String],
    repository: Option<&str>,
) -> Option<Result<Action, ArgvError>> {
    github_actions::argv::is_github(argv).then(|| Action::from_argv(argv, repository))
}

/// Where a reply that proposed a GitHub change shows its card: loaded
/// from [`proposal`] into the thread, with a plain link for a browser
/// without scripts.
pub(crate) fn thread_entry(
    chat: &str,
    index: usize,
    reply: Option<&openagents_chat::router::Meta>,
) -> Markup {
    let github = reply
        .and_then(|reply| reply.command.as_deref())
        .is_some_and(github_actions::argv::is_github);
    if !github {
        return html! {};
    }
    let href = format!("/chat/{chat}/github/proposed/{index}");
    html! {
        div.oa-github-card hx-get=(href) hx-trigger="load" hx-swap="outerHTML" {
            p { a href=(href) { "Review this change on GitHub" } }
        }
    }
}

/// The card in the thread for a proposal: Confirm with the sealed action,
/// or what to do when the command can't become one.
pub(crate) fn thread_card(
    app: &App,
    owner: &str,
    chat: &str,
    proposal: &Result<Action, ArgvError>,
) -> Markup {
    let tools = format!("/chat/{chat}/github");
    match proposal {
        Ok(action) => {
            let sealed = seal(app, owner, chat, action.clone());
            let csrf = crate::pages::chat::csrf(app, owner);
            html! {
                div.oa-github-card {
                    h3 { (action.card().heading) }
                    (card_body(&csrf, chat, action, &sealed))
                }
            }
        }
        Err(ArgvError::NeedsRepository) => html! {
            div.oa-github-card {
                p { "Pick the repository for this change in " a href=(tools) { "GitHub tools" } "." }
            }
        },
        Err(_) => html! {
            div.oa-github-card {
                p { "This change needs a detail filled in first. Finish it in " a href=(tools) { "GitHub tools" } "." }
            }
        },
    }
}

/// A reply's proposed GitHub change, sealed into a card: the thread loads
/// it in place, and a browser without scripts opens it as a page.
async fn proposal(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, usize)>,
) -> Response {
    let ready = match ready(&app, &headers, &id).await {
        Ok(ready) => ready,
        Err(response) => return response,
    };
    let chat = &ready.chat;
    let proposal = chat
        .messages
        .get(index)
        .and_then(|message| crate::suggestions::message_reply(chat, message))
        .and_then(|reply| reply.command.as_deref())
        .and_then(|argv| {
            proposed(
                argv,
                Some(ready.repository.as_str()).filter(|repo| !repo.is_empty()),
            )
        });
    let Some(proposal) = proposal else {
        return crate::ui_page::problem(
            &headers,
            StatusCode::NOT_FOUND,
            "No change here",
            "That reply proposed no change on GitHub.",
            (&format!("/chat/{id}"), "Back to the chat"),
        );
    };
    let card = thread_card(&app, &ready.owner, &id, &proposal);
    if headers.contains_key("hx-request") {
        let mut response = card.into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return crate::chat_html::protect(response);
    }
    page(
        &headers,
        &ready,
        &format!("/chat/{id}/github"),
        "GitHub tools",
        card,
    )
}
