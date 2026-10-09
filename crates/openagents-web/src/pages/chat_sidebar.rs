//! Organizing chats in the left panel (#11036, `docs/web/sidebar.md`
//! "Interactions"): a row's "…" menu with Pin / Unpin, Rename and Archive,
//! a Pinned group on top, search over titles and messages, and the Archived
//! page with Restore.
//!
//! Every change is a plain `POST` form with the chat's CSRF token: without
//! JavaScript it redirects back; with HTMX it answers with the list, which
//! replaces `#chat-sidebar` in place. Pins and archives live on the chat
//! record (`pinned_unix`, `archived_unix`), so they survive a reload.
//!
//! For a signed-in person with projects (#11034, [`crate::projects`]), the
//! list groups chats under a Projects heading, one group per project, and
//! the row menu offers moving a chat to a project (`project` on the chat
//! record). Names come from the account, so nobody else sees them.

use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, ControlSize};
use openagents_ui::icons::Icon;
use openagents_ui::shell::{ChatGroup, GROUP_ROWS, HxGet, NavItem, RowAction, RowMenu, RowRename};

use crate::projects::Sidebar;

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
        .route("/chat/{id}/project", get(move_page).post(move_chat))
}

/// The most projects a row's menu lists as "Move to …" entries; with more,
/// it links the Move to project page instead.
const MENU_PROJECTS: usize = 6;

/// What the list shows: the open chat (rows then load with HTMX), the
/// search text, and the chat just archived (its Undo notice).
#[derive(Clone, Copy, Default)]
pub(super) struct View<'a> {
    pub current: Option<&'a str>,
    pub hx: bool,
    pub q: &'a str,
    pub archived: Option<&'a str>,
    /// The signed-in person's projects ([`crate::projects::sidebar`]).
    pub projects: Option<&'a Sidebar>,
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
        .filter(|chat| {
            chat.archived_unix.is_none() && row_status(chat) == Some(ChatStatus::Working)
        })
        .map(|chat| chat.id.clone())
        .collect();
    let projects = crate::projects::sidebar(app).await;
    let view = View {
        projects: projects.as_deref(),
        ..view
    };
    (build(list, &rows, &csrf(app, owner), view), working)
}

/// The chat's project, when it is one of the viewer's.
fn project_of<'a>(chat: &Conversation, view: View<'a>) -> Option<&'a oa_auth::repos::Project> {
    let id = chat.project.as_deref()?;
    view.projects?.project(id)
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
    let unpinned: Vec<&Conversation> = shown
        .iter()
        .copied()
        .filter(|chat| chat.pinned_unix.is_none())
        .collect();
    let mut list = list.pinned(pinned.iter().map(|chat| row(chat, csrf, view, false)));
    if let Some(sidebar) = view.projects {
        for project in &sidebar.status.projects {
            let chats: Vec<&Conversation> = unpinned
                .iter()
                .copied()
                .filter(|chat| chat.project.as_deref() == Some(project.id.as_str()))
                .collect();
            if !q.is_empty() && chats.is_empty() {
                continue;
            }
            // The open chat always stays visible, even in a closed group.
            let at = chats
                .iter()
                .position(|chat| view.current == Some(chat.id.as_str()));
            let mut group = ChatGroup::new(
                project.id.clone(),
                project.name.clone(),
                format!("/?project={}", project.id),
            )
            .open(at.is_some() || !q.is_empty() || !sidebar.closed.contains(&project.id))
            .more_open(at.is_some_and(|at| at >= GROUP_ROWS))
            .items(chats.iter().map(|chat| row(chat, csrf, view, true)));
            if sidebar.reconnect() {
                group = group.note("Reconnect GitHub", crate::projects::PAGE);
            }
            list = list.project(group);
        }
    }
    list = list.items(
        unpinned
            .iter()
            .filter(|chat| project_of(chat, view).is_none())
            .map(|chat| row(chat, csrf, view, false)),
    );
    let open = rows.iter().any(|chat| chat.archived_unix.is_none());
    // The search box waits until people have more chats (owner,
    // 2026-10-09); `/chat/list?q=` and its keyboard shortcuts stay.
    // if open || !q.is_empty() {
    //     list = list.search(
    //         ChatSearch::new(LIST, ROWS)
    //             .value(q)
    //             .field("current", view.current.unwrap_or_default()),
    //     );
    // }
    let _ = open;
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
    let archived = rows.iter().any(|chat| chat.archived_unix.is_some());
    // Signed in: where to connect a repository or manage projects.
    let projects = view.projects.map(|sidebar| {
        if sidebar.status.projects.is_empty() {
            "Connect a GitHub repository"
        } else {
            "Manage projects"
        }
    });
    if archived || projects.is_some() {
        list = list.after(html! {
            @if archived {
                p.oa-chat-list-more { a.oa-chat-list-link href=(ARCHIVED) { "Archived chats" } }
            }
            @if let Some(label) = projects {
                p.oa-chat-list-more { a.oa-chat-list-link href=(crate::projects::PAGE) { (label) } }
            }
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

/// One chat row with its "…" menu. Inside a project's group (`grouped`)
/// line 2 drops the repository: the group's heading names it.
fn row(chat: &Conversation, csrf: &str, view: View<'_>, grouped: bool) -> NavItem {
    let id = &chat.id;
    let current = view.current.unwrap_or_default();
    let mut item = NavItem::new(chat.title.clone(), format!("/chat/{id}"))
        .current(view.current == Some(id.as_str()))
        .row_id(format!("chat-row-{id}"));
    if let Some(detail) = line_two(chat, !grouped) {
        item = item.detail(detail);
    }
    let working = row_status(chat) == Some(ChatStatus::Working);
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
    let mut menu = RowMenu::new(format!("chat-menu-{id}"), chat.title.clone())
        .action(pin)
        .action(rename);
    for action in move_actions(chat, csrf, view) {
        menu = menu.action(action);
    }
    item.menu(
        menu.action(archive)
            .action(RowAction::open("Delete", format!("/chat/{id}/delete")).icon(Icon::Trash)),
    )
}

/// "Move to …" entries for the viewer's projects, and leaving the chat's
/// project; a link to the Move to project page when there are many.
fn move_actions(chat: &Conversation, csrf: &str, view: View<'_>) -> Vec<RowAction> {
    let Some(sidebar) = view.projects else {
        return Vec::new();
    };
    let id = &chat.id;
    let current = view.current.unwrap_or_default();
    let inside = project_of(chat, view);
    let others: Vec<&oa_auth::repos::Project> = sidebar
        .status
        .projects
        .iter()
        .filter(|p| inside.is_none_or(|inside| inside.id != p.id))
        .collect();
    let post = |label: String, project: &str| {
        RowAction::post(label, format!("/chat/{id}/project"))
            .icon(Icon::Folder)
            .field("csrf", csrf)
            .field("current", current)
            .field("project", project)
            .target(SIDEBAR)
            .swap("outerHTML")
    };
    let mut actions = Vec::new();
    if others.len() > MENU_PROJECTS {
        actions.push(
            RowAction::open("Move to project", format!("/chat/{id}/project")).icon(Icon::Folder),
        );
    } else {
        actions.extend(
            others
                .iter()
                .map(|p| post(format!("Move to {}", p.name), &p.id)),
        );
    }
    if let Some(inside) = inside {
        actions.push(post(format!("Remove from {}", inside.name), ""));
    }
    actions
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
    #[serde(default)]
    project: String,
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
pub(super) async fn update(
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
            projects: None,
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
                let offer = work::offer(&app, &headers, &chat).await;
                html! {
                    title { (chat.title) " · OpenAgents" }
                    (work::breadcrumb(&chat, offer.as_ref()).swap_oob(true))
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
    let projects = crate::projects::sidebar(&app).await;
    let view = View {
        current,
        hx: current.is_some(),
        q: &search.q,
        archived: None,
        projects: projects.as_deref(),
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

/// Moves a chat into one of the viewer's projects, or out of its project
/// (`project` empty).
async fn move_chat(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Change>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let target = form.project.trim();
    let project = if target.is_empty() {
        None
    } else {
        let sidebar = crate::projects::sidebar(&app).await;
        match sidebar.as_deref().and_then(|s| s.project(target)) {
            Some(project) => Some(project.id.clone()),
            None => {
                return refusal(StatusCode::BAD_REQUEST, "Pick one of your projects.");
            }
        }
    };
    let result = update(&app, &owner, &id, |chat| {
        if chat.project == project {
            return false;
        }
        chat.project.clone_from(&project);
        true
    })
    .await;
    match result {
        Ok(_) => changed(&app, &headers, &owner, &form, None, html! {}).await,
        Err(r) => r,
    }
}

/// The Move to project page, for a row menu with many projects (and
/// without JavaScript): one button per project, and No project.
async fn move_page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(owner) = crate::ask::visitor(&headers).filter(|_| valid_id(&id)) else {
        return missing();
    };
    let chat = match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(loaded)) => loaded.conversation,
        Ok(None) => return missing(),
        Err(error) => return unavailable(error),
    };
    let sidebar = crate::projects::sidebar(&app).await;
    let projects = sidebar
        .as_deref()
        .map(|s| s.status.projects.as_slice())
        .unwrap_or_default();
    let token = csrf(&app, &owner);
    let inside = chat.project.as_deref();
    let choice = |label: &str, value: &str, chosen: bool| {
        html! {
            li.oa-chat-archive-row {
                span { (label) }
                @if chosen {
                    span { "Current" }
                } @else {
                    form method="post" action=(format!("/chat/{id}/project")) {
                        input type="hidden" name="csrf" value=(token);
                        input type="hidden" name="current" value=(id);
                        input type="hidden" name="project" value=(value);
                        (Button::new("Move here")
                            .kind(ButtonType::Submit)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary))
                    }
                }
            }
        }
    };
    let page = UiPage::new("Move to project")
        .path(format!("/chat/{id}/project"))
        .head(crate::chat_html::head())
        .sidebar_section(chat_list(&app, &owner, Some(id.as_str()), false, false).await)
        .content(crate::ui_page::prose(html! {
            h1 { "Move to project" }
            p { (chat.title) }
            @if projects.is_empty() {
                p {
                    "You don't have any projects yet. "
                    a href=(crate::projects::PAGE) { "Connect a GitHub repository" }
                }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for project in projects {
                        (choice(&project.name, &project.id, inside == Some(project.id.as_str())))
                    }
                    (choice("No project", "", inside.is_none_or(|p| !projects.iter().any(|x| x.id == p))))
                }
            }
        }));
    crate::chat_html::protect(page.respond(&headers))
}
