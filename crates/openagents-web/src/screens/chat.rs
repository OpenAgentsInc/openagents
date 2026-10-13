//! The chat screens and their organisms: a new chat, a thread, work on a
//! computer, the chat list, the composer, a work run, and the demo, over
//! sample chats built in memory.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use axum::response::Response;
use maud::{Markup, html};
use openagents_ui::shell::{Breadcrumb, ChatList};
use serde_json::{Value, json};

use super::auth::{COMPUTER, signed_in};
use super::{Group, Screen, state};
use crate::chat_store::Conversation;
use crate::composer_row::{Choices, Environment, Picked, Target};
use crate::pages::chat::{self, Notice, Parts};
use crate::phone_api::{Item, Question};
use crate::projects::Sidebar;

const CSRF: &str = "sample-ticket";
const PROJECT: &str = "prj_5a5a5a5a5a5a5a5a";
const OTHER_PROJECT: &str = "prj_6b6b6b6b6b6b6b6b";
const ENVIRONMENT: &str = "env_sample";

pub(super) fn screens() -> Vec<Screen> {
    vec![
        Screen {
            id: "chat-new",
            title: "New chat",
            group: Group::Page,
            routes: &["/"],
            states: vec![
                state("no-computers", "No computers", new_no_computers),
                state("computer-online", "A computer online", new_online),
                state("computer-offline", "Computer offline", new_offline),
                state("project", "Project picked", new_project),
            ],
        },
        Screen {
            id: "chat-thread",
            title: "Chat",
            group: Group::Page,
            routes: &["/chat/{id}", "/chat/{id}/delete"],
            states: vec![
                state("answer", "Question and answer", thread_answer),
                state("code-and-links", "Answer with code and links", thread_code),
                state("working", "Working", thread_working),
                state("failed", "Answer failed", thread_failed),
                state("unknown", "Answer not confirmed", thread_unknown),
                state("long", "Long chat with earlier messages", thread_long),
                state("pinned", "Pinned", thread_pinned),
                state("archived", "Archived", thread_archived),
                state("delete", "Asking before deleting", thread_delete),
                state(
                    "delete-coder",
                    "Asking before deleting a Coder chat",
                    thread_delete_coder,
                ),
            ],
        },
        Screen {
            id: "chat-shared",
            title: "Shared chat",
            group: Group::Page,
            routes: &["/share/{token}"],
            states: vec![state(
                "shared",
                "Read by anyone with the link",
                shared_answer,
            )],
        },
        Screen {
            id: "chat-work",
            title: "Chat running on a computer",
            group: Group::Page,
            routes: &[
                "/chat/{id}",
                "/chat/{id}/approval",
                "/chat/{id}/files",
                "/chat/{id}/claude",
                "/chat/{id}/continue",
            ],
            states: vec![
                state("running", "Running", work_running),
                state("approval", "Waiting for approval", work_approval),
                state("approved", "Approved", work_approved),
                state("denied", "Denied", work_denied),
                state("finished", "Finished with changes", work_finished),
                state("stopped", "Stopped", work_stopped),
                state("coder-offline", "Coder not online", work_offline),
                state("files", "Files sent", work_files),
                state("run-claude", "Run Claude Code", work_run_claude),
                state(
                    "run-claude-error",
                    "Run Claude Code, nothing written",
                    work_run_claude_error,
                ),
                state("continue", "Continue on a Cloud computer", work_continue),
            ],
        },
        Screen {
            id: "chat-sidebar",
            title: "Chat list",
            group: Group::Organism,
            routes: &[
                "every chat page",
                "/chat/archived",
                "/chat/delete-all",
                "/chat/{id}/project",
            ],
            states: vec![
                state("empty", "No chats", sidebar_empty),
                state("grouped", "Pinned, projects, and days", sidebar_grouped),
                state("search", "Search with results", sidebar_search),
                state("search-none", "Search with no results", sidebar_search_none),
                state("rename", "Rename", sidebar_rename),
                state("archived", "Archived chats", sidebar_archived),
                state(
                    "archived-empty",
                    "No archived chats",
                    sidebar_archived_empty,
                ),
                state("delete-all", "Delete all chats", sidebar_delete_all),
                state("move", "Move to project", sidebar_move),
                state("move-none", "Move with no projects", sidebar_move_none),
            ],
        },
        Screen {
            id: "composer",
            title: "Composer",
            group: Group::Organism,
            routes: &["/", "/chat/{id}"],
            states: vec![
                state("empty", "Empty", composer_empty),
                state("typing", "Typing", composer_typing),
                state("refused", "Message refused", composer_refused),
                state("row", "Project, branch, and where it runs", composer_row),
                state("where-it-runs", "Where it runs open", composer_targets),
                state("projects", "Project open", composer_projects),
                state("connect", "Claude needs connecting", composer_connect),
                state("suggestions", "Suggestions", composer_suggestions),
                state("terminal", "Reply to Coder", composer_terminal),
                state("terminal-note", "Coder not online note", composer_note),
            ],
        },
        Screen {
            id: "work-run",
            title: "Work run",
            group: Group::Page,
            routes: &["/work", "/work/{id}"],
            states: vec![
                state("list-empty", "List: no runs, no key saved", runs_empty),
                state("list", "List: runs", runs_list),
                state("queued", "Queued", run_queued),
                state("running", "Running", run_running),
                state("finished", "Finished", run_finished),
                state("failed", "Failed", run_failed),
                state("stopped", "Stopped", run_stopped),
            ],
        },
        Screen {
            id: "demo",
            title: "Demo",
            group: Group::Page,
            routes: &["/demo", "/demo/{chat}"],
            states: vec![
                state("index", "Demo", demo_index),
                state("chat", "A demo chat", demo_chat),
                state("missing", "Demo chat not found", demo_missing),
            ],
        },
    ]
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn owner() -> String {
    format!("account:{}", "5a".repeat(32))
}

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

fn user(text: &str, request: &str) -> Value {
    json!({ "role": "user", "text": text, "request_id": request })
}

fn assistant(text: &str, request: &str) -> Value {
    json!({ "role": "assistant", "text": text, "request_id": request })
}

fn answered(request: &str) -> Value {
    json!({ "id": request, "digest": "sample", "outcome": "answered" })
}

/// A sample chat: `extra` fields over a titled chat with `messages`.
fn conversation(n: u32, title: &str, messages: Vec<Value>, extra: Value) -> Conversation {
    let mut value = json!({
        "id": id(n),
        "owner": owner(),
        "revision": 1,
        "title": title,
        "messages": messages,
        "pending": null,
        "requests": [],
        "updated_unix": now() - u64::from(n) * 3_600,
    });
    if let (Some(base), Value::Object(extra)) = (value.as_object_mut(), extra) {
        base.extend(extra);
    }
    serde_json::from_value(value).expect("a sample chat")
}

fn answer_chat() -> Conversation {
    conversation(
        1,
        "Plan a weekend trip",
        vec![
            user("Plan a quiet weekend in the mountains for two.", "r1"),
            assistant(
                "Here's a plan for a quiet weekend:\n\n\
                 1. **Saturday morning**: drive up early and check in by noon.\n\
                 2. **Saturday afternoon**: an easy lakeside walk, about 5 km.\n\
                 3. **Sunday**: breakfast in town, then a scenic drive home.\n\n\
                 Pack layers: mornings are cold even in summer.",
                "r1",
            ),
        ],
        json!({ "requests": [answered("r1")] }),
    )
}

fn code_chat() -> Conversation {
    conversation(
        2,
        "Reverse a list in Rust",
        vec![
            user("How do I reverse a list in Rust?", "r1"),
            assistant(
                "Call `reverse` on a mutable vector:\n\n\
                 ```rust\nlet mut items = vec![1, 2, 3];\nitems.reverse();\nassert_eq!(items, [3, 2, 1]);\n```\n\n\
                 To keep the original, iterate backwards with `iter().rev()`.\n\n\
                 Read more in [the Vec docs](https://doc.rust-lang.org/std/vec/struct.Vec.html) \
                 and [Rust by Example](https://doc.rust-lang.org/rust-by-example/).",
                "r1",
            ),
        ],
        json!({ "requests": [{
            "id": "r1", "digest": "sample", "outcome": "answered",
            "reply": { "plugins": ["claude-code", "codex"] }
        }] }),
    )
}

fn working_chat() -> Conversation {
    conversation(
        3,
        "Summarize my notes",
        vec![user(
            "Summarize these meeting notes in three bullets.",
            "r1",
        )],
        json!({
            "pending": { "request_id": "r1", "started_unix": now() - 4 },
            "requests": [{ "id": "r1", "digest": "sample", "outcome": "pending" }],
        }),
    )
}

fn failed_chat(outcome: &str) -> Conversation {
    conversation(
        4,
        "Translate a letter",
        vec![user("Translate this letter into French.", "r1")],
        json!({ "requests": [{ "id": "r1", "digest": "sample", "outcome": outcome }] }),
    )
}

fn long_chat() -> Conversation {
    let mut messages = Vec::new();
    let mut requests = Vec::new();
    for turn in 1..=20 {
        let request = format!("r{turn}");
        messages.push(user(&format!("Sample question number {turn}."), &request));
        messages.push(assistant(
            &format!("Sample answer number {turn}."),
            &request,
        ));
        requests.push(answered(&request));
    }
    conversation(
        5,
        "A long sample chat",
        messages,
        json!({ "requests": requests }),
    )
}

fn task(state: &str, title: &str, kind: &str) -> Value {
    let mut task = json!({
        "id": "task-sample-1",
        "kind": kind,
        "environment": ENVIRONMENT,
        "title": title,
        "state": state,
        "started_unix": now() - 300,
        "after_message": 1,
    });
    if state != "working" {
        task["finished_unix"] = json!(now() - 20);
    }
    task
}

fn task_chat(state: &str) -> Conversation {
    conversation(
        6,
        "Fix the sample login bug",
        vec![
            user(
                "Fix the login bug in the sample app and open a pull request.",
                "r1",
            ),
            assistant(
                "I'm on it. Claude Code is working on this in your project.",
                "r1",
            ),
        ],
        json!({
            "requests": [answered("r1")],
            "project": PROJECT,
            "environment": { "id": ENVIRONMENT, "repository": "sample-org/sample-app" },
            "tasks": [task(state, "Fix the sample login bug", "claude")],
        }),
    )
}

fn terminal_chat(extra: Vec<Value>) -> Conversation {
    let mut messages = vec![
        user("Deploy the sample site to staging.", "r1"),
        json!({ "role": "tool", "text": "Ran the sample build. All checks passed." }),
        assistant("The build passed. I'm ready to deploy to staging.", "r1"),
    ];
    messages.extend(extra);
    conversation(
        7,
        "Deploy the sample site",
        messages,
        json!({
            "requests": [answered("r1")],
            "terminal": {
                "computer": COMPUTER,
                "session": "sample-session",
                "title": "Deploy the sample site",
                "digest": "sample",
            },
        }),
    )
}

fn files_chat() -> Conversation {
    conversation(
        8,
        "Review my photos",
        vec![
            user("Which of these photos is sharper?", "r1"),
            assistant("The second one: the edges of the leaves are crisper.", "r1"),
        ],
        json!({ "requests": [{
            "id": "r1", "digest": "sample", "outcome": "answered",
            "files": [
                { "id": "0123456789abcdef0123456789abcdef", "name": "sample-photo-1.jpg", "kind": "jpeg", "size": 482_113 },
                { "id": "fedcba9876543210fedcba9876543210", "name": "sample-photo-2.jpg", "kind": "jpeg", "size": 501_902 },
                { "id": "00112233445566778899aabbccddeeff", "name": "notes.txt", "kind": "text", "size": 1_204 },
            ],
        }] }),
    )
}

fn project(id: &str, name: &str) -> oa_auth::repos::Project {
    oa_auth::repos::Project {
        id: id.to_owned(),
        name: name.to_owned(),
        repository_id: 1,
        repository: format!("sample-org/{name}"),
        default_branch: "main".to_owned(),
        private: false,
        created_unix: now() - 86_400 * 30,
        installation_id: None,
    }
}

fn projects() -> Sidebar {
    Sidebar {
        status: oa_auth::repos::Status {
            access: oa_auth::repos::Access::Connected {
                login: "sample-person".to_owned(),
                private: false,
            },
            projects: vec![
                project(PROJECT, "sample-app"),
                project(OTHER_PROJECT, "sample-site"),
            ],
        },
        closed: Default::default(),
    }
}

/// Every sample chat the list shows: one pinned, two in projects, the rest
/// across today, yesterday, and earlier.
fn rows() -> Vec<Conversation> {
    let mut pinned = answer_chat();
    pinned.pinned_unix = Some(now() - 60);
    let mut in_project = task_chat("working");
    in_project.project = Some(PROJECT.to_owned());
    let mut site = code_chat();
    site.project = Some(OTHER_PROJECT.to_owned());
    let mut yesterday = failed_chat("answered");
    yesterday.updated_unix = now() - 86_400 - 3_600;
    let mut earlier = long_chat();
    earlier.updated_unix = now() - 86_400 * 9;
    let mut archived = files_chat();
    archived.archived_unix = Some(now() - 600);
    vec![
        pinned,
        in_project,
        site,
        working_chat(),
        yesterday,
        earlier,
        archived,
    ]
}

fn list(rows: &[Conversation], current: Option<&str>, q: &str) -> Markup {
    let sidebar = projects();
    let view = chat::sidebar::View {
        current,
        hx: true,
        q,
        archived: None,
        projects: Some(&sidebar),
    };
    let list = chat::sidebar::build(ChatList::new().id("chat-sidebar"), rows, CSRF, view);
    html! { (list) }
}

fn dock(chat: &Conversation, row: Option<Markup>, notice: Option<&Notice>) -> Markup {
    chat::composer_with(
        &format!("/chat/{}", chat.id),
        "Continue this chat",
        row,
        html! { p #chat-feedback.oa-composer-feedback role="status" aria-live="polite" {} },
        notice,
    )
}

/// `chat`'s page, with the list beside it and the composer under it.
fn thread(headers: &HeaderMap, chat: &Conversation, sidebar: Vec<Conversation>) -> Response {
    let parts = Parts {
        breadcrumb: chat::work::breadcrumb(chat, None),
        actions: Some(chat::work::actions(chat, None, Some(false), false)),
        sidebar: Some(list(&sidebar, Some(&chat.id), "")),
        thread: chat::content(chat, None, html! {}, true),
        dock: dock(chat, None, None),
    };
    chat::chat_page(headers, chat, parts)
}

fn shared_answer(headers: &HeaderMap) -> Response {
    chat::share::shared_page(headers, &code_chat(), None)
}

fn thread_answer(headers: &HeaderMap) -> Response {
    thread(headers, &answer_chat(), rows())
}

fn thread_code(headers: &HeaderMap) -> Response {
    thread(headers, &code_chat(), rows())
}

fn thread_working(headers: &HeaderMap) -> Response {
    thread(headers, &working_chat(), rows())
}

fn thread_failed(headers: &HeaderMap) -> Response {
    thread(headers, &failed_chat("failed"), rows())
}

fn thread_unknown(headers: &HeaderMap) -> Response {
    thread(headers, &failed_chat("unknown"), rows())
}

fn thread_long(headers: &HeaderMap) -> Response {
    thread(headers, &long_chat(), rows())
}

fn thread_pinned(headers: &HeaderMap) -> Response {
    let mut chat = answer_chat();
    chat.pinned_unix = Some(now() - 60);
    thread(headers, &chat, rows())
}

fn thread_archived(headers: &HeaderMap) -> Response {
    let mut chat = files_chat();
    chat.archived_unix = Some(now() - 600);
    thread(headers, &chat, rows())
}

fn thread_delete(headers: &HeaderMap) -> Response {
    let chat = answer_chat();
    chat::delete_page(headers, &chat, CSRF, list(&rows(), Some(&chat.id), ""))
}

fn thread_delete_coder(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(Vec::new());
    chat::delete_page(headers, &chat, CSRF, list(&rows(), Some(&chat.id), ""))
}

fn offer() -> chat::work::Offer {
    chat::work::Offer {
        id: ENVIRONMENT.into(),
        repository: "sample-org/sample-app".into(),
        version: Some(3),
        runnable: true,
    }
}

fn work_run_claude(headers: &HeaderMap) -> Response {
    let chat = task_chat("done");
    chat::work::run_form_page(CSRF, headers, &chat, &offer(), "", 1, None)
}

fn work_run_claude_error(headers: &HeaderMap) -> Response {
    let chat = task_chat("done");
    chat::work::run_form_page(
        CSRF,
        headers,
        &chat,
        &offer(),
        "",
        2,
        Some("Write what Claude Code should do."),
    )
}

fn work_continue(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(Vec::new());
    chat::continued::form_page(CSRF, headers, &chat, COMPUTER, &offer(), "", None)
}

fn work_running(headers: &HeaderMap) -> Response {
    thread(headers, &task_chat("working"), rows())
}

fn work_finished(headers: &HeaderMap) -> Response {
    thread(headers, &task_chat("done"), rows())
}

fn work_stopped(headers: &HeaderMap) -> Response {
    thread(headers, &task_chat("stopped"), rows())
}

fn work_files(headers: &HeaderMap) -> Response {
    thread(headers, &files_chat(), rows())
}

/// A Coder chat while Coder is online: `card` over the reply composer.
fn terminal(headers: &HeaderMap, chat: &Conversation, card: Markup) -> Response {
    let parts = Parts {
        breadcrumb: Breadcrumb::new(chat.title.clone()),
        actions: None,
        sidebar: Some(list(&rows(), Some(&chat.id), "")),
        thread: chat::content(chat, None, html! {}, true),
        dock: html! {
            (card)
            (chat::terminal_composer(chat, COMPUTER, None, html! {}))
        },
    };
    chat::chat_page(headers, chat, parts)
}

fn approval_item() -> Item {
    Item {
        id: "sample-item".to_owned(),
        kind: "task".to_owned(),
        title: "Deploy the sample site".to_owned(),
        engine: None,
        status: "waiting".to_owned(),
        started_unix: now() - 120,
        finished_unix: None,
        cost_usd: None,
        tokens: None,
        session: Some("sample-session".to_owned()),
        question: Some(Question {
            id: "sample-question".to_owned(),
            text: "Deploy the sample site to staging?\nThis replaces what staging shows now."
                .to_owned(),
        }),
        line: None,
        choices: Vec::new(),
    }
}

fn work_approval(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(Vec::new());
    let card = chat::approval::card(CSRF, &chat, Some(&approval_item()));
    terminal(headers, &chat, card)
}

fn work_approved(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(vec![
        json!({ "role": "tool", "text": "You approved the deploy. Deployed the sample site to staging." }),
        assistant("Staging now shows the new sample site.", "r1"),
    ]);
    let card = chat::approval::card(CSRF, &chat, None);
    terminal(headers, &chat, card)
}

fn work_denied(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(vec![
        json!({ "role": "tool", "text": "You denied the deploy." }),
        assistant("Okay, I didn't deploy. Staging is unchanged.", "r1"),
    ]);
    let card = chat::approval::card(CSRF, &chat, None);
    terminal(headers, &chat, card)
}

fn work_offline(headers: &HeaderMap) -> Response {
    let chat = terminal_chat(Vec::new());
    let parts = Parts {
        breadcrumb: Breadcrumb::new(chat.title.clone()),
        actions: None,
        sidebar: Some(list(&rows(), Some(&chat.id), "")),
        thread: chat::content(&chat, None, html! {}, true),
        dock: chat::terminal_note(COMPUTER),
    };
    chat::chat_page(headers, &chat, parts)
}

fn choices(computers: &[&str]) -> Choices {
    Choices {
        projects: vec![
            project(PROJECT, "sample-app"),
            project(OTHER_PROJECT, "sample-site"),
        ],
        connected: true,
        environments: vec![Environment {
            id: ENVIRONMENT.to_owned(),
            repository: "sample-org/sample-app".to_owned(),
            branch: "main".to_owned(),
            version: 1,
        }],
        computers: computers.iter().map(|c| (*c).to_owned()).collect(),
        offline: Vec::new(),
        account: true,
        claude_connect: false,
        claude_subscription: false,
        claude_signed_in: Vec::new(),
        claude_keys: false,
    }
}

fn nothing_picked() -> Picked {
    Picked {
        project: None,
        branch: None,
        target: Target::Chat,
    }
}

fn app_picked() -> Picked {
    Picked {
        project: Some(project(PROJECT, "sample-app")),
        branch: Some("main".to_owned()),
        target: Target::Coder(COMPUTER.to_owned()),
    }
}

fn row(choices: &Choices, picked: &Picked) -> Option<Markup> {
    Some(crate::composer_row::row(
        Some(choices),
        picked,
        None,
        None,
        false,
    ))
}

/// The new chat's page around `row`, `notice`, and `before` the composer.
fn home(
    headers: &HeaderMap,
    row: Option<Markup>,
    notice: Option<&Notice>,
    after: Markup,
    sidebar: &[Conversation],
) -> Response {
    let dock = html! {
        (chat::composer_with("/chat", "Start a chat", row, after, notice))
        (crate::ui_page::legal_links())
    };
    crate::pages::home::stage(dock)
        .account(signed_in())
        .sidebar_section(list(sidebar, None, ""))
        .respond(headers)
}

fn new_no_computers(headers: &HeaderMap) -> Response {
    home(
        headers,
        row(&choices(&[]), &nothing_picked()),
        None,
        html! {},
        &rows(),
    )
}

fn new_online(headers: &HeaderMap) -> Response {
    home(
        headers,
        row(&choices(&[COMPUTER]), &nothing_picked()),
        None,
        html! {},
        &rows(),
    )
}

fn new_offline(headers: &HeaderMap) -> Response {
    // A computer that isn't online isn't offered: Where it runs is here.
    let picked = Picked {
        project: Some(project(PROJECT, "sample-app")),
        branch: Some("main".to_owned()),
        target: Target::Chat,
    };
    home(
        headers,
        row(&choices(&[]), &picked),
        None,
        html! {},
        &rows(),
    )
}

fn new_project(headers: &HeaderMap) -> Response {
    home(
        headers,
        row(&choices(&[COMPUTER]), &app_picked()),
        None,
        html! {},
        &rows(),
    )
}

fn composer_empty(headers: &HeaderMap) -> Response {
    home(headers, None, None, html! {}, &[])
}

fn composer_typing(headers: &HeaderMap) -> Response {
    let notice = Notice {
        text: String::new(),
        draft: "Plan a quiet weekend in the mountains".to_owned(),
    };
    home(headers, None, Some(&notice), html! {}, &[])
}

fn composer_refused(headers: &HeaderMap) -> Response {
    let notice = Notice {
        text: "That message is too long. Shorten it and send again.".to_owned(),
        draft: "A very long sample message".to_owned(),
    };
    home(headers, None, Some(&notice), html! {}, &[])
}

fn composer_row(headers: &HeaderMap) -> Response {
    home(
        headers,
        row(&choices(&[COMPUTER]), &app_picked()),
        None,
        html! {},
        &[],
    )
}

fn composer_panel(headers: &HeaderMap, kind: &str) -> Response {
    let choices = choices(&[COMPUTER]);
    let picked = app_picked();
    let panel = crate::composer_row::panel(kind, &choices, &picked, None);
    home(headers, row(&choices, &picked), None, panel, &[])
}

fn composer_targets(headers: &HeaderMap) -> Response {
    composer_panel(headers, "target")
}

fn composer_projects(headers: &HeaderMap) -> Response {
    composer_panel(headers, "project")
}

fn composer_connect(headers: &HeaderMap) -> Response {
    let mut choices = choices(&[]);
    choices.claude_connect = true;
    choices.claude_subscription = true;
    let picked = Picked {
        project: Some(project(PROJECT, "sample-app")),
        branch: Some("main".to_owned()),
        target: Target::Claude(ENVIRONMENT.to_owned()),
    };
    let card = crate::composer_row::connect_card(ENVIRONMENT, true, false, None);
    home(headers, row(&choices, &picked), None, card, &[])
}

fn composer_suggestions(headers: &HeaderMap) -> Response {
    let starters = crate::suggestions::starter_chips(&[], |message| {
        vec![("q".to_owned(), message.to_owned())]
    });
    let dock = html! {
        (starters)
        (chat::composer("/chat", "Start a chat", None, html! {}))
        (crate::ui_page::legal_links())
    };
    crate::pages::home::stage(dock)
        .account(signed_in())
        .respond(headers)
}

fn composer_terminal(headers: &HeaderMap) -> Response {
    terminal(headers, &terminal_chat(Vec::new()), html! {})
}

fn composer_note(headers: &HeaderMap) -> Response {
    work_offline(headers)
}

/// The chat list page beside an empty new chat.
fn sidebar_page(headers: &HeaderMap, list: Markup) -> Response {
    let dock = chat::composer("/chat", "Start a chat", None, html! {});
    crate::pages::home::stage(dock)
        .account(signed_in())
        .sidebar_section(list)
        .respond(headers)
}

fn sidebar_empty(headers: &HeaderMap) -> Response {
    sidebar_page(headers, list(&[], None, ""))
}

fn sidebar_grouped(headers: &HeaderMap) -> Response {
    sidebar_page(headers, list(&rows(), Some(&id(1)), ""))
}

fn sidebar_search(headers: &HeaderMap) -> Response {
    sidebar_page(headers, list(&rows(), None, "sample"))
}

fn sidebar_search_none(headers: &HeaderMap) -> Response {
    sidebar_page(headers, list(&rows(), None, "volcano"))
}

fn sidebar_rename(headers: &HeaderMap) -> Response {
    let chat = answer_chat();
    let field = chat::sidebar::rename_row(&chat, "", CSRF);
    chat::sidebar::rename_page(headers, &chat, field, list(&rows(), None, ""))
}

fn archived_rows() -> Vec<Conversation> {
    let mut a = files_chat();
    a.archived_unix = Some(now() - 600);
    let mut b = long_chat();
    b.archived_unix = Some(now() - 86_400 * 3);
    vec![a, b]
}

fn sidebar_archived(headers: &HeaderMap) -> Response {
    chat::sidebar::archived_page(
        headers,
        &archived_rows(),
        CSRF,
        false,
        Some(list(&rows(), None, "")),
    )
}

fn sidebar_archived_empty(headers: &HeaderMap) -> Response {
    chat::sidebar::archived_page(headers, &[], CSRF, false, Some(list(&[], None, "")))
}

fn sidebar_delete_all(headers: &HeaderMap) -> Response {
    let rows = rows();
    // Signed out, this page deletes every chat saved in the browser.
    let visitor = "5a".repeat(16);
    chat::delete_all::confirm_page(
        headers,
        Some(&visitor),
        rows.len(),
        CSRF,
        Some(list(&rows, None, "")),
    )
}

fn sidebar_move(headers: &HeaderMap) -> Response {
    let chat = task_chat("working");
    chat::sidebar::move_page_for(
        headers,
        &chat,
        &projects().status.projects,
        CSRF,
        list(&rows(), Some(&chat.id), ""),
    )
}

fn sidebar_move_none(headers: &HeaderMap) -> Response {
    let chat = answer_chat();
    chat::sidebar::move_page_for(headers, &chat, &[], CSRF, list(&rows(), Some(&chat.id), ""))
}

fn run_page(
    headers: &HeaderMap,
    state: &str,
    lines: &[(f64, &str)],
    outcome: Value,
    why: Option<&str>,
) -> Response {
    let run = work_run(state, lines, outcome, why);
    let body = crate::work_runs::run_body(&run, now(), CSRF);
    crate::work_runs::framed(
        headers,
        signed_in(),
        "Work run",
        "/work/run-sample-1",
        false,
        body,
    )
}

fn runs_page(headers: &HeaderMap, runs: &[crate::work_runs::WorkRun], saved: bool) -> Response {
    let body = crate::work_runs::list_body(runs, saved, "sample-org/sample-app", "", CSRF, now());
    crate::work_runs::framed(
        headers,
        signed_in(),
        "Work on issues",
        crate::work_runs::PAGE,
        false,
        body,
    )
}

fn runs_empty(headers: &HeaderMap) -> Response {
    runs_page(headers, &[], false)
}

fn runs_list(headers: &HeaderMap) -> Response {
    let runs = [
        work_run("running", LINES, Value::Null, None),
        work_run(
            "failed",
            LINES,
            Value::Null,
            Some("The tests failed twice."),
        ),
    ];
    runs_page(headers, &runs, true)
}

fn work_run(
    state: &str,
    lines: &[(f64, &str)],
    outcome: Value,
    why: Option<&str>,
) -> crate::work_runs::WorkRun {
    let created = now() - 900;
    let finished = state != "waiting" && state != "running";
    let value = json!({
        "schema": "openagents.web.work-run.v1",
        "id": "run-sample-1",
        "owner": owner(),
        "requester": { "account": "sample-account", "workspace": "sample-workspace", "members_epoch": 1 },
        "repo": "sample-org/sample-app",
        "issue": 42,
        "title": "Fix the sample login bug",
        "land": "pr",
        "engine": "briefed",
        "source": "web",
        "created_unix": created,
        "updated_unix": now() - 30,
        "state": state,
        "started_unix": if state == "waiting" { Value::Null } else { json!(created + 30) },
        "lines": lines.iter().map(|(secs, text)| json!({ "secs": secs, "text": text })).collect::<Vec<_>>(),
        "outcome": outcome,
        "why": why,
        "cancel": state == "cancelled",
        "finished_unix": if finished { json!(now() - 30) } else { Value::Null },
    });
    serde_json::from_value(value).expect("a sample run")
}

const LINES: &[(f64, &str)] = &[
    (0.0, "Reading the issue"),
    (12.0, "Planning the change"),
    (95.0, "Editing src/login.rs"),
    (240.0, "Running the tests"),
];

fn run_queued(headers: &HeaderMap) -> Response {
    run_page(headers, "waiting", &[], Value::Null, None)
}

fn run_running(headers: &HeaderMap) -> Response {
    run_page(headers, "running", LINES, Value::Null, None)
}

fn run_finished(headers: &HeaderMap) -> Response {
    let outcome = json!({
        "engine": "briefed",
        "ok": true,
        "cost_usd": 0.42,
        "secs": 310.0,
        "checks": [{ "name": "Tests", "ok": true, "passed": 128 }, { "name": "Format", "ok": true }],
        "files": ["src/login.rs", "src/session.rs", "tests/login.rs"],
        "added": 42,
        "removed": 7,
        "pr": "https://github.com/sample-org/sample-app/pull/43",
    });
    run_page(headers, "done", LINES, outcome, None)
}

fn run_failed(headers: &HeaderMap) -> Response {
    let outcome = json!({
        "engine": "briefed",
        "ok": false,
        "secs": 280.0,
        "checks": [{ "name": "Tests", "ok": false, "passed": 120 }],
    });
    run_page(
        headers,
        "failed",
        LINES,
        outcome,
        Some("The tests failed after the change."),
    )
}

fn run_stopped(headers: &HeaderMap) -> Response {
    run_page(
        headers,
        "cancelled",
        &LINES[..2],
        Value::Null,
        Some("You stopped this run."),
    )
}

fn demo_index(headers: &HeaderMap) -> Response {
    crate::demo::sample(headers, "environment")
}

fn demo_chat(headers: &HeaderMap) -> Response {
    crate::demo::sample(headers, "lease-fix")
}

fn demo_missing(headers: &HeaderMap) -> Response {
    crate::demo::sample(headers, "no-such-chat")
}
