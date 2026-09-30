//! The phone's durable outbox for Coder chat commands.
//!
//! A message, queued message, steer, or stop becomes a NIP-HOST
//! `task.command` with an ID this device mints once. The command waits here,
//! in the app's encrypted store, until the host answers it: a transport
//! failure keeps it and tries again later with the same ID, so a crash, a
//! relaunch, or a long time offline never sends a second command, and the
//! host never runs one twice. A signed refusal ends it with the host's
//! reason. The host lets a command live 24 hours after it was minted.

use coder_computers::cache::Cache;
use coder_host::{CommandAction, TaskCommand};
use serde::{Deserialize, Serialize};

/// The key the outbox is stored under.
const KEY: &str = "coder-outbox";
/// The most commands waiting at once.
pub const MAX_PENDING: usize = 64;
/// The first wait before trying a command again, in seconds; it doubles up
/// to [`MAX_BACKOFF`].
const FIRST_BACKOFF: u64 = 2;
const MAX_BACKOFF: u64 = 60;

/// One command waiting for its host's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub host: String,
    pub command: TaskCommand,
    /// Unix seconds before which it is not tried again.
    pub next_try: u64,
    pub tries: u32,
    /// When an attempt last failed to reach the host. Presence the host
    /// published after this, as its answer to a nudge, makes the command
    /// due at once.
    #[serde(default)]
    pub unreached_at: Option<u64>,
}

/// A command before it has an ID.
#[derive(Clone, Copy, Debug)]
pub struct Draft<'a> {
    pub task: &'a str,
    pub action: CommandAction,
    /// The task revision this device last read.
    pub based_on: u64,
    pub text: &'a str,
    /// For a steer: this device chose the engine's emulated steering.
    pub emulate: bool,
}

impl Draft<'_> {
    /// Freeze the same command bytes for every native client and transport.
    pub fn command(self, now: u64) -> TaskCommand {
        TaskCommand {
            command: coder_host::access::protocol::random_id(),
            task: self.task.into(),
            action: self.action,
            based_on: self.based_on,
            text: self.text.into(),
            emulate: self.emulate,
            issued_at: now,
        }
    }
}

/// What became of one attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// The host answered: the command is done here.
    Answered,
    /// The host refused it with this reason: done, and worth saying.
    Refused(String),
    /// The host was not reached: try again later with the same ID.
    Unreached,
}

/// The durable outbox.
pub struct Outbox {
    cache: Option<Cache>,
    pending: Vec<Pending>,
}

impl Outbox {
    /// An outbox kept in `cache`, loaded from it. Without a cache it keeps
    /// commands only while the app runs.
    pub fn open(cache: Option<Cache>) -> Self {
        let pending = cache
            .as_ref()
            .and_then(|cache| cache.read::<Vec<Pending>>(KEY).ok().flatten())
            .unwrap_or_default();
        Self { cache, pending }
    }

    /// Mint an ID for `draft`, date it `now`, and keep it. Returns `None`
    /// when the outbox is full.
    pub fn push(&mut self, host: &str, draft: Draft<'_>, now: u64) -> Option<&Pending> {
        if self.pending.len() >= MAX_PENDING {
            return None;
        }
        self.pending.push(Pending {
            host: host.to_owned(),
            command: draft.command(now),
            next_try: now,
            tries: 0,
            unreached_at: None,
        });
        self.save();
        self.pending.last()
    }

    /// The commands due to be tried at `now`, oldest first.
    #[cfg(test)]
    pub fn due(&self, now: u64) -> Vec<Pending> {
        self.due_or_awake(now, &|_| None)
    }

    /// The commands due at `now`, and those whose host has published
    /// presence since they last failed to reach it: `awake` gives a host's
    /// newest presence time.
    pub fn due_or_awake(&self, now: u64, awake: &dyn Fn(&str) -> Option<u64>) -> Vec<Pending> {
        self.pending
            .iter()
            .filter(|pending| {
                pending.next_try <= now
                    || pending
                        .unreached_at
                        .zip(awake(&pending.host))
                        .is_some_and(|(failed, seen)| seen > failed)
            })
            .cloned()
            .collect()
    }

    /// The commands still waiting for `task`.
    pub fn waiting(&self, task: &str) -> usize {
        self.pending
            .iter()
            .filter(|pending| pending.command.task == task)
            .count()
    }

    /// Whether the command with ID `command` still waits for its host.
    pub fn holds(&self, command: &str) -> bool {
        self.pending
            .iter()
            .any(|pending| pending.command.command == command)
    }

    /// Record what an attempt at the command with ID `command` did.
    pub fn settle(&mut self, command: &str, attempt: &Attempt, now: u64) {
        match attempt {
            Attempt::Answered | Attempt::Refused(_) => {
                self.pending
                    .retain(|pending| pending.command.command != command);
            }
            Attempt::Unreached => {
                if let Some(pending) = self
                    .pending
                    .iter_mut()
                    .find(|pending| pending.command.command == command)
                {
                    pending.tries = pending.tries.saturating_add(1);
                    pending.unreached_at = Some(now);
                    let wait = FIRST_BACKOFF
                        .saturating_mul(1 << pending.tries.min(6))
                        .min(MAX_BACKOFF);
                    pending.next_try = now.saturating_add(wait);
                }
            }
        }
        self.save();
    }

    fn save(&self) {
        if let Some(cache) = &self.cache {
            let _ = cache.write(KEY, &self.pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft<'a>(task: &'a str, action: CommandAction, based_on: u64, text: &'a str) -> Draft<'a> {
        Draft {
            task,
            action,
            based_on,
            text,
            emulate: false,
        }
    }

    fn cache(dir: &std::path::Path) -> Cache {
        let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        Cache::open(dir, &secret).unwrap()
    }

    #[test]
    fn a_command_survives_a_relaunch_with_its_id_until_the_host_answers() {
        let temp = tempfile::tempdir().unwrap();
        let task = "a".repeat(64);
        let id = {
            let mut outbox = Outbox::open(Some(cache(temp.path())));
            let pending = outbox
                .push("host", draft(&task, CommandAction::Queue, 4, "Next."), 100)
                .unwrap();
            pending.command.command.clone()
        };
        // A relaunch reads the same command back, with the same ID.
        let mut outbox = Outbox::open(Some(cache(temp.path())));
        let due = outbox.due(100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].command.command, id);
        assert_eq!(due[0].command.issued_at, 100);
        // Unreached: it waits, then comes back unchanged.
        outbox.settle(&id, &Attempt::Unreached, 100);
        assert!(outbox.due(101).is_empty());
        let again = outbox.due(200);
        assert_eq!(again[0].command, due[0].command);
        assert_eq!(outbox.waiting(&task), 1);
        // Presence the host published after the failure, as its answer to
        // a nudge, makes it due before its backoff ends.
        outbox.settle(&id, &Attempt::Unreached, 200);
        assert!(outbox.due(201).is_empty());
        assert!(outbox.due_or_awake(201, &|_| Some(199)).is_empty());
        assert_eq!(outbox.due_or_awake(201, &|_| Some(201)).len(), 1);
        // A refusal or an answer ends it, durably.
        outbox.settle(&id, &Attempt::Refused("Stale".into()), 200);
        assert_eq!(outbox.waiting(&task), 0);
        assert!(
            Outbox::open(Some(cache(temp.path())))
                .due(u64::MAX)
                .is_empty()
        );
    }

    #[test]
    fn the_outbox_is_bounded() {
        let mut outbox = Outbox::open(None);
        for _ in 0..MAX_PENDING {
            assert!(
                outbox
                    .push("host", draft("task", CommandAction::Send, 1, "Hi"), 1)
                    .is_some()
            );
        }
        assert!(
            outbox
                .push("host", draft("task", CommandAction::Send, 1, "Hi"), 1)
                .is_none()
        );
    }
}
