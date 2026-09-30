//! The host's chat threads for granted devices: NIP-HOST `thread.list`,
//! `thread.read`, and `thread.send`.
//!
//! These run the same chat service commands the local operator socket runs
//! for the desktop app and `openagents chat` ([`crate::control::apply_chat`]),
//! on the same store, so a thread started in any of them is one thread. The
//! access layer has already checked the device's grant and the right each
//! operation needs (`observe` to read, `operate` to send). What leaves the
//! host is the wire form in `coder_access::thread`: titles, turns, the reply
//! streaming, and a link to Coder work, and never the router's judgments.

use coder_access::Code;
use coder_access::thread::{
    self, MAX_THREADS, MAX_TITLE, ThreadCoder, ThreadPage, ThreadRole, ThreadRow, ThreadTurn,
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
    let mut rows: Vec<ThreadRow> = rows.iter().map(row).collect();
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
    Ok(page(&summary, &snapshot))
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

fn coder(spawned: Option<&Spawned>) -> Option<ThreadCoder> {
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
    (hex(&spawned.host) && hex(&spawned.task)).then(|| ThreadCoder {
        host: spawned.host.clone(),
        task: spawned.task.clone(),
        project,
        at: spawned.at,
    })
}

fn row(summary: &Summary) -> ThreadRow {
    ThreadRow {
        thread: summary.id.clone(),
        title: title(&summary.title),
        started: summary.started,
        updated: summary.updated,
        pinned: summary.pinned,
        coder: coder(summary.coder.as_ref()),
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
    }
}

fn page(summary: &Summary, snapshot: &Snapshot) -> ThreadPage {
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
        coder: coder(snapshot.coder.as_ref().or(summary.coder.as_ref())),
    };
    thread::fit(&mut page);
    page
}
