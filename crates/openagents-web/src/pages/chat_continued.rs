//! Continue a Coder chat on a Cloud computer (#11050).
//!
//! A chat synced from Coder (#11046) is answered by Coder on its own
//! computer; while Coder there is online, a reply typed on the website
//! waits for it (#11048). When that computer is offline and the chat's
//! project has a saved environment (`/environments`, #11037), the chat's
//! page offers **Continue on a Cloud computer** (`/chat/{id}/continue`):
//!
//! - the person writes what's next; it joins the chat as their message;
//! - Claude Code runs in the environment's saved version on a fresh
//!   computer, with the chat so far carried into its prompt ([`context`]);
//! - the thread shows the run as a task row (Cloud computer, Working …),
//!   like #11037's, linking to the run for the people allowed agent work;
//! - when the run is done, its answer joins the chat as the next message
//!   ([`super::work::observe`]), in the same write that marks it done.
//!
//! Both messages are also kept on the chat's Coder record
//! (`Terminal::continued`), so Coder's next upload of its own transcript,
//! which doesn't have them yet, keeps them after it
//! (`crate::coder_sync::save`). The chat is marked as waiting for Coder
//! ([`crate::coder_sync::mark_waiting`]): when Coder on the computer is
//! back, it takes them into its own copy with the replies typed on the
//! website (#11052), and each is dropped here once Coder's upload carries
//! it.
//!
//! The offer shows only when it can really work: environments are set up
//! here and the request may do agent work (`crate::agent_work`: the local
//! address, or a signed-in site admin on a public host), a Claude key is available (the person's
//! own, saved in Settings, or this server's), the
//! project's environment has a saved version, Coder on the computer hasn't
//! checked in lately, and nothing is answering the chat now.

use coder_environment_operator::studio::claude::MAX_PROMPT;
use openagents_ui::actions::{ButtonLink, ButtonVariant, ControlSize};
use openagents_ui::forms::{Field, Textarea};

use crate::chat_store::{
    ChatEnvironment, ChatTask, MAX_CONTINUED, MAX_CONTINUED_BYTES, TaskKind, TaskState,
};
use crate::coder_sync::MAX_MESSAGES;

use super::work::{self, Offer};
use super::*;

/// The longest message the person may write here, in bytes; the rest of
/// the run's prompt carries the chat so far.
pub(super) const MAX_NEXT: usize = 6 * 1024;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/chat/{id}/continue", get(page).post(start))
}

/// The chat is a Coder chat that may continue elsewhere now: not deleted,
/// its computer not online, no reply waiting for Coder, and nothing
/// answering it.
pub(super) fn may_continue(chat: &Conversation, online: bool) -> bool {
    !online
        && chat
            .terminal
            .as_ref()
            .is_some_and(|terminal| terminal.deleted_unix.is_none() && terminal.replies.is_empty())
        && !chat.working()
        && !work::running(chat)
}

/// Whether Coder on the chat's computer checked in lately.
pub(super) async fn online(app: &App, chat: &Conversation) -> bool {
    let Some(terminal) = &chat.terminal else {
        return false;
    };
    app.config
        .chat_store
        .computers(&chat.owner)
        .await
        .is_ok_and(|computers| crate::coder_sync::online(&computers, &terminal.computer))
}

/// The environment a Coder chat may continue in, when it can start now.
pub(super) async fn offer(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    online: bool,
) -> Option<Offer> {
    if !may_continue(chat, online) || !work::links(app, headers).await {
        return None;
    }
    let studio = app.config.environments.as_ref()?.clone();
    if !crate::environments::claude_ready(app, &studio, headers).await {
        return None;
    }
    let rows = crate::agent_work::scope(app, headers).await?.rows(&studio);
    let projects = crate::projects::sidebar(app).await;
    let row = work::pick(&rows, chat, work::repository(chat, projects.as_deref()))?;
    let saved = row.saved?;
    Some(Offer {
        id: row.id.clone(),
        repository: row.repository.clone(),
        version: Some(saved),
        runnable: true,
    })
}

/// What a Coder chat's page shows under the note when it may continue on
/// a Cloud computer.
pub(super) fn button(chat: &Conversation) -> Markup {
    html! {
        p #chat-continue {
            (ButtonLink::new("Continue on a Cloud computer", format!("/chat/{}/continue", chat.id))
                .color(Color::Secondary)
                .variant(ButtonVariant::Outline)
                .size(ControlSize::Sm)
                .pill(true))
        }
    }
}

/// Adds a message the website wrote to a Coder chat: to its transcript and
/// to the messages its next upload keeps. An answer that looks like it
/// holds a credential is not shown.
pub(super) fn add_message(chat: &mut Conversation, role: Role, text: &str) {
    let text = if secret_screen::credential_in(text).is_some() {
        "This answer looked like it held a password or key, so it isn't shown here.".to_owned()
    } else {
        cut(text.trim(), MAX_CONTINUED_BYTES)
    };
    let message = Message {
        role,
        text,
        request_id: None,
    };
    chat.messages.push(message.clone());
    if chat.messages.len() > MAX_MESSAGES {
        chat.messages.drain(..chat.messages.len() - MAX_MESSAGES);
    }
    if let Some(terminal) = &mut chat.terminal {
        terminal.continued.push(message);
        if terminal.continued.len() > MAX_CONTINUED {
            let extra = terminal.continued.len() - MAX_CONTINUED;
            terminal.continued.drain(..extra);
            terminal.continued_taken = terminal.continued_taken.saturating_sub(extra);
        }
    }
    chat.updated_unix = now();
}

/// `text` cut to at most `limit` bytes on a character boundary.
fn cut(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Claude Code's prompt: what this is, the chat so far (the newest
/// messages that fit `budget` bytes in all, oldest first; tool output
/// left out), and the person's new message.
pub(super) fn context(messages: &[Message], computer: &str, next: &str, budget: usize) -> String {
    let head = format!(
        "This continues a conversation from Coder on {computer}, which is offline now. \
         You are on a fresh computer with the repository. The conversation so far, \
         oldest first:\n\n"
    );
    context_with(head, messages, next, budget)
}

/// [`context`] with its own opening paragraph.
pub(super) fn context_with(
    head: String,
    messages: &[Message],
    next: &str,
    budget: usize,
) -> String {
    const LEFT_OUT: &str = "(Earlier messages are left out.)\n\n";
    let tail = format!("Now the person says:\n\n{}", next.trim());
    let room = budget.saturating_sub(head.len() + tail.len() + LEFT_OUT.len());
    let spoken: Vec<&Message> = messages
        .iter()
        .filter(|message| message.role != Role::Tool && !message.text.trim().is_empty())
        .collect();
    let mut kept = Vec::new();
    let mut used = 0;
    for message in spoken.iter().rev() {
        let who = match message.role {
            Role::User => "Person",
            _ => "Assistant",
        };
        let entry = format!("{who}: {}\n\n", message.text.trim());
        if used + entry.len() > room {
            // The newest message alone is cut rather than left out.
            if kept.is_empty() && room > who.len() + 8 {
                kept.push(format!("{}…\n\n", cut(&entry, room - 8).trim_end()));
            }
            break;
        }
        used += entry.len();
        kept.push(entry);
    }
    let mut prompt = head;
    if kept.len() < spoken.len() {
        prompt.push_str(LEFT_OUT);
    }
    if spoken.is_empty() {
        prompt.push_str("(No messages yet.)\n\n");
    }
    for entry in kept.iter().rev() {
        prompt.push_str(entry);
    }
    prompt.push_str(&tail);
    prompt
}

/// Records the person's message and the run on the chat.
pub(super) fn record(
    chat: &mut Conversation,
    environment: ChatEnvironment,
    task: ChatTask,
    next: &str,
) {
    add_message(chat, Role::User, next);
    work::record(chat, environment, task);
}

#[derive(Deserialize)]
struct ContinueForm {
    csrf: String,
    environment: String,
    prompt: String,
}

/// The chat, its computer, and the environment it may continue in.
async fn continuable(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
) -> Result<(Conversation, String, Offer), Response> {
    let loaded = load_owned(app, owner, id).await?;
    let chat = loaded.conversation;
    let Some(computer) = chat.terminal.as_ref().map(|t| t.computer.clone()) else {
        return Err(missing());
    };
    let online = online(app, &chat).await;
    match offer(app, headers, &chat, online).await {
        Some(offer) => Ok((chat, computer, offer)),
        None if online => Err(refusal(
            StatusCode::CONFLICT,
            &format!("Coder on {computer} is online again. Reply in the chat."),
        )),
        None if chat.terminal.is_some() && work::links(app, headers).await => Err(refusal(
            StatusCode::CONFLICT,
            "This chat can't continue on a Cloud computer now. Go back to the chat.",
        )),
        None => Err(missing()),
    }
}

fn form(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    computer: &str,
    offer: &Offer,
    prompt: &str,
    error: Option<&str>,
) -> Response {
    let id = &chat.id;
    let field = Field::new("chat-continue-prompt", "What next?").error_opt(error);
    let aria = field.aria();
    let field = field.control(
        Textarea::new("prompt")
            .id("chat-continue-prompt")
            .rows(4)
            .maxlength(MAX_NEXT as u32)
            .value(prompt)
            .aria(aria),
    );
    let page = UiPage::new("Continue on a Cloud computer")
        .path(format!("/chat/{id}/continue"))
        .head(crate::chat_html::head())
        .breadcrumb(
            Breadcrumb::new("Continue on a Cloud computer")
                .crumb(chat.title.clone(), format!("/chat/{id}")),
        )
        .content(crate::ui_page::prose(html! {
            h1 { "Continue on a Cloud computer" }
            p {
                "Coder on " (computer) " isn't online. Claude Code continues this chat in "
                a href=(format!("/environments/{}", offer.id)) { (offer.label()) }
                " on a fresh computer. It reads the chat so far, and its answer shows in this chat."
            }
            form method="post" action=(format!("/chat/{id}/continue")) {
                input type="hidden" name="csrf" value=(csrf(app, &chat.owner));
                input type="hidden" name="environment" value=(offer.id);
                (field)
                (Button::new("Continue").kind(ButtonType::Submit))
            }
        }))
        .status(if error.is_some() {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::OK
        });
    crate::chat_html::protect(page.respond(headers))
}

async fn page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    match continuable(&app, &headers, &owner, &id).await {
        Ok((chat, computer, offer)) => form(&app, &headers, &chat, &computer, &offer, "", None),
        Err(response) => response,
    }
}

/// Why the person's message can't be sent, if it can't.
pub(super) fn check(next: &str) -> Result<(), &'static str> {
    if next.is_empty() {
        return Err("Write what's next.");
    }
    if next.len() > MAX_NEXT {
        return Err("Write at most 6 KB.");
    }
    if secret_screen::credential_in(next).is_some() {
        return Err("This looks like it holds a password or key, so it wasn't sent.");
    }
    Ok(())
}

async fn start(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(posted): Form<ContinueForm>,
) -> Response {
    let owner = match validate_form(&app, &headers, &posted.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let (chat, computer, offer) = match continuable(&app, &headers, &owner, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if offer.id != posted.environment {
        return refusal(
            StatusCode::CONFLICT,
            "This chat's environment changed. Reload the page.",
        );
    }
    let Some(studio) = app.config.environments.clone() else {
        return missing();
    };
    let next = posted.prompt.trim();
    if let Err(error) = check(next) {
        return form(&app, &headers, &chat, &computer, &offer, next, Some(error));
    }
    let prompt = context(&chat.messages, &computer, next, MAX_PROMPT - 512);
    let own = crate::cloud::byo::run_key(&app, &headers).await;
    let run = match studio.run_claude(&offer.id, &prompt, own) {
        Ok(run) => run,
        Err(error) => {
            return form(
                &app,
                &headers,
                &chat,
                &computer,
                &offer,
                next,
                Some(error.as_str()),
            );
        }
    };
    let version = studio
        .claude_run(&offer.id, &run)
        .and_then(|run| run.version)
        .or(offer.version);
    let environment = ChatEnvironment {
        id: offer.id.clone(),
        repository: offer.repository.clone(),
        version,
        removed: false,
    };
    let task = ChatTask {
        id: run,
        kind: TaskKind::Continue,
        environment: offer.id.clone(),
        title: work::title(next),
        state: TaskState::Working,
        started_unix: now(),
        after_message: 0,
        version,
        finished_unix: None,
    };
    let recorded = sidebar::update(&app, &owner, &id, |chat| {
        if chat.terminal.is_none() {
            return false;
        }
        record(chat, environment.clone(), task.clone(), next);
        true
    })
    .await;
    let chat = match recorded {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    // Coder takes the message into its own copy when it is back.
    let _ = crate::coder_sync::mark_waiting(&app.config.chat_store, &owner, &chat).await;
    work::watch(app, owner, id.clone());
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

#[cfg(test)]
mod tests {
    use super::work::Seen;
    use super::*;
    use crate::chat_store::Terminal;

    fn coder_chat() -> Conversation {
        Conversation {
            id: "11111111-1111-4111-8111-111111111111".into(),
            owner: "v_owner".into(),
            revision: 1,
            title: "Fix the build".into(),
            messages: vec![
                message(Role::User, "Why does the build fail?"),
                message(Role::Tool, "cargo build\nerror[E0425]"),
                message(Role::Assistant, "A missing import in lib.rs."),
            ],
            pending: None,
            requests: Vec::new(),
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            terminal: Some(Terminal {
                computer: "Studio".into(),
                session: "coder-new-1".into(),
                title: "Fix the build".into(),
                digest: "d".repeat(64),
                working_unix: None,
                deleted_unix: None,
                replies: Vec::new(),
                reply_ids: Vec::new(),
                continued: Vec::new(),
                continued_taken: 0,
            }),
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
            branch: None,
        }
    }

    fn message(role: Role, text: &str) -> Message {
        Message {
            role,
            text: text.into(),
            request_id: None,
        }
    }

    fn environment() -> ChatEnvironment {
        ChatEnvironment {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            removed: false,
        }
    }

    fn task() -> ChatTask {
        ChatTask {
            id: "claude-env-1-1".into(),
            kind: TaskKind::Continue,
            environment: "env-1".into(),
            title: "Fix it".into(),
            state: TaskState::Working,
            started_unix: 1,
            after_message: 0,
            version: Some(3),
            finished_unix: None,
        }
    }

    #[test]
    fn only_an_offline_idle_coder_chat_may_continue() {
        let mut chat = coder_chat();
        assert!(may_continue(&chat, false));
        assert!(!may_continue(&chat, true), "online: reply to Coder instead");
        chat.terminal.as_mut().unwrap().working_unix = Some(now());
        assert!(!may_continue(&chat, false), "Coder is answering");
        chat.terminal.as_mut().unwrap().working_unix = None;
        chat.terminal
            .as_mut()
            .unwrap()
            .replies
            .push(crate::chat_store::WebReply {
                id: "r".into(),
                text: "waiting".into(),
                sent_unix: 1,
            });
        assert!(!may_continue(&chat, false), "a reply waits for Coder");
        chat.terminal.as_mut().unwrap().replies.clear();
        chat.tasks.push(task());
        assert!(!may_continue(&chat, false), "a run is going");
        chat.tasks[0].state = TaskState::Done;
        assert!(may_continue(&chat, false));
        chat.terminal.as_mut().unwrap().deleted_unix = Some(1);
        assert!(!may_continue(&chat, false));
        let mut web = coder_chat();
        web.terminal = None;
        assert!(
            !may_continue(&web, false),
            "web chats run Claude Code instead"
        );
    }

    #[test]
    fn the_run_reads_the_chat_so_far_newest_first_to_fit() {
        let chat = coder_chat();
        let prompt = context(&chat.messages, "Studio", " Fix it now. ", MAX_PROMPT);
        assert!(prompt.starts_with("This continues a conversation from Coder on Studio"));
        let asked = prompt.find("Person: Why does the build fail?").unwrap();
        let answered = prompt
            .find("Assistant: A missing import in lib.rs.")
            .unwrap();
        assert!(asked < answered, "{prompt}");
        assert!(!prompt.contains("error[E0425]"), "tool output is left out");
        assert!(prompt.ends_with("Now the person says:\n\nFix it now."));
        assert!(!prompt.contains("left out"));

        let long: Vec<Message> = (0..400)
            .map(|n| message(Role::User, &format!("message {n} {}", "x".repeat(100))))
            .collect();
        let prompt = context(&long, "Studio", "next", MAX_PROMPT - 512);
        assert!(prompt.len() <= MAX_PROMPT - 512, "{}", prompt.len());
        assert!(prompt.contains("(Earlier messages are left out.)"));
        assert!(prompt.contains("message 399 ") && !prompt.contains("message 0 "));

        let huge = vec![message(Role::Assistant, &"é".repeat(20_000))];
        let prompt = context(&huge, "Studio", "next", 4096);
        assert!(prompt.len() <= 4096 && prompt.contains("Assistant: é"));

        assert!(context(&[], "Studio", "next", 4096).contains("(No messages yet.)"));
    }

    #[test]
    fn the_message_and_the_answer_land_in_the_chat_once() {
        let mut chat = coder_chat();
        record(&mut chat, environment(), task(), "Fix it now.");
        assert_eq!(chat.messages.len(), 4);
        assert_eq!(chat.messages[3], message(Role::User, "Fix it now."));
        assert_eq!(
            chat.tasks[0].after_message, 4,
            "the row follows the message"
        );
        assert_eq!(chat.environment, Some(environment()));
        assert_eq!(chat.terminal.as_ref().unwrap().continued.len(), 1);

        assert!(observe_none(&chat, read(TaskState::Working, None)));
        let done = work::observe(
            &chat,
            read(TaskState::Done, Some("Fixed: added the import.")),
        )
        .expect("the run finished");
        assert_eq!(done.tasks[0].state, TaskState::Done);
        assert_eq!(
            done.messages.last(),
            Some(&message(Role::Assistant, "Fixed: added the import."))
        );
        assert_eq!(done.terminal.as_ref().unwrap().continued.len(), 2);
        // Finished: reading it again adds nothing.
        assert!(observe_none(&done, read(TaskState::Done, Some("again"))));

        let failed = work::observe(&chat, read(TaskState::Failed, Some("partial"))).unwrap();
        assert_eq!(failed.messages.len(), 4, "a failed run adds no answer");

        // A web chat's Claude Code run (from the header or the composer's
        // Where it runs) gets its answer in the chat too.
        let mut web = chat.clone();
        web.tasks[0].kind = TaskKind::Claude;
        let web = work::observe(&web, read(TaskState::Done, Some("answer"))).unwrap();
        assert_eq!(web.messages.len(), 5);
        assert_eq!(
            web.messages.last(),
            Some(&message(Role::Assistant, "answer"))
        );
    }

    fn read(state: TaskState, reply: Option<&'static str>) -> impl Fn(&str, &str) -> Option<Seen> {
        move |_, _| {
            Some(Seen {
                state,
                version: Some(3),
                reply: reply.map(str::to_owned),
            })
        }
    }

    fn observe_none(chat: &Conversation, read: impl Fn(&str, &str) -> Option<Seen>) -> bool {
        work::observe(chat, read).is_none()
    }

    #[test]
    fn an_answer_holding_a_credential_is_not_shown_and_sizes_stay_bounded() {
        let mut chat = coder_chat();
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        add_message(&mut chat, Role::Assistant, &format!("Use {key} as the key"));
        let shown = &chat.messages.last().unwrap().text;
        assert!(!shown.contains("sk-ant"), "{shown}");
        for n in 0..(MAX_CONTINUED + 5) {
            add_message(&mut chat, Role::User, &format!("m{n}"));
        }
        assert_eq!(
            chat.terminal.as_ref().unwrap().continued.len(),
            MAX_CONTINUED
        );
        add_message(&mut chat, Role::Assistant, &"é".repeat(MAX_CONTINUED_BYTES));
        assert!(chat.messages.last().unwrap().text.len() <= MAX_CONTINUED_BYTES);
        assert_eq!(check(""), Err("Write what's next."));
        assert!(check(&"x".repeat(MAX_NEXT + 1)).is_err());
        assert!(check("Fix it").is_ok());
    }

    #[test]
    fn the_offer_and_its_page_read_plainly() {
        let chat = coder_chat();
        let html = button(&chat).into_string();
        assert!(
            html.contains(&format!(r#"href="/chat/{}/continue""#, chat.id)),
            "{html}"
        );
        assert!(html.contains("Continue on a Cloud computer"));
        crate::copy_guard::assert_plain("/chat/x", &html);
        let note = html! { (terminal_note("Studio")) (button(&chat)) }.into_string();
        crate::copy_guard::assert_plain("/chat/x", &note);
        let mut chat = chat;
        record(&mut chat, environment(), task(), "Fix it now.");
        let rows = work::rows(&chat, 4, true).into_string();
        assert!(
            rows.contains("Cloud computer") && rows.contains("Fix it"),
            "{rows}"
        );
        assert!(rows.contains(r#"href="/environments/env-1/runs/claude-env-1-1""#));
        crate::copy_guard::assert_plain("/chat/x", &rows);
    }

    #[tokio::test]
    async fn continuing_is_hidden_on_a_public_host_and_needs_environments() {
        use tower::ServiceExt;
        let dir = tempfile::tempdir().unwrap();
        let mut config = crate::Config::development(dir.path().join("tasks"));
        config.public_hosts = vec!["openagents.com".into()];
        let path = format!("/chat/{}/continue", coder_chat().id);
        let send = |host: &'static str| {
            let config = config.clone();
            let path = path.clone();
            async move {
                let request = axum::http::Request::builder()
                    .uri(path)
                    .header(header::HOST, host)
                    .header("x-openagents-local", "1")
                    .body(axum::body::Body::empty())
                    .unwrap();
                crate::router(config)
                    .oneshot(request)
                    .await
                    .unwrap()
                    .status()
            }
        };
        // A public host without environments: nothing there (#11162).
        assert_eq!(send("openagents.com").await, StatusCode::NOT_FOUND);
        // Locally, without environments set up (or signed in), nothing.
        assert_eq!(send("127.0.0.1:4300").await, StatusCode::NOT_FOUND);
    }
}
