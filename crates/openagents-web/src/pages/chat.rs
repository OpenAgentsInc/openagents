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
use openagents_ui::actions::{Button, ButtonType, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
// Disabled until the composer's context, model and voice controls do
// something (see `composer`):
// use openagents_ui::shell::{ComposerAction, ModelPickerTrigger};
use openagents_ui::shell::{
    Breadcrumb, ChatList, ChatStatus, Composer, Message as ThreadMessage, ScrollToBottom,
    composer_panel_host,
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
                .post(send_new),
        )
        .route("/chat/{id}", get(show).post(send_follow))
        .route("/chat/{id}/workspace", get(workspace))
        .route("/chat/{id}/delete", get(confirm_delete).post(delete))
        .route("/chat/{id}/transcript", get(transcript))
        .route("/chat/{id}/events", get(events))
        .route("/chat/{id}/messages/{index}/original", get(original))
        .merge(sidebar::routes())
        .merge(delete_all::routes())
        .merge(live::routes())
        .merge(work::routes())
        .merge(agents::routes())
        .merge(continued::routes())
        .merge(approval::routes())
        .merge(computer::routes())
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[derive(Deserialize)]
struct Prompt {
    q: String,
    request_id: String,
    csrf: String,
    #[serde(default)]
    selection: String,
    /// The project picked in the composer's selector row (`prj_…`), if
    /// any ([`crate::composer_row`]).
    /// Absent when the composer has no selector row.
    #[serde(default)]
    project: Option<String>,
    /// The branch of that project picked there.
    #[serde(default)]
    branch: Option<String>,
    /// Where the message runs there (empty: answered here).
    #[serde(default)]
    target: Option<String>,
    /// The files added in the composer, their ids joined by commas
    /// ([`crate::chat_files`]). Empty for none.
    #[serde(default)]
    files: String,
}

impl Prompt {
    fn wanted(&self) -> crate::composer_row::Wanted {
        crate::composer_row::Wanted {
            project: self.project.clone().unwrap_or_default(),
            branch: self.branch.clone().unwrap_or_default(),
            target: self.target.clone().unwrap_or_default(),
            chat: None,
            focus: None,
        }
    }
}

#[derive(Deserialize)]
struct Removal {
    csrf: String,
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

/// The owner a page shows chats for, and a fresh visitor id when the
/// browser has none yet (signed in, the account's; see
/// [`crate::chat_owner`]). The fresh id goes into the cookie.
pub(crate) async fn visitor(app: &App, headers: &HeaderMap) -> (String, Option<String>) {
    match crate::chat_owner::who(app, headers).await.reader() {
        Some(owner) => (owner.to_owned(), None),
        None => (crate::ask::new_visitor(), Some(String::new())),
    }
}

/// The owner whose chats this request may show; see [`crate::chat_owner`].
pub(crate) async fn reader(app: &App, headers: &HeaderMap) -> Option<String> {
    crate::chat_owner::who(app, headers)
        .await
        .reader()
        .map(str::to_owned)
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

pub(crate) async fn validate_form(
    app: &App,
    headers: &HeaderMap,
    supplied: &str,
) -> Result<String, Response> {
    let owner = match crate::chat_owner::who(app, headers).await {
        crate::chat_owner::Who::Unchecked(_) => {
            return Err(refusal(
                StatusCode::SERVICE_UNAVAILABLE,
                "We couldn't check your sign-in. Try again in a minute.",
            ));
        }
        who => match who.writer() {
            Some(owner) => owner.to_owned(),
            None => {
                return Err(refusal(
                    StatusCode::FORBIDDEN,
                    "Open the homepage before sending a message.",
                ));
            }
        },
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

async fn command(
    app: &App,
    headers: &HeaderMap,
    prompt: &Prompt,
) -> Result<(String, String), Response> {
    let owner = validate_form(app, headers, &prompt.csrf).await?;
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
    let (owner, text) = match command(&app, &headers, &prompt).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = prompt.request_id.clone();
    let selection = match selected(&app, &owner, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    // A new chat's files were added under its id, the request's
    // ([`crate::chat_files`]).
    let files =
        match crate::chat_files::take(&app.config.chat_store, &owner, &id, &prompt.files).await {
            Ok(files) => files,
            Err(message) => return refusal(StatusCode::BAD_REQUEST, message),
        };
    let digest = request_digest(
        &crate::chat_files::with_files(&text, &files),
        selection.as_ref(),
    );
    match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(record))
            if record
                .conversation
                .requests
                .first()
                .is_some_and(|r| r.digest == digest) =>
        {
            return to_chat(&headers, &id);
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
    let picked =
        match crate::composer_row::checked(&app, &headers, &owner, &prompt.wanted(), true).await {
            Ok(picked) => picked,
            Err(message) => return refusal(StatusCode::CONFLICT, message),
        };
    let project = picked.project.as_ref().map(|project| project.id.clone());
    match &picked.target {
        crate::composer_row::Target::Chat => {}
        crate::composer_row::Target::Coder(_) if !files.is_empty() => {
            return refusal(StatusCode::BAD_REQUEST, crate::chat_files::NOT_TO_CODER);
        }
        crate::composer_row::Target::Coder(computer) => {
            return start_on_coder(&app, &owner, computer, &id, &text, project).await;
        }
        crate::composer_row::Target::Claude(_) => {
            return start_claude(&app, &headers, &owner, &id, text, digest, &picked, files).await;
        }
    }
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
                // The same new chat sent twice (a double click): the first
                // send holds the answer and is saving the chat, so this
                // one goes to it too.
                if saved_soon(&app, &owner, &id, &id, &digest).await.is_some() {
                    return to_chat(&headers, &id);
                }
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
            files,
            reply: None,
        }],
        selection,
        updated_unix: now(),
        pinned_unix: None,
        archived_unix: None,
        project,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: picked.branch.clone(),
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
        let repo = repo_read(
            &app,
            &headers,
            picked.project.as_ref(),
            picked.branch.as_deref(),
        )
        .await;
        spawn_answer(app.clone(), loaded, admitted_at, repo);
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

/// The chat `id` once it holds the message `request` with `digest`, if
/// that happens within two seconds: a second send of the same message (a
/// double click) waits for the first send to save it, then shows it.
async fn saved_soon(
    app: &App,
    owner: &str,
    id: &str,
    request: &str,
    digest: &str,
) -> Option<Loaded> {
    for _ in 0..20 {
        if let Ok(Some(record)) = app.config.chat_store.load(owner, id).await {
            match record
                .conversation
                .requests
                .iter()
                .find(|r| r.id == request)
            {
                Some(found) if found.digest == digest => return Some(record),
                Some(_) => return None,
                None => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// A new chat whose first message goes to Coder on `computer`
/// ([`crate::coder_sync::start_from_web`]); on to its page.
async fn start_on_coder(
    app: &App,
    owner: &str,
    computer: &str,
    request: &str,
    text: &str,
    project: Option<String>,
) -> Response {
    use crate::coder_sync::Started;
    let store = &app.config.chat_store;
    match crate::coder_sync::start_from_web(store, owner, computer, request, text, project).await {
        Ok(Started::Started(id)) => {
            crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
        }
        Ok(Started::Offline) => refusal(
            StatusCode::CONFLICT,
            &format!("Coder on {computer} isn't online now. Pick where it runs again."),
        ),
        Ok(Started::Full) => refusal(
            StatusCode::CONFLICT,
            "Your account has no room for more chats. Delete some first.",
        ),
        Ok(Started::Busy) => refusal(
            StatusCode::CONFLICT,
            "Coder has too many messages waiting. Try again when it has answered them.",
        ),
        Ok(Started::Secret) => refusal(
            StatusCode::BAD_REQUEST,
            "This looks like it holds a password or key, so it wasn't sent.",
        ),
        Err(e) => unavailable(e),
    }
}

/// A new chat whose first message starts Claude Code in the picked
/// project's environment ([`work::begin`]); the run's answer joins the
/// chat when it is done. On to its page. Files sent with the message go
/// in the computer's working directory ([`crate::chat_files::for_run`]).
#[allow(clippy::too_many_arguments)]
async fn start_claude(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
    text: String,
    digest: String,
    picked: &crate::composer_row::Picked,
    files: Vec<crate::chat_files::FileRef>,
) -> Response {
    let attached = match crate::chat_files::for_run(&app.config.chat_store, owner, id, &files).await
    {
        Ok(attached) => attached,
        Err(message) => return refusal(StatusCode::BAD_REQUEST, message),
    };
    let Some(env) = claude_environment(app, headers, owner, picked, true).await else {
        return refusal(
            StatusCode::CONFLICT,
            "Claude Code can't run there now. Pick where it runs again.",
        );
    };
    // Issues the message names go to the briefed agent (#11258).
    let named = work::issues_named(&text, Some(&env.repository));
    let working = if named.is_empty() {
        None
    } else {
        match work::begin_work(app, headers, owner, &named, "chat").await {
            Ok(tasks) => Some(tasks),
            Err(message) => return refusal(StatusCode::CONFLICT, &message),
        }
    };
    let (environment, task) = if working.is_some() {
        (None, None)
    } else {
        match work::begin(
            app,
            headers,
            &env,
            picked.branch.as_deref(),
            &[],
            &text,
            attached,
        )
        .await
        {
            Ok((environment, task)) => (Some(environment), Some(task)),
            Err(message) => return refusal(StatusCode::CONFLICT, &message),
        }
    };
    let mut record = Conversation {
        id: id.to_owned(),
        owner: owner.to_owned(),
        revision: 1,
        title: text.chars().take(64).collect(),
        messages: vec![Message {
            role: Role::User,
            text,
            request_id: Some(id.to_owned()),
        }],
        pending: None,
        requests: vec![Request {
            id: id.to_owned(),
            digest,
            outcome: Outcome::Answered,
            selection: None,
            cloud: None,
            files,
            reply: None,
        }],
        selection: None,
        updated_unix: now(),
        pinned_unix: None,
        archived_unix: None,
        project: picked.project.as_ref().map(|project| project.id.clone()),
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: picked.branch.clone(),
    };
    if let (Some(environment), Some(task)) = (environment, task.clone()) {
        work::record(&mut record, environment, task);
    }
    if let Some(tasks) = working {
        work::record_work(&mut record, tasks);
    }
    match app.config.chat_store.create(&record).await {
        Ok(_) => {}
        Err(Error::Conflict) => {
            if let Some(task) = &task {
                work::abandon(app, task);
            }
            return crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response());
        }
        Err(e) => {
            if let Some(task) = &task {
                work::abandon(app, task);
            }
            return unavailable(e);
        }
    }
    work::watch(app.clone(), owner.to_owned(), id.to_owned());
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

/// The environment the picked Claude Code target runs in, checked again.
async fn claude_environment(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    picked: &crate::composer_row::Picked,
    new_chat: bool,
) -> Option<crate::composer_row::Environment> {
    let crate::composer_row::Target::Claude(id) = &picked.target else {
        return None;
    };
    let choices = crate::composer_row::choices(app, headers, owner, new_chat).await?;
    let env = choices.environment(picked.project.as_ref()?)?;
    (env.id == *id).then(|| env.clone())
}

pub(crate) async fn load(app: &App, headers: &HeaderMap, id: &str) -> Result<Loaded, Response> {
    let Some(owner) = reader(app, headers).await else {
        return Err(missing());
    };
    load_owned(app, &owner, id).await
}

/// [`load`] for an owner already resolved: a chat's live updates resolve
/// the owner once, not every second.
async fn load_owned(app: &App, owner: &str, id: &str) -> Result<Loaded, Response> {
    if !valid_id(id) {
        return Err(missing());
    }
    let owner = owner.to_owned();
    let mut loaded = app
        .config
        .chat_store
        .load(&owner, id)
        .await
        .map_err(unavailable)?
        .filter(|loaded| !loaded.conversation.deleted())
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
    // A running task's new state is written before anything shows the chat,
    // and showing it clears a waiting Done.
    Ok(work::mark_opened(app, work::sync(app, loaded).await).await)
}

async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    show_page(&app, &headers, &id, None).await
}

/// The chat's page; with `notice`, a refused message shown in the composer
/// with its text back in the box (a plain-form submit that was refused).
async fn show_page(app: &App, headers: &HeaderMap, id: &str, notice: Option<&Notice>) -> Response {
    let (app, headers, id) = (app.clone(), headers.clone(), id.to_owned());
    let record = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let chat = &record.conversation;
    if let Some(terminal) = &chat.terminal {
        return show_terminal(&app, &headers, chat, &terminal.computer, notice).await;
    }
    let row = crate::composer_row::for_chat(&app, &headers, chat, false).await;
    let dock = html! {
        (ticket(&app, chat, false))
        p #chat-feedback.oa-composer-feedback role="status" aria-live="polite" {}
    };
    let chips = crate::suggestions::reply_chips(&app, chat).await;
    let offer = work::offer(&app, &headers, chat).await;
    let links = work::links(&app, &headers).await;
    let page = UiPage::new(chat.title.clone())
        .path(format!("/chat/{id}"))
        .app()
        .breadcrumb(work::breadcrumb(chat, offer.as_ref()))
        .actions(work::actions(chat, offer.as_ref(), false))
        .head(crate::chat_html::head())
        .boosted()
        .sidebar_section(chat_list(&app, &chat.owner, Some(&chat.id), true, false).await)
        .content(html! {
            // The thread is private: HTMX never snapshots it into history.
            div #chat-content.oa-thread-view hx-history="false" { (content(chat, None, chips, links)) }
        })
        .composer(composer_with(
            &format!("/chat/{id}"),
            "Continue this chat",
            Some(row),
            dock,
            notice,
        ));
    crate::chat_html::protect(page.respond(&headers))
}

/// A chat synced from Coder (#11047): the transcript, and a composer
/// whose replies Coder on that computer takes and answers (#11048) while
/// it is online ([`crate::coder_sync::online`]); otherwise one line saying
/// where it runs, and, when the chat's project has a saved environment,
/// Continue on a Cloud computer (#11050, [`continued`]). Rows are plain
/// links here, since a web chat can't load into this composer.
async fn show_terminal(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    computer: &str,
    notice: Option<&Notice>,
) -> Response {
    let id = &chat.id;
    let online = continued::online(app, chat).await;
    let dock = if online {
        // A question Coder waits on there (a deploy, a merge) shows above
        // the composer with Approve and Deny (#11170).
        let card = approval::current(app, chat).await;
        html! {
            (card)
            (terminal_composer(app, chat, computer, notice))
        }
    } else if continued::offer(app, headers, chat, online).await.is_some() {
        html! { (terminal_note(computer)) (continued::button(chat)) }
    } else {
        terminal_note(computer)
    };
    let links = work::links(app, headers).await;
    let page = UiPage::new(chat.title.clone())
        .path(format!("/chat/{id}"))
        .app()
        .breadcrumb(Breadcrumb::new(chat.title.clone()))
        .head(crate::chat_html::head())
        .boosted()
        .sidebar_section(chat_list(app, &chat.owner, Some(id.as_str()), false, false).await)
        .content(html! {
            div #chat-content.oa-thread-view hx-history="false" { (content(chat, None, html! {}, links)) }
        })
        .composer(dock);
    crate::chat_html::protect(page.respond(headers))
}

/// What a Coder chat shows where the composer would be when Coder on its
/// computer isn't online.
pub(crate) fn terminal_note(computer: &str) -> Markup {
    html! {
        p.oa-thread-notice #chat-terminal-note {
            "This chat runs in Coder on " (computer) ". To reply here, open Coder there and type "
            code { "/sync on" } "."
        }
    }
}

/// The composer on a Coder chat while Coder on its computer is online: a
/// reply waits for Coder there, which answers it with that computer's
/// tools.
fn terminal_composer(
    app: &App,
    chat: &Conversation,
    computer: &str,
    notice: Option<&Notice>,
) -> Markup {
    let composer = Composer::new("chat-form", format!("/chat/{}", chat.id))
        .label("Reply in Coder")
        .enhanced(true)
        .input_id("chat-input")
        .body_id("chat-card")
        .max_chars(MAX_CHARS)
        .placeholder(format!("Reply to Coder on {computer}"))
        .autofocus(true)
        .after(html! {
            (ticket(app, chat, false))
            p #chat-feedback.oa-composer-feedback role="status" aria-live="polite" {}
        });
    match notice {
        Some(notice) => composer
            .draft(notice.draft.clone())
            .status(html! { (notice.text) })
            .render(),
        None => composer.render(),
    }
}

/// A reply sent from a Coder chat's page: it waits for Coder on the
/// computer ([`crate::coder_sync::queue_reply`]), and the thread's live
/// stream shows it, then Coder's answer.
async fn reply_to_coder(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
    request_id: &str,
    text: &str,
) -> Response {
    use crate::coder_sync::Queued;
    let store = &app.config.chat_store;
    match crate::coder_sync::queue_reply(store, owner, id, request_id, text).await {
        Ok(Queued::Queued) => {}
        Ok(Queued::Offline(computer)) => {
            return refusal(
                StatusCode::CONFLICT,
                &format!("Coder on {computer} isn't online now. Open Coder there to reply."),
            );
        }
        Ok(Queued::Full) => {
            return refusal(
                StatusCode::CONFLICT,
                "Wait for Coder to answer your earlier replies.",
            );
        }
        Ok(Queued::Secret) => {
            return refusal(
                StatusCode::BAD_REQUEST,
                "This looks like it holds a password or key, so it wasn't sent.",
            );
        }
        Ok(Queued::Missing) => return missing(),
        Err(e) => return unavailable(e),
    }
    let chat = match store.load(owner, id).await {
        Ok(Some(loaded)) => loaded.conversation,
        Ok(None) => return missing(),
        Err(e) => return unavailable(e),
    };
    if headers.get("HX-Request").is_some_and(|v| v == "true") {
        crate::chat_html::protect(
            html! { (ticket(app, &chat, true)) (chat_list(app, owner, Some(id), false, true).await) }
                .into_response(),
        )
    } else {
        crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
    }
}

async fn workspace(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let record = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    // A Coder chat has no composer to load into: open it as a page.
    if record.conversation.terminal.is_some()
        || record
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
    let row = crate::composer_row::for_chat(&app, &headers, chat, true).await;
    let chips = crate::suggestions::reply_chips(&app, chat).await;
    let offer = work::offer(&app, &headers, chat).await;
    let links = work::links(&app, &headers).await;
    let body = html! { title {(chat.title) " · OpenAgents"} (work::breadcrumb(chat, offer.as_ref()).swap_oob(true)) (work::actions(chat, offer.as_ref(), true)) (content(chat,None,chips,links)) (ticket(&app,chat,true)) (row) (chat_list(&app,&chat.owner,Some(&chat.id),true,true).await) };
    let mut response = crate::chat_html::protect(body.into_response());
    response.headers_mut().insert(
        "HX-Push-Url",
        HeaderValue::from_str(&format!("/chat/{id}")).expect("UUID URL"),
    );
    response
}

/// The visitor's own chat as stored, without the checks that only matter
/// for showing it: a chat that can no longer be opened can still be
/// deleted.
async fn stored(app: &App, owner: &str, id: &str) -> Result<Loaded, Response> {
    if !valid_id(id) {
        return Err(missing());
    }
    app.config
        .chat_store
        .load(owner, id)
        .await
        .map_err(unavailable)?
        .filter(|loaded| !loaded.conversation.deleted())
        .ok_or_else(missing)
}

/// The confirm step: one sentence, Delete, and Cancel. It works without
/// scripts; the chat is removed only by the form's POST.
async fn confirm_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    let record = match stored(&app, &owner, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let chat = &record.conversation;
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            p { "Delete this chat? This can't be undone." }
            @if let Some(terminal) = &chat.terminal {
                p { "It's deleted in Coder on " (terminal.computer) " too." }
            }
        }))
        form method="post" action=(format!("/chat/{id}/delete")) {
            input type="hidden" name="csrf" value=(csrf(&app, &owner));
            div.oa-page-actions {
                (Button::new("Delete")
                    .kind(ButtonType::Submit)
                    .color(Color::Danger))
                (crate::ui_page::action_link("Cancel", &format!("/chat/{id}")))
            }
        }
        // Every chat at once (#11038): its own confirm step.
        (MarkdownRoot::new(html! { p { a href=(delete_all::PATH) { "Delete all chats instead" } } }))
    });
    let page = UiPage::new(chat.title.clone())
        .path(format!("/chat/{id}/delete"))
        .breadcrumb(Breadcrumb::new(chat.title.clone()))
        .sidebar_section(chat_list(&app, &owner, Some(&chat.id), false, false).await)
        .content(content);
    crate::chat_html::protect(page.respond(&headers))
}

/// Remove the chat from the store for good and go to a new chat. An
/// answer still being written is waited for, so it can't be lost halfway.
async fn delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(removal): Form<Removal>,
) -> Response {
    let owner = match validate_form(&app, &headers, &removal.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let record = match stored(&app, &owner, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let stale = match &record.conversation.pending {
        Some(pending) if now().saturating_sub(pending.started_unix) <= LEASE_SECONDS => {
            return not_deleted(
                &id,
                StatusCode::CONFLICT,
                "Wait for the answer to finish, then delete this chat.",
            );
        }
        Some(pending) => Some(pending.request_id.clone()),
        None => None,
    };
    // A Coder chat is deleted in Coder too, the next time it checks
    // (`crate::coder_sync`).
    match app.config.chat_store.remove(&record).await {
        Ok(_) => {}
        Err(Error::Conflict) => {
            return not_deleted(
                &id,
                StatusCode::CONFLICT,
                "This chat just changed. Try again.",
            );
        }
        Err(e) => {
            eprintln!("openagents-web: conversation storage: {e}");
            return not_deleted(
                &id,
                StatusCode::SERVICE_UNAVAILABLE,
                "We couldn't delete this chat right now. Try again.",
            );
        }
    }
    if let Some(request_id) = stale {
        let _ = app.config.chat_store.release(&owner, &request_id).await;
    }
    crate::chat_html::protect(Redirect::to("/").into_response())
}

/// The confirm form posts without scripts, so a refusal is a whole page
/// with a way back to the chat.
fn not_deleted(id: &str, status: StatusCode, text: &str) -> Response {
    crate::chat_html::protect(problem(
        status,
        "Chat not deleted",
        text,
        (&format!("/chat/{id}"), "Back to the chat"),
    ))
}

async fn follow(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(prompt): Form<Prompt>,
) -> Response {
    let (owner, text) = match command(&app, &headers, &prompt).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let loaded = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if loaded.conversation.terminal.is_some() {
        if !prompt.files.trim().is_empty() {
            return refusal(StatusCode::BAD_REQUEST, crate::chat_files::CODER_CHAT);
        }
        return reply_to_coder(&app, &headers, &owner, &id, &prompt.request_id, &text).await;
    }
    let selection = match selected(&app, &owner, &prompt) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let files =
        match crate::chat_files::take(&app.config.chat_store, &owner, &id, &prompt.files).await {
            Ok(files) => files,
            Err(message) => return refusal(StatusCode::BAD_REQUEST, message),
        };
    let hash = request_digest(
        &crate::chat_files::with_files(&text, &files),
        selection.as_ref(),
    );
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
    let picked =
        match crate::composer_row::checked(&app, &headers, &owner, &prompt.wanted(), false).await {
            Ok(picked) => picked,
            Err(message) => return refusal(StatusCode::CONFLICT, message),
        };
    // The row's project and branch, recorded when the composer has the row.
    let place = prompt.project.is_some().then(|| {
        (
            picked.project.as_ref().map(|project| project.id.clone()),
            picked.branch.clone(),
        )
    });
    if matches!(picked.target, crate::composer_row::Target::Claude(_)) {
        return follow_claude(
            &app,
            &headers,
            loaded,
            &prompt.request_id,
            text,
            hash,
            &picked,
            place,
            files,
        )
        .await;
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
                if let Some(saved) = saved_soon(&app, &owner, &id, &prompt.request_id, &hash).await
                {
                    return accepted(&app, &headers, &saved.conversation).await;
                }
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
    if let Some((project, branch)) = place {
        next.project = project;
        next.branch = branch;
    }
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
        files,
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
            if matches!(e, Error::Conflict) {
                // The same message sent twice (a double click): the other
                // send saved it first, so this one shows it too.
                let sent = next.requests.last().map(|r| r.digest.as_str());
                if let Some(saved) =
                    saved_soon(&app, &owner, &id, &prompt.request_id, sent.unwrap_or("")).await
                {
                    return accepted(&app, &headers, &saved.conversation).await;
                }
                return refusal(StatusCode::CONFLICT, "This chat just changed. Try again.");
            }
            return unavailable(e);
        }
    };
    if cloud.is_none() {
        // The chat's project: the row's, else the one the chat is in.
        let sidebar = match &picked.project {
            Some(_) => None,
            None => crate::projects::sidebar(&app).await,
        };
        let project = picked.project.as_ref().or_else(|| {
            let id = loaded.conversation.project.as_deref()?;
            sidebar.as_deref()?.project(id)
        });
        let branch = picked
            .branch
            .as_deref()
            .or(loaded.conversation.branch.as_deref());
        let repo = repo_read(&app, &headers, project, branch).await;
        spawn_answer(app.clone(), loaded.clone(), admitted_at, repo);
    }
    accepted(&app, &headers, &loaded.conversation).await
}

/// What became of a message an app (the phone, #11107) sent to a web chat.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AppSent {
    /// OpenAgents is answering it (or already answered this request).
    Answering,
    /// The chat can't take a message now; the words say why.
    Busy(&'static str),
    /// The text is empty or too long.
    Invalid,
    /// No such chat for this owner.
    Missing,
    Unavailable,
}

/// A follow-up sent from an app with its own token (#11107): the web's
/// [`follow`] without the composer's form or selector row. The chat keeps
/// its repository and environment; the same `request_id` is taken once.
pub(crate) async fn follow_from_app(
    app: &App,
    owner: &str,
    id: &str,
    request_id: &str,
    text: &str,
    files: Vec<crate::chat_files::FileRef>,
) -> AppSent {
    let Some(text) = normalize(text) else {
        return AppSent::Invalid;
    };
    if !valid_id(request_id) || !valid_id(id) {
        return AppSent::Invalid;
    }
    let store = &app.config.chat_store;
    let loaded = match store.load(owner, id).await {
        Ok(Some(loaded)) => loaded,
        Ok(None) => return AppSent::Missing,
        Err(e) => {
            eprintln!("openagents-web: conversation storage: {e}");
            return AppSent::Unavailable;
        }
    };
    let chat = &loaded.conversation;
    if chat.terminal.is_some() || chat.deleted() {
        return AppSent::Missing;
    }
    let selection = chat.selection.clone();
    let hash = request_digest(
        &crate::chat_files::with_files(&text, &files),
        selection.as_ref(),
    );
    if let Some(request) = chat.requests.iter().find(|r| r.id == request_id) {
        return if request.digest == hash {
            AppSent::Answering
        } else {
            AppSent::Busy("This message was already sent with different text.")
        };
    }
    if chat.requests.iter().any(|r| r.cloud.is_some())
        || selection.as_ref().is_some_and(|s| s.runtime.is_some())
        || work::running(chat)
    {
        return AppSent::Busy("Continue this chat on openagents.com.");
    }
    if chat.pending.is_some() {
        return AppSent::Busy("OpenAgents is still answering your previous message.");
    }
    if chat.messages.len() + 2 > MAX_MESSAGES {
        return AppSent::Busy("This chat is full. Start a new chat to keep going.");
    }
    let admitted_at = now();
    match store
        .claim(owner, request_id, admitted_at + LEASE_SECONDS)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return if saved_soon(app, owner, id, request_id, &hash)
                .await
                .is_some()
            {
                AppSent::Answering
            } else {
                AppSent::Busy("OpenAgents is still answering your previous message.")
            };
        }
        Err(e) => {
            eprintln!("openagents-web: conversation storage: {e}");
            return AppSent::Unavailable;
        }
    }
    let mut next = chat.clone();
    next.revision += 1;
    next.updated_unix = now();
    next.messages.push(Message {
        role: Role::User,
        text,
        request_id: Some(request_id.to_owned()),
    });
    next.messages.push(Message {
        role: Role::Assistant,
        text: String::new(),
        request_id: Some(request_id.to_owned()),
    });
    next.pending = Some(Pending {
        request_id: request_id.to_owned(),
        started_unix: now(),
        job_id: None,
    });
    next.requests.push(Request {
        id: request_id.to_owned(),
        digest: hash.clone(),
        outcome: Outcome::Pending,
        selection,
        cloud: None,
        files,
        reply: None,
    });
    match store.compare_and_swap(&loaded, &next).await {
        Ok(saved) => {
            // An app's own token reaches no GitHub connection here.
            spawn_answer(app.clone(), saved, admitted_at, None);
            AppSent::Answering
        }
        Err(e) => {
            let _ = store.release(owner, request_id).await;
            if matches!(e, Error::Conflict)
                && saved_soon(app, owner, id, request_id, &hash)
                    .await
                    .is_some()
            {
                return AppSent::Answering;
            }
            if matches!(e, Error::Conflict) {
                return AppSent::Busy("This chat just changed. Try again.");
            }
            eprintln!("openagents-web: conversation storage: {e}");
            AppSent::Unavailable
        }
    }
}

/// A message on a web chat that starts Claude Code in the picked
/// project's environment: it joins the chat with the run as a task after
/// it, in one write ([`work::begin`]).
#[allow(clippy::too_many_arguments)]
async fn follow_claude(
    app: &App,
    headers: &HeaderMap,
    loaded: Loaded,
    request_id: &str,
    text: String,
    digest: String,
    picked: &crate::composer_row::Picked,
    place: Option<(Option<String>, Option<String>)>,
    files: Vec<crate::chat_files::FileRef>,
) -> Response {
    let chat = &loaded.conversation;
    if work::running(chat) {
        return refusal(
            StatusCode::CONFLICT,
            "Claude Code is still working in this chat. Wait for it to finish.",
        );
    }
    // Files sent with the message go in the computer's working directory
    // ([`crate::chat_files::for_run`], #11174).
    let attached =
        match crate::chat_files::for_run(&app.config.chat_store, &chat.owner, &chat.id, &files)
            .await
        {
            Ok(attached) => attached,
            Err(message) => return refusal(StatusCode::BAD_REQUEST, message),
        };
    let Some(env) = claude_environment(app, headers, &chat.owner, picked, false).await else {
        return refusal(
            StatusCode::CONFLICT,
            "Claude Code can't run there now. Pick where it runs again.",
        );
    };
    // Issues the message names go to the briefed agent (#11258).
    let named = work::issues_named(&text, Some(&env.repository));
    let working = if named.is_empty() {
        None
    } else {
        match work::begin_work(app, headers, &chat.owner, &named, "chat").await {
            Ok(tasks) => Some(tasks),
            Err(message) => return refusal(StatusCode::CONFLICT, &message),
        }
    };
    let (environment, task) = if working.is_some() {
        (None, None)
    } else {
        match work::begin(
            app,
            headers,
            &env,
            picked.branch.as_deref(),
            &chat.messages,
            &text,
            attached,
        )
        .await
        {
            Ok((environment, task)) => (Some(environment), Some(task)),
            Err(message) => return refusal(StatusCode::CONFLICT, &message),
        }
    };
    let mut next = chat.clone();
    next.revision += 1;
    if let Some((project, branch)) = place {
        next.project = project;
        next.branch = branch;
    }
    next.messages.push(Message {
        role: Role::User,
        text,
        request_id: Some(request_id.to_owned()),
    });
    next.requests.push(Request {
        id: request_id.to_owned(),
        digest,
        outcome: Outcome::Answered,
        selection: chat.selection.clone(),
        cloud: None,
        files,
        reply: None,
    });
    if let (Some(environment), Some(task)) = (environment, task.clone()) {
        work::record(&mut next, environment, task);
    }
    if let Some(tasks) = working {
        work::record_work(&mut next, tasks);
    }
    let saved = match app.config.chat_store.compare_and_swap(&loaded, &next).await {
        Ok(saved) => saved,
        Err(e) => {
            if let Some(task) = &task {
                work::abandon(app, task);
            }
            return unavailable(e);
        }
    };
    work::watch(app.clone(), chat.owner.clone(), chat.id.clone());
    accepted(app, headers, &saved.conversation).await
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
            html! {
                (ticket(app,chat,true)) (chat_list(app,&chat.owner,Some(&chat.id),true,true).await)
                div #chat-form-status.oa-composer-status role="status" aria-live="polite" hx-swap-oob="true" {}
            }
                .into_response(),
        )
    } else {
        crate::chat_html::protect(Redirect::to(&format!("/chat/{}", chat.id)).into_response())
    }
}

fn spawn_answer(
    app: App,
    loaded: Loaded,
    admitted_at: u64,
    repo: Option<crate::repo_snapshot::RepoRead>,
) {
    tokio::spawn(async move {
        answer(app, loaded, admitted_at, repo).await;
    });
}

/// How to read the chat's project's repository for this turn
/// ([`crate::repo_snapshot`]): its `owner/name` and branch, with the
/// signed-in person's GitHub token, fetched for this turn only. `None`
/// outside a project, or for a private repository with no connection.
async fn repo_read(
    app: &App,
    headers: &HeaderMap,
    project: Option<&oa_auth::repos::Project>,
    branch: Option<&str>,
) -> Option<crate::repo_snapshot::RepoRead> {
    let project = project?;
    let token = match app.config.cloud.as_deref() {
        Some(service) => service.github_token(headers).await.ok(),
        None => None,
    };
    if project.private && token.is_none() {
        return None;
    }
    let base = app.config.github.as_ref().map_or_else(
        || "https://api.github.com".to_string(),
        |github| github.endpoints.api_url.trim_end_matches('/').to_string(),
    );
    Some(crate::repo_snapshot::RepoRead {
        base,
        token,
        repository: project.repository.clone(),
        branch: branch
            .filter(|branch| !branch.is_empty())
            .unwrap_or(&project.default_branch)
            .to_string(),
        private: project.private,
    })
}

async fn answer(
    app: App,
    mut loaded: Loaded,
    admitted_at: u64,
    repo: Option<crate::repo_snapshot::RepoRead>,
) {
    let chat = &loaded.conversation;
    let owner = chat.owner.clone();
    let request_id = chat
        .pending
        .as_ref()
        .expect("dispatch owns pending request")
        .request_id
        .clone();
    // The files sent with this message (#11174): text files read as data;
    // images and PDFs as content parts when this server has a model that
    // takes them ([`crate::chat_vision`]), else a line naming each one.
    let sent = chat
        .requests
        .iter()
        .find(|r| r.id == request_id)
        .map(|r| r.files.clone())
        .unwrap_or_default();
    let store = &app.config.chat_store;
    let vision = sent
        .iter()
        .any(|file| file.kind != crate::chat_files::Kind::Text)
        .then(|| crate::chat_vision::doors(&app))
        .flatten();
    let parts = match vision {
        Some(_) => crate::chat_vision::parts(store, &owner, &chat.id, &sent).await,
        None => Vec::new(),
    };
    let opened: Vec<&str> = parts.iter().map(|part| part.id.as_str()).collect();
    let attached = crate::chat_files::for_answer(store, &owner, &chat.id, &sent).await;
    let seen = if parts.is_empty() {
        attached.clone()
    } else {
        crate::chat_files::for_model(
            store,
            &owner,
            &chat.id,
            &sent,
            crate::chat_files::ANSWER_BYTES,
            &opened,
        )
        .await
    };
    let turns_with = |attached: &str| -> Vec<Turn> {
        chat.messages
            .iter()
            .filter(|m| m.role != Role::Tool && !m.text.is_empty())
            .rev()
            .take(crate::ask::MAX_TURNS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|m| {
                if m.role == Role::User {
                    let mut text = m
                        .text
                        .chars()
                        .take(crate::ask::MAX_TURN_CHARS)
                        .collect::<String>();
                    if m.request_id.as_deref() == Some(request_id.as_str()) {
                        text.push_str(attached);
                    }
                    Turn::user(text)
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
            .collect()
    };
    let turns = turns_with(&seen);
    let fallback_turns = turns_with(&attached);
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
    // The person's memory notes from their account (#11182), so the
    // answer knows what Coder knows; none for a visitor not signed in.
    let memory = if door.is_some() {
        crate::account_memory::chat_notes(&app.config.chat_store, &owner).await
    } else {
        Vec::new()
    };
    // In a project with Google Drive sources, Gemini answers from them,
    // or hands the message to the hosted chat (#11238).
    let drive = match (&door, parts.is_empty()) {
        (Some(_), true) => {
            crate::connections::chat::door(&app, &owner, chat.project.as_deref()).await
        }
        _ => None,
    };
    // Images or PDFs go to the door that takes them; the hosted chat
    // answers with the words only when it can't (#11174).
    let door: Option<Box<dyn openagents_chat::basic_coder::Door>> = match (door, vision) {
        (Some(door), Some(doors)) if !parts.is_empty() => {
            Some(Box::new(crate::chat_vision::VisionDoor {
                doors,
                parts,
                fallback: Some((door, fallback_turns)),
            }))
        }
        (Some(door), _) => match drive {
            Some(mut drive) => {
                drive.fallback = Some(door);
                Some(Box::new(drive))
            }
            None => Some(door),
        },
        (None, _) => None,
    };
    // The chat's project's repository, read for this turn with the
    // person's GitHub connection, so a question about it is answered from
    // it. A slow or failed read costs the turn its repository, never its
    // answer.
    let repository = match (&door, repo) {
        (Some(_), Some(repo)) => tokio::time::timeout(Duration::from_secs(8), repo.read())
            .await
            .ok()
            .flatten(),
        _ => None,
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
                    memory,
                    repository,
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
                // What the chips under the answer read (`crate::suggestions`),
                // and how it was served: its tier and route, which the
                // reply's marker carries for the chat goldens
                // (docs/web/chat-goldens.md).
                r.done
                    .then(|| {
                        let mut meta = openagents_chat::suggestions::chip_meta(&r.meta);
                        meta.tier = r.meta.tier.clone();
                        meta.route = r.meta.route.clone();
                        meta.switched = r.meta.switched.clone();
                        meta
                    })
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
                    if ended {
                        // A count only (#11153): never the chat or its text.
                        let outcome = if done {
                            "answer_shown"
                        } else {
                            "answer_failed"
                        };
                        app.config.analytics.event(outcome, "");
                    }
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

/// The visitor's chats in the left panel: a search box, the Pinned group in
/// pin order, then the rest newest first (the store sorts by last update),
/// `current` marked, archived chats left out (see [`sidebar`]). On a chat
/// page (`hx`) a row also loads its conversation into `#chat-content`;
/// elsewhere rows are plain links. Responses that change the list carry it
/// again with `oob`, replacing `#chat-sidebar` in place. An unavailable
/// store leaves the list empty rather than showing an error in the sidebar.
///
/// A whole page (not `oob`) also gets the tab's live stream beside the list
/// ([`live::connector`]), which keeps row statuses current; an `oob`
/// replacement leaves that connection alone.
pub(crate) async fn chat_list(
    app: &App,
    owner: &str,
    current: Option<&str>,
    hx: bool,
    oob: bool,
) -> Markup {
    let view = sidebar::View {
        current,
        hx,
        ..sidebar::View::default()
    };
    let drawn = now();
    let (list, working) = sidebar::render_working(app, owner, view, oob).await;
    html! {
        (list)
        @if !oob { (live::connector_at(drawn, working.iter().map(String::as_str), current, hx)) }
    }
}

/// A chat row's second line: the repository and branch it was started
/// with, then its environment and version ("Environment v3"). Inside a
/// project's group (`repository` false) the repository is left out: the
/// group's heading names it. The list is the owner's own, so the names
/// never reach anyone else.
pub(crate) fn line_two(chat: &Conversation, repository: bool) -> Option<String> {
    // A Coder chat says so, and on which computer (#11047); a chat synced
    // from the phone says Phone (#11107).
    if let Some(terminal) = &chat.terminal {
        let surface = if crate::phone_api::phone_session(&terminal.session) {
            "Phone"
        } else {
            "Terminal"
        };
        return Some(format!("{surface} · {}", terminal.computer));
    }
    let mut parts = Vec::new();
    if let Some(source) = chat.selection.as_ref().and_then(|s| s.repository.as_ref()) {
        if repository {
            parts.push(source.repository.clone());
        }
        if !source.branch.is_empty() {
            parts.push(source.branch.clone());
        }
    } else if let Some(branch) = &chat.branch {
        // The branch picked in the composer's selector row; the project
        // group names the repository.
        parts.push(branch.clone());
    }
    parts.extend(work::detail(chat));
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A chat row's status: "Working" while an answer runs, "Failed" when the
/// last one did not finish, and nothing otherwise (see `docs/web/sidebar.md`).
/// The row's status in a slot the live stream can replace (`oob`), so the
/// spinner clears the moment the answer lands.
pub(crate) fn row_status_slot(chat: &Conversation, oob: bool) -> Markup {
    html! {
        span id=(format!("chat-row-status-{}", chat.id)) class="oa-chat-row-status"
            hx-swap-oob=[oob.then_some("true")] {
            @if let Some(status) = row_status(chat) { (status) }
        }
    }
}

/// Working also while a task started from the chat runs, Failed when the
/// newest task failed and nothing was sent since, and Done when a long task
/// finished and the chat wasn't opened since ([`work`]).
fn row_status(chat: &Conversation) -> Option<ChatStatus> {
    if chat.working() || work::running(chat) {
        return Some(ChatStatus::Working);
    }
    if work::failed(chat) {
        return Some(ChatStatus::Failed);
    }
    if work::unseen_done(chat) {
        return Some(ChatStatus::Done);
    }
    match chat.requests.last()?.outcome {
        Outcome::Failed => Some(ChatStatus::Failed),
        _ => None,
    }
}

/// The chat's hidden composer fields. With `oob` they replace the page's
/// copies.
pub(crate) fn ticket(app: &App, chat: &Conversation, oob: bool) -> Markup {
    let selection = chat.selection.clone().unwrap_or_default();
    html! { div #chat-ticket hx-swap-oob=[oob.then_some("outerHTML")] {
        input type="hidden" id="chat-selected" name="chat" value=(chat.id) form="chat-form";
        input type="hidden" name="request_id" value=(new_id()) form="chat-form";
        input type="hidden" name="csrf" value=(csrf(app,&chat.owner)) form="chat-form";

    } (crate::composer::state_field(app, &chat.owner, &selection, oob)) }
}

/// The thread, its suggestion chips, and the scroll-to-bottom button over
/// it. The title is the header row's breadcrumb, not part of the thread.
/// `links` links task rows to their runs ([`work::links`]).
fn content(chat: &Conversation, before: Option<usize>, chips: Markup, links: bool) -> Markup {
    html! {
        section #chat-thread.oa-thread aria-label="Chat" {
            div.oa-thread-column hx-ext="sse" sse-connect=(format!("/chat/{}/events?after={}",chat.id,chat.revision)) sse-close="retired" {
                div #chat-transcript sse-swap="transcript,retired" hx-swap="innerHTML" { (messages(chat,before,links)) (chips) }
            }
        }
        (ScrollToBottom::new("#chat-thread"))
    }
}

/// One stored message as a thread turn. Assistant text is rendered Markdown
/// (the renderer escapes it), followed by the plugin cards its answer came
/// with (`plugins`, `docs/web/plugin-card.md`); user and status text is
/// escaped as written. A reply sits between two hidden markers: the first
/// names how it was served (`reply`: its tier, route, and prepared
/// answer, once answered), so the chat goldens can read a reply from the
/// page exactly as a person gets it (docs/web/chat-goldens.md).
/// A reply still `streaming` shows only the part that renders cleanly so
/// far, and grows smoothly to each new render (#11112).
fn turn(
    message: &Message,
    index: usize,
    plugins: &[String],
    reply: Option<&openagents_chat::router::Meta>,
    streaming: bool,
    signed_in: bool,
) -> ThreadMessage {
    // A chat an account owns is read signed in; an answer's buttons for
    // the other case are left out.
    let reader = crate::markdown::Reader {
        signed_in: Some(signed_in),
        id: format!("chat-message-{index}"),
    };
    match message.role {
        Role::User => ThreadMessage::user(&message.text),
        Role::Assistant => ThreadMessage::assistant(html! {
            span hidden data-oa-reply=(index)
                data-oa-tier=[reply.and_then(|r| r.tier.as_deref())]
                data-oa-route=[reply.and_then(|r| r.route.as_deref())]
                data-oa-answer=[reply.and_then(|r| r.answer.as_deref())] {}
            @if streaming {
                (MarkdownRoot::new(PreEscaped(crate::markdown::render_streaming_for(&message.text, &reader))).streaming(true))
            } @else {
                (MarkdownRoot::new(PreEscaped(crate::markdown::render_reply_for(&message.text, &reader))))
            }
            (crate::suggestions::plugin_cards(plugins))
            span hidden data-oa-reply-end {}
            // The first model provider missed this turn and another model
            // answered it (#11132): one quiet line, outside the reply's
            // markers so the chat goldens read the reply alone.
            @if let Some(switched) = reply.and_then(|r| r.switched.as_ref()) {
                p.oa-thread-notice data-oa-switched=(switched.provider.word()) { (switched.line()) }
            }
        })
        .author("OpenAgents"),
        Role::Tool => ThreadMessage::status(&message.text),
    }
    .id(format!("chat-message-{index}"))
}

/// Whether `message` is the reply still being written for the pending
/// request.
fn streaming(chat: &Conversation, message: &Message) -> bool {
    message.role == Role::Assistant
        && chat
            .pending
            .as_ref()
            .is_some_and(|p| message.request_id.as_deref() == Some(p.request_id.as_str()))
}

/// The messages in the window, each followed by the tasks started after it
/// ([`work::rows`]).
fn messages(chat: &Conversation, before: Option<usize>, links: bool) -> Markup {
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
        @if start == 0 { (work::rows(chat, 0, links)) }
        @for (index,message) in chat.messages[start..end].iter().enumerate() {
            (turn(
                message,
                index + start,
                crate::suggestions::message_plugins(chat, message),
                crate::suggestions::message_reply(chat, message),
                streaming(chat, message),
                crate::chat_store::is_account_owner(&chat.owner),
            ))
            // The files sent with it (#11174).
            (crate::chat_files::shown(&chat.id, crate::chat_files::of_message(chat, message)))
            // A reply that proposed a change on GitHub: its confirm card (#11167).
            (crate::github_tools::thread_entry(&chat.id, index + start, crate::suggestions::message_reply(chat, message)))
            // A reply the router read as work on code, about an issue: "Work on this issue" (#11258).
            (crate::work_runs::thread_entry(&chat.id, index + start, crate::suggestions::message_reply(chat, message)))
            (work::rows(chat, index + start + 1, links))
        }
        // Replies sent here that Coder hasn't taken yet (#11048).
        @if let Some(terminal) = chat.terminal.as_ref().filter(|_| before.is_none()) {
            @for (index, reply) in terminal.replies.iter().enumerate() {
                (ThreadMessage::user(&reply.text).id(format!("chat-reply-{index}")))
            }
            // Screenshots and files asked for here (#11185).
            @for (index, ask) in terminal.asks.iter().enumerate() {
                (computer::turn(chat, &terminal.computer, index, ask))
            }
        }
        // The chat's agents (#11164): they load themselves.
        @if before.is_none() { (agents::slot(chat)) }
        div #chat-status.oa-thread-status role="status" aria-live="polite"
            data-oa-composer-busy=[(chat.working() || work::running(chat)).then_some("chat-form")] {
            @if chat.working() {(openagents_ui::actions::Busy::new("Working"))}
            @else if let Some(terminal) = chat.terminal.as_ref().filter(|t| !t.replies.is_empty()) {"Waiting for Coder on " (terminal.computer) "."}
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
            let links = work::links(&app, &headers).await;
            crate::chat_html::protect(
                html! { (messages(&v.conversation, window.before, links)) (chips) }.into_response(),
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
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    let loaded = match load_owned(&app, &owner, &id).await {
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
    let links = work::links(&app, &headers).await;
    let shutdown = app.config.shutdown.clone();
    let stream = futures_util::stream::unfold(
        (app, owner, id, cursor, 0u16),
        move |(app, owner, id, mut cursor, mut ticks)| async move {
            if ticks >= 300 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let event = match load_owned(&app, &owner, &id).await {
                Ok(v) if v.conversation.revision > cursor => {
                    let revision = v.conversation.revision;
                    let missed = revision.saturating_sub(cursor + 1);
                    let _ = missed; // A resume re-renders the full transcript; nothing to announce.
                    let chips = crate::suggestions::reply_chips(&app, &v.conversation).await;
                    let body = html! { (messages(&v.conversation,None,links)) (chips) (row_status_slot(&v.conversation, true)) }.into_string();
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
                (app, owner, id, cursor, ticks + 1),
            ))
        },
    );
    crate::chat_html::protect(
        Sse::new(shutdown.until(stream))
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
/// A refusal: a short line, carried on the response ([`Refused`]) so the
/// send handlers ([`send_new`], [`send_follow`]) can show it in the
/// composer instead of as a page of its own ([`shown`]).
fn refusal(status: StatusCode, text: &str) -> Response {
    let mut response = crate::chat_html::protect(
        (status, html! {p.oa-thread-error role="alert" {(text)}}).into_response(),
    );
    response.extensions_mut().insert(Refused(text.to_owned()));
    response
}

/// Why a message was refused, on the refusal's response.
#[derive(Clone)]
struct Refused(String);

/// A refused message shown in the composer: why, and the text put back in
/// the box.
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) draft: String,
}

/// The response header that marks an HTMX send's refusal: the shell
/// script lets HTMX swap it (out of band, into the composer's status) and
/// keeps the draft in the box.
pub(crate) const REFUSED_HEADER: &str = "x-openagents-refused";

fn hx_request(headers: &HeaderMap) -> bool {
    headers.get("HX-Request").is_some_and(|v| v == "true")
}

/// On to the chat's page: a redirect the browser (or a boosted HTMX
/// request) follows, or `HX-Redirect` for a plain HTMX request.
fn to_chat(headers: &HeaderMap, id: &str) -> Response {
    let boosted = headers.get("HX-Boosted").is_some_and(|v| v == "true");
    if hx_request(headers) && !boosted {
        let mut response = crate::chat_html::protect(StatusCode::OK.into_response());
        response.headers_mut().insert(
            "HX-Redirect",
            HeaderValue::from_str(&format!("/chat/{id}")).expect("UUID URL"),
        );
        return response;
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

/// A message sent from the homepage composer ([`start`]); a refusal shows
/// in the composer ([`shown`]).
async fn send_new(
    State(app): State<App>,
    headers: HeaderMap,
    Form(prompt): Form<Prompt>,
) -> Response {
    let draft = prompt.q.clone();
    let project = prompt.project.clone();
    let response = start(State(app.clone()), headers.clone(), Form(prompt)).await;
    shown(&app, &headers, response, None, draft, project).await
}

/// A message sent on a chat's page ([`follow`]); a refusal shows in the
/// composer ([`shown`]).
async fn send_follow(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(prompt): Form<Prompt>,
) -> Response {
    let draft = prompt.q.clone();
    let response = follow(
        State(app.clone()),
        headers.clone(),
        Path(id.clone()),
        Form(prompt),
    )
    .await;
    shown(&app, &headers, response, Some(&id), draft, None).await
}

/// A send's answer, with a refusal moved into the composer: for HTMX, a
/// line in the composer's status region (out of band; nothing else on the
/// page changes and the draft stays); without script, the whole page the
/// message was sent from, with the line under the composer and the text
/// back in the box. Never a page of bare text.
async fn shown(
    app: &App,
    headers: &HeaderMap,
    response: Response,
    chat: Option<&str>,
    draft: String,
    project: Option<String>,
) -> Response {
    let Some(Refused(text)) = response.extensions().get::<Refused>().cloned() else {
        return response;
    };
    let status = response.status();
    if hx_request(headers) {
        return inline_refusal(status, &text);
    }
    let notice = Notice { text, draft };
    let mut page = match chat {
        Some(id) => show_page(app, headers, id, Some(&notice)).await,
        None => super::home::page(app, headers, project.as_deref(), Some(&notice)).await,
    };
    if chat.is_some() && page.status() != StatusCode::OK {
        page = super::home::page(app, headers, None, Some(&notice)).await;
    }
    *page.status_mut() = status;
    page
}

/// The refusal line for an HTMX send, answered with the refusal's status:
/// it replaces the composer's status region out of band (the shell script
/// lets a response with [`REFUSED_HEADER`] swap), and the page stays as it
/// is (no main swap, no new URL).
fn inline_refusal(status: StatusCode, text: &str) -> Response {
    let mut response = crate::chat_html::protect(
        (
            status,
            html! {
                div #chat-form-status.oa-composer-status role="status" aria-live="polite"
                    hx-swap-oob="true" { (text) }
            },
        )
            .into_response(),
    );
    let headers = response.headers_mut();
    headers.insert("HX-Reswap", HeaderValue::from_static("none"));
    headers.insert("HX-Push-Url", HeaderValue::from_static("false"));
    headers.insert(REFUSED_HEADER, HeaderValue::from_static("1"));
    response
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
/// script), the selector row above it when there is one (`row`,
/// [`crate::composer_row`], replaced out of band as `#composer-row`),
/// the panel host they load into (`#composer-panel`), and `after` under the
/// form. The chat posts with HTMX and keeps the draft until the server
/// accepts it; the homepage posts a plain form and follows the redirect to
/// the new chat, with or without JavaScript.
pub(crate) fn composer(action: &str, label: &str, row: Option<Markup>, after: Markup) -> Markup {
    composer_with(action, label, row, after, None)
}

/// [`composer`] with a refused message's notice in its status region and
/// the text back in the box.
pub(crate) fn composer_with(
    action: &str,
    label: &str,
    row: Option<Markup>,
    after: Markup,
    notice: Option<&Notice>,
) -> Markup {
    let mut composer = Composer::new("chat-form", action)
        .label(label)
        .enhanced(action.starts_with("/chat/"))
        .input_id("chat-input")
        .body_id("chat-card")
        .max_chars(MAX_CHARS)
        .placeholder("Ask OpenAgents anything")
        .autofocus(true)
        // Images, PDFs, and text files: picked here, or pasted or dropped
        // on the text box (#11174, [`crate::chat_files`]).
        .leading(crate::chat_files::picker())
        .attachments(crate::chat_files::tray())
        // The "+" button stays off: it only opened a panel restating the
        // repository selector.
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
    if let Some(row) = row {
        composer = composer.selectors(row);
    }
    if let Some(notice) = notice {
        composer = composer
            .draft(notice.draft.clone())
            .status(html! { (notice.text) });
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

#[path = "chat_sidebar.rs"]
mod sidebar;

#[path = "chat_agents.rs"]
mod agents;
#[path = "chat_approval.rs"]
mod approval;
#[path = "chat_computer.rs"]
mod computer;
#[path = "chat_continued.rs"]
mod continued;
#[path = "chat_delete_all.rs"]
pub(crate) mod delete_all;
#[path = "chat_live.rs"]
mod live;
#[path = "chat_work.rs"]
mod work;

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
