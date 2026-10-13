//! Sharing a chat (read-only, by link).
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /chat/{id}/share` | The share step as a page (no script), or with `?part=dialog` as a dialog over the chat |
//! | `POST /chat/{id}/share` `csrf`, `shared=true\|false` | Share or stop sharing (the owner's signed-in browser) |
//! | `GET`/`POST /v1/threads/{id}/share` `{shared}` | The same for the apps' own token: `200 {shared, url}`, `404 unknown`, `409 not_shareable` |
//! | `GET /share/{token}` | The shared chat for anyone with the link: title and messages, nothing else; `404` once sharing stops |
//!
//! Only a signed-in account's chat can be shared; a chat kept by a
//! signed-out browser has no Share button. The link is
//! `/share/{token}`, 128 random bits, never the chat's id, so knowing a
//! chat's id doesn't open it and a new share after a stop gets a new link.
//!
//! What a share shows: the messages' words (the same thread view as the
//! chat page, read-only). It never shows the files sent with a message
//! (#11174) and never serves them: they stay behind the owner's sign-in.
//! A chat in a project whose files are set to only you (`only_me`,
//! #11471) can't be shared, and a share stops working if its project is
//! set to only you later.
//!
//! Storage: one small record per share, `shared-chats/{token}.json`
//! (`{owner, chat}`), and one beside the account's chats,
//! `chat-shares/{chat}.json` (`{token}`). A link opens only while both
//! agree and the chat exists, so stopping (or deleting the chat) ends it
//! even if one record lingers.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::actions::{
    Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize, CopyButton,
};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::icons::Icon;
use openagents_ui::overlays::Dialog;
use openagents_ui::shell::{AppShell, Breadcrumb, Document, MainMode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{csrf, missing, reader, valid_id, validate_form};
#[path = "chat_share_card.rs"]
mod card;
use crate::App;
use crate::chat_store::{Conversation, Error, Store, is_account_owner};

/// Where a shared chat is read.
pub(crate) const PUBLIC: &str = "/share";
/// The share dialog's id.
pub(crate) const DIALOG: &str = "chat-share-dialog";
const SCHEMA: &str = "openagents.web.chat-share.v1";
const ASK: &str = "Anyone with the link can read it. You can stop sharing any time.";
const SHARED: &str = "Anyone with the link can read this chat.";
const STOPPED: &str = "You stopped sharing. The link no longer works.";
const PRIVATE_PROJECT: &str = "This chat is in a project set to only you, so it can't be shared.";
const SIGN_IN: &str = "Log in to share a chat.";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/share", get(step).post(change))
        .route("/v1/threads/{id}/share", get(api_read).post(api_change))
        .route("/share/{token}", get(public_page))
        .route("/share/{token}/card/{file}", get(card_image))
}

/// The global record: which chat a link opens, and who shared it when,
/// as its preview picture names them.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    schema: String,
    owner: String,
    chat: String,
    #[serde(default)]
    shared_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

/// Who shares a chat, as the preview picture shows them: the account's
/// name (never an email address) and GitHub picture.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sharer {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

impl Sharer {
    /// The signed-in browser's account, when it can be read.
    async fn of(app: &App, headers: &HeaderMap) -> Self {
        let Some(service) = app.config.cloud.as_deref() else {
            return Self::default();
        };
        match service.authenticate(headers).await {
            Ok(viewer) => Self {
                name: Some(viewer.account_label.trim().to_owned())
                    .filter(|name| !name.is_empty() && !name.contains('@')),
                avatar_url: viewer.avatar_url,
            },
            Err(_) => Self::default(),
        }
    }

    /// The account an app's own token (`Authorization: Bearer sess_…`)
    /// signs in, read the way a browser's session is.
    async fn of_app(app: &App, headers: &HeaderMap) -> Self {
        let Some(token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|token| token.bytes().all(|b| b.is_ascii_graphic() && b != b';'))
        else {
            return Self::default();
        };
        let mut session = HeaderMap::new();
        if let Some(host) = headers.get(header::HOST) {
            session.insert(header::HOST, host.clone());
        }
        let Ok(cookie) = HeaderValue::from_str(&format!("oa_cloud_session={token}")) else {
            return Self::default();
        };
        session.insert(header::COOKIE, cookie);
        Self::of(app, &session).await
    }
}

/// The account's record: the chat's current link.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    schema: String,
    token: String,
}

fn link_key(token: &str) -> String {
    format!("shared-chats/{token}.json")
}

fn current_key(owner: &str, chat: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("chat-shares/{chat}.json"))
}

/// A share token: 32 lowercase hex characters (128 random bits).
fn valid_token(token: &str) -> bool {
    token.len() == 32
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn new_token() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The chat's link token while it is shared.
pub(crate) async fn token(store: &Store, owner: &str, chat: &str) -> Result<Option<String>, Error> {
    if !valid_id(chat) {
        return Ok(None);
    }
    let Some((bytes, _)) = store.read_key(&current_key(owner, chat)?).await? else {
        return Ok(None);
    };
    let current: Current =
        serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("The share is invalid."))?;
    Ok(valid_token(&current.token).then_some(current.token))
}

/// Why `chat` can't be shared, in the owner's words; `None` when it can.
pub(crate) async fn refusal(
    store: &Store,
    chat: &Conversation,
) -> Result<Option<&'static str>, Error> {
    if !is_account_owner(&chat.owner) {
        return Ok(Some(SIGN_IN));
    }
    if let Some(project) = &chat.project
        && crate::projects::files::protection(store, &chat.owner, project).await?
            == crate::projects::files::Protection::OnlyMe
    {
        return Ok(Some(PRIVATE_PROJECT));
    }
    Ok(None)
}

/// Share `chat`: its link token, the same one while it stays shared.
pub(crate) async fn share(
    store: &Store,
    owner: &str,
    chat: &str,
    sharer: &Sharer,
) -> Result<String, Error> {
    if let Some(token) = token(store, owner, chat).await? {
        return Ok(token);
    }
    let token = new_token();
    let link = serde_json::to_vec(&Link {
        schema: SCHEMA.into(),
        owner: owner.into(),
        chat: chat.into(),
        shared_unix: crate::chat_store::now_unix(),
        name: sharer.name.clone(),
        avatar_url: sharer.avatar_url.clone(),
    })
    .map_err(|_| Error::Invalid("The share is invalid."))?;
    store.write_key(&link_key(&token), link, None).await?;
    let current = serde_json::to_vec(&Current {
        schema: SCHEMA.into(),
        token: token.clone(),
    })
    .map_err(|_| Error::Invalid("The share is invalid."))?;
    match store
        .write_key(&current_key(owner, chat)?, current, None)
        .await
    {
        Ok(_) => Ok(token),
        Err(Error::Conflict) => {
            // Shared at the same moment from elsewhere: keep that link.
            remove_link(store, &token).await?;
            token_or_conflict(store, owner, chat).await
        }
        Err(error) => {
            let _ = remove_link(store, &token).await;
            Err(error)
        }
    }
}

async fn token_or_conflict(store: &Store, owner: &str, chat: &str) -> Result<String, Error> {
    token(store, owner, chat).await?.ok_or(Error::Conflict)
}

async fn remove_link(store: &Store, token: &str) -> Result<(), Error> {
    let key = link_key(token);
    if let Some((_, generation)) = store.read_key(&key).await? {
        match store.delete_key(&key, &generation).await {
            Ok(_) | Err(Error::Conflict) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Stop sharing `chat`: its link stops working at once.
pub(crate) async fn stop(store: &Store, owner: &str, chat: &str) -> Result<(), Error> {
    if !valid_id(chat) {
        return Ok(());
    }
    let key = current_key(owner, chat)?;
    let Some((bytes, generation)) = store.read_key(&key).await? else {
        return Ok(());
    };
    match store.delete_key(&key, &generation).await {
        Ok(_) | Err(Error::Conflict) => {}
        Err(error) => return Err(error),
    }
    if let Ok(current) = serde_json::from_slice::<Current>(&bytes)
        && valid_token(&current.token)
    {
        card::forget(&current.token);
        remove_link(store, &current.token).await?;
    }
    Ok(())
}

/// The chat a link opens, for anyone, and its record: `None` when there
/// is no such link, sharing stopped, the chat is gone, or it can no
/// longer be shared.
async fn public(store: &Store, token: &str) -> Result<Option<(Conversation, Link)>, Error> {
    if !valid_token(token) {
        return Ok(None);
    }
    let Some((bytes, _)) = store.read_key(&link_key(token)).await? else {
        return Ok(None);
    };
    let Ok(link) = serde_json::from_slice::<Link>(&bytes) else {
        return Ok(None);
    };
    if self::token(store, &link.owner, &link.chat)
        .await?
        .as_deref()
        != Some(token)
    {
        return Ok(None);
    }
    let Some(loaded) = store.load(&link.owner, &link.chat).await? else {
        return Ok(None);
    };
    let chat = loaded.conversation;
    if chat.deleted() || refusal(store, &chat).await?.is_some() {
        return Ok(None);
    }
    Ok(Some((chat, link)))
}

fn url(origin: &str, token: &str) -> String {
    format!("{origin}{PUBLIC}/{token}")
}

// ------------------------------------------------------------------ the button

/// The header's Share button: "Share", or "Shared" while the chat is
/// shared. It opens the share dialog over the chat; without script, the
/// share step's page. `oob` replaces the page's copy.
pub(crate) fn button(chat: &str, shared: bool, oob: bool) -> Markup {
    let page = format!("/chat/{chat}/share");
    html! {
        span #chat-share hx-swap-oob=[oob.then_some("outerHTML")] {
            (ButtonLink::new(if shared { "Shared" } else { "Share" }, &page)
                .color(Color::Secondary)
                .variant(ButtonVariant::Outline)
                .size(ControlSize::Sm)
                .pill(true)
                .icon_start(if shared { Icon::Globe } else { Icon::Share })
                .attr("hx-get", format!("{page}?part=dialog"))
                .attr("hx-target", "body")
                .attr("hx-swap", "beforeend")
                .attr("aria-haspopup", "dialog"))
        }
    }
}

/// Whether the header offers Share for `chat`, and whether it is shared:
/// `None` for a chat a signed-out browser keeps.
pub(crate) async fn offered(app: &App, chat: &Conversation) -> Option<bool> {
    if !is_account_owner(&chat.owner) {
        return None;
    }
    Some(matches!(
        token(&app.config.chat_store, &chat.owner, &chat.id).await,
        Ok(Some(_))
    ))
}

// ------------------------------------------------------------------ the step

/// What the share step shows.
enum Shown<'a> {
    /// Not shared: the question, and Share.
    Ask { note: Option<&'a str> },
    /// Shared: the link, Copy link, and Stop sharing.
    Shared { url: String },
    /// It can't be shared, and why.
    Refused(&'a str),
}

/// The share step's words and buttons. In the dialog, Cancel and Done
/// close it and the forms swap this panel; on the page they go back.
fn panel(chat: &str, csrf: &str, shown: &Shown<'_>, dialog: bool) -> Markup {
    let action = format!("/chat/{chat}/share");
    let close = |label: &str| {
        if dialog {
            html! {
                form method="dialog" {
                    (Button::new(label)
                        .kind(ButtonType::Submit)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
            }
        } else {
            html! {
                (ButtonLink::new(label, format!("/chat/{chat}"))
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary))
            }
        }
    };
    let form = |shared: bool, button: Button| {
        html! {
            form method="post" action=(action)
                hx-post=[dialog.then_some(action.as_str())]
                hx-target=[dialog.then_some("#chat-share-panel")]
                hx-swap=[dialog.then_some("innerHTML")] {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="shared" value=(if shared { "true" } else { "false" });
                (button.kind(ButtonType::Submit))
            }
        }
    };
    html! {
        @match shown {
            Shown::Ask { note } => {
                @if let Some(note) = note {
                    p.oa-share-note role="status" { (note) }
                }
                p { (ASK) }
                div.oa-dialog-footer {
                    (close("Cancel"))
                    (form(true, Button::new("Share")))
                }
            }
            Shown::Shared { url } => {
                p { (SHARED) }
                div.oa-share-link {
                    a #chat-share-url href=(url) target="_blank" rel="noopener noreferrer" { (url) }
                }
                div.oa-dialog-footer {
                    (form(false, Button::new("Stop sharing")
                        .variant(ButtonVariant::Soft)
                        .color(Color::Danger)))
                    (CopyButton::new(url.as_str())
                        .label("Copy link")
                        .copied_label("Link copied"))
                }
            }
            Shown::Refused(why) => {
                p { (why) }
                div.oa-dialog-footer { (close("OK")) }
            }
        }
    }
}

fn wrapped(chat: &str, csrf: &str, shown: &Shown<'_>, dialog: bool) -> Markup {
    html! { div #chat-share-panel { (panel(chat, csrf, shown, dialog)) } }
}

#[derive(Deserialize)]
struct Part {
    part: Option<String>,
}

/// What the step shows for `chat` now.
async fn current<'a>(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    note: Option<&'a str>,
) -> Result<Shown<'a>, Error> {
    let store = &app.config.chat_store;
    if let Some(why) = refusal(store, chat).await? {
        return Ok(Shown::Refused(why));
    }
    Ok(match token(store, &chat.owner, &chat.id).await? {
        Some(token) => Shown::Shared {
            url: url(&crate::traces::origin(app, headers), &token),
        },
        None => Shown::Ask { note },
    })
}

async fn owned_chat(app: &App, owner: &str, id: &str) -> Result<Conversation, Response> {
    if !valid_id(id) {
        return Err(missing());
    }
    match app.config.chat_store.load(owner, id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => Ok(loaded.conversation),
        Ok(_) => Err(missing()),
        Err(error) => Err(unavailable(&error)),
    }
}

fn unavailable(error: &Error) -> Response {
    eprintln!("openagents-web: chat share: {error}");
    crate::chat_html::protect(crate::layout::problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "Share",
        "Sharing isn't working right now. Try again in a minute.",
        ("/", "Home"),
    ))
}

/// `GET /chat/{id}/share`: the step as a dialog (`?part=dialog`) or a page.
async fn step(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(part): Query<Part>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    let chat = match owned_chat(&app, &owner, &id).await {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    let shown = match current(&app, &headers, &chat, None).await {
        Ok(shown) => shown,
        Err(error) => return unavailable(&error),
    };
    let csrf = csrf(&app, &owner);
    if part.part.as_deref() == Some("dialog") {
        let dialog = Dialog::new(DIALOG, "Share this chat", wrapped(&id, &csrf, &shown, true))
            .open_on_load(true);
        return crate::chat_html::protect(Html(html! { (dialog) }.into_string()).into_response());
    }
    page(&app, &headers, &chat, &csrf, &shown).await
}

async fn page(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    csrf: &str,
    shown: &Shown<'_>,
) -> Response {
    let sidebar = super::chat_list(app, &chat.owner, Some(&chat.id), false, false).await;
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! { h1 { "Share this chat" } }))
        (wrapped(&chat.id, csrf, shown, false))
    });
    let page = crate::ui_page::UiPage::new(chat.title.clone())
        .path(format!("/chat/{}/share", chat.id))
        .breadcrumb(Breadcrumb::new(chat.title.clone()))
        .sidebar_section(sidebar)
        .content(content);
    crate::chat_html::protect(page.respond(headers))
}

#[derive(Deserialize)]
struct Change {
    csrf: String,
    shared: String,
}

/// `POST /chat/{id}/share`: share or stop, from the owner's own page.
async fn change(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Change>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let chat = match owned_chat(&app, &owner, &id).await {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    let store = &app.config.chat_store;
    let wanted = form.shared == "true";
    let refused = match refusal(store, &chat).await {
        Ok(refused) => refused,
        Err(error) => return unavailable(&error),
    };
    let done = match (wanted, refused) {
        (true, Some(_)) => Ok(()),
        (true, None) => share(store, &owner, &id, &Sharer::of(&app, &headers).await)
            .await
            .map(|_| ()),
        (false, _) => stop(store, &owner, &id).await,
    };
    if let Err(error) = done {
        return unavailable(&error);
    }
    let note = (!wanted).then_some(STOPPED);
    let shown = match current(&app, &headers, &chat, note).await {
        Ok(shown) => shown,
        Err(error) => return unavailable(&error),
    };
    if headers.get("HX-Request").is_some_and(|v| v == "true") {
        let shared = matches!(shown, Shown::Shared { .. });
        let body = html! {
            (panel(&id, &form.csrf, &shown, true))
            (button(&id, shared, true))
        };
        return crate::chat_html::protect(Html(body.into_string()).into_response());
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}/share")).into_response())
}

// ------------------------------------------------------------------ the API

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiChange {
    shared: bool,
}

fn api_state(app: &App, headers: &HeaderMap, token: Option<&str>) -> Value {
    json!({
        "shared": token.is_some(),
        "url": token.map(|token| url(&crate::traces::origin(app, headers), token)),
    })
}

async fn api_chat(app: &App, headers: &HeaderMap, id: &str) -> Result<Conversation, Response> {
    let owner = crate::coder_sync::owner(app, headers).await?;
    match app.config.chat_store.load(&owner, id).await {
        Ok(Some(loaded)) if valid_id(id) && !loaded.conversation.deleted() => {
            Ok(loaded.conversation)
        }
        Ok(_) | Err(Error::Invalid(_)) => Err(crate::coder_sync::refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "No such chat.",
        )),
        Err(error) => Err(crate::coder_sync::stored(&error)),
    }
}

/// `GET /v1/threads/{id}/share`: `{shared, url}`.
async fn api_read(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let chat = match api_chat(&app, &headers, &id).await {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    match token(&app.config.chat_store, &chat.owner, &chat.id).await {
        Ok(token) => {
            crate::coder_sync::answer(StatusCode::OK, api_state(&app, &headers, token.as_deref()))
        }
        Err(error) => crate::coder_sync::stored(&error),
    }
}

/// `POST /v1/threads/{id}/share` `{shared}`: `{shared, url}`.
async fn api_change(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let chat = match api_chat(&app, &headers, &id).await {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<ApiChange>(&body) else {
        return crate::coder_sync::refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {\"shared\": true} or {\"shared\": false}.",
        );
    };
    let store = &app.config.chat_store;
    let result = if sent.shared {
        match refusal(store, &chat).await {
            Ok(Some(why)) => {
                return crate::coder_sync::refused(StatusCode::CONFLICT, "not_shareable", why);
            }
            Ok(None) => share(
                store,
                &chat.owner,
                &chat.id,
                &Sharer::of_app(&app, &headers).await,
            )
            .await
            .map(Some),
            Err(error) => Err(error),
        }
    } else {
        stop(store, &chat.owner, &chat.id).await.map(|()| None)
    };
    match result {
        Ok(token) => {
            crate::coder_sync::answer(StatusCode::OK, api_state(&app, &headers, token.as_deref()))
        }
        Err(error) => crate::coder_sync::stored(&error),
    }
}

// ------------------------------------------------------------------ the page

/// `GET /share/{token}`: the shared chat, for anyone with the link.
async fn public_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Response {
    match public(&app.config.chat_store, &token).await {
        Ok(Some((chat, link))) => {
            let card = card_for(&app, &chat, &link).await;
            let social = Social {
                origin: crate::traces::origin(&app, &headers),
                token: &token,
                card: &card,
            };
            shared_page(&headers, &chat, Some(&social))
        }
        Ok(None) => private(missing()),
        Err(error) => private(unavailable(&error)),
    }
}

/// `text` cut to at most `limit` characters at a word, with "…" when cut.
fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let cut: String = text.chars().take(limit).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!(
        "{}…",
        cut.trim_end_matches(|c: char| c.is_ascii_punctuation())
    )
}

/// What a share's preview picture shows: the title, the first question
/// and the start of the first answer (their words only), and the sharer.
async fn card_for(app: &App, chat: &Conversation, link: &Link) -> card::Card {
    use crate::chat_store::Role;
    let first = |role: Role| {
        chat.messages
            .iter()
            .find(|m| m.role == role && !m.text.trim().is_empty())
            .map(|m| m.text.as_str())
            .unwrap_or_default()
    };
    let ask = first(Role::User)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    card::Card {
        title: clip(chat.title.trim(), 140),
        ask: clip(&ask, 280),
        reply: clip(&card::plain(first(Role::Assistant)), 400),
        name: link.name.clone(),
        npub: crate::vault::npub(crate::vault::store(app), &link.owner).await,
        avatar_url: link.avatar_url.clone(),
        date: card::date(if link.shared_unix > 0 {
            link.shared_unix
        } else {
            chat.updated_unix
        }),
    }
}

/// A share's preview: where it lives and its picture.
pub(crate) struct Social<'a> {
    origin: String,
    token: &'a str,
    card: &'a card::Card,
}

impl Social<'_> {
    fn image(&self) -> String {
        format!(
            "{}{PUBLIC}/{}/card/{}.png",
            self.origin,
            self.token,
            self.card.digest()
        )
    }

    fn tags(&self) -> Markup {
        let url = url(&self.origin, self.token);
        let description = if self.card.reply.is_empty() {
            "A chat shared from OpenAgents.".to_owned()
        } else {
            clip(&self.card.reply, 200)
        };
        let alt = format!("{} — a chat shared from OpenAgents", self.card.title);
        let image = self.image();
        html! {
            meta name="description" content=(description);
            link rel="canonical" href=(url);
            meta property="og:type" content="article";
            meta property="og:site_name" content="OpenAgents";
            meta property="og:title" content=(self.card.title);
            meta property="og:description" content=(description);
            meta property="og:url" content=(url);
            meta property="og:image" content=(image);
            meta property="og:image:type" content="image/png";
            meta property="og:image:width" content=(card::WIDTH);
            meta property="og:image:height" content=(card::HEIGHT);
            meta property="og:image:alt" content=(alt);
            meta name="twitter:card" content="summary_large_image";
            meta name="twitter:title" content=(self.card.title);
            meta name="twitter:description" content=(description);
            meta name="twitter:image" content=(image);
            meta name="twitter:image:alt" content=(alt);
        }
    }
}

/// `GET /share/{token}/card/{digest}.png`: the share's preview picture,
/// while it is shared and the digest is its current one; `404` otherwise.
async fn card_image(
    State(app): State<App>,
    Path((token, file)): Path<(String, String)>,
) -> Response {
    let Some(digest) = file.strip_suffix(".png") else {
        return private(missing());
    };
    let (chat, link) = match public(&app.config.chat_store, &token).await {
        Ok(Some(found)) => found,
        Ok(None) => return private(missing()),
        Err(error) => return private(unavailable(&error)),
    };
    let card = card_for(&app, &chat, &link).await;
    if card.digest() != digest {
        return private(missing());
    }
    let key = format!("{token}:{digest}");
    let avatar = match &card.avatar_url {
        Some(url) => crate::account::picture(url, 128)
            .await
            .and_then(|(_, bytes)| card::avatar_png(&bytes)),
        None => None,
    };
    // A picture that couldn't be fetched now is drawn with initials but
    // neither kept nor cached long, so the next ask tries again.
    let whole = card.avatar_url.is_none() || avatar.is_some();
    let drawn = tokio::task::spawn_blocking(move || {
        let draw = || card::render(&card, avatar.as_deref()).ok();
        if whole {
            card::cached(&key, draw)
        } else {
            draw().map(std::sync::Arc::new)
        }
    })
    .await
    .ok()
    .flatten();
    let Some(png) = drawn else {
        return private(unavailable(&Error::Invalid(
            "The preview couldn't be drawn.",
        )));
    };
    let mut response = (*png).clone().into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if whole {
            "public, max-age=31536000, immutable"
        } else {
            "public, max-age=300"
        }),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    private(response)
}

/// A shared chat's page: the title and the messages, read-only, with
/// the OpenAgents mark home. No sidebar, composer, or controls. `social`
/// adds the preview tags (Open Graph and Twitter).
pub(crate) fn shared_page(
    headers: &HeaderMap,
    chat: &Conversation,
    social: Option<&Social<'_>>,
) -> Response {
    let shell = AppShell::new()
        .mode(MainMode::App)
        .breadcrumb(Breadcrumb::new(chat.title.clone()))
        .actions(html! { a.oa-wordmark.oa-share-mark href="/" { "OpenAgents" } })
        .content(html! {
            div.oa-thread-view {
                section.oa-thread aria-label="Shared chat" {
                    div.oa-thread-column { (super::shared_messages(chat)) }
                }
            }
        });
    let document = Document::new(chat.title.clone())
        .theme(crate::theme::from_headers(headers))
        .head(html! {
            link rel="icon" type="image/svg+xml" href="/favicon.svg";
            meta name="robots" content="noindex, nofollow";
            meta name="referrer" content="no-referrer";
            @if let Some(social) = social { (social.tags()) }
            (maud::PreEscaped(crate::theme::style_tag()))
            (maud::PreEscaped(crate::theme::script_tags()))
        })
        .body(shell);
    let response = crate::chat_html::protect(
        Html(maud::Render::render(&document).into_string()).into_response(),
    );
    private(response)
}

/// A share's answers name no address to search engines or other sites.
fn private(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        "X-Robots-Tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
