//! Durable visitor-owned conversations over the existing web chat worker.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{Html, IntoResponse, Redirect, Response, Sse};
use axum::routing::{get, post};
use hmac::{Hmac, Mac};
use maud::{Markup, PreEscaped, html};
use openagents_chat::basic_coder::{self, Reply, Turn};
use openagents_chat::router::{Context, Surface};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{
    Conversation, Error, Loaded, Message, Outcome, Pending, Request, Role, Selection,
};
use crate::layout::{self, problem};

const MAX_CHARS: usize = 4_000;
const MAX_MESSAGES: usize = 96;
const WINDOW: usize = 24;
const LEASE_SECONDS: u64 = 180;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat", post(start))
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
            "The message ticket is invalid. Reload this page.",
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
            "The message ticket is invalid. Reload this page.",
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
                "This message identity was already used for different text.",
            );
        }
        Ok(None) => {}
        Err(e) => return unavailable(e),
    }
    let cloud = if selection.as_ref().is_some_and(|s| s.runtime.is_some()) {
        match crate::cloud::composer::stage(
            &app,
            &headers,
            &owner,
            &id,
            selection.as_ref().unwrap(),
            &text,
            None,
        )
        .await
        {
            Ok(v) => Some(v),
            Err(r) => return r,
        }
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
            crate::cloud::composer::authorize(app, headers, &runtime).await?;
        }
    }
    Ok(loaded)
}

async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let record = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Some(cloud) = record
        .conversation
        .requests
        .iter()
        .rev()
        .find_map(|r| r.cloud.as_ref())
    {
        return crate::cloud::composer::view(&app, &headers, cloud).await;
    }
    let body = html! {
        div.chat-shell hx-history="false" {
            (sidebar(&app,&record.conversation).await)
            section.chat-main aria-label="Conversation" {
                div #chat-content { (content(&record.conversation, None)) }
                div.chat-dock.chat-column {
                    (PreEscaped(composer(&format!("/chat/{id}"),"Continue this chat", record.conversation.selection.as_ref())))
                    div #composer-panel {}
                    (ticket(&app,&record.conversation,false))
                    p #chat-feedback role="status" aria-live="polite" {}
                }
            }
        }
    };
    let html = layout::app_document(&record.conversation.title, None, &body.into_string()).replace(
        "</head>",
        &format!("{}</head>", crate::chat_html::head().into_string()),
    );
    crate::chat_html::protect(Html(html).into_response())
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
    let body = html! { title {(record.conversation.title) " · OpenAgents"} (content(&record.conversation,None)) (ticket(&app,&record.conversation,true)) (sidebar(&app,&record.conversation).await) };
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
                "This message identity was already used for different text.",
            );
        }
        return accepted(&app, &headers, &loaded.conversation).await;
    }
    if selection != loaded.conversation.selection {
        return refusal(
            StatusCode::CONFLICT,
            "The source or runtime selection changed. Reload this chat before sending.",
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
            "This chat is full. Start another chat; the original messages remain available.",
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
            "Continue Cloud work through its current job controls.",
        );
    }
    let cloud = if selection.as_ref().is_some_and(|s| s.runtime.is_some()) {
        match crate::cloud::composer::stage(
            &app,
            &headers,
            &owner,
            &prompt.request_id,
            selection.as_ref().unwrap(),
            &text,
            previous,
        )
        .await
        {
            Ok(v) => Some(v),
            Err(r) => return r,
        }
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
        crate::chat_html::protect(
            html! { (ticket(app,chat,true)) (sidebar(app,chat).await) }.into_response(),
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
        let (text, done, failure) = {
            let r = basic_coder::lock(&reply);
            (
                r.text.clone(),
                r.done,
                r.failure.as_ref().map(|e| e.describe()),
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
                            "The answer's outcome is unknown. The request will not be submitted again automatically."
                        } else {
                            "We couldn't get an answer. Try asking again."
                        }.into(),
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

async fn sidebar(app: &App, chat: &Conversation) -> Markup {
    let chats = app.config.chat_store.list(&chat.owner).await;
    html! {
        aside #chat-sidebar.chat-sidebar hx-swap-oob="outerHTML" aria-label="Chats" {
            a.new-chat href="/" { "+ New chat" }
            h2 { "Chats" }
            nav aria-label="Your conversations" {
                @match chats {
                    Ok(chats)=> { @for row in chats {
                        a href=(format!("/chat/{}",row.id)) hx-get=(format!("/chat/{}/workspace",row.id)) hx-target="#chat-content" hx-swap="innerHTML" hx-sync="#chat-content:replace" aria-current=[(row.id==chat.id).then_some("page")] { (row.title) }
                    } p.dim { "Showing up to 256 recent chats." } }
                    Err(_)=> {p.error {"The chat list is unavailable. Your current conversation is retained."}}
                }
            }
            a href="/demo" { "Onboarding demo" }
        }
    }
}

pub(crate) fn ticket(app: &App, chat: &Conversation, oob: bool) -> Markup {
    let selection = chat.selection.clone().unwrap_or_default();
    html! { div #chat-ticket hx-swap-oob=[oob.then_some("outerHTML")] {
        input type="hidden" id="chat-selected" name="chat" value=(chat.id) form="chat-form";
        input type="hidden" name="request_id" value=(new_id()) form="chat-form";
        input type="hidden" name="csrf" value=(csrf(app,&chat.owner)) form="chat-form";

    } (crate::composer::state_field(app, &chat.owner, &selection, oob)) @if oob { (crate::composer::controls(&selection, true)) } }
}

fn content(chat: &Conversation, before: Option<usize>) -> Markup {
    html! {
        header.chat-heading { h1 {(chat.title)} div {a href=(format!("/chat/{}/transcript?before=24",chat.id)) hx-get=(format!("/chat/{}/transcript?before=24",chat.id)) hx-target="#chat-transcript" data-chat-history="start" {"Beginning"} a href=(format!("/chat/{}/transcript",chat.id)) hx-get=(format!("/chat/{}/transcript",chat.id)) hx-target="#chat-transcript" data-chat-history="end" {"Latest"}} }
        section #chat-thread.thread aria-label="Chat" {
            div.chat-column hx-ext="sse" sse-connect=(format!("/chat/{}/events?after={}",chat.id,chat.revision)) sse-close="retired" {
                div #chat-transcript sse-swap="transcript,retired" hx-swap="innerHTML" { (messages(chat,before)) }
            }
        }
    }
}

fn messages(chat: &Conversation, before: Option<usize>) -> Markup {
    let end = before
        .unwrap_or(chat.messages.len())
        .min(chat.messages.len());
    let start = end.saturating_sub(WINDOW);
    html! {
        input type="hidden" id="chat-history-window" value=(if before.is_some() {"older"}else{"latest"});
        @if start>0 {p.dim {"Showing messages " (start+1) "–" (end) " of " (chat.messages.len()) ". " a href=(format!("/chat/{}/transcript?before={start}",chat.id)) hx-get=(format!("/chat/{}/transcript?before={start}",chat.id)) hx-target="#chat-transcript" {"Read earlier messages"}}}
        @if before.is_some() && end<chat.messages.len() {p {a href=(format!("/chat/{}/transcript?before={}",chat.id,(end+WINDOW).min(chat.messages.len()))) hx-get=(format!("/chat/{}/transcript?before={}",chat.id,(end+WINDOW).min(chat.messages.len()))) hx-target="#chat-transcript" {"Read newer messages"} " · " a href=(format!("/chat/{}/transcript",chat.id)) hx-get=(format!("/chat/{}/transcript",chat.id)) hx-target="#chat-transcript" data-chat-history="end" {"Latest"}}}
        @for (index,message) in chat.messages[start..end].iter().enumerate() {
            article.chat-message id=(format!("chat-message-{}",index+start)) {
                h2 { (match message.role {Role::User=>"You",Role::Assistant=>"OpenAgents",Role::Tool=>"Status"}) }
                @if message.role==Role::Assistant {div.md {(PreEscaped(crate::markdown::render(&message.text)))}}
                @else {p.chat-plain {(message.text)}}
            }
        }
        p #chat-status role="status" aria-live="polite" {
            @if chat.pending.is_some() {"OpenAgents is answering…"}
            @else if chat.requests.last().is_some_and(|r|r.outcome==Outcome::Unknown) {"The previous request's outcome is unknown. It will not be repeated automatically."}
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
            crate::chat_html::protect(messages(&v.conversation, window.before).into_response())
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
            "The original offset is outside this message.",
        );
    }
    let mut end = (offset + 64 * 1024).min(bytes.len());
    if !message.text.is_char_boundary(offset) {
        return refusal(
            StatusCode::BAD_REQUEST,
            "Choose a UTF-8 character boundary for this offset.",
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
                    "This event cursor belongs to another conversation or is invalid.",
                );
            }
        },
    };
    if cursor > loaded.conversation.revision {
        return refusal(
            StatusCode::CONFLICT,
            "This event cursor is ahead of the retained conversation. Reload the chat.",
        );
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
                    let body=html! { @if missed>0 {p.dim {"Resumed from the retained snapshot; " (missed) " intermediate projections were superseded. All original messages remain available."}} (messages(&v.conversation,None)) }.into_string();
                    cursor = revision;
                    Event::default()
                        .id(format!("{id}:{revision}"))
                        .event("transcript")
                        .data(body)
                }
                Ok(_) => Event::default().comment("current"),
                Err(_) => {
                    ticks = 299;
                    Event::default().id(format!("{id}:{cursor}")).event("retired").data("<p class=\"error\">The conversation is unavailable. Reopen it to check access.</p>")
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
    crate::chat_html::protect((status, html! {p.error role="alert" {(text)}}).into_response())
}
fn unavailable(error: Error) -> Response {
    eprintln!("openagents-web: conversation storage: {error}");
    refusal(
        StatusCode::SERVICE_UNAVAILABLE,
        "The conversation store is unavailable. Your message was not repeated; try again with the same ticket.",
    )
}
/// A 24-pixel lucide icon path set, drawn at `size` with a 2-pixel stroke.
fn icon(size: u8, paths: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" \
viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" \
stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\" focusable=\"false\">\
{paths}</svg>"
    )
}

const CHEVRON_DOWN: &str = "<path d=\"m6 9 6 6 6-6\"/>";
const PLUS: &str = "<path d=\"M5 12h14\"/><path d=\"M12 5v14\"/>";
const MIC: &str = "<path d=\"M12 19v3\"/><path d=\"M19 10v2a7 7 0 0 1-14 0v-2\"/>\
<rect x=\"9\" y=\"2\" width=\"6\" height=\"13\" rx=\"3\"/>";
const ARROW_UP: &str = "<path d=\"m5 12 7-7 7 7\"/><path d=\"M12 19V5\"/>";

/// The homepage and chat share source/runtime controls and a stable text box.
pub(crate) fn composer(action: &str, label: &str, selection: Option<&Selection>) -> String {
    let transport = if action.starts_with("/chat/") {
        format!(
            " hx-post=\"{action}\" hx-swap=\"none\" hx-disabled-elt=\"find button[type=submit]\" hx-sync=\"this:drop\""
        )
    } else {
        String::new()
    };
    let selection = selection.cloned().unwrap_or_default();
    let pickers = crate::composer::controls(&selection, false).into_string();
    let round = "tw:inline-flex tw:items-center tw:justify-center tw:size-6 tw:shrink-0 \
tw:rounded-full tw:p-0";
    let quiet = "tw:bg-noir-surface-raised tw:text-noir-content-secondary \
tw:hover:bg-noir-stroke-subtle tw:hover:text-noir-content tw:active:bg-noir-stroke";
    format!(
        "<section class=\"composer tw:w-full tw:max-w-[640px]\" aria-label=\"{label}\">\
<form id=\"chat-form\" action=\"{action}\" method=\"post\"{transport}>\
<div class=\"tw:flex tw:items-center tw:gap-2 tw:min-h-8 tw:px-1.5 tw:pb-1.5\">{pickers}</div>\
<div id=\"chat-card\" class=\"chat-composer-card tw:relative tw:flex tw:flex-col tw:overflow-hidden \
tw:w-full tw:h-[155px] tw:rounded-xl tw:cursor-text tw:border tw:border-noir-stroke-subtle \
tw:bg-noir-surface-subtle tw:focus-within:border-noir-stroke\">\
<label class=\"unseen\" for=\"chat-input\">Message</label>\
<textarea id=\"chat-input\" name=\"q\" rows=\"2\" maxlength=\"{MAX_CHARS}\" required autofocus \
placeholder=\"Ask OpenAgents to build, fix bugs, explore\" \
class=\"tw:block tw:flex-1 tw:w-full tw:min-h-[32px] tw:max-h-[360px] tw:m-0 tw:px-3 tw:py-3 \
tw:border-0 tw:bg-transparent tw:resize-none tw:font-mono tw:text-sm \
tw:text-noir-content tw:placeholder:text-noir-content-secondary tw:outline-none \
tw:focus-visible:outline-none\"></textarea>\
<div class=\"tw:flex tw:items-center tw:gap-3 tw:px-3 tw:py-3\">\
<button type=\"button\" aria-label=\"Add context and tools\" \
hx-get=\"/composer/context\" hx-include=\"#composer-state,#chat-selected,[name=csrf][form=chat-form]\" hx-target=\"#composer-panel\" hx-swap=\"innerHTML\" hx-sync=\"#composer-panel:replace\" class=\"{round} {quiet}\">{plus}</button>\
<button type=\"button\" title=\"Model\" hx-get=\"/composer/model\" hx-include=\"#composer-state,#chat-selected,[name=csrf][form=chat-form]\" hx-target=\"#composer-panel\" hx-swap=\"innerHTML\" hx-sync=\"#composer-panel:replace\" \
class=\"tw:inline-flex tw:items-center tw:gap-1 tw:h-6 tw:pl-2 tw:pr-1.5 tw:rounded-full \
tw:bg-transparent tw:text-xs tw:text-noir-content-secondary tw:hover:bg-noir-surface-raised \
tw:hover:text-noir-content tw:active:bg-noir-stroke-subtle\">Auto{chevron}</button>\
<div class=\"tw:flex-1\"></div>\
<button type=\"button\" aria-label=\"Voice input\" hx-get=\"/composer/voice\" hx-include=\"#composer-state,#chat-selected,[name=csrf][form=chat-form]\" hx-target=\"#composer-panel\" hx-swap=\"innerHTML\" hx-sync=\"#composer-panel:replace\" \
title=\"Voice input availability\" class=\"{round} {quiet}\">{mic}</button>\
<button type=\"submit\" aria-label=\"Send\" title=\"Send\" class=\"{round} \
tw:bg-noir-accent-solid tw:text-noir-on-accent-solid tw:hover:bg-noir-content-secondary \
tw:active:bg-noir-content-tertiary\">{arrow}</button>\
</div></div></form></section>",
        plus = icon(14, PLUS),
        chevron = icon(12, CHEVRON_DOWN),
        mic = icon(14, MIC),
        arrow = icon(16, ARROW_UP),
    )
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
