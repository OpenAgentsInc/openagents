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
//! list groups chats under a Projects heading, one group per project, while
//! Chats still lists every chat with its project named (#11598), and
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
pub(crate) struct View<'a> {
    pub current: Option<&'a str>,
    pub hx: bool,
    pub q: &'a str,
    pub archived: Option<&'a str>,
    /// The signed-in person's projects ([`crate::projects::sidebar`]).
    pub projects: Option<&'a Sidebar>,
    /// Chats other people started in shared projects (#11586), listed in
    /// those projects' groups, and who started each by chat id.
    pub others: &'a [(Conversation, String)],
    /// The account's computer sessions (#11897), newest first, and whether
    /// each one's computer is online. They list under Chats with the chats.
    pub sessions: &'a [(crate::session_index::Row, bool)],
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
    let others = match (&projects, crate::projects::request_headers()) {
        (Some(sidebar), Some(headers)) => {
            others(app, &headers, &sidebar.status.projects, owner).await
        }
        _ => Vec::new(),
    };
    let sessions = sessions(app, owner).await;
    let view = View {
        projects: projects.as_deref(),
        others: &others,
        sessions: &sessions,
        ..view
    };
    (build(list, &rows, &csrf(app, owner), view), working)
}

/// The signed-in account's computer sessions, newest first; none when
/// they can't be read.
async fn sessions(app: &App, owner: &str) -> Vec<(crate::session_index::Row, bool)> {
    if !crate::chat_store::is_account_owner(owner) {
        return Vec::new();
    }
    let index = match crate::session_index::load(&app.config.chat_store, owner).await {
        Ok(index) => index,
        Err(error) => {
            eprintln!("openagents-web: session list: {error}");
            return Vec::new();
        }
    };
    let now = crate::chat_store::now_unix();
    index
        .newest()
        .into_iter()
        .map(|row| (row.clone(), index.online(row, now)))
        .collect()
}

/// Chats other people started in the shared projects `owner` is in, with
/// who started each (#11586).
async fn others(
    app: &App,
    headers: &HeaderMap,
    projects: &[oa_auth::repos::Project],
    owner: &str,
) -> Vec<(Conversation, String)> {
    let mut found = Vec::new();
    for project in projects
        .iter()
        .filter(|p| crate::projects::shared_chats::shared(p))
    {
        let names = crate::projects::shared_chats::starters(app, headers, project).await;
        if let Ok(chats) = crate::projects::shared_chats::chats_in(app, project, owner, &[]).await {
            for chat in chats.into_iter().filter(|chat| chat.owner != owner) {
                let by = crate::projects::shared_chats::started_by(&chat, owner, &names);
                found.push((chat, by));
            }
        }
    }
    found.sort_by(|a, b| b.0.updated_unix.cmp(&a.0.updated_unix));
    found
}

/// The chat's project, when it is one of the viewer's.
fn project_of<'a>(chat: &Conversation, view: View<'a>) -> Option<&'a oa_auth::repos::Project> {
    let id = chat.project.as_deref()?;
    view.projects?.project(id)
}

/// The list from loaded rows (newest first, as the store sorts them).
pub(crate) fn build(list: ChatList, rows: &[Conversation], csrf: &str, view: View<'_>) -> ChatList {
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
            let theirs: Vec<&(Conversation, String)> = view
                .others
                .iter()
                .filter(|(chat, _)| {
                    chat.project.as_deref() == Some(project.id.as_str())
                        && chat.archived_unix.is_none()
                        && matches(chat, q)
                })
                .collect();
            if !q.is_empty() && chats.is_empty() && theirs.is_empty() {
                continue;
            }
            // The open chat always stays visible, even in a closed group.
            let at = chats
                .iter()
                .copied()
                .chain(theirs.iter().map(|(chat, _)| chat))
                .position(|chat| view.current == Some(chat.id.as_str()));
            let mut group = ChatGroup::new(
                project.id.clone(),
                project.name.clone(),
                // A viewer starts no chat there: the group opens the project.
                if crate::projects::api::role_of(project).edits() {
                    format!("/?project={}", project.id)
                } else {
                    crate::projects::page::href(&project.id)
                },
            );
            if let Some(primary) = project.primary() {
                group =
                    group.repository(primary.repository.clone(), primary.default_branch.clone());
            }
            let mut group = group
                .edit(crate::projects::page::href(&project.id))
                .open(at.is_some() || !q.is_empty() || !sidebar.closed.contains(&project.id))
                .more_open(at.is_some_and(|at| at >= GROUP_ROWS))
                .items(
                    chats
                        .iter()
                        .map(|chat| row(chat, csrf, view, true))
                        .chain(theirs.iter().map(|(chat, by)| their_row(chat, by, view))),
                );
            if sidebar.reconnect() {
                group = group.note("Reconnect GitHub", crate::projects::PAGE);
            } else if crate::projects::api::shared(project) {
                // Shared projects list with the rest, marked (#11589).
                group = group.note(
                    crate::projects::page::shared_label(crate::projects::api::role_of(project)),
                    format!("{}#members", crate::projects::page::href(&project.id)),
                );
            }
            list = list.project(group);
        }
    }
    // Every chat lists under Chats, newest first, project chats too, each
    // naming its project (#11598): a chat filed under a closed project
    // group is never out of sight.
    // Computer sessions mix in by last activity (#11897).
    let mut sessions = view
        .sessions
        .iter()
        .filter(|(session, _)| {
            q.is_empty() || session.title.to_lowercase().contains(&q.to_lowercase())
        })
        .peekable();
    let mut chats = unpinned.iter().copied().peekable();
    let mut items = Vec::new();
    loop {
        let session_first = match (chats.peek(), sessions.peek()) {
            (None, None) => break,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(chat), Some((session, _))) => session.last_activity_unix > chat.updated_unix,
        };
        if session_first {
            let (session, online) = sessions.next().unwrap();
            items.push(session_row(session, *online, view));
        } else {
            items.push(row(chats.next().unwrap(), csrf, view, false));
        }
    }
    list = list.items(items);
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
    // Signed in: "Manage projects" is a button on the Projects heading;
    // with no projects yet, a link to make one.
    let projects = view.projects.and_then(|sidebar| {
        sidebar
            .status
            .projects
            .is_empty()
            .then_some("Make a project")
    });
    if view
        .projects
        .is_some_and(|sidebar| !sidebar.status.projects.is_empty())
    {
        list = list.projects_action("Manage projects", crate::projects::PAGE);
    }
    // Signed in with no terminal chats yet: where to connect one (#11089).
    // It is the empty list's way in too.
    let terminal = view.projects.is_some() && !rows.iter().any(|chat| chat.terminal.is_some());
    if archived || terminal || projects.is_some() {
        list = list.after(html! {
            @if archived {
                p.oa-chat-list-more { a.oa-chat-list-link href=(ARCHIVED) { "Archived chats" } }
            }
            @if terminal {
                p.oa-chat-list-more { a.oa-chat-list-link href=(crate::terminal_connect::PAGE) { "Connect your terminal" } }
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
            && chat.messages.iter().any(|m| {
                !matches!(m.role, Role::Tool | Role::Thinking) && m.text.to_lowercase().contains(&q)
            }))
}

/// One chat row with its "…" menu. Inside a project's group (`grouped`)
/// line 2 drops the repository: the group's heading names it.
fn row(chat: &Conversation, csrf: &str, view: View<'_>, grouped: bool) -> NavItem {
    let id = &chat.id;
    let current = view.current.unwrap_or_default();
    // The copy in a project's group has its own row id; both share the
    // status slot, which the live stream replaces wherever it is.
    let row_id = if grouped {
        format!("chat-project-row-{id}")
    } else {
        format!("chat-row-{id}")
    };
    let mut item = NavItem::new(chat.title.clone(), format!("/chat/{id}"))
        .current(view.current == Some(id.as_str()))
        .row_id(row_id.clone());
    // Under Chats, a project's chat names its project first (#11598).
    let project = project_of(chat, view).filter(|_| !grouped);
    let detail = match (project, line_two(chat, !grouped && project.is_none())) {
        (Some(project), Some(rest)) => Some(format!("{} · {rest}", project.name)),
        (Some(project), None) => Some(project.name.clone()),
        (None, rest) => rest,
    };
    if let Some(detail) = detail {
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
        .target(format!("#{row_id}"))
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

/// A computer session (#11897): "On <computer>" on line two, its status,
/// and no menu (its computer pins, renames, and deletes it).
fn session_row(session: &crate::session_index::Row, online: bool, view: View<'_>) -> NavItem {
    let mut detail = Vec::new();
    if let Some(project) = session
        .project
        .as_deref()
        .and_then(|id| view.projects?.project(id))
    {
        detail.push(project.name.clone());
    }
    detail.push(format!("On {}", session.computer));
    if !online {
        detail.push("Offline".into());
    } else if let Some(label) = session.status.label() {
        detail.push(label.into());
    }
    NavItem::new(session.title.clone(), super::sessions::href(&session.id))
        .current(view.current == Some(session.id.as_str()))
        .row_id(format!("chat-session-row-{}", session.id))
        .detail(detail.join(" · "))
}

/// A chat someone else started in a shared project: who started it, and
/// no menu (its starter pins, renames, moves, archives, and deletes it).
fn their_row(chat: &Conversation, by: &str, view: View<'_>) -> NavItem {
    let id = &chat.id;
    let mut item = NavItem::new(chat.title.clone(), format!("/chat/{id}"))
        .current(view.current == Some(id.as_str()))
        .row_id(format!("chat-project-row-{id}"))
        .detail(format!("Started by {by}"))
        .trailing(row_status_slot(chat, false));
    if view.hx {
        item = item.hx(HxGet::new(format!("/chat/{id}/workspace"))
            .target("#chat-content")
            .swap("innerHTML")
            .sync("#chat-content:replace"));
    }
    item
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
        .filter(|p| crate::projects::api::role_of(p).edits())
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
            others: &[],
            sessions: &[],
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
            if archive && crate::computers::client().is_some() && app.config.cloud.is_some() {
                if let Err(_) = crate::computer_runs::archive(&app, &headers, &owner, &id).await {
                    eprintln!("openagents-web: archived chat computer cleanup failed");
                }
            }
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
    let field = rename_row(&chat, current, &csrf(&app, &owner));
    if hx(&headers) {
        return crate::chat_html::protect(field.render().into_response());
    }
    let list = chat_list(&app, &owner, None, false, false).await;
    rename_page(&headers, &chat, field, list)
}

/// The inline field that renames `chat`'s row, back on `current`'s page.
pub(crate) fn rename_row(chat: &Conversation, current: &str, token: &str) -> RowRename {
    let id = &chat.id;
    let back = if current.is_empty() {
        "/".to_owned()
    } else {
        format!("/chat/{current}")
    };
    RowRename::new(
        format!("chat-row-{id}"),
        format!("/chat/{id}/rename"),
        chat.title.clone(),
        back,
    )
    .max_chars(TITLE_CHARS)
    .cancel_hx(format!("{LIST}?current={current}"))
    .field("csrf", token)
    .field("current", current)
    .target(SIDEBAR)
    .swap("outerHTML")
}

/// The Rename chat page, without JavaScript: the field, beside the list.
pub(crate) fn rename_page(
    headers: &HeaderMap,
    chat: &Conversation,
    field: RowRename,
    list: Markup,
) -> Response {
    let page = UiPage::new("Rename chat")
        .path(format!("/chat/{}/rename", chat.id))
        .head(crate::chat_html::head())
        .sidebar_section(list)
        .content(crate::ui_page::prose(html! {
            h1 { "Rename chat" }
            ul.oa-nav-list role="list" { (field) }
        }));
    crate::chat_html::protect(page.respond(headers))
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
        others: &[],
        sessions: &[],
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
    let list = match &owner {
        Some(owner) => Some(chat_list(&app, owner, None, false, false).await),
        None => None,
    };
    archived_page(&headers, &rows, &token, delete_all, list)
}

/// The Archived chats page: `rows`, newest first, each with Restore.
pub(crate) fn archived_page(
    headers: &HeaderMap,
    rows: &[Conversation],
    token: &str,
    delete_all: bool,
    list: Option<Markup>,
) -> Response {
    let mut page = UiPage::new("Archived chats")
        .path(ARCHIVED)
        .head(crate::chat_html::head())
        .content(crate::ui_page::prose(html! {
            h1 { "Archived chats" }
            @if rows.is_empty() {
                p { "No archived chats." }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for chat in rows {
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
    if let Some(list) = list {
        page = page.sidebar_section(list);
    }
    crate::chat_html::protect(page.respond(headers))
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
    let mut shared = None;
    let project = if target.is_empty() {
        None
    } else {
        let sidebar = crate::projects::sidebar(&app).await;
        // A viewer adds no chats to a shared project (#11586).
        match sidebar
            .as_deref()
            .and_then(|s| s.project(target))
            .filter(|p| crate::projects::api::role_of(p).edits())
        {
            Some(project) => {
                shared = Some(project.clone());
                Some(project.id.clone())
            }
            None => {
                return refusal(StatusCode::BAD_REQUEST, "Pick one of your projects.");
            }
        }
    };
    let result = update(&app, &owner, &id, |chat| {
        if chat.project == project && chat.filed {
            return false;
        }
        // The person's choice: sync no longer files it (#11541).
        chat.project.clone_from(&project);
        chat.filed = true;
        true
    })
    .await;
    match result {
        Ok(chat) => {
            crate::pages::chat::note_shared(&app, shared.as_ref(), &owner, &chat).await;
            changed(&app, &headers, &owner, &form, None, html! {}).await
        }
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
    let list = chat_list(&app, &owner, Some(id.as_str()), false, false).await;
    move_page_for(&headers, &chat, projects, &csrf(&app, &owner), list)
}

/// The Move to project page for `chat`: one button per project, and No
/// project.
pub(crate) fn move_page_for(
    headers: &HeaderMap,
    chat: &Conversation,
    projects: &[oa_auth::repos::Project],
    token: &str,
    list: Markup,
) -> Response {
    let id = &chat.id;
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
        .sidebar_section(list)
        .content(crate::ui_page::prose(html! {
            h1 { "Move to project" }
            p { (chat.title) }
            @if projects.is_empty() {
                p {
                    "You don't have any projects yet. "
                    a href=(crate::projects::PAGE) { "Make a project" }
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
    crate::chat_html::protect(page.respond(headers))
}
