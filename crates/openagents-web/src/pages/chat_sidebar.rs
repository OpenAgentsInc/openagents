//! Organizing chats in the left panel (#11036, `docs/web/sidebar.md`
//! "Interactions"): a row's "…" menu with Pin / Unpin, Rename and Archive,
//! a Pinned group on top, search over titles and messages, and the Archived
//! page with Restore.
//!
//! Every change is a plain `POST` form with the chat's CSRF token: without
//! JavaScript it redirects back; with HTMX it answers with the list, which
//! replaces `#chat-sidebar` in place. Pins and archives live on the chat
//! record (`pinned_unix`, `archived_unix`), so they survive a reload.

use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, ControlSize};
use openagents_ui::icons::Icon;
use openagents_ui::shell::{ChatSearch, HxGet, NavItem, RowAction, RowMenu, RowRename};

use super::*;

/// The longest chat name a person can give.
pub(super) const TITLE_CHARS: usize = 120;

const LIST: &str = "/chat/list";
const ARCHIVED: &str = "/chat/archived";
const SIDEBAR: &str = "#chat-sidebar";
const ROWS: &str = "#chat-sidebar-rows";

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(LIST, get(list))
        .route(ARCHIVED, get(archived))
        .route("/chat/{id}/pin", post(pin))
        .route("/chat/{id}/archive", post(archive))
        .route("/chat/{id}/rename", get(rename_field).post(rename))
}

/// What the list shows: the open chat (rows then load with HTMX), the
/// search text, and the chat just archived (its Undo notice).
#[derive(Clone, Copy, Default)]
pub(super) struct View<'a> {
    pub current: Option<&'a str>,
    pub hx: bool,
    pub q: &'a str,
    pub archived: Option<&'a str>,
}

/// The list for `owner`; see [`super::chat_list`].
pub(super) async fn render(app: &App, owner: &str, view: View<'_>, oob: bool) -> ChatList {
    render_working(app, owner, view, oob).await.0
}

/// [`render`], and the ids of the listed chats drawn as Working (for the
/// live stream, `super::live`).
pub(super) async fn render_working(
    app: &App,
    owner: &str,
    view: View<'_>,
    oob: bool,
) -> (ChatList, Vec<String>) {
    let list = ChatList::new().id("chat-sidebar").swap_oob(oob);
    let rows = match app.config.chat_store.list(owner).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("openagents-web: chat list: {error}");
            return (list, Vec::new());
        }
    };
    let working = rows
        .iter()
        .filter(|chat| chat.archived_unix.is_none() && chat.pending.is_some())
        .map(|chat| chat.id.clone())
        .collect();
    (build(list, &rows, &csrf(app, owner), view), working)
}

/// The list from loaded rows (newest first, as the store sorts them).
pub(super) fn build(list: ChatList, rows: &[Conversation], csrf: &str, view: View<'_>) -> ChatList {
    let q = view.q.trim();
    let shown: Vec<&Conversation> = rows
        .iter()
        .filter(|chat| chat.archived_unix.is_none() && matches(chat, q))
        .collect();
    let mut pinned: Vec<&Conversation> = shown
        .iter()
        .copied()
        .filter(|chat| chat.pinned_unix.is_some())
        .collect();
    pinned.sort_by(|a, b| {
        a.pinned_unix
            .cmp(&b.pinned_unix)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut list = list
        .pinned(pinned.iter().map(|chat| row(chat, csrf, view)))
        .items(
            shown
                .iter()
                .filter(|chat| chat.pinned_unix.is_none())
                .map(|chat| row(chat, csrf, view)),
        );
    let open = rows.iter().any(|chat| chat.archived_unix.is_none());
    if open || !q.is_empty() {
        list = list.search(
            ChatSearch::new(LIST, ROWS)
                .value(q)
                .field("current", view.current.unwrap_or_default()),
        );
    }
    if !q.is_empty() {
        list = list.empty("No chats found");
    }
    if let Some(id) = view.archived {
        list = list.notice(html! {
            span { "Chat archived." }
            form method="post" action=(format!("/chat/{id}/archive"))
                hx-post=(format!("/chat/{id}/archive")) hx-target=(SIDEBAR) hx-swap="outerHTML" {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="current" value=(view.current.unwrap_or_default());
                input type="hidden" name="archived" value="0";
                button type="submit" { "Undo" }
            }
        });
    }
    if rows.iter().any(|chat| chat.archived_unix.is_some()) {
        list = list.after(html! {
            p.oa-chat-list-more { a.oa-chat-list-link href=(ARCHIVED) { "Archived chats" } }
        });
    }
    list
}

/// Whether a chat matches the search: its title as you type, and its
/// messages from two characters.
fn matches(chat: &Conversation, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let q = q.to_lowercase();
    chat.title.to_lowercase().contains(&q)
        || (q.chars().count() >= 2
            && chat
                .messages
                .iter()
                .any(|m| m.role != Role::Tool && m.text.to_lowercase().contains(&q)))
}

/// One chat row with its "…" menu.
fn row(chat: &Conversation, csrf: &str, view: View<'_>) -> NavItem {
    let id = &chat.id;
    let current = view.current.unwrap_or_default();
    let mut item = NavItem::new(chat.title.clone(), format!("/chat/{id}"))
        .current(view.current == Some(id.as_str()))
        .row_id(format!("chat-row-{id}"));
    if let Some(detail) = row_detail(chat) {
        item = item.detail(detail);
    }
    let working = chat.pending.is_some();
    item = item.trailing(row_status_slot(chat, false));
    if view.hx {
        item = item.hx(HxGet::new(format!("/chat/{id}/workspace"))
            .target("#chat-content")
            .swap("innerHTML")
            .sync("#chat-content:replace"));
    }
    let pinned = chat.pinned_unix.is_some();
    let pin = RowAction::post(
        if pinned { "Unpin" } else { "Pin" },
        format!("/chat/{id}/pin"),
    )
    .icon(if pinned { Icon::Unpin } else { Icon::Pin })
    .field("csrf", csrf)
    .field("current", current)
    .field("pinned", if pinned { "0" } else { "1" })
    .target(SIDEBAR)
    .swap("outerHTML");
    let rename = RowAction::get("Rename", format!("/chat/{id}/rename"))
        .icon(Icon::Pencil)
        .field("current", current)
        .target(format!("#chat-row-{id}"))
        .swap("outerHTML");
    let mut archive = RowAction::post("Archive", format!("/chat/{id}/archive"))
        .icon(Icon::Archive)
        .field("csrf", csrf)
        .field("current", current)
        .field("archived", "1")
        .target(SIDEBAR)
        .swap("outerHTML");
    if working {
        archive = archive.confirm("This chat is still working. Archive it anyway?");
    }
    item.menu(
        RowMenu::new(format!("chat-menu-{id}"), chat.title.clone())
            .action(pin)
            .action(rename)
            .action(archive)
            .action(RowAction::open("Delete", format!("/chat/{id}/delete")).icon(Icon::Trash)),
    )
}

#[derive(Default, Deserialize)]
struct Change {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    current: String,
    #[serde(default)]
    pinned: String,
    #[serde(default)]
    archived: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    back: String,
}

#[derive(Default, Deserialize)]
struct Search {
    #[serde(default)]
    current: String,
    #[serde(default)]
    q: String,
}

fn current(value: &str) -> Option<&str> {
    valid_id(value).then_some(value)
}

fn hx(headers: &HeaderMap) -> bool {
    headers.get("HX-Request").is_some_and(|v| v == "true")
}

/// Applies `change` to the chat, retrying when an answer wrote it at the
/// same moment. `change` returns false when there is nothing to do.
async fn update(
    app: &App,
    owner: &str,
    id: &str,
    change: impl Fn(&mut Conversation) -> bool,
) -> Result<Conversation, Response> {
    if !valid_id(id) {
        return Err(missing());
    }
    for _ in 0..4 {
        let loaded = app
            .config
            .chat_store
            .load(owner, id)
            .await
            .map_err(unavailable)?
            .ok_or_else(missing)?;
        let mut next = loaded.conversation.clone();
        if !change(&mut next) {
            return Ok(next);
        }
        next.revision += 1;
        match app.config.chat_store.compare_and_swap(&loaded, &next).await {
            Ok(saved) => return Ok(saved.conversation),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(unavailable(error)),
        }
    }
    Err(unavailable(Error::Conflict))
}

/// The answer to a change: the list for HTMX (with `extra` out of band),
/// else a redirect back to the page the form was on.
async fn changed(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    form: &Change,
    archived: Option<&str>,
    extra: Markup,
) -> Response {
    let current = current(&form.current);
    if hx(headers) {
        let view = View {
            current,
            hx: current.is_some(),
            q: "",
            archived,
        };
        let list = render(app, owner, view, false).await;
        return crate::chat_html::protect(html! { (list) (extra) }.into_response());
    }
    let back = if form.back == "archived" {
        ARCHIVED.to_owned()
    } else {
        current.map_or_else(|| "/".to_owned(), |id| format!("/chat/{id}"))
    };
    crate::chat_html::protect(Redirect::to(&back).into_response())
}

async fn pin(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Change>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let pin = form.pinned == "1";
    // Pin order: a new pin goes after every earlier one, even within a second.
    let stamp = if pin {
        let last = match app.config.chat_store.list(&owner).await {
            Ok(rows) => rows.iter().filter_map(|c| c.pinned_unix).max(),
            Err(error) => return unavailable(error),
        };
        now().max(last.map_or(0, |last| last + 1))
    } else {
        0
    };
    let result = update(&app, &owner, &id, |chat| {
        if pin == chat.pinned_unix.is_some() {
            return false;
        }
        chat.pinned_unix = pin.then_some(stamp);
        true
    })
    .await;
    match result {
        Ok(_) => changed(&app, &headers, &owner, &form, None, html! {}).await,
        Err(r) => r,
    }
}

async fn archive(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Change>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let archive = form.archived == "1";
    let stamp = now();
    let result = update(&app, &owner, &id, |chat| {
        if archive == chat.archived_unix.is_some() {
            return false;
        }
        chat.archived_unix = archive.then_some(stamp);
        true
    })
    .await;
    match result {
        Ok(_) => {
            let notice = archive.then_some(id.as_str());
            changed(&app, &headers, &owner, &form, notice, html! {}).await
        }
        Err(r) => r,
    }
}

/// A name a person typed: trimmed, at most [`TITLE_CHARS`], one line.
pub(super) fn clean_title(value: &str) -> Option<String> {
    let title = value.trim();
    (!title.is_empty()
        && title.chars().count() <= TITLE_CHARS
        && !title.chars().any(char::is_control))
    .then(|| title.to_owned())
}

async fn rename(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Change>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(title) = clean_title(&form.title) else {
        return refusal(
            StatusCode::BAD_REQUEST,
            "Enter a name of at most 120 characters.",
        );
    };
    let result = update(&app, &owner, &id, |chat| {
        if chat.title == title {
            return false;
        }
        chat.title = title.clone();
        true
    })
    .await;
    match result {
        Ok(chat) => {
            // The open chat's header and tab follow its new name.
            let extra = if current(&form.current) == Some(id.as_str()) {
                html! {
                    title { (chat.title) " · OpenAgents" }
                    (Breadcrumb::new(chat.title.clone()).swap_oob(true))
                }
            } else {
                html! {}
            };
            changed(&app, &headers, &owner, &form, None, extra).await
        }
        Err(r) => r,
    }
}

/// The rename field: in place of the row for HTMX, else its own page.
async fn rename_field(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(search): Query<Search>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await.filter(|_| valid_id(&id)) else {
        return missing();
    };
    let chat = match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(loaded)) => loaded.conversation,
        Ok(None) => return missing(),
        Err(error) => return unavailable(error),
    };
    let current = current(&search.current).unwrap_or_default();
    let back = if current.is_empty() {
        "/".to_owned()
    } else {
        format!("/chat/{current}")
    };
    let field = RowRename::new(
        format!("chat-row-{id}"),
        format!("/chat/{id}/rename"),
        chat.title.clone(),
        back,
    )
    .max_chars(TITLE_CHARS)
    .cancel_hx(format!("{LIST}?current={current}"))
    .field("csrf", csrf(&app, &owner))
    .field("current", current)
    .target(SIDEBAR)
    .swap("outerHTML");
    if hx(&headers) {
        return crate::chat_html::protect(field.render().into_response());
    }
    let page = UiPage::new("Rename chat")
        .path(format!("/chat/{id}/rename"))
        .head(crate::chat_html::head())
        .sidebar_section(chat_list(&app, &owner, None, false, false).await)
        .content(crate::ui_page::prose(html! {
            h1 { "Rename chat" }
            ul.oa-nav-list role="list" { (field) }
        }));
    crate::chat_html::protect(page.respond(&headers))
}

/// The list for HTMX (a search, or Cancel on a rename); without JavaScript
/// a search page with the matching chats.
async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Query(search): Query<Search>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return crate::chat_html::protect(
            html! { (ChatList::new().id("chat-sidebar")) }.into_response(),
        );
    };
    let current = current(&search.current);
    let view = View {
        current,
        hx: current.is_some(),
        q: &search.q,
        archived: None,
    };
    if hx(&headers) {
        return crate::chat_html::protect(
            render(&app, &owner, view, false)
                .await
                .render()
                .into_response(),
        );
    }
    let rows = app.config.chat_store.list(&owner).await.unwrap_or_default();
    let q = search.q.trim();
    let found: Vec<&Conversation> = rows
        .iter()
        .filter(|chat| chat.archived_unix.is_none() && matches(chat, q))
        .collect();
    let sidebar = build(
        ChatList::new().id("chat-sidebar"),
        &rows,
        &csrf(&app, &owner),
        View {
            current: None,
            hx: false,
            ..view
        },
    );
    let page = UiPage::new("Search chats")
        .path(LIST)
        .head(crate::chat_html::head())
        .sidebar_section(sidebar)
        .content(crate::ui_page::prose(html! {
            h1 { "Search chats" }
            @if found.is_empty() {
                p { "No chats found" }
            } @else {
                ul {
                    @for chat in &found {
                        li { a href=(format!("/chat/{}", chat.id)) { (chat.title) } }
                    }
                }
            }
        }));
    crate::chat_html::protect(page.respond(&headers))
}

/// Archived chats, newest archived first, each with Restore.
async fn archived(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = reader(&app, &headers).await;
    let mut rows = match &owner {
        Some(owner) => match app.config.chat_store.list(owner).await {
            Ok(rows) => rows,
            Err(error) => return unavailable(error),
        },
        None => Vec::new(),
    };
    // Signed out, this page is where every chat in the browser can be
    // deleted at once; signed in, Settings is (#11038).
    let delete_all = !rows.is_empty()
        && owner
            .as_deref()
            .is_some_and(|owner| !crate::chat_store::is_account_owner(owner));
    rows.retain(|chat| chat.archived_unix.is_some());
    rows.sort_by(|a, b| {
        b.archived_unix
            .cmp(&a.archived_unix)
            .then_with(|| a.id.cmp(&b.id))
    });
    let token = owner
        .as_deref()
        .map(|owner| csrf(&app, owner))
        .unwrap_or_default();
    let mut page = UiPage::new("Archived chats")
        .path(ARCHIVED)
        .head(crate::chat_html::head())
        .content(crate::ui_page::prose(html! {
            h1 { "Archived chats" }
            @if rows.is_empty() {
                p { "No archived chats." }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for chat in &rows {
                        li.oa-chat-archive-row {
                            a href=(format!("/chat/{}", chat.id)) { (chat.title) }
                            form method="post" action=(format!("/chat/{}/archive", chat.id)) {
                                input type="hidden" name="csrf" value=(token);
                                input type="hidden" name="archived" value="0";
                                input type="hidden" name="back" value="archived";
                                (Button::new("Restore")
                                    .kind(ButtonType::Submit)
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
            @if delete_all {
                h2 { "Delete all chats" }
                p { "Remove every chat saved in this browser, archived or not." }
                p { (crate::ui_page::action_link("Delete all chats", super::delete_all::PATH)) }
            }
        }));
    if let Some(owner) = &owner {
        page = page.sidebar_section(chat_list(&app, owner, None, false, false).await);
    }
    crate::chat_html::protect(page.respond(&headers))
}
