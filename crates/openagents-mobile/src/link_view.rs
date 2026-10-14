//! The account surface's screens (`crate::account_link`): sign in and
//! where this phone's chats live, the account's chats, one chat, what runs
//! on the computers, a message to one agent, memory, and scheduled
//! prompts. Rust Native views the hosts draw as they draw the Computers
//! screens.

use crate::account_link::{
    Agents, CLOUD_BOARD, Item, MessageFile, Open, Screen, SignIn, State, Thread, detail,
};
use openagents_chat::basic_coder::Turn;
use openagents_chat_app::projection::{Appearance, Projection, Reply};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};

use crate::account_link::Intent;

/// The most messages an open chat draws.
const SHOWN_MESSAGES: usize = 60;
/// The longest message drawn; the rest is cut.
const SHOWN_TEXT: usize = 6 * 1024;

fn ink() -> Color {
    openagents_chat_app::visual::inks().text
}

fn quiet() -> Color {
    openagents_chat_app::visual::inks().quiet
}

fn done() -> Color {
    openagents_chat_app::visual::inks().done
}

fn warn() -> Color {
    openagents_chat_app::visual::inks().warning
}

/// The surface for the screen `state` shows.
pub(crate) fn root(state: &State, composer: u64) -> Node<Intent> {
    draw(state, composer, SHOWN_MESSAGES, SHOWN_TEXT)
}

/// [`root`] with the newest few messages only, when the whole chat is too
/// big for one view.
pub(crate) fn root_small(state: &State, composer: u64) -> Node<Intent> {
    draw(state, composer, 12, 1024)
}

fn draw(state: &State, composer: u64, messages: usize, text: usize) -> Node<Intent> {
    let signed = state.session.is_some();
    match state.screen {
        _ if !signed => account(state),
        Screen::Account => account(state),
        Screen::Chats => chats(state),
        Screen::Chat => match &state.open {
            Some(open) => chat(state, open, composer, messages, text),
            None => chats(state),
        },
        Screen::Running => running(state),
        Screen::Message => message(state, composer),
        Screen::Memory => memory(state),
        Screen::Note => note(state, composer),
        Screen::Schedules => schedules(state),
        Screen::NewSchedule => new_schedule(state, composer),
        Screen::Projects => projects(state, composer),
    }
}

/// Sign in, or the account and where this phone's chats live.
fn account(state: &State) -> Node<Intent> {
    let mut children = vec![];
    match (&state.session, &state.sign_in) {
        (Some(session), _) => {
            children.push(bar(
                "link-account-header",
                Lead::Menu,
                "Your account",
                None,
                None,
            ));
            children.push(text(
                "link-signed-in",
                &format!("Signed in as {}", session.label),
                TextRole::Body,
                ink(),
                false,
            ));
            if state.origin != crate::account_link::PRODUCTION {
                children.push(status(
                    "link-site",
                    &format!("On {}", state.origin.trim_start_matches("https://")),
                    quiet(),
                ));
            }
            children.extend(notice(state));
            children.push(choice_card(state));
            children.push(button(
                "link-open-chats",
                "Your chats",
                Some(Glyph::History),
                Intent::Show {
                    screen: Screen::Chats,
                },
            ));
            children.push(button(
                "link-open-projects",
                "Projects",
                Some(Glyph::Folder),
                Intent::Show {
                    screen: Screen::Projects,
                },
            ));
            children.push(button(
                "link-open-running",
                "Running",
                Some(Glyph::Terminal),
                Intent::Show {
                    screen: Screen::Running,
                },
            ));
            children.push(button(
                "link-open-memory",
                "Memory",
                Some(Glyph::Person),
                Intent::Show {
                    screen: Screen::Memory,
                },
            ));
            children.push(button(
                "link-open-schedules",
                "Scheduled prompts",
                Some(Glyph::History),
                Intent::Show {
                    screen: Screen::Schedules,
                },
            ));
            children.push(button(
                "link-sign-out",
                "Sign out",
                Some(Glyph::Key),
                Intent::SignOut,
            ));
        }
        (None, SignIn::Waiting { user_code, .. }) => {
            children.push(bar(
                "link-account-header",
                Lead::Menu,
                "Approve this phone",
                None,
                None,
            ));
            children.push(text("link-code", user_code, TextRole::Heading, ink(), true));
            children.push(surface("link-qr", "link-qr", "Sign-in QR code"));
            children.push(status(
                "link-how",
                "Scan the code with a phone or computer where you're signed in, or enter the code at openagents.com/device.",
                quiet(),
            ));
            children.push(button(
                "link-open-page",
                "Approve on this phone",
                Some(Glyph::Check),
                Intent::OpenPage,
            ));
            children.push(button(
                "link-cancel",
                "Cancel",
                Some(Glyph::Back),
                Intent::CancelSignIn,
            ));
        }
        (None, SignIn::Starting) => {
            children.push(bar(
                "link-account-header",
                Lead::Menu,
                "Sign in",
                None,
                None,
            ));
            children.push(working("link-starting", "Getting a code…"));
        }
        (None, sign_in) => {
            children.push(bar(
                "link-account-header",
                Lead::Menu,
                "Sign in",
                None,
                None,
            ));
            children.push(text(
                "link-why",
                "See your chats from openagents.com and your computers, reply to them, and watch what Coder runs.",
                TextRole::Body,
                ink(),
                false,
            ));
            if let SignIn::Failed(error) = sign_in {
                children.push(status("link-error", error, warn()));
            }
            children.extend(notice(state));
            children.push(button(
                "link-sign-in",
                "Sign in",
                Some(Glyph::Person),
                Intent::SignIn,
            ));
        }
    }
    page(children)
}

/// "Where should this phone's chats live?", or the answer with a switch.
fn choice_card(state: &State) -> Node<Intent> {
    let mut lines = vec![text(
        "link-choice-title",
        "This phone's chats",
        TextRole::Body,
        ink(),
        true,
    )];
    if state.choice.is_none() && state.choice_read {
        lines.push(status(
            "link-choice-ask",
            "Sync them to your account, or keep them on this phone?",
            quiet(),
        ));
    }
    lines.push(choice(
        "link-choice-all",
        "Sync all my chats",
        Glyph::Cloud,
        state.choice == Some(true),
        Intent::Choose { all: true },
    ));
    lines.push(choice(
        "link-choice-local",
        "Keep chats on this phone",
        Glyph::Computer,
        state.choice == Some(false),
        Intent::Choose { all: false },
    ));
    card("link-choice", lines)
}

/// The account's chats, pinned first, then newest first, with **Show older
/// chats** while the website has more.
fn chats(state: &State) -> Node<Intent> {
    let mut children = vec![bar(
        "link-chats-header",
        Lead::of(state.from_account),
        "Your chats",
        None,
        None,
    )];
    children.extend(notice(state));
    if state.choice.is_none() && state.choice_read {
        children.push(choice_card(state));
    }
    // A chat started here on openagents.com takes photos and files with
    // its first message (#11349).
    children.push(button(
        "link-new-chat",
        "New chat",
        Some(Glyph::Compose),
        Intent::NewChat,
    ));
    if !state.threads_read {
        children.push(working("link-chats-loading", "Loading your chats…"));
        return page(children);
    }
    if state.threads.is_empty() {
        children.push(status(
            "link-chats-empty",
            "No chats on your account yet. Chat on openagents.com, or in Coder on a computer run coder login, then /sync on.",
            quiet(),
        ));
        return page(children);
    }
    // One list, pinned first, then newest first, as openagents.com lists
    // them (#11541); each row names its project and where it lives.
    let mut order: Vec<&Thread> = state.threads.iter().collect();
    order.sort_by_key(|t| std::cmp::Reverse((t.pinned, t.updated_unix)));
    for thread in order {
        let mut label = if thread.title.trim().is_empty() {
            "New chat".to_owned()
        } else {
            thread.title.clone()
        };
        label.push('\n');
        if thread.working {
            label.push_str("Working…");
        } else {
            label.push_str(&row_detail(state, thread));
        }
        children.push(button(
            &format!("link-thread-{}", thread.id),
            &label,
            None,
            Intent::Open {
                id: thread.id.clone(),
            },
        ));
    }
    if state.threads_more {
        children.push(button(
            "link-more-threads",
            "Show older chats",
            Some(Glyph::History),
            Intent::MoreThreads,
        ));
    }
    page(children)
}

/// A chat row's second line: its project, then where it lives, and for a
/// computer's chat whether that computer is offline.
fn row_detail(state: &State, thread: &Thread) -> String {
    let mut line = detail(state, thread);
    let offline = thread.surface == "terminal"
        && thread
            .computer
            .as_deref()
            .and_then(|name| state.computers.iter().find(|c| c.name == name))
            .is_some_and(|c| !c.online);
    if offline {
        line.push_str(" · Offline");
    }
    line
}

/// One chat: its messages, and a reply box when it takes replies.
fn chat(state: &State, open: &Open, composer: u64, limit: usize, cut_at: usize) -> Node<Intent> {
    let thread = open.thread.as_ref();
    let title = thread
        .map(|t| t.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            if open.new {
                "New chat".into()
            } else {
                "Chat".into()
            }
        });
    // A new chat in a project names it until the website has the chat.
    let where_ = thread.map(|thread| detail(state, thread)).or_else(|| {
        open.project
            .as_ref()
            .and_then(|id| state.projects.get(id))
            .map(|name| format!("In {name}"))
    });
    let mut children = vec![bar(
        "link-chat-header",
        Lead::of(state.from_list),
        &title,
        where_.as_deref().map(|line| ("link-chat-place", line)),
        Some(bar_button(
            "link-chat-new",
            "New chat",
            Glyph::Compose,
            Intent::NewChat,
        )),
    )];
    children.extend(notice(state));
    if let Some(error) = &open.error {
        children.push(status("link-chat-error", error, warn()));
        children.push(button(
            "link-retry",
            "Try again",
            Some(Glyph::History),
            Intent::Retry,
        ));
    }
    let mut rows: Vec<Node<Intent>> = vec![];
    if !open.loaded {
        rows.push(working("link-chat-loading", "Loading the chat…"));
    } else {
        let start = open.messages.len().saturating_sub(limit);
        // Each message's files list under it: `attached[0]` before the
        // first turn, `attached[n + 1]` after turn `n`.
        let mut turns: Vec<Turn> = vec![];
        let mut attached: Vec<Vec<&MessageFile>> = vec![vec![]];
        for m in open.messages[start..]
            .iter()
            // Steps and the model's thinking (#11543) stay on the website.
            .filter(|m| m.role != "tool" && m.role != "thinking")
            .chain(&open.sent)
        {
            if !m.text.trim().is_empty() {
                let text = cut(&m.text, cut_at);
                turns.push(if m.role == "user" {
                    Turn::user(text)
                } else {
                    Turn::assistant(text, None)
                });
                attached.push(vec![]);
            }
            if let Some(slot) = attached.last_mut() {
                slot.extend(&m.files);
            }
        }
        let busy = thread.is_some_and(|t| t.working) || !open.sent.is_empty() || open.waiting > 0;
        let label = match thread {
            Some(t) if t.surface == "terminal" && !t.working => {
                format!(
                    "Waiting for Coder on {}…",
                    t.computer.as_deref().unwrap_or("your computer")
                )
            }
            _ => "Working…".to_owned(),
        };
        let mut projection = Projection::default();
        rows = projection.rows(
            &turns,
            0,
            Reply {
                busy,
                partial: "",
                failure: None,
            },
            &Appearance {
                prefix: "link-m",
                body_suffix: "-md",
                streaming_key: "link-streaming".into(),
                working_key: "link-working",
                working_label: &label,
                failed_key: "link-failed",
                status_style: Style {
                    foreground: Some(quiet()),
                    ..Style::default()
                },
                markdown_style: Style {
                    foreground: Some(ink()),
                    ..Style::default()
                },
            },
        );
        let tail = rows.split_off(turns.len().min(rows.len()));
        let mut messages = std::mem::take(&mut rows).into_iter();
        for (slot, files) in attached.iter().enumerate() {
            if slot > 0 {
                rows.extend(messages.next());
            }
            for (n, file) in files.iter().enumerate() {
                rows.push(status(
                    &format!("link-file-{slot}-{n}"),
                    &file_line(file),
                    quiet(),
                ));
            }
        }
        rows.extend(messages);
        rows.extend(tail);
    }
    children.push(Node {
        key: "link-transcript".into(),
        style: Style::default(),
        element: Element::Transcript {
            label: "Messages".into(),
            children: rows,
            earlier: None,
            source: None,
        },
    });
    let (enabled, placeholder) = match thread {
        None if open.new => (true, "Message OpenAgents".to_owned()),
        Some(t) if t.can_reply => (true, "Reply".to_owned()),
        Some(t) if t.surface == "terminal" => (
            false,
            format!(
                "Open Coder on {} to reply",
                t.computer.as_deref().unwrap_or("that computer")
            ),
        ),
        Some(t) if t.surface == "phone" => (false, "This chat lives on its phone".into()),
        Some(t) if t.working => (false, "Wait for the answer".into()),
        Some(_) => (false, "This chat can't take replies here".into()),
        None => (false, "Reply".into()),
    };
    let sending = open.sending;
    // Photos and files for the reply (#11174): a web chat takes up to
    // four. Each shows as a pill that removes it; the attach control is
    // inside the composer, leading its field.
    let takes = crate::account_link::takes_photos(open);
    if takes {
        for (index, (name, _)) in open.photos.iter().enumerate() {
            children.push(button(
                &format!("link-photo-{index}"),
                &format!("Remove {name}"),
                Some(Glyph::Paperclip),
                Intent::RemovePhoto { index },
            ));
        }
    }
    let attach =
        takes && enabled && !sending && open.photos.len() < crate::account_link::MAX_PHOTOS;
    children.push(Node {
        key: "link-composer".into(),
        style: Style::default(),
        element: Element::Composer {
            token: format!("link-reply-{}-{composer}", open.id),
            placeholder,
            max_bytes: crate::account_link::MAX_REPLY_BYTES,
            enabled: enabled && !sending,
            busy: false,
            stop: None,
            choices: vec![],
            draft: None,
            focus: false,
            attach: attach.then_some(Intent::AddPhoto),
        },
    });
    page(children)
}

/// What runs on each computer: status, time, cost, and its controls.
fn running(state: &State) -> Node<Intent> {
    let mut children = vec![bar(
        "link-running-header",
        Lead::of(state.from_account),
        "Running",
        None,
        None,
    )];
    children.extend(notice(state));
    if !state.agents_read {
        children.push(working("link-running-loading", "Checking your computers…"));
        return page(children);
    }
    let now = crate::account_link::unix_now();
    let mut any = false;
    for (index, computer) in state.agents.iter().enumerate() {
        if computer.items.is_empty() {
            continue;
        }
        any = true;
        children.push(text(
            &format!("link-computer-{index}"),
            &board_title(computer),
            TextRole::Status,
            quiet(),
            true,
        ));
        let mut items: Vec<&Item> = computer.items.iter().collect();
        // What asks first, then what works, then the newest finished.
        items.sort_by_key(|i| {
            (
                match i.status.as_str() {
                    "asking" => 0,
                    "working" => 1,
                    _ => 2,
                },
                std::cmp::Reverse(i.finished_unix.unwrap_or(i.started_unix)),
            )
        });
        for item in items.into_iter().take(24) {
            children.push(item_card(computer, item, now));
        }
    }
    if !any {
        children.push(status(
            "link-running-empty",
            "Nothing is running on your computers. Coder shows its work here when it's signed in with coder login and syncing with /sync on.",
            quiet(),
        ));
        for (index, computer) in state
            .computers
            .iter()
            .filter(|c| c.name != state.name)
            .enumerate()
        {
            children.push(status(
                &format!("link-known-{index}"),
                &format!(
                    "{} · {}",
                    computer.name,
                    if computer.online { "Online" } else { "Offline" }
                ),
                quiet(),
            ));
        }
    }
    page(children)
}

fn item_card(computer: &Agents, item: &Item, now: u64) -> Node<Intent> {
    let key = format!("link-item-{}-{}", slug(&computer.name), slug(&item.id));
    let title = if item.title.trim().is_empty() {
        "Coder".to_owned()
    } else {
        item.title.clone()
    };
    let word = match item.status.as_str() {
        "working" => "Working",
        "asking" => "Needs you",
        "done" => "Done",
        "failed" => "Failed",
        "stopped" => "Stopped",
        _ => "Idle",
    };
    let end = if item.status == "working" || item.status == "asking" {
        now
    } else {
        item.finished_unix.unwrap_or(now)
    };
    let mut facts = vec![word.to_owned()];
    if item.started_unix > 0 {
        facts.push(elapsed(end.saturating_sub(item.started_unix)));
    }
    if let Some(cost) = item.cost_usd.filter(|c| *c > 0.0) {
        facts.push(format!("${cost:.2}"));
    }
    if let Some(engine) = item.engine.as_deref().and_then(engine_name) {
        facts.push(engine);
    }
    let color = match item.status.as_str() {
        "asking" | "failed" => warn(),
        "done" => done(),
        _ => quiet(),
    };
    let mut lines = vec![
        text(&format!("{key}-title"), &title, TextRole::Body, ink(), true),
        status(&format!("{key}-facts"), &facts.join(" · "), color),
    ];
    // A line that only repeats the status ("stopped") is left out.
    if let Some(line) = item
        .line
        .as_ref()
        .filter(|l| !l.trim().is_empty() && !l.trim().eq_ignore_ascii_case(word))
    {
        lines.push(status(&format!("{key}-line"), line, quiet()));
    }
    let act = |action: &str, question: Option<String>| Intent::Act {
        computer: computer.name.clone(),
        item: item.id.clone(),
        action: action.into(),
        question,
    };
    let mut controls = vec![];
    if let Some(question) = item.question.as_ref().filter(|_| item.status == "asking") {
        lines.push(text(
            &format!("{key}-question"),
            &question.text,
            TextRole::Body,
            ink(),
            false,
        ));
        controls.push(button(
            &format!("{key}-approve"),
            "Approve",
            Some(Glyph::Check),
            act("approve", Some(question.id.clone())),
        ));
        controls.push(button(
            &format!("{key}-deny"),
            "Deny",
            Some(Glyph::Flag),
            act("deny", Some(question.id.clone())),
        ));
    }
    // A run a usage limit paused: its choices, and nothing else to tap
    // (#11295).
    for (choice, label, glyph) in [
        ("your_key", "Continue on your key", Glyph::Check),
        ("openagents", "Continue with OpenAgents", Glyph::Ask),
    ] {
        if item.choices.iter().any(|c| c == choice) {
            controls.push(button(
                &format!("{key}-{choice}"),
                label,
                Some(glyph),
                act(choice, None),
            ));
        }
    }
    // A landing queue entry (#11250) takes no Stop or Message here.
    if item.running() && item.kind != "landing" && item.choices.is_empty() {
        controls.push(button(
            &format!("{key}-stop"),
            "Stop",
            Some(Glyph::Stop),
            act("stop", None),
        ));
        // A cloud run (Solver in the cloud, #11561, and the other work
        // runs) takes Stop only: the website refuses a message to one.
        if computer.name != CLOUD_BOARD {
            controls.push(button(
                &format!("{key}-message"),
                "Message",
                Some(Glyph::Ask),
                Intent::Message {
                    computer: computer.name.clone(),
                    item: item.id.clone(),
                },
            ));
        }
    }
    for (index, (label, url)) in github_refs(item).into_iter().enumerate() {
        controls.push(button(
            &format!("{key}-ref-{index}"),
            &label,
            Some(Glyph::Forward),
            Intent::OpenLink { url },
        ));
    }
    if !controls.is_empty() {
        lines.push(Node {
            key: format!("{key}-controls"),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Wrap,
                children: controls,
            },
        });
    }
    card(&key, lines)
}

/// A board's heading: a computer with whether it is online, or a list
/// that isn't one computer (OpenAgents Cloud's runs, paused Environments
/// runs) by its name alone.
fn board_title(computer: &Agents) -> String {
    if !crate::account_link::is_computer(&computer.name) {
        return computer.name.clone();
    }
    format!(
        "{} · {}",
        computer.name,
        if computer.online { "Online" } else { "Offline" }
    )
}

/// The engine as a row names it: an engine's own name ("Claude Code"),
/// never a model's (Coder names no vendor model on the
/// phone). A word that looks like a model name (`claude-opus-5-5`,
/// `openai/gpt-6`) is left out.
fn engine_name(engine: &str) -> Option<String> {
    let engine = engine.trim();
    if engine.is_empty()
        || engine
            .chars()
            .any(|c| c.is_ascii_digit() || matches!(c, '/' | ':'))
    {
        return None;
    }
    let named = match engine.to_ascii_lowercase().as_str() {
        "claude" | "claude code" => "Claude Code",
        "codex" => "Codex",
        "grok" | "grok build" => "Grok Build",
        "opencode" => "OpenCode",
        "devin" => "Devin",
        "bare" | "coder" => "Coder",
        "briefed agent" | "briefed" => "Coder",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_owned());
    }
    let mut chars = engine.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
}

/// The repository a row's links point into.
const DEFAULT_REPOSITORY: &str = "OpenAgentsInc/openagents";

/// Whether `url` is a GitHub page a row may open.
pub(crate) fn github_link(url: &str) -> bool {
    url.starts_with("https://github.com/")
        && url.len() <= 200
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '<' | '>' | '\\'))
}

fn repository_ok(repository: &str) -> bool {
    repository.split('/').count() == 2
        && repository.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}

/// The GitHub pages a fleet row names (#11228, #11250), as the website's
/// Agents page links them: the issue its title starts with
/// (`#11250 in OpenAgentsInc/openagents`), the commit its line names
/// (`landed 248c7ce8d6`), and a pull request's address. Each is a button
/// with its label.
pub(crate) fn github_refs(item: &Item) -> Vec<(String, String)> {
    let mut words = item.title.split_whitespace();
    let mut repository = DEFAULT_REPOSITORY.to_owned();
    let title: Vec<&str> = item.title.split_whitespace().collect();
    if let Some(at) = title.iter().position(|w| *w == "in")
        && let Some(named) = title.get(at + 1)
        && repository_ok(named)
    {
        repository = (*named).to_owned();
    }
    let mut refs = vec![];
    if let Some(number) = words
        .next()
        .and_then(|w| w.strip_prefix('#'))
        .map(|w| w.trim_end_matches([':', ',', '.']))
        .filter(|n| !n.is_empty() && n.len() <= 9 && n.bytes().all(|b| b.is_ascii_digit()))
    {
        refs.push((
            format!("Issue #{number}"),
            format!("https://github.com/{repository}/issues/{number}"),
        ));
    }
    let line = item.line.as_deref().unwrap_or_default();
    let mut previous = "";
    for word in line.split_whitespace() {
        let core = word.trim_end_matches(['.', ',', ':', ';', ')']);
        let lower = previous.to_ascii_lowercase();
        if matches!(lower.as_str(), "landed" | "committed")
            && (7..=40).contains(&core.len())
            && core.bytes().all(|b| b.is_ascii_hexdigit())
        {
            let short: String = core.chars().take(7).collect();
            refs.push((
                format!("Commit {short}"),
                format!("https://github.com/{repository}/commit/{core}"),
            ));
        } else if github_link(core) && core.contains("/pull/") {
            refs.push(("Pull request".to_owned(), core.to_owned()));
        }
        previous = core;
    }
    refs.dedup_by(|a, b| a.1 == b.1);
    refs.truncate(3);
    refs
}

/// The memory notes on the account (#11182): those that apply everywhere,
/// then each project's, each opening to change or delete it.
/// The account's projects (#11558): a name is enough to make one, and a
/// project opens a new chat inside it.
fn projects(state: &State, composer: u64) -> Node<Intent> {
    let mut children = vec![bar(
        "link-projects-header",
        Lead::of(state.from_account),
        "Projects",
        None,
        None,
    )];
    children.extend(notice(state));
    children.push(Node {
        key: "link-project-composer".into(),
        style: Style::default(),
        element: Element::Composer {
            token: format!("link-project-{composer}"),
            placeholder: "New project name".into(),
            max_bytes: crate::account_link::MAX_PROJECT_NAME * 4,
            enabled: !state.projects_busy,
            busy: state.projects_busy,
            stop: None,
            choices: vec![],
            draft: None,
            focus: false,
            attach: None,
        },
    });
    if !state.projects_read {
        children.push(working("link-projects-loading", "Loading your projects…"));
        return page(children);
    }
    if state.project_list.is_empty() {
        children.push(status(
            "link-projects-empty",
            "No projects yet. Type a name above to make one. You can add repositories later on openagents.com.",
            quiet(),
        ));
        return page(children);
    }
    for project in &state.project_list {
        let mut label = project.name.clone();
        label.push('\n');
        label.push_str(&project_line(project));
        children.push(button(
            &format!("link-project-{}", slug(&project.id)),
            &label,
            Some(Glyph::Folder),
            Intent::ProjectChat {
                id: project.id.clone(),
            },
        ));
    }
    children.push(status(
        "link-projects-how",
        "Tap a project to start a chat in it.",
        quiet(),
    ));
    page(children)
}

/// A project row's second line: its repositories and its chats.
fn project_line(project: &crate::account_link::Project) -> String {
    let repositories = match (&project.primary, project.repositories) {
        (None, _) | (_, 0) => "No repository".to_owned(),
        (Some(primary), 1) => primary.clone(),
        (Some(primary), n) => format!("{primary} and {} more", n - 1),
    };
    match project.chats {
        0 => repositories,
        1 => format!("{repositories} · 1 chat"),
        n => format!("{repositories} · {n} chats"),
    }
}

fn memory(state: &State) -> Node<Intent> {
    let mut children = vec![bar("link-memory-header", Lead::Back, "Memory", None, None)];
    children.push(status(
        "link-memory-what",
        "What Coder remembers about you, from computers with sync on. Chats here use the notes that apply everywhere.",
        quiet(),
    ));
    children.extend(notice(state));
    if !state.memory_read {
        children.push(working("link-memory-loading", "Loading your notes…"));
        return page(children);
    }
    children.push(button(
        "link-memory-new",
        "Add a note",
        Some(Glyph::Add),
        Intent::NewNote,
    ));
    if state.memory.is_empty() {
        children.push(status(
            "link-memory-empty",
            "No notes yet. Tell Coder \"remember …\" on a computer with sync on (/sync on in Coder), or add one here.",
            quiet(),
        ));
        return page(children);
    }
    let mut groups: Vec<(String, Vec<&crate::account_link::Note>)> = vec![];
    for note in &state.memory {
        let group = if note.everywhere() {
            "Everywhere".to_owned()
        } else {
            note.project_name
                .clone()
                .unwrap_or_else(|| "A project".to_owned())
        };
        match groups.iter_mut().find(|(name, _)| *name == group) {
            Some((_, rows)) => rows.push(note),
            None => groups.push((group, vec![note])),
        }
    }
    groups.sort_by_key(|(name, _)| name != "Everywhere");
    for (index, (name, rows)) in groups.into_iter().enumerate() {
        children.push(status(
            &format!("link-memory-group-{index}"),
            &name,
            quiet(),
        ));
        for note in rows {
            let key = format!("link-memory-{}", slug(&note.id));
            let mut lines = vec![button(
                &format!("{key}-open"),
                note.name.as_deref().unwrap_or("Note"),
                None,
                Intent::EditNote {
                    id: note.id.clone(),
                },
            )];
            let hint = match note.description.as_deref().filter(|d| !d.trim().is_empty()) {
                Some(description) => format!(
                    "{} · {}",
                    crate::account_link::kind_label(note.kind.as_deref()),
                    cut(description, 160)
                ),
                None => crate::account_link::kind_label(note.kind.as_deref()).to_owned(),
            };
            lines.push(status(&format!("{key}-hint"), &hint, quiet()));
            children.push(card(&key, lines));
        }
    }
    page(children)
}

/// One memory note: what it says, a composer to change it, and Delete; or
/// a composer for a new note.
fn note(state: &State, composer: u64) -> Node<Intent> {
    let editing = state.editing.clone().unwrap_or_default();
    let found = state
        .memory
        .iter()
        .find(|note| !editing.is_empty() && note.id == editing);
    let title = found
        .and_then(|note| note.name.clone())
        .unwrap_or_else(|| "New note".into());
    let mut children = vec![bar("link-note-header", Lead::Back, &title, None, None)];
    match found {
        Some(note) => {
            children.push(status(
                "link-note-kind",
                crate::account_link::kind_label(note.kind.as_deref()),
                quiet(),
            ));
            children.push(text(
                "link-note-body",
                &cut(note.body.as_deref().unwrap_or_default(), SHOWN_TEXT),
                TextRole::Body,
                ink(),
                false,
            ));
            children.push(status(
                "link-note-how",
                "Change what it says below, then send. It reaches Coder at its next sync.",
                quiet(),
            ));
        }
        None => children.push(status(
            "link-note-how",
            "Write what to remember. The first line names it, and it applies everywhere.",
            quiet(),
        )),
    }
    children.extend(notice(state));
    children.push(Node {
        key: "link-note-composer".into(),
        style: Style::default(),
        element: Element::Composer {
            token: format!("link-note-{composer}"),
            placeholder: if found.is_some() {
                "What to remember".into()
            } else {
                "Remember that…".into()
            },
            max_bytes: crate::account_link::MAX_NOTE_BYTES,
            enabled: !state.memory_busy,
            busy: state.memory_busy,
            stop: None,
            choices: vec![],
            draft: found
                .and_then(|note| note.body.clone())
                .filter(|body| body.len() <= crate::account_link::MAX_NOTE_BYTES),
            focus: true,
            attach: None,
        },
    });
    if let Some(note) = found {
        children.push(button(
            "link-note-delete",
            "Delete",
            Some(Glyph::Stop),
            Intent::DeleteNote {
                id: note.id.clone(),
            },
        ));
    }
    page(children)
}

/// The scheduled prompts on the account (#11177): when each runs, on which
/// computer, where it goes, with Pause or Resume and Delete.
fn schedules(state: &State) -> Node<Intent> {
    let mut children = vec![bar(
        "link-schedules-header",
        Lead::Back,
        "Scheduled prompts",
        None,
        None,
    )];
    children.push(status(
        "link-schedules-what",
        "Prompts that run on your computers on a schedule, even while Coder is closed there.",
        quiet(),
    ));
    children.extend(notice(state));
    if !state.schedules_read {
        children.push(working(
            "link-schedules-loading",
            "Loading your scheduled prompts…",
        ));
        return page(children);
    }
    if state.schedule_computers.is_empty() {
        children.push(status(
            "link-schedules-no-computer",
            "No computer runs Coder with sync on yet. Type /sync on in Coder on a computer to schedule prompts there.",
            quiet(),
        ));
    } else {
        children.push(button(
            "link-schedules-new",
            "Add",
            Some(Glyph::Add),
            Intent::NewSchedule,
        ));
    }
    if state.schedules.is_empty() {
        children.push(status(
            "link-schedules-empty",
            "No scheduled prompts yet.",
            quiet(),
        ));
        return page(children);
    }
    for schedule in &state.schedules {
        let key = format!("link-scheduled-{}", slug(&schedule.id));
        let mut lines = vec![text(
            &format!("{key}-prompt"),
            &cut(&schedule.prompt, 280),
            TextRole::Body,
            ink(),
            false,
        )];
        let mut when = schedule.when();
        if schedule.paused {
            when.push_str(" · Paused");
        }
        lines.push(status(
            &format!("{key}-when"),
            &when,
            if schedule.paused { warn() } else { quiet() },
        ));
        lines.push(status(
            &format!("{key}-where"),
            &format!("Runs on {} · {}", schedule.computer, schedule.goes()),
            quiet(),
        ));
        lines.push(button(
            &format!("{key}-pause"),
            if schedule.paused { "Resume" } else { "Pause" },
            Some(if schedule.paused {
                Glyph::Restore
            } else {
                Glyph::Stop
            }),
            Intent::PauseSchedule {
                id: schedule.id.clone(),
                paused: !schedule.paused,
            },
        ));
        lines.push(button(
            &format!("{key}-delete"),
            "Delete",
            Some(Glyph::Archive),
            Intent::DeleteSchedule {
                id: schedule.id.clone(),
            },
        ));
        children.push(card(&key, lines));
    }
    page(children)
}

/// A new scheduled prompt: the computer, how often, where it goes, and a
/// composer for the prompt itself.
fn new_schedule(state: &State, composer: u64) -> Node<Intent> {
    use crate::account_link::Repeat;
    let mut children = vec![bar(
        "link-new-schedule-header",
        Lead::Back,
        "New scheduled prompt",
        None,
        None,
    )];
    let Some(draft) = &state.draft else {
        return page(children);
    };
    children.extend(notice(state));
    // The computer.
    let mut lines = vec![text(
        "link-new-schedule-computer-title",
        "Runs on",
        TextRole::Body,
        ink(),
        true,
    )];
    for (index, name) in state.schedule_computers.iter().enumerate() {
        lines.push(choice(
            &format!("link-new-schedule-computer-{index}"),
            name,
            Glyph::Computer,
            draft.computer == *name,
            Intent::ScheduleComputer { name: name.clone() },
        ));
    }
    children.push(card("link-new-schedule-computer", lines));
    // How often.
    let repeats = [
        ("daily", "Every day", Repeat::Daily),
        ("weekdays", "Weekdays", Repeat::Weekdays),
        ("weekends", "Weekends", Repeat::Weekends),
        ("hour", "Every hour", Repeat::Hours(1)),
        ("2h", "Every 2 hours", Repeat::Hours(2)),
        ("6h", "Every 6 hours", Repeat::Hours(6)),
    ];
    let mut lines = vec![text(
        "link-new-schedule-when-title",
        "When",
        TextRole::Body,
        ink(),
        true,
    )];
    for (key, label, repeat) in repeats {
        lines.push(choice(
            &format!("link-new-schedule-repeat-{key}"),
            label,
            Glyph::History,
            draft.repeat == repeat,
            Intent::ScheduleRepeat { repeat },
        ));
    }
    if !matches!(draft.repeat, Repeat::Hours(_)) {
        lines.push(status(
            "link-new-schedule-time",
            &format!("At {} on the computer's clock", draft.time()),
            ink(),
        ));
        lines.push(button(
            "link-new-schedule-earlier",
            "Earlier",
            Some(Glyph::ArrowDown),
            Intent::ScheduleTime { minutes: -30 },
        ));
        lines.push(button(
            "link-new-schedule-later",
            "Later",
            Some(Glyph::ArrowUp),
            Intent::ScheduleTime { minutes: 30 },
        ));
    }
    children.push(card("link-new-schedule-when", lines));
    // Where it goes.
    let mut lines = vec![
        text(
            "link-new-schedule-where-title",
            "Where",
            TextRole::Body,
            ink(),
            true,
        ),
        choice(
            "link-new-schedule-where-new",
            "Start a new Coder run",
            Glyph::Compose,
            draft.chat.is_none(),
            Intent::ScheduleChat { session: None },
        ),
    ];
    for (index, chat) in state
        .schedule_chats
        .iter()
        .filter(|chat| chat.computer == draft.computer)
        .take(8)
        .enumerate()
    {
        let title = if chat.title.trim().is_empty() {
            "Untitled chat"
        } else {
            chat.title.as_str()
        };
        lines.push(choice(
            &format!("link-new-schedule-where-{index}"),
            &format!("Post in {}", cut(title, 80)),
            Glyph::History,
            draft.chat.as_deref() == Some(chat.session.as_str()),
            Intent::ScheduleChat {
                session: Some(chat.session.clone()),
            },
        ));
    }
    children.push(card("link-new-schedule-where", lines));
    let (days, time, every_secs) = draft.when();
    children.push(status(
        "link-new-schedule-how",
        &format!(
            "{}. Write the prompt below, then send to add it.",
            crate::account_link::when_words(&days, time.as_deref(), every_secs)
        ),
        quiet(),
    ));
    children.push(Node {
        key: "link-new-schedule-composer".into(),
        style: Style::default(),
        element: Element::Composer {
            token: format!("link-schedule-{composer}"),
            placeholder: "What to do each time".into(),
            max_bytes: crate::account_link::MAX_SCHEDULE_PROMPT,
            enabled: !state.schedules_busy && !draft.computer.is_empty(),
            busy: state.schedules_busy,
            stop: None,
            choices: vec![],
            draft: None,
            focus: true,
            attach: None,
        },
    });
    page(children)
}

/// Write a message to one agent.
fn message(state: &State, composer: u64) -> Node<Intent> {
    let title = state
        .messaging
        .as_ref()
        .map(|(_, _, title)| title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Coder".into());
    let computer = state
        .messaging
        .as_ref()
        .map(|(computer, _, _)| computer.clone())
        .unwrap_or_default();
    let mut children = vec![bar("link-message-header", Lead::Back, &title, None, None)];
    children.push(status(
        "link-message-where",
        &format!("Coder on {computer} reads it at its next step."),
        quiet(),
    ));
    children.extend(notice(state));
    children.push(Node {
        key: "link-message-composer".into(),
        style: Style::default(),
        element: Element::Composer {
            token: format!("link-message-{composer}"),
            placeholder: format!("Message {title}"),
            max_bytes: crate::account_link::MAX_REPLY_BYTES,
            enabled: state.messaging.is_some(),
            busy: false,
            stop: None,
            choices: vec![],
            draft: None,
            focus: true,
            attach: None,
        },
    });
    page(children)
}

/// "4m 12s", "1h 3m", or "12s".
fn elapsed(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
    }
}

fn cut(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Letters, digits, and `-` only, for node keys.
fn slug(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(48)
        .collect()
}

fn notice(state: &State) -> Option<Node<Intent>> {
    state
        .notice
        .as_ref()
        .map(|notice| status("link-notice", notice, quiet()))
}

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack("link", Space::Md, children);
    node.style.padding_top = Some(Space::Md);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node
}

fn card(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack(key, Space::Xs, children);
    node.style.background = Some(openagents_chat_app::visual::inks().card);
    node.style.radius = Some(14);
    node.style.padding_top = Some(Space::Sm);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Sm);
    node.style.padding_start = Some(Space::Md);
    node
}

/// What a top bar starts with: the menu, on a screen the drawer opens, or
/// a back button, on a screen opened from another one.
enum Lead {
    Menu,
    Back,
}

impl Lead {
    /// Back when the screen was opened from another one, else the menu.
    fn of(pushed: bool) -> Self {
        if pushed { Lead::Back } else { Lead::Menu }
    }
}

/// The longest title a top bar shows on one line; the rest is cut.
const BAR_TITLE: usize = 28;

/// A screen's one top bar (#11472): the menu or back button at the leading
/// edge, the title on one line with what it is under it, and an action at
/// the trailing edge. Nothing else heads the screen.
fn bar(
    key: &str,
    lead: Lead,
    title: &str,
    subtitle: Option<(&str, &str)>,
    trailing: Option<Node<Intent>>,
) -> Node<Intent> {
    let (label, glyph, intent, slot) = match lead {
        Lead::Menu => ("Open menu", Glyph::Menu, Intent::Menu, "menu"),
        Lead::Back => ("Back", Glyph::Back, Intent::Back, "back"),
    };
    let mut lines = vec![text(
        &format!("{key}-title"),
        &one_line(title, BAR_TITLE),
        TextRole::Body,
        ink(),
        true,
    )];
    if let Some((subtitle_key, subtitle)) = subtitle {
        lines.push(status(subtitle_key, &one_line(subtitle, 40), quiet()));
    }
    let mut children = vec![
        Node {
            key: format!("{key}-{slot}"),
            style: Style {
                foreground: Some(ink()),
                ..Style::default()
            },
            element: Element::Button {
                shortcut: None,
                label: label.into(),
                enabled: true,
                icon: Some(Icon {
                    glyph,
                    circular: true,
                    pill: false,
                }),
                intent,
            },
        },
        stack(&format!("{key}-titles"), Space::None, lines),
    ];
    if let Some(mut trailing) = trailing {
        trailing.style.align = Some(rust_native::style::TextAlign::End);
        children.push(trailing);
    }
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            align: Some(rust_native::style::TextAlign::Center),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    }
}

/// `text` on one line: its first line, cut to `limit` characters.
fn one_line(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= limit {
        return line.to_owned();
    }
    let kept: String = line.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// A round icon button for a top bar's trailing edge.
fn bar_button(key: &str, label: &str, glyph: Glyph, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(ink()),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: Some(Icon {
                glyph,
                circular: true,
                pill: false,
            }),
            intent,
        },
    }
}

fn stack(key: &str, gap: Space, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(gap),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn text(key: &str, value: &str, role: TextRole, foreground: Color, bold: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: bold.then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn status(key: &str, value: &str, color: Color) -> Node<Intent> {
    text(key, value, TextRole::Status, color, false)
}

/// A sent file's line: its name, kind, and size, as `Photo.png · PNG · 1 KB`.
fn file_line(file: &MessageFile) -> String {
    let size = if file.size < 1024 {
        format!("{} bytes", file.size)
    } else if file.size < 1024 * 1024 {
        format!("{} KB", file.size.div_ceil(1024))
    } else {
        format!("{:.1} MB", file.size as f64 / (1024.0 * 1024.0))
    };
    let mut parts = vec![file.name.clone()];
    if !file.kind.is_empty() {
        parts.push(file.kind.to_uppercase());
    }
    parts.push(size);
    parts.join(" · ")
}

fn working(key: &str, label: &str) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Working {
            label: label.into(),
        },
    }
}

fn surface(key: &str, resource: &str, label: &str) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Surface {
            resource: resource.into(),
            label: label.into(),
        },
    }
}

fn button(key: &str, label: &str, glyph: Option<Glyph>, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(ink()),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: glyph.map(|glyph| Icon {
                glyph,
                circular: false,
                pill: true,
            }),
            intent,
        },
    }
}

fn choice(key: &str, label: &str, glyph: Glyph, selected: bool, intent: Intent) -> Node<Intent> {
    // A button, not a v3 `Choice`: every host draws buttons. The chosen one
    // carries a check.
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(ink()),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: Some(Icon {
                glyph: if selected { Glyph::Check } else { glyph },
                circular: false,
                pill: true,
            }),
            intent,
        },
    }
}
