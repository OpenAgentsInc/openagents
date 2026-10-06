//! Terminal session records (NIP-TERM's sessions feature): which
//! terminals, threads, and other resources belong together, and their
//! default layout, kept by the host across restarts.
//!
//! The book is one private JSON file beside the access store
//! (`terminal-sessions.json`), replaced whole by a temporary file and a
//! rename, so a crash during a write leaves the previous revision or the
//! new one and never a mix. Only the host process writes it, under one
//! lock. A file this host cannot read, or one a newer host wrote, refuses
//! every session operation as `unavailable` and is left as it is.
//!
//! A record never holds terminal output, a title, a directory, a command
//! line, or an environment value. Reading a session opens nothing: a
//! terminal member reads `live`, `closed`, or `lost` (a terminal of an
//! earlier host generation), and resource members come back exactly as
//! they were written, unresolved. Removing a session closes nothing.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use coder_pty::ext::{
    Member, MemberState, SESSIONS_MAX, SessionEntry, SessionList, SessionRead, SessionRecord,
    SessionRemove, SessionWrite,
};
use coder_pty::wire::{Reason, Refusal, Status, TerminalRef, Value};
use serde::{Deserialize, Serialize};

/// The file name beside the access store.
pub const FILE: &str = "terminal-sessions.json";
/// The book's schema. A later schema migrates from this one when it reads
/// a file with it.
pub const VERSION: &str = "coder-host.terminal-sessions.v1";
/// How many applied writes the book remembers to answer exact retries.
const RECENT_MAX: usize = 256;
/// The largest file the book reads.
const FILE_MAX: u64 = 4 * 1024 * 1024;

/// The result of one session operation.
pub type Outcome = Result<(Status, Value), Refusal>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    v: String,
    sessions: BTreeMap<String, SessionRecord>,
}

struct Applied {
    key: String,
    body: String,
    value: Value,
}

/// The host's session records.
pub struct Book {
    path: PathBuf,
    recent: Mutex<VecDeque<Applied>>,
}

impl std::fmt::Debug for Book {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Book").field("path", &self.path).finish()
    }
}

impl Book {
    /// The book beside the access store at `access`.
    #[must_use]
    pub fn beside(access: &Path) -> Self {
        let directory = access.parent().unwrap_or(access);
        Self::at(directory.join(FILE))
    }

    /// The book in the file `path`.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self {
            path,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Reads one session, with each terminal member's state from `state`.
    pub fn read(
        &self,
        request: &SessionRead,
        state: impl Fn(&TerminalRef) -> MemberState,
    ) -> Outcome {
        let _held = self.hold();
        let stored = self.load()?;
        let record = stored
            .sessions
            .get(&request.session)
            .cloned()
            .ok_or_else(missing)?;
        Ok((
            Status::Accepted,
            Value::Session {
                record: with_states(record, &state),
            },
        ))
    }

    /// Lists every session by ID.
    pub fn list(&self, request: &SessionList) -> Outcome {
        let _ = request;
        let _held = self.hold();
        let stored = self.load()?;
        let sessions = stored
            .sessions
            .values()
            .map(|record| SessionEntry {
                session: record.session.clone().unwrap_or_default(),
                revision: record.revision,
                name: record.name.clone(),
                members: u16::try_from(record.members.len()).unwrap_or(u16::MAX),
            })
            .collect();
        Ok((Status::Accepted, Value::Sessions { sessions }))
    }

    /// Creates a session, or replaces one at revision `base`. A write whose
    /// base is not the current revision refuses as `stale`; nothing merges.
    pub fn write(
        &self,
        principal: &str,
        request: &SessionWrite,
        state: impl Fn(&TerminalRef) -> MemberState,
    ) -> Outcome {
        let mut recent = self.hold();
        let (key, body) = identity(principal, &request.request, request);
        if let Some(outcome) = retry(&recent, &key, &body) {
            return outcome;
        }
        let mut stored = self.load()?;
        let mut record = request.record.clone();
        match &request.session {
            None => {
                if stored.sessions.len() >= SESSIONS_MAX {
                    return Err(Refusal::new(
                        Reason::LimitExceeded,
                        "the host keeps its most sessions",
                    ));
                }
                request.admit(0)?;
                record.session = Some(coder_reach::new_id());
                record.revision = 1;
            }
            Some(session) => {
                let current = stored.sessions.get(session).ok_or_else(missing)?;
                request.admit(current.revision)?;
                record.revision = current.revision + 1;
            }
        }
        record.check()?;
        let session = record.session.clone().unwrap_or_default();
        stored.sessions.insert(session, record.clone());
        self.save(&stored)?;
        let value = Value::Session {
            record: with_states(record, &state),
        };
        remember(&mut recent, key, body, value.clone());
        Ok((Status::Accepted, value))
    }

    /// Removes a session at revision `base`. Its terminals keep running.
    pub fn remove(&self, principal: &str, request: &SessionRemove) -> Outcome {
        let mut recent = self.hold();
        let (key, body) = identity(principal, &request.request, request);
        if let Some(outcome) = retry(&recent, &key, &body) {
            return outcome;
        }
        let mut stored = self.load()?;
        let current = stored.sessions.get(&request.session).ok_or_else(missing)?;
        if current.revision != request.base {
            return Err(Refusal::new(
                Reason::Stale,
                "the session changed since that revision",
            ));
        }
        stored.sessions.remove(&request.session);
        self.save(&stored)?;
        remember(&mut recent, key, body, Value::Done);
        Ok((Status::Accepted, Value::Done))
    }

    fn hold(&self) -> std::sync::MutexGuard<'_, VecDeque<Applied>> {
        self.recent.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The stored sessions: none when the file does not exist yet.
    fn load(&self) -> Result<Stored, Refusal> {
        let unreadable = || {
            Refusal::new(
                Reason::Unavailable,
                "the host's session records cannot be read",
            )
        };
        let bytes = match std::fs::metadata(&self.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Stored {
                    v: VERSION.into(),
                    sessions: BTreeMap::new(),
                });
            }
            Err(_) => return Err(unreadable()),
            Ok(metadata) if metadata.len() > FILE_MAX => return Err(unreadable()),
            Ok(_) => std::fs::read(&self.path).map_err(|_| unreadable())?,
        };
        let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| unreadable())?;
        if stored.v != VERSION {
            return Err(Refusal::new(
                Reason::Unavailable,
                "another host version wrote the session records",
            ));
        }
        for (id, record) in &stored.sessions {
            if record.session.as_deref() != Some(id.as_str()) || record.check().is_err() {
                return Err(unreadable());
            }
        }
        Ok(stored)
    }

    fn save(&self, stored: &Stored) -> Result<(), Refusal> {
        let bytes = serde_json::to_vec(stored)
            .map_err(|_| Refusal::new(Reason::Unavailable, "the sessions cannot be encoded"))?;
        crate::serve::write_private(&self.path, &bytes).map_err(|_| {
            Refusal::new(
                Reason::Unavailable,
                "the host's session records cannot be written",
            )
        })
    }
}

fn missing() -> Refusal {
    Refusal::new(Reason::Unavailable, "no such session")
}

/// The record as a reader sees it: every terminal member's state now.
fn with_states(
    mut record: SessionRecord,
    state: &impl Fn(&TerminalRef) -> MemberState,
) -> SessionRecord {
    for member in &mut record.members {
        if let Member::Terminal {
            terminal,
            state: slot,
            ..
        } = member
        {
            *slot = Some(state(terminal));
        }
    }
    record
}

fn identity(principal: &str, request: &str, body: &impl Serialize) -> (String, String) {
    (
        format!("{principal} {request}"),
        serde_json::to_string(body).unwrap_or_default(),
    )
}

fn retry(recent: &VecDeque<Applied>, key: &str, body: &str) -> Option<Outcome> {
    let applied = recent.iter().find(|applied| applied.key == key)?;
    Some(if applied.body == body {
        Ok((Status::Duplicate, applied.value.clone()))
    } else {
        Err(Refusal::new(
            Reason::IdempotencyConflict,
            "this request ID was used with different content",
        ))
    })
}

fn remember(recent: &mut VecDeque<Applied>, key: String, body: String, value: Value) {
    recent.push_back(Applied { key, body, value });
    if recent.len() > RECENT_MAX {
        recent.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_pty::ext::{Layout, Node, Tab};

    const GENERATION: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const OLD: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn id(n: u64) -> String {
        format!("{n:064x}")
    }

    fn record(session: Option<String>, generation: &str) -> SessionRecord {
        SessionRecord {
            session,
            revision: 0,
            name: "build".into(),
            members: vec![
                Member::Terminal {
                    member: 1,
                    terminal: TerminalRef {
                        generation: generation.into(),
                        terminal: id(99),
                    },
                    state: None,
                },
                Member::Resource {
                    member: 2,
                    resource: serde_json::json!({"kind": "thread", "thread": id(7)}),
                },
            ],
            layout: Layout {
                tabs: vec![Tab {
                    name: "main".into(),
                    root: Node::Split {
                        axis: coder_pty::ext::Axis::Columns,
                        ratio: 500,
                        first: Box::new(Node::Pane { member: 1 }),
                        second: Box::new(Node::Pane { member: 2 }),
                    },
                }],
                active: 0,
            },
        }
    }

    fn state(terminal: &TerminalRef) -> MemberState {
        if terminal.generation == GENERATION {
            MemberState::Live
        } else {
            MemberState::Lost
        }
    }

    fn created(book: &Book, n: u64) -> SessionRecord {
        let write = SessionWrite::new(id(n), None, 0, record(None, GENERATION));
        match book.write("phone", &write, state) {
            Ok((Status::Accepted, Value::Session { record })) => record,
            other => panic!("write: {other:?}"),
        }
    }

    #[test]
    fn a_session_survives_a_new_book_and_reads_lost_terminals_after_a_restart() {
        let temp = tempfile::tempdir().unwrap();
        let book = Book::at(temp.path().join(FILE));
        let record = created(&book, 1);
        assert_eq!(record.revision, 1);
        let session = record.session.clone().unwrap();

        // A second client, after the host restarted: the same layout and
        // references, and the old terminal reads lost.
        let again = Book::at(temp.path().join(FILE));
        let after_restart = |terminal: &TerminalRef| {
            if terminal.generation == OLD {
                MemberState::Live
            } else {
                MemberState::Lost
            }
        };
        let Ok((_, Value::Session { record: read })) =
            again.read(&SessionRead::new(id(2), session.clone()), after_restart)
        else {
            panic!("read")
        };
        assert_eq!(read.layout, record.layout);
        assert_eq!(read.members[1], record.members[1]);
        assert_eq!(read.members[0].state(), Some(MemberState::Lost));
        let Ok((_, Value::Sessions { sessions })) = again.list(&SessionList::new(id(3))) else {
            panic!("list")
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].members, 2);
    }

    #[test]
    fn concurrent_revisions_retries_and_removal_answer_truthfully() {
        let temp = tempfile::tempdir().unwrap();
        let book = Book::at(temp.path().join(FILE));
        let session = created(&book, 1).session.unwrap();
        let update = |request: u64, base: u64| {
            SessionWrite::new(
                id(request),
                Some(session.clone()),
                base,
                record(Some(session.clone()), GENERATION),
            )
        };
        // Two clients write from revision 1: the second is stale.
        book.write("phone", &update(10, 1), state).unwrap();
        assert_eq!(
            book.write("laptop", &update(11, 1), state)
                .unwrap_err()
                .reason,
            Reason::Stale
        );
        // An exact retry is answered once; a reused ID conflicts.
        let again = book.write("phone", &update(10, 1), state).unwrap();
        assert_eq!(again.0, Status::Duplicate);
        assert_eq!(
            book.write("phone", &update(10, 2), state)
                .unwrap_err()
                .reason,
            Reason::IdempotencyConflict
        );
        assert_eq!(
            book.remove("phone", &SessionRemove::new(id(12), session.clone(), 1))
                .unwrap_err()
                .reason,
            Reason::Stale
        );
        book.remove("phone", &SessionRemove::new(id(13), session.clone(), 2))
            .unwrap();
        assert_eq!(
            book.read(&SessionRead::new(id(14), session), state)
                .unwrap_err()
                .reason,
            Reason::Unavailable
        );
    }

    #[test]
    fn a_damaged_or_newer_file_refuses_and_is_left_alone() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(FILE);
        let book = Book::at(path.clone());
        // A crash before the rename leaves a temporary file, never a mix.
        std::fs::write(temp.path().join(format!(".{FILE}.1")), b"{").unwrap();
        created(&book, 1);
        for damaged in [
            &b"{\"v\":"[..],
            br#"{"v":"coder-host.terminal-sessions.v9","sessions":{}}"#,
        ] {
            std::fs::write(&path, damaged).unwrap();
            let refusal = book.list(&SessionList::new(id(2))).unwrap_err();
            assert_eq!(refusal.reason, Reason::Unavailable);
            let write = SessionWrite::new(id(3), None, 0, record(None, GENERATION));
            assert_eq!(
                book.write("phone", &write, state).unwrap_err().reason,
                Reason::Unavailable
            );
            assert_eq!(std::fs::read(&path).unwrap(), damaged);
        }
    }
}
