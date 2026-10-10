//! The account surface's screens (`crate::account_link`): sign in and
//! where this phone's chats live, the account's chats, one chat, what runs
//! on the computers, and a message to one agent. Rust Native views the
//! hosts draw as they draw the Computers screens.

use crate::account_link::{Agents, Item, Open, Screen, SignIn, State, Thread, place};
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
    }
}

/// Sign in, or the account and where this phone's chats live.
fn account(state: &State) -> Node<Intent> {
    let mut children = vec![];
    match (&state.session, &state.sign_in) {
        (Some(session), _) => {
            children.push(heading("link-title", "Your account"));
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
                "link-sign-out",
                "Sign out",
                Some(Glyph::Key),
                Intent::SignOut,
            ));
        }
        (None, SignIn::Waiting { user_code, .. }) => {
            children.push(heading("link-title", "Approve this phone"));
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
            children.push(heading("link-title", "Sign in"));
            children.push(working("link-starting", "Getting a code…"));
        }
        (None, sign_in) => {
            children.push(heading("link-title", "Sign in"));
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

/// The account's chats: web chats, then each computer's terminal chats,
/// then phones'.
fn chats(state: &State) -> Node<Intent> {
    let mut children = vec![header("link-chats-header", "Your chats", Intent::Back)];
    children.extend(notice(state));
    if state.choice.is_none() && state.choice_read {
        children.push(choice_card(state));
    }
    if !state.threads_read {
        children.push(working("link-chats-loading", "Loading your chats…"));
        return page(children);
    }
    if state.threads.is_empty() {
        children.push(status(
            "link-chats-empty",
            "No chats on your account yet. Chat on openagents.com, or run `coder login` then `/sync on` in Coder on a computer.",
            quiet(),
        ));
        return page(children);
    }
    let mut groups: Vec<(String, Vec<&Thread>)> = vec![];
    let mut order: Vec<&Thread> = state.threads.iter().collect();
    order.sort_by_key(|t| std::cmp::Reverse((t.pinned, t.updated_unix)));
    for thread in order {
        let group = match (thread.surface.as_str(), thread.computer.as_deref()) {
            ("terminal", Some(computer)) => format!("Terminal · {computer}"),
            ("phone", Some(computer)) => format!("Phone · {computer}"),
            _ => "openagents.com".to_owned(),
        };
        match groups.iter_mut().find(|(name, _)| *name == group) {
            Some((_, rows)) => rows.push(thread),
            None => groups.push((group, vec![thread])),
        }
    }
    // The web's chats first, then computers by their newest chat.
    groups.sort_by_key(|(name, _)| name != "openagents.com");
    let mut shown = 0;
    for (index, (name, rows)) in groups.into_iter().enumerate() {
        let online = rows
            .first()
            .and_then(|t| t.computer.as_deref())
            .and_then(|computer| {
                state
                    .computers
                    .iter()
                    .find(|c| c.name == computer)
                    .map(|c| c.online)
            });
        let title = match online {
            Some(true) if name.starts_with("Terminal") => format!("{name} · Online"),
            Some(false) if name.starts_with("Terminal") => format!("{name} · Offline"),
            _ => name,
        };
        children.push(text(
            &format!("link-group-{index}"),
            &title,
            TextRole::Status,
            quiet(),
            true,
        ));
        for thread in rows {
            if shown >= 120 {
                break;
            }
            shown += 1;
            let mut label = if thread.title.trim().is_empty() {
                "New chat".to_owned()
            } else {
                thread.title.clone()
            };
            if thread.working {
                label.push_str("\nWorking…");
            } else if let Some(line) = thread.line.as_ref().filter(|_| thread.surface == "web") {
                label.push('\n');
                label.push_str(line);
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
    }
    page(children)
}

/// One chat: its messages, and a reply box when it takes replies.
fn chat(state: &State, open: &Open, composer: u64, limit: usize, cut_at: usize) -> Node<Intent> {
    let thread = open.thread.as_ref();
    let title = thread
        .map(|t| t.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Chat".into());
    let mut children = vec![header("link-chat-header", &title, Intent::Back)];
    if let Some(thread) = thread {
        children.push(status("link-chat-place", &place(thread), quiet()));
    }
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
        let turns: Vec<Turn> = open.messages[start..]
            .iter()
            .filter(|m| m.role != "tool" && !m.text.trim().is_empty())
            .map(|m| {
                let text = cut(&m.text, cut_at);
                if m.role == "user" {
                    Turn::user(text)
                } else {
                    Turn::assistant(text, None)
                }
            })
            .chain(open.sent.iter().map(|text| Turn::user(cut(text, cut_at))))
            .collect();
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
        },
    });
    page(children)
}

/// What runs on each computer: status, time, cost, and its controls.
fn running(state: &State) -> Node<Intent> {
    let mut children = vec![header("link-running-header", "Running", Intent::Back)];
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
            &format!(
                "{} · {}",
                computer.name,
                if computer.online { "Online" } else { "Offline" }
            ),
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
            "Nothing is running on your computers. Coder shows its work here when it's signed in with `coder login` and syncing with `/sync on`.",
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
    if let Some(engine) = &item.engine {
        facts.push(engine.clone());
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
    if let Some(line) = item.line.as_ref().filter(|l| !l.trim().is_empty()) {
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
    if item.running() {
        controls.push(button(
            &format!("{key}-stop"),
            "Stop",
            Some(Glyph::Stop),
            act("stop", None),
        ));
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

/// The memory notes on the account (#11182): those that apply everywhere,
/// then each project's, each opening to change or delete it.
fn memory(state: &State) -> Node<Intent> {
    let mut children = vec![header("link-memory-header", "Memory", Intent::Back)];
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
            "No notes yet. Tell Coder \"remember …\" on a computer with sync on (`/sync on`), or add one here.",
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
    let mut children = vec![header("link-note-header", &title, Intent::Back)];
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
    let mut children = vec![header("link-message-header", &title, Intent::Back)];
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
    node.style.padding_top = Some(Space::Sm);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Sm);
    node.style.padding_start = Some(Space::Md);
    node
}

fn header(key: &str, title: &str, back: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            align: Some(rust_native::style::TextAlign::Center),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children: vec![
                Node {
                    key: format!("{key}-back"),
                    style: Style {
                        foreground: Some(ink()),
                        ..Style::default()
                    },
                    element: Element::Button {
                        shortcut: None,
                        label: "Back".into(),
                        enabled: true,
                        icon: Some(Icon {
                            glyph: Glyph::Back,
                            circular: true,
                            pill: false,
                        }),
                        intent: back,
                    },
                },
                heading(&format!("{key}-title"), title),
            ],
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

fn heading(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Heading, ink(), true)
}

fn status(key: &str, value: &str, color: Color) -> Node<Intent> {
    text(key, value, TextRole::Status, color, false)
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
