//! Coder's memory on the account (#11182): while the person's `/sync`
//! choice is on and this computer is signed in, Coder sends its notes and
//! deletions ([`Memory::account_records`]) to openagents.com and merges
//! the account's list back ([`Memory::merge`]), so a note saved here shows
//! in Settings, Memory on the website and in the web chat, and a note
//! changed or deleted there (or on another computer) changes here too.
//!
//! Coder looks for changed notes every [`coder_sync::memory::LOOK_EVERY`]
//! and asks for the account's list at least every
//! [`coder_sync::memory::PULL_EVERY`], one exchange at a time, off the
//! terminal's thread. With sync off nothing is sent.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use coder_sync::Answer;
use coder_sync::memory::{LOOK_EVERY, PULL_EVERY};
use serde_json::Value;

use crate::App;
use crate::memory::{Memory, SyncRecord};

/// How long Coder waits after an exchange that didn't go through.
const RETRY_AFTER: Duration = Duration::from_secs(60);
/// How long Coder waits when the website doesn't keep memory yet.
const MISSING_AFTER: Duration = Duration::from_secs(600);

/// Where the memory exchange stands.
#[derive(Default)]
pub(crate) struct MemorySync {
    /// When Coder may next look for changes.
    next_look: Option<Instant>,
    /// When the account's list last came back (or was last asked for).
    pulled: Option<Instant>,
    /// The digest of this computer's notes as of the last exchange.
    sent: Option<u64>,
    /// The exchange running now.
    running: Option<mpsc::Receiver<Outcome>>,
}

/// How one exchange ended.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The account has this computer's notes; `changed` notes changed here.
    Done { changed: usize, digest: u64 },
    /// The website doesn't keep memory (an older website).
    Missing,
    /// Not reached, refused, or signed out: try again later.
    Failed,
}

/// A digest of `records`, to tell whether anything changed since the last
/// exchange.
fn digest(records: &[SyncRecord]) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(records)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

/// Whether an exchange is due: this computer's notes changed since the
/// last one, or the account's list hasn't been asked for lately.
fn due(sent: Option<u64>, now: u64, pulled: Option<Instant>) -> bool {
    sent != Some(now) || pulled.is_none_or(|at| at.elapsed() >= PULL_EVERY)
}

/// One exchange: send `memory`'s records, merge the account's back.
fn exchange(saved: &openagents_login::Saved, memory: &Memory) -> Outcome {
    let records: Vec<Value> = memory
        .account_records()
        .iter()
        .filter_map(|record| serde_json::to_value(record).ok())
        .collect();
    match coder_sync::memory::exchange_now(saved, &records) {
        Ok(remote) => {
            let remote: Vec<SyncRecord> = remote
                .into_iter()
                .filter_map(|record| serde_json::from_value(record).ok())
                .collect();
            let changed = memory.merge(&remote);
            Outcome::Done {
                changed,
                digest: digest(&memory.account_records()),
            }
        }
        Err(Answer::Unknown) => Outcome::Missing,
        Err(_) => Outcome::Failed,
    }
}

impl App {
    /// Each tick: take the finished exchange, and start the next one when
    /// it is due, sync is on, and this computer is signed in.
    pub(crate) fn poll_memory(&mut self) {
        let Some(dir) = self.account_dir.clone() else {
            return;
        };
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let Some(sync) = &mut self.sync else {
            return;
        };
        let state = &mut sync.memory;
        if let Some(running) = &state.running {
            match running.try_recv() {
                Ok(outcome) => {
                    state.running = None;
                    match outcome {
                        Outcome::Done { changed, digest } => {
                            state.sent = Some(digest);
                            if changed > 0 && self.notice.is_none() {
                                self.notice = Some(if changed == 1 {
                                    "Memory: 1 note changed from your account.".into()
                                } else {
                                    format!("Memory: {changed} notes changed from your account.")
                                });
                            }
                        }
                        Outcome::Missing => {
                            state.next_look = Some(Instant::now() + MISSING_AFTER);
                        }
                        Outcome::Failed => {
                            state.next_look = Some(Instant::now() + RETRY_AFTER);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => state.running = None,
            }
        }
        let Some(sync) = &mut self.sync else {
            return;
        };
        // Private by default: nothing is sent while sync is off.
        if !sync.settings.on || sync.worker.is_none() {
            return;
        }
        let state = &mut sync.memory;
        let now = Instant::now();
        if state.next_look.is_some_and(|at| at > now) {
            return;
        }
        state.next_look = Some(now + LOOK_EVERY);
        let Some(memory) = Memory::discover(&cwd) else {
            return;
        };
        if !due(state.sent, digest(&memory.account_records()), state.pulled) {
            return;
        }
        let Some(saved) = crate::account_sync::signed_in(&dir) else {
            return;
        };
        state.pulled = Some(now);
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(exchange(&saved, &memory));
        });
        state.running = Some(receive);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exchange_is_due_when_notes_changed_or_the_list_is_old() {
        let recent = Some(Instant::now());
        assert!(due(None, 1, recent), "never sent");
        assert!(due(Some(1), 2, recent), "changed here");
        assert!(!due(Some(1), 1, recent), "nothing new");
        assert!(due(Some(1), 1, None), "never asked");
    }

    #[test]
    fn the_digest_follows_the_notes() {
        let record = |body: &str| SyncRecord {
            id: "mem-1".into(),
            scope: crate::memory::Scope::User,
            project: None,
            project_name: None,
            kind: Some(crate::memory::Kind::User),
            name: Some("Tabs".into()),
            description: None,
            body: Some(body.into()),
            updated: 1,
            deleted: false,
        };
        assert_eq!(digest(&[record("a")]), digest(&[record("a")]));
        assert_ne!(digest(&[record("a")]), digest(&[record("b")]));
    }
}
