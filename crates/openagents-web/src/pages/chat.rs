//! Durable visitor-owned conversations over the existing web chat worker.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Redirect, Response, Sse};
use axum::routing::{get, post};
use hmac::{Hmac, Mac};
use maud::{Markup, PreEscaped, Render, html};
use openagents_chat::basic_coder::{self, Reply, Turn};
use openagents_chat::router::{Context, Surface};
use openagents_ui::content::MarkdownRoot;
// Disabled until the composer's context, model and voice controls do
// something (see `composer`):
// use openagents_ui::icons::Icon;
// use openagents_ui::shell::{ComposerAction, ModelPickerTrigger};
use openagents_ui::shell::{
    Breadcrumb, ChatList, ChatStatus, Composer, HxGet, Message as ThreadMessage, NavItem,
    ScrollToBottom, composer_panel_host,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{
    Conversation, Error, Loaded, Message, Outcome, Pending, Request, Role, Selection,
};
use crate::layout::problem;
use crate::ui_page::UiPage;

const MAX_CHARS: usize = 4_000;
const MAX_MESSAGES: usize = 96;
const WINDOW: usize = 24;
const LEASE_SECONDS: u64 = 180;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        // `/chat` has no page of its own: a new chat starts on the home page.
        .route(
            "/chat",
            get(|| async { crate::chat_html::protect(Redirect::to("/").into_response()) })
                .post(start),
        )
        .route("/chat/{id}", get(show).post(follow))
        .route("/chat/{id}/workspace", get(workspace))
        .route("/chat/{id}/transcript", get(transcript))
        .route("/chat/{id}/events", get(events))
        .route("/chat/{id}/messages/{index}/original", get(original))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[derive(Deserialize)]
struct Prompt {
    q: String,
    request_id: String,
    csrf: String,
    #[serde(default)]
    selection: String,
}

#[derive(Default, Deserialize)]
struct Window {
    before: Option<usize>,
    after: Option<u64>,
    offset: Option<usize>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn visitor(headers: &HeaderMap) -> (String, Option<String>) {
    match crate::ask::visitor(headers) {
        Some(id) => (id, None),
        None => (crate::ask::new_visitor(), Some(String::new())),
    }
}

pub(crate) fn cookie(app: &App, owner: &str, fresh: bool, response: &mut Response) {
    if fresh {
        let secure = if app.config.secure_cookies {
            "; Secure"
        } else {
            ""
        };
        if let Ok(value) = HeaderValue::from_str(&format!(
            "{}={owner}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax{secure}",
            crate::ask::COOKIE
        )) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
}

pub(crate) fn csrf(app: &App, owner: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&app.config.ask_salt).expect("HMAC accepts 32 bytes");
    mac.update(b"openagents.web.chat.csrf.v1:");
    mac.update(owner.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn validate_form(
    app: &App,
    headers: &HeaderMap,
    supplied: &str,
) -> Result<String, Response> {
    let Some(owner) = crate::ask::visitor(headers) else {
        return Err(refusal(
            StatusCode::FORBIDDEN,
            "Open the homepage before sending a message.",
        ));
    };
    let expected = csrf(app, &owner);
    // Compare digests in constant time without exposing the visitor signing key.
    let mut mac = Hmac::<Sha256>::new_from_slice(expected.as_bytes()).expect("HMAC accepts text");
    mac.update(supplied.as_bytes());
    let mut correct =
        Hmac::<Sha256>::new_from_slice(expected.as_bytes()).expect("HMAC accepts text");
    correct.update(expected.as_bytes());
    if mac.verify_slice(&correct.finalize().into_bytes()).is_err() {
        return Err(refusal(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload this page.",
        ));
    }
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v != "same-origin" && v != "none")
    {
        return Err(refusal(
            StatusCode::FORBIDDEN,
            "Send messages from this site.",
        ));
    }
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let scheme = if app.config.secure_cookies {
            "https"
        } else {
            "http"
        };
        if origin != format!("{scheme}://{host}") {
            return Err(refusal(
                StatusCode::FORBIDDEN,
                "Send messages from this site.",
            ));
        }
    }
    Ok(owner)
}

fn command(app: &App, headers: &HeaderMap, prompt: &Prompt) -> Result<(String, String), Response> {
    let owner = validate_form(app, headers, &prompt.csrf)?;
    if !valid_id(&prompt.request_id) {
        return Err(refusal(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload this page.",
        ));
    }
    let Some(text) = normalize(&prompt.q) else {
        return Err(refusal(
            StatusCode::BAD_REQUEST,
            "Enter a message of at most 4,000 characters.",
        ));
    };
    Ok((owner, text))
}

fn selected(app: &App, owner: &str, prompt: &Prompt) -> Result<Option<Selection>, Response> {
    let selection = crate::composer::state(app, owner, &prompt.selection)?;
    Ok((selection != Selection::default()).then_some(selection))
}

async fn start(State(app): State<App>, headers: HeaderMap, Form(prompt): Form<Prompt>) -> Response {
    let (owner, text) = match command(&app, &headers, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = prompt.request_id.clone();
    let selection = match selected(&app, &owner, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let digest = request_digest(&text, selection.as_ref());
    match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(record))
            if record
                .conversation
                .requests
                .first()
                .is_some_and(|r| r.digest == digest) =>
        {
            return crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response());
        }
        Ok(Some(_)) => {
            return refusal(
                StatusCode::CONFLICT,
                "This message was already sent with different text. Reload the chat.",
            );
        }
        Ok(None) => {}
        Err(e) => return unavailable(e),
    }
    // Running a chat on a connected computer left with the Cloud pages
    // (docs/web/cloud-reset.md); Environments replaces it.
    let cloud: Option<crate::chat_store::CloudRequest> =
        if selection.as_ref().is_some_and(|s| s.runtime.is_some()) {
            return refusal(StatusCode::GONE, crate::composer::RUNTIME_GONE);
        } else {
            None
        };
    let admitted_at = now();
    if cloud.is_none() {
        match app
            .config
            .chat_store
            .claim(&owner, &id, admitted_at + LEASE_SECONDS)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return refusal(
                    StatusCode::CONFLICT,
                    "OpenAgents is still answering your previous message.",
                );
            }
            Err(e) => return unavailable(e),
        }
    }
    let record = Conversation {
        id: id.clone(),
        owner: owner.clone(),
        revision: 1,
        title: if cloud.is_some() {
            "Cloud work".into()
        } else {
            text.chars().take(64).collect()
        },
        messages: if cloud.is_some() {
            vec![Message {
                role: Role::User,
                text: text.clone(),
                request_id: Some(id.clone()),
            }]
        } else {
            vec![
                Message {
                    role: Role::User,
                    text,
                    request_id: Some(id.clone()),
                },
                Message {
                    role: Role::Assistant,
                    text: String::new(),
                    request_id: Some(id.clone()),
                },
            ]
        },
        pending: cloud.is_none().then(|| Pending {
            request_id: id.clone(),
            started_unix: now(),
            job_id: None,
        }),
        requests: vec![Request {
            id: id.clone(),
            digest,
            outcome: Outcome::Pending,
            selection: selection.clone(),
            cloud: cloud.clone(),
            reply: None,
        }],
        selection,
        updated_unix: now(),
    };
    let loaded = match app.config.chat_store.create(&record).await {
        Ok(v) => v,
        Err(Error::Conflict) => {
            return crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response());
        }
        Err(e) => {
            let _ = app.config.chat_store.release(&owner, &id).await;
            return unavailable(e);
        }
    };
    if cloud.is_none() {
        spawn_answer(app.clone(), loaded, admitted_at);
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

pub(crate) async fn load(app: &App, headers: &HeaderMap, id: &str) -> Result<Loaded, Response> {
    let Some(owner) = crate::ask::visitor(headers).filter(|_| valid_id(id)) else {
        return Err(missing());
    };
    let mut loaded = app
        .config
        .chat_store
        .load(&owner, id)
        .await
        .map_err(unavailable)?
        .ok_or_else(missing)?;
    if let Some(pending) = &loaded.conversation.pending
        && now().saturating_sub(pending.started_unix) > LEASE_SECONDS
    {
        let request_id = pending.request_id.clone();
        let mut next = loaded.conversation.clone();
        next.pending = None;
        next.revision += 1;
        next.updated_unix = now();
        if let Some(request) = next.requests.iter_mut().find(|r| r.id == request_id) {
            request.outcome = Outcome::Unknown;
        }
        match app.config.chat_store.compare_and_swap(&loaded, &next).await {
            Ok(v) => {
                loaded = v;
                let _ = app.config.chat_store.release(&owner, &request_id).await;
            }
            Err(Error::Conflict) => {
                loaded = app
                    .config
                    .chat_store
                    .load(&owner, id)
                    .await
                    .map_err(unavailable)?
                    .ok_or_else(missing)?;
            }
            Err(e) => return Err(unavailable(e)),
        }
    }
    // Check every returned generation, including a reload after a storage conflict.
    // Frozen native selections stay private after later composer changes.
    let mut runtimes = Vec::new();
    if let Some(runtime) = loaded
        .conversation
        .selection
        .as_ref()
        .and_then(|s| s.runtime.as_ref())
    {
        runtimes.push(runtime.clone());
    }
    for request in &loaded.conversation.requests {
        if let Some(runtime) = request.selection.as_ref().and_then(|s| s.runtime.as_ref()) {
            runtimes.push(runtime.clone());
        }
    }
    let mut checked = std::collections::HashSet::new();
    for runtime in runtimes {
        let scope = (
            runtime.binding.clone(),
            runtime.account.clone(),
            runtime.workspace.clone(),
            runtime.members_epoch,
            runtime.project.clone(),
        );
        if checked.insert(scope) {
            return Err(refusal(StatusCode::GONE, crate::composer::RUNTIME_GONE));
        }
    }
    Ok(loaded)
}

async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let record = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let chat = &record.conversation;
    let selection = chat.selection.clone().unwrap_or_default();
    let selectors = crate::composer::selectors_shown(&app, &headers, &selection).await;
    let dock = html! {
        (ticket(&app, chat, false, false))
        p #chat-feedback.oa-composer-feedback role="status" aria-live="polite" {}
    };
    let chips = crate::suggestions::reply_chips(&app, chat).await;
    let page = UiPage::new(chat.title.clone())
        .path(format!("/chat/{id}"))
        .app()
        .breadcrumb(Breadcrumb::new(chat.title.clone()))
        .head(crate::chat_html::head())
        .sidebar_section(chat_list(&app, &chat.owner, Some(&chat.id), true, false).await)
        .content(html! {
            // The thread is private: HTMX never snapshots it into history.
            div #chat-content.oa-thread-view hx-history="false" { (content(chat, None, chips)) }
        })
        .composer(composer(
            &format!("/chat/{id}"),
            "Continue this chat",
            chat.selection.as_ref(),
            selectors,
            dock,
        ));
    crate::chat_html::protect(page.respond(&headers))
}

async fn workspace(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let record = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if record
        .conversation
        .requests
        .iter()
        .any(|r| r.cloud.is_some())
    {
        let mut response = crate::chat_html::protect(StatusCode::OK.into_response());
        response.headers_mut().insert(
            "HX-Redirect",
            HeaderValue::from_str(&format!("/chat/{id}")).expect("UUID URL"),
        );
        return response;
    }
    let chat = &record.conversation;
    let selectors = crate::composer::selectors_shown(
        &app,
        &headers,
        &chat.selection.clone().unwrap_or_default(),
    )
    .await;
    let chips = crate::suggestions::reply_chips(&app, chat).await;
    let body = html! { title {(chat.title) " · OpenAgents"} (Breadcrumb::new(chat.title.clone()).swap_oob(true)) (content(chat,None,chips)) (ticket(&app,chat,true,selectors)) (chat_list(&app,&chat.owner,Some(&chat.id),true,true).await) };
    let mut response = crate::chat_html::protect(body.into_response());
    response.headers_mut().insert(
        "HX-Push-Url",
        HeaderValue::from_str(&format!("/chat/{id}")).expect("UUID URL"),
    );
    response
}

async fn follow(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(prompt): Form<Prompt>,
) -> Response {
    let (owner, text) = match command(&app, &headers, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let loaded = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let selection = match selected(&app, &owner, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let hash = request_digest(&text, selection.as_ref());
    if let Some(request) = loaded
        .conversation
        .requests
        .iter()
        .find(|r| r.id == prompt.request_id)
    {
        if request.digest != hash {
            return refusal(
                StatusCode::CONFLICT,
                "This message was already sent with different text. Reload the chat.",
            );
        }
        return accepted(&app, &headers, &loaded.conversation).await;
    }
    if selection != loaded.conversation.selection {
        return refusal(
            StatusCode::CONFLICT,
            "Your repository or environment changed. Reload the chat and send again.",
        );
    }
    if loaded.conversation.pending.is_some() {
        return refusal(
            StatusCode::CONFLICT,
            "OpenAgents is still answering your previous message.",
        );
    }
    if loaded.conversation.messages.len() + 2 > MAX_MESSAGES {
        return refusal(
            StatusCode::BAD_REQUEST,
            "This chat is full. Start a new chat to keep going.",
        );
    }
    let previous = loaded
        .conversation
        .requests
        .iter()
        .rev()
        .find_map(|r| r.cloud.as_ref());
    if previous.is_some() && selection.as_ref().is_none_or(|s| s.runtime.is_none()) {
        return refusal(
            StatusCode::CONFLICT,
            "Continue this work from its job page in Cloud.",
        );
    }
    // Running a chat on a connected computer left with the Cloud pages
    // (docs/web/cloud-reset.md); Environments replaces it.
    let cloud: Option<crate::chat_store::CloudRequest> =
        if selection.as_ref().is_some_and(|s| s.runtime.is_some()) {
            return refusal(StatusCode::GONE, crate::composer::RUNTIME_GONE);
        } else {
            None
        };
    let admitted_at = now();
    if cloud.is_none() {
        match app
            .config
            .chat_store
            .claim(&owner, &prompt.request_id, admitted_at + LEASE_SECONDS)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return refusal(
                    StatusCode::CONFLICT,
                    "OpenAgents is still answering your previous message.",
                );
            }
            Err(e) => return unavailable(e),
        }
    }
    let mut next = loaded.conversation.clone();
    next.revision += 1;
    next.updated_unix = now();
    next.messages.push(Message {
        role: Role::User,
        text,
        request_id: Some(prompt.request_id.clone()),
    });
    if cloud.is_none() {
        next.messages.push(Message {
            role: Role::Assistant,
            text: String::new(),
            request_id: Some(prompt.request_id.clone()),
        });
    }
    next.pending = cloud.is_none().then(|| Pending {
        request_id: prompt.request_id.clone(),
        started_unix: now(),
        job_id: None,
    });
    next.requests.push(Request {
        id: prompt.request_id.clone(),
        digest: hash,
        outcome: Outcome::Pending,
        selection,
        cloud: cloud.clone(),
        reply: None,
    });
    let loaded = match app.config.chat_store.compare_and_swap(&loaded, &next).await {
        Ok(v) => v,
        Err(e) => {
            let _ = app
                .config
                .chat_store
                .release(&owner, &prompt.request_id)
                .await;
            return unavailable(e);
        }
    };
    if cloud.is_none() {
        spawn_answer(app.clone(), loaded.clone(), admitted_at);
    }
    accepted(&app, &headers, &loaded.conversation).await
}

async fn accepted(app: &App, headers: &HeaderMap, chat: &Conversation) -> Response {
    if headers.get("HX-Request").is_some_and(|v| v == "true") {
        if chat.requests.last().is_some_and(|r| r.cloud.is_some()) {
            let mut response = crate::chat_html::protect(StatusCode::OK.into_response());
            response.headers_mut().insert(
                "HX-Redirect",
                HeaderValue::from_str(&format!("/chat/{}", chat.id)).expect("UUID URL"),
            );
            return response;
        }
        let selectors = crate::composer::selectors_shown(
            app,
            headers,
            &chat.selection.clone().unwrap_or_default(),
        )
        .await;
        crate::chat_html::protect(
            html! { (ticket(app,chat,true,selectors)) (chat_list(app,&chat.owner,Some(&chat.id),true,true).await) }
                .into_response(),
        )
    } else {
        crate::chat_html::protect(Redirect::to(&format!("/chat/{}", chat.id)).into_response())
    }
}

fn spawn_answer(app: App, loaded: Loaded, admitted_at: u64) {
    tokio::spawn(async move {
        answer(app, loaded, admitted_at).await;
    });
}

async fn answer(app: App, mut loaded: Loaded, admitted_at: u64) {
    let chat = &loaded.conversation;
    let owner = chat.owner.clone();
    let request_id = chat
        .pending
        .as_ref()
        .expect("dispatch owns pending request")
        .request_id
        .clone();
    let turns: Vec<Turn> = chat
        .messages
        .iter()
        .filter(|m| m.role != Role::Tool && !m.text.is_empty())
        .rev()
        .take(crate::ask::MAX_TURNS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|m| {
            if m.role == Role::User {
                Turn::user(
                    m.text
                        .chars()
                        .take(crate::ask::MAX_TURN_CHARS)
                        .collect::<String>(),
                )
            } else {
                Turn::assistant(
                    m.text
                        .chars()
                        .take(crate::ask::MAX_TURN_CHARS)
                        .collect::<String>(),
                    None,
                )
            }
        })
        .collect();
    let reply = Arc::new(Mutex::new(Reply::default()));
    // Storage admission must leave time for the bounded worker lifetime.
    // An expired admission never dispatches work, even if its write succeeded.
    let door = if now().saturating_sub(admitted_at) <= 30 {
        app.config
            .chat
            .door(crate::ask::key(&app.config.ask_salt, &owner))
            .ok()
    } else {
        None
    };
    let mut job = match door {
        Some(door) => Some(
            door.ask(
                turns,
                Context {
                    surface: Surface::Web,
                    project: chat
                        .selection
                        .as_ref()
                        .and_then(|s| s.repository.as_ref())
                        .map(|r| openagents_chat::router::Project {
                            name: format!(
                                "Public {} @{}",
                                r.repository.chars().take(75).collect::<String>(),
                                r.revision
                            ),
                            path: None,
                        }),
                    ..Context::default()
                },
                reply.clone(),
            ),
        ),
        None => None,
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(130);
    let door_unavailable = job.is_none();
    let mut asked = door_unavailable;
    let mut shown = String::new();
    let mut settled = false;
    loop {
        if let Some(job) = &mut job {
            tokio::select! { ()=job,if !asked=>asked=true, ()=tokio::time::sleep(Duration::from_millis(400))=>{} }
        }
        let (text, done, failure, meta) = {
            let r = basic_coder::lock(&reply);
            (
                r.text.clone(),
                r.done,
                r.failure.as_ref().map(|e| e.describe()),
                // What the chips under the answer read (`crate::suggestions`).
                r.done
                    .then(|| openagents_chat::suggestions::chip_meta(&r.meta))
                    .filter(|meta| !meta.is_empty()),
            )
        };
        let expired = tokio::time::Instant::now() >= deadline;
        let ended = done || failure.is_some() || asked || expired;
        if text != shown || ended {
            let mut next = loaded.conversation.clone();
            if next
                .pending
                .as_ref()
                .is_none_or(|p| p.request_id != request_id)
            {
                break;
            }
            next.messages.last_mut().expect("assistant record").text = text.clone();
            next.revision += 1;
            next.updated_unix = now();
            if ended {
                next.pending = None;
                if let Some(r) = next.requests.iter_mut().find(|r| r.id == request_id) {
                    if done {
                        r.reply = meta.clone();
                    }
                    r.outcome = if done {
                        Outcome::Answered
                    } else if door_unavailable {
                        Outcome::Failed
                    } else {
                        Outcome::Unknown
                    };
                }
                if let Some(error) = failure {
                    next.messages.push(Message {
                        role: Role::Tool,
                        text: error,
                        request_id: Some(request_id.clone()),
                    });
                } else if !done {
                    next.messages.push(Message {
                        role: Role::Tool,
                        text: if expired {
                            "We couldn't confirm this went through. Try asking again."
                        } else {
                            "We couldn't get an answer. Try asking again."
                        }
                        .into(),
                        request_id: Some(request_id.clone()),
                    });
                }
            }
            match app.config.chat_store.compare_and_swap(&loaded, &next).await {
                Ok(v) => {
                    loaded = v;
                    shown = text;
                    settled = ended;
                }
                Err(_) => {
                    // A write may have committed even if its response was lost.
                    // Recover its generation without submitting another worker job.
                    match app
                        .config
                        .chat_store
                        .load(&owner, &loaded.conversation.id)
                        .await
                    {
                        Ok(Some(current))
                            if current
                                .conversation
                                .pending
                                .as_ref()
                                .is_some_and(|p| p.request_id == request_id) =>
                        {
                            loaded = current;
                        }
                        Ok(Some(current))
                            if current
                                .conversation
                                .requests
                                .iter()
                                .any(|r| r.id == request_id && r.outcome != Outcome::Pending) =>
                        {
                            loaded = current;
                            settled = true;
                            break;
                        }
                        _ => {}
                    }
                    if expired {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }
        if ended {
            break;
        }
    }
    if settled
        && (loaded
            .conversation
            .requests
            .iter()
            .any(|r| r.id == request_id && r.outcome == Outcome::Answered)
            || door_unavailable)
    {
        let _ = app.config.chat_store.release(&owner, &request_id).await;
    }
}

/// The visitor's recent chats in the left panel, newest first (the store
/// sorts by last update), `current` marked. On a chat page (`hx`) a row also
/// loads its conversation into `#chat-content`; elsewhere rows are plain
/// links. Responses that change the list carry it again with `oob`,
/// replacing `#chat-sidebar` in place. An unavailable store leaves the list
/// empty rather than showing an error in the sidebar.
pub(crate) async fn chat_list(
    app: &App,
    owner: &str,
    current: Option<&str>,
    hx: bool,
    oob: bool,
) -> ChatList {
    let list = ChatList::new().id("chat-sidebar").swap_oob(oob);
    let rows = match app.config.chat_store.list(owner).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("openagents-web: chat list: {error}");
            return list;
        }
    };
    list.items(rows.iter().map(|row| {
        let mut item = NavItem::new(row.title.clone(), format!("/chat/{}", row.id))
            .current(current == Some(row.id.as_str()));
        if let Some(detail) = row_detail(row) {
            item = item.detail(detail);
        }
        if let Some(status) = row_status(row) {
            item = item.trailing(status);
        }
        if hx {
            item.hx(HxGet::new(format!("/chat/{}/workspace", row.id))
                .target("#chat-content")
                .swap("innerHTML")
                .sync("#chat-content:replace"))
        } else {
            item
        }
    }))
}

/// A chat row's second line: the repository and branch it was started
/// with. The list is the owner's own, so the names never reach anyone else.
fn row_detail(chat: &Conversation) -> Option<String> {
    let source = chat.selection.as_ref()?.repository.as_ref()?;
    Some(if source.branch.is_empty() {
        source.repository.clone()
    } else {
        format!("{} · {}", source.repository, source.branch)
    })
}

/// A chat row's status: "Working" while an answer runs, "Failed" when the
/// last one did not finish, and nothing otherwise (see `docs/web/sidebar.md`).
fn row_status(chat: &Conversation) -> Option<ChatStatus> {
    if chat.pending.is_some() {
        return Some(ChatStatus::Working);
    }
    match chat.requests.last()?.outcome {
        Outcome::Failed => Some(ChatStatus::Failed),
        _ => None,
    }
}

/// The chat's hidden composer fields. With `oob` they replace the page's
/// copies, and the selector row (`selectors`, when the composer shows it,
/// see [`crate::composer::selectors_shown`]) is replaced too.
pub(crate) fn ticket(app: &App, chat: &Conversation, oob: bool, selectors: bool) -> Markup {
    let selection = chat.selection.clone().unwrap_or_default();
    html! { div #chat-ticket hx-swap-oob=[oob.then_some("outerHTML")] {
        input type="hidden" id="chat-selected" name="chat" value=(chat.id) form="chat-form";
        input type="hidden" name="request_id" value=(new_id()) form="chat-form";
        input type="hidden" name="csrf" value=(csrf(app,&chat.owner)) form="chat-form";

    } (crate::composer::state_field(app, &chat.owner, &selection, oob)) @if oob && selectors { (crate::composer::controls(&selection, true)) } }
}

    let chips = crate::suggestions::reply_chips(&app, chat).await;
    let body = html! { title {(chat.title) " · OpenAgents"} (Breadcrumb::new(chat.title.clone()).swap_oob(true)) (content(chat,None,chips)) (ticket(&app,chat,true,selectors)) (chat_list(&app,&chat.owner,Some(&chat.id),true,true).await) };
    html! {
        section #chat-thread.oa-thread aria-label="Chat" {
            div.oa-thread-column hx-ext="sse" sse-connect=(format!("/chat/{}/events?after={}",chat.id,chat.revision)) sse-close="retired" {
                div #chat-transcript sse-swap="transcript,retired" hx-swap="innerHTML" { (messages(chat,before)) (chips) }
            }
        }
        (ScrollToBottom::new("#chat-thread"))
    }
}

/// One stored message as a thread turn. Assistant text is rendered Markdown
/// (the renderer escapes it); user and status text is escaped as written.
fn turn(message: &Message, index: usize) -> ThreadMessage {
    match message.role {
        Role::User => ThreadMessage::user(&message.text),
        Role::Assistant => ThreadMessage::assistant(MarkdownRoot::new(PreEscaped(
            crate::markdown::render(&message.text),
        )))
        .author("OpenAgents"),
        Role::Tool => ThreadMessage::status(&message.text),
    }
    .id(format!("chat-message-{index}"))
}

fn messages(chat: &Conversation, before: Option<usize>) -> Markup {
    let end = before
        .unwrap_or(chat.messages.len())
        .min(chat.messages.len());
    let start = end.saturating_sub(WINDOW);
    html! {
        input type="hidden" id="chat-history-window" value=(if before.is_some() {"older"}else{"latest"});
        @if start>0 {p.oa-thread-notice {"Showing messages " (start+1) "–" (end) " of " (chat.messages.len()) ". " a href=(format!("/chat/{}/transcript?before={start}",chat.id)) hx-get=(format!("/chat/{}/transcript?before={start}",chat.id)) hx-target="#chat-transcript" {"Read earlier messages"}}}
        @if before.is_some() && end<chat.messages.len() {
            p.oa-thread-notice {a href=(format!("/chat/{}/transcript?before={}",chat.id,(end+WINDOW).min(chat.messages.len()))) hx-get=(format!("/chat/{}/transcript?before={}",chat.id,(end+WINDOW).min(chat.messages.len()))) hx-target="#chat-transcript" {"Read newer messages"}}
            // The newest messages are not loaded: the scroll-to-bottom
            // button follows this link to them.
            a hidden href=(format!("/chat/{}/transcript",chat.id)) hx-get=(format!("/chat/{}/transcript",chat.id)) hx-target="#chat-transcript" data-chat-history="end" data-oa-scroll-tail {}
        }
        @for (index,message) in chat.messages[start..end].iter().enumerate() {
            (turn(message, index + start))
        }
        p #chat-status.oa-thread-status role="status" aria-live="polite" {
            @if chat.pending.is_some() {span.oa-thread-working {(openagents_ui::actions::LoadingIndicator::new().decorative()) span {"Working"}}}
            @else if chat.requests.last().is_some_and(|r|r.outcome==Outcome::Unknown) {"We couldn't confirm your last message went through. Try asking again."}
            @else {""}
        }
    }
}

async fn transcript(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(window): Query<Window>,
) -> Response {
    match load(&app, &headers, &id).await {
        Ok(v) => {
            // The chips under the last answer show with the latest messages.
            let chips = match window.before {
                None => crate::suggestions::reply_chips(&app, &v.conversation).await,
                Some(_) => html! {},
            };
            crate::chat_html::protect(
                html! { (messages(&v.conversation, window.before)) (chips) }.into_response(),
            )
        }
        Err(r) => r,
    }
}

async fn original(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, usize)>,
    Query(window): Query<Window>,
) -> Response {
    let loaded = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(message) = loaded.conversation.messages.get(index) else {
        return missing();
    };
    let bytes = message.text.as_bytes();
    let offset = window.offset.unwrap_or(0);
    if offset > bytes.len() {
        return refusal(
            StatusCode::BAD_REQUEST,
            "That position is past the end of this message.",
        );
    }
    let mut end = (offset + 64 * 1024).min(bytes.len());
    if !message.text.is_char_boundary(offset) {
        return refusal(
            StatusCode::BAD_REQUEST,
            "That position splits a character. Pick another.",
        );
    }
    while !message.text.is_char_boundary(end) {
        end -= 1;
    }
    let mut response = crate::chat_html::protect(
        (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            bytes[offset..end].to_vec(),
        )
            .into_response(),
    );
    response.headers_mut().insert(
        "X-Original-Bytes",
        HeaderValue::from_str(&bytes.len().to_string()).unwrap(),
    );
    if end < bytes.len() {
        response.headers_mut().insert(
            header::LINK,
            HeaderValue::from_str(&format!(
                "</chat/{id}/messages/{index}/original?offset={end}>; rel=next"
            ))
            .unwrap(),
        );
    }
    response
}

async fn events(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(window): Query<Window>,
) -> Response {
    let loaded = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let cursor = match headers.get("Last-Event-ID") {
        None => window.after.unwrap_or(0),
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix(&format!("{id}:")))
            .and_then(|v| v.parse::<u64>().ok())
        {
            Some(cursor) => cursor,
            None => {
                return refusal(
                    StatusCode::CONFLICT,
                    "Something went wrong. Reload the chat.",
                );
            }
        },
    };
    if cursor > loaded.conversation.revision {
        return refusal(StatusCode::CONFLICT, "This chat is out of date. Reload it.");
    }
    let stream = futures_util::stream::unfold(
        (app, headers, id, cursor, 0u16),
        |(app, headers, id, mut cursor, mut ticks)| async move {
            if ticks >= 300 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let event = match load(&app, &headers, &id).await {
                Ok(v) if v.conversation.revision > cursor => {
                    let revision = v.conversation.revision;
                    let missed = revision.saturating_sub(cursor + 1);
                    let _ = missed; // A resume re-renders the full transcript; nothing to announce.
                    let chips = crate::suggestions::reply_chips(&app, &v.conversation).await;
                    let body = html! { (messages(&v.conversation,None)) (chips) }.into_string();
                    cursor = revision;
                    Event::default()
                        .id(format!("{id}:{revision}"))
                        .event("transcript")
                        .data(body)
                }
                Ok(_) => Event::default().comment("current"),
                Err(_) => {
                    ticks = 299;
                    Event::default().id(format!("{id}:{cursor}")).event("retired").data("<p class=\"oa-thread-error\" role=\"alert\">The conversation is unavailable. Reopen it to check access.</p>")
                }
            };
            Some((
                Ok::<_, Infallible>(event),
                (app, headers, id, cursor, ticks + 1),
            ))
        },
    );
    crate::chat_html::protect(
        Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn request_digest(text: &str, selection: Option<&Selection>) -> String {
    match selection {
        None => digest(text),
        Some(selection) => {
            digest(&serde_json::to_string(&(text, selection)).expect("selection serializes"))
        }
    }
}
fn refusal(status: StatusCode, text: &str) -> Response {
    crate::chat_html::protect(
        (status, html! {p.oa-thread-error role="alert" {(text)}}).into_response(),
    )
}
fn unavailable(error: Error) -> Response {
    eprintln!("openagents-web: conversation storage: {error}");
    refusal(
        StatusCode::SERVICE_UNAVAILABLE,
        "We couldn't save your chat right now. Try again.",
    )
}
/// The homepage and chat share one composer: a stable text box
/// (`#chat-input` in `#chat-card`, which the browser adapter binds; Enter
/// sends, Shift+Enter adds a line, through the adapter or else the shell
/// script), the source/runtime selectors when `selectors` (replaced out of
/// band as `#composer-controls`; see [`crate::composer::selectors_shown`]),
/// the panel host they load into (`#composer-panel`), and `after` under the
/// form. The chat posts with HTMX and keeps the draft until the server
/// accepts it; the homepage posts a plain form and follows the redirect to
/// the new chat, with or without JavaScript.
pub(crate) fn composer(
    action: &str,
    label: &str,
    selection: Option<&Selection>,
    selectors: bool,
    after: Markup,
) -> Markup {
    let selection = selection.cloned().unwrap_or_default();
    let mut composer = Composer::new("chat-form", action)
        .label(label)
        .enhanced(action.starts_with("/chat/"))
        .input_id("chat-input")
        .body_id("chat-card")
        .max_chars(MAX_CHARS)
        .placeholder("Ask OpenAgents anything")
        .autofocus(true)
        // Disabled until the composer can attach files or tools: the "+"
        // button only opened a panel restating the repository selector and
        // saying uploads are not available.
        // .leading(
        //     ComposerAction::new(Icon::Plus, "Add context and tools")
        //         .hx(crate::composer::load("context")),
        // )
        // Disabled until there is a model to choose: "Auto" only opened a
        // panel describing the managed Web answer service.
        // .model_picker(ModelPickerTrigger::new("Auto").hx(crate::composer::load("model")))
        // Disabled until voice input works: the mic only opened a panel
        // saying voice input is not available.
        // .trailing(
        //     ComposerAction::new(Icon::Mic, "Voice input")
        //         .title("Voice input availability")
        //         .hx(crate::composer::load("voice")),
        // )
        .after(html! { (composer_panel_host("composer-panel")) (after) });
    if selectors {
        composer = composer.selectors(crate::composer::controls(&selection, false));
    }
    composer.render()
}

fn normalize(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_CHARS {
        return None;
    }
    Some(trimmed.to_owned())
}

fn valid_id(id: &str) -> bool {
    let mut parts = id.split('-');
    matches!(
        (
            parts.next().map(|p| p.len() == 8 && hex(p)),
            parts.next().map(|p| p.len() == 4 && hex(p)),
            parts
                .next()
                .map(|p| p.len() == 4 && p.starts_with('4') && hex(p)),
            parts.next().map(|p| p.len() == 4
                && p.as_bytes()
                    .first()
                    .is_some_and(|b| matches!(b, b'8' | b'9' | b'a' | b'b'))
                && hex(p)),
            parts.next().map(|p| p.len() == 12 && hex(p)),
            parts.next(),
        ),
        (
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            None
        )
    )
}

fn hex(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn new_id() -> String {
    let mut bytes = secp256k1::rand::random::<[u8; 16]>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn missing() -> Response {
    problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

#[cfg(test)]
mod uuid_tests {
    use super::*;

    #[test]
    fn ids_are_version_four_uuids() {
        let id = new_id();
        assert!(valid_id(&id), "{id}");
        assert!(!valid_id("not-a-uuid"));
        assert!(!valid_id("00000000-0000-0000-0000-000000000000"));
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
