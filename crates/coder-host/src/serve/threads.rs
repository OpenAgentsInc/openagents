//! The host's chat threads for granted devices: NIP-HOST `thread.list`,
//! `thread.read`, `thread.send`, and `thread.stop`.
//!
//! These run the same chat service commands the local operator socket runs
//! for the desktop app and `openagents chat` ([`crate::control::apply_chat`]),
//! on the same store, so a thread started in any of them is one thread. The
//! access layer has already checked the device's grant and the right each
//! operation needs (`observe` to read, `operate` to send or stop). What leaves the
//! host is the wire form in `coder_access::thread`: titles, turns, the reply
//! streaming, a link to Coder work, and each turn's offers, cards, and
//! follow-up chips. The router's typed judgment stays on the host.
//!
//! A Coder run `openagents chat` started on this computer is bound to its
//! thread under the host name `local`. When the host's task owner holds
//! that task (the run used the store the host serves), the link names the
//! host's own key, so a device opens, follows, and stops it like any task
//! here. Otherwise the page says the run is outside the host, and a device
//! offers no control for it.

use coder_access::Code;
use coder_access::thread::{
    self, MAX_THREADS, MAX_TITLE, ThreadCoder, ThreadOutside, ThreadPage, ThreadRole, ThreadRow,
    ThreadTurn,
};
use openagents_chat::basic_chats::{Spawned, Summary};
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::service::{Command, Snapshot};

use super::Shared;
use crate::control::{ChatRefusal, apply_chat};

/// The most list pages read while looking for one thread's row.
const LIST_PAGES: usize = 64;

fn run(shared: &Shared, command: Command) -> Result<Snapshot, Code> {
    apply_chat(shared, command).map_err(|refusal| match refusal {
        ChatRefusal::Unavailable(_) | ChatRefusal::Chat(_) => Code::Unavailable,
    })
}

/// Every row of the host's list, page by page, bound to one list version.
fn rows(shared: &Shared) -> Result<Vec<Summary>, Code> {
    let first = run(shared, Command::List {})?;
    let mut rows = first.chats.clone();
    for _ in 0..LIST_PAGES {
        if rows.len() >= first.list_total {
            break;
        }
        let page = run(
            shared,
            Command::ListMore {
                after: rows.len(),
                version: first.list_version,
            },
        )?;
        if page.chats.is_empty() {
            break;
        }
        rows.extend(page.chats);
    }
    Ok(rows)
}

/// `thread.list`: the newest threads that are not archived.
pub(crate) fn list(shared: &Shared) -> Result<Vec<ThreadRow>, Code> {
    let mut rows: Vec<Summary> = rows(shared)?
        .into_iter()
        .filter(|row| !row.archived && thread::is_id(&row.id))
        .collect();
    rows.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    rows.truncate(MAX_THREADS);
    let mut rows: Vec<ThreadRow> = rows.iter().map(|summary| row(shared, summary)).collect();
    // Long titles on every row could outgrow one reply; drop the oldest.
    while serde_json::to_vec(&rows).map_or(true, |bytes| bytes.len() > thread::MAX_PAGE_BYTES) {
        rows.pop();
    }
    Ok(rows)
}

/// `thread.read`: one page of `id`, the newest turns or those before
/// `before`. An unknown thread is `unavailable`.
pub(crate) fn read(shared: &Shared, id: &str, before: Option<u64>) -> Result<ThreadPage, Code> {
    let before = before
        .map(|before| usize::try_from(before).map_err(|_| Code::Bounds))
        .transpose()?;
    let snapshot = run(
        shared,
        Command::Read {
            chat: id.to_owned(),
            before,
        },
    )?;
    let summary = find(shared, id, &snapshot)?;
    Ok(page(shared, &summary, &snapshot))
}

/// `thread.send`: append `text` to `id` under the device's send ID, which
/// asks OpenAgents for the reply. A replay of a send already in the thread
/// is accepted again; different text under its ID, a thread that is
/// answering, or an archived one refuses as `conflict`.
pub(crate) fn send(shared: &Shared, id: &str, request: &str, text: &str) -> Result<(), Code> {
    let snapshot = run(
        shared,
        Command::Read {
            chat: id.to_owned(),
            before: None,
        },
    )?;
    if let Some(previous) = snapshot
        .turns
        .iter()
        .find(|turn| turn.request.as_deref() == Some(request))
    {
        return if previous.text == text.trim() {
            Ok(())
        } else {
            Err(Code::Conflict)
        };
    }
    let summary = find(shared, id, &snapshot)?;
    if snapshot.busy || summary.archived {
        return Err(Code::Conflict);
    }
    run(
        shared,
        Command::Send {
            chat: id.to_owned(),
            request: request.to_owned(),
            text: text.to_owned(),
        },
    )
    .map(|_| ())
}

/// `thread.stop`: stop receiving the reply streaming into `id` in answer
/// to the message whose send ID is `request` (`None` for a message sent
/// without one), through the chat service's own stop, as the desktop's
/// stop does: what streamed is kept as a stopped reply, and the hosted
/// worker may still finish. When the thread is not answering that message
/// (the reply ended, it was stopped already, or a newer message is being
/// answered) nothing changes, so the stop is idempotent per thread and
/// send ID. An unknown thread is `unavailable`.
pub(crate) fn stop(shared: &Shared, id: &str, request: Option<&str>) -> Result<(), Code> {
    let snapshot = run(
        shared,
        Command::Read {
            chat: id.to_owned(),
            before: None,
        },
    )?;
    find(shared, id, &snapshot)?;
    let answering = snapshot.busy
        && snapshot
            .turns
            .last()
            .is_some_and(|turn| turn.role == Role::User && turn.request.as_deref() == request);
    if answering {
        run(
            shared,
            Command::Stop {
                chat: id.to_owned(),
            },
        )?;
    }
    Ok(())
}

/// The thread's row: in the snapshot's first list page, or a later one.
fn find(shared: &Shared, id: &str, snapshot: &Snapshot) -> Result<Summary, Code> {
    if let Some(summary) = snapshot.chats.iter().find(|row| row.id == id) {
        return Ok(summary.clone());
    }
    rows(shared)?
        .into_iter()
        .find(|row| row.id == id)
        .ok_or(Code::Unavailable)
}

fn title(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut end = clean.len().min(MAX_TITLE);
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    clean[..end].to_owned()
}

/// Where a thread's Coder link points, as a device may use it.
enum Link {
    /// A task on a Coder host: this one, or another.
    Host(ThreadCoder),
    /// A local run in a task store this host does not serve.
    Outside(ThreadOutside),
}

fn link(shared: &Shared, thread: &str, spawned: Option<&Spawned>) -> Option<Link> {
    let spawned = spawned?;
    let project = spawned
        .project
        .clone()
        .filter(|project| project.len() <= 128 && !project.chars().any(char::is_control));
    let hex = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !hex(&spawned.task) {
        return None;
    }
    let host = if spawned.host == openagents_chat::thread::LOCAL_HOST {
        if !shared.tasks.local_run(&spawned.task, thread) {
            return Some(Link::Outside(ThreadOutside {
                task: spawned.task.clone(),
                project,
                at: spawned.at,
            }));
        }
        shared.host_key.clone()
    } else if hex(&spawned.host) {
        spawned.host.clone()
    } else {
        return None;
    };
    Some(Link::Host(ThreadCoder {
        host,
        task: spawned.task.clone(),
        project,
        at: spawned.at,
    }))
}

fn row(shared: &Shared, summary: &Summary) -> ThreadRow {
    let coder = match link(shared, &summary.id, summary.coder.as_ref()) {
        Some(Link::Host(coder)) => Some(coder),
        _ => None,
    };
    ThreadRow {
        thread: summary.id.clone(),
        title: title(&summary.title),
        started: summary.started,
        updated: summary.updated,
        pinned: summary.pinned,
        coder,
    }
}

fn turn(turn: &Turn) -> ThreadTurn {
    ThreadTurn {
        role: match turn.role {
            Role::User => ThreadRole::User,
            Role::Assistant => ThreadRole::Assistant,
        },
        text: turn.text.clone(),
        at: turn.at,
        stopped: turn.stopped,
        model: turn.model.clone().filter(|model| model.len() <= 256),
        request: turn
            .request
            .clone()
            .filter(|request| thread::is_id(request)),
        extras: extras(turn),
    }
}

/// Offers, follow-ups, and cards the phone can read again. The typed
/// judgment stays in the chat store.
fn extras(turn: &Turn) -> thread::ThreadExtras {
    let Some(meta) = turn.meta.as_ref() else {
        return thread::ThreadExtras::default();
    };
    let offers = meta
        .offers
        .iter()
        .filter_map(|offer| {
            let mut value = offer.wire();
            // The computer's prediction rides on its own offer; a phone
            // that does not know the field reads the offer as before.
            if *offer == openagents_chat::router::Offer::RunCoder
                && let Some(runner) = meta
                    .runner
                    .as_ref()
                    .and_then(|runner| serde_json::to_value(runner).ok())
            {
                value["runner"] = runner;
            }
            let fits = serde_json::to_vec(&value)
                .is_ok_and(|bytes| bytes.len() <= thread::MAX_EXTRA_VALUE);
            (fits && openagents_chat::router::Offer::parse(&value).is_some()).then_some(value)
        })
        .take(thread::MAX_OFFERS)
        .collect();
    let followups = meta
        .followups
        .iter()
        .filter_map(|followup| {
            let label = followup.label.trim();
            let count = label.chars().count();
            if !(1..=thread::MAX_FOLLOWUP_CHARS).contains(&count)
                || label.chars().any(char::is_control)
            {
                return None;
            }
            Some(thread::ThreadFollowup {
                answer: followup.answer.clone().filter(|answer| answer_tag(answer)),
                label: label.to_owned(),
            })
        })
        .take(thread::MAX_FOLLOWUPS)
        .collect();
    let cards = meta
        .cards
        .iter()
        .filter(|card| {
            nostr::cj_conversation::parse_card(card).is_ok()
                && serde_json::to_vec(card)
                    .is_ok_and(|bytes| bytes.len() <= thread::MAX_EXTRA_VALUE)
        })
        .take(thread::MAX_CARDS)
        .cloned()
        .collect();
    thread::ThreadExtras {
        offers,
        followups,
        cards,
    }
}

/// A bank id the follow-up may name: short, lowercase ASCII.
fn answer_tag(text: &str) -> bool {
    (1..=96).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._@-:".contains(&byte)
        })
}

fn page(shared: &Shared, summary: &Summary, snapshot: &Snapshot) -> ThreadPage {
    let (coder, outside) = match link(
        shared,
        &summary.id,
        snapshot.coder.as_ref().or(summary.coder.as_ref()),
    ) {
        Some(Link::Host(coder)) => (Some(coder), None),
        Some(Link::Outside(outside)) => (None, Some(outside)),
        None => (None, None),
    };
    let mut page = ThreadPage {
        thread: summary.id.clone(),
        title: title(&summary.title),
        start: snapshot.start as u64,
        total: snapshot.total as u64,
        turns: snapshot.turns.iter().map(turn).collect(),
        busy: snapshot.busy,
        partial: if snapshot.busy {
            snapshot.partial.clone()
        } else {
            String::new()
        },
        failure: snapshot.failure.clone(),
        coder,
        outside,
    };
    thread::fit(&mut page);
    page
}
