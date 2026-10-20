//! Alice's durable work queue (`agents/NAME/queue.json`,
//! `openagents.agent-queue.v1`; #10931).
//!
//! `openagents agent queue alice add TEXT` appends a waiting entry; every
//! time she finishes a request, [`take`] hands her the oldest waiting one
//! as her next task-mode request, so a night's issues run back to back.
//! An entry's `computer` is the same word `agent ask --computer` reads:
//! `auto` (the first computer her policy allows with a free slot, else
//! this host), `local`, or a computer's name — resolved when the entry
//! starts, not when it was queued, so a box that comes up mid-queue takes
//! work that waited for it.
//!
//! - **Durable.** The file is the queue: a restart reloads it, and an
//!   entry marked `running` when no run is under way returns to `waiting`
//!   the next time she goes idle, so a crash never loses it.
//! - **Ordered.** She is still one request at a time, so entries start and
//!   their changes reach the Merge station in the order they were added.
//! - **Visible.** `queue list` prints each entry's state; `add` and every
//!   start and end are journaled. `agent stop` clears the file along with
//!   her in-memory queue.

use serde::{Deserialize, Serialize};

/// The queue file beside her record.
pub const QUEUE_FILE: &str = "queue.json";
/// Its schema.
pub const QUEUE_SCHEMA: &str = "openagents.agent-queue.v1";
/// The most finished entries `list` keeps.
const DONE_KEEP: usize = 8;
/// The most entries the queue holds waiting.
pub const QUEUE_MAX: usize = 32;

/// The queue document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            schema: QUEUE_SCHEMA.to_string(),
            entries: Vec::new(),
        }
    }
}

/// One queued request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Its identity: a queue-unique short hex word.
    pub id: String,
    /// The request's text.
    pub text: String,
    /// The workspace label it names, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Where it may run: `auto`, `local`, or a computer's name; `auto`
    /// when unnamed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    /// When it was added, as seconds.
    pub at: u64,
    /// Where it stands.
    #[serde(default)]
    pub state: State,
}

/// Where an entry stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// It waits for her.
    #[default]
    Waiting,
    /// A request took it; no run of hers may be under way when the queue
    /// is read, so a running entry older than that moment is lost work
    /// and goes back to `waiting` on reconcile.
    Running,
    /// Its request finished.
    Done,
    /// Its request failed.
    Failed,
}

impl Queue {
    /// The queue beside `store`'s record; the empty one when there is
    /// none or it cannot be read.
    #[must_use]
    pub fn load(store: &super::agent::Store) -> Self {
        let path = queue_path(store);
        std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|queue| queue.schema == QUEUE_SCHEMA)
            .unwrap_or_default()
    }

    /// Write the queue back, owner-only.
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn save(&self, store: &super::agent::Store) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        super::autostart::write_private(&queue_path(store), &bytes)
    }

    /// Append `entry` as `waiting`, with its identity.
    ///
    /// # Errors
    /// The queue already holds [`QUEUE_MAX`] waiting entries.
    pub fn add(&mut self, mut entry: Entry, now: u64, key: &str) -> Result<(), String> {
        let waiting = self
            .entries
            .iter()
            .filter(|entry| entry.state == State::Waiting || entry.state == State::Running)
            .count();
        if waiting >= QUEUE_MAX {
            return Err(format!(
                "the queue already holds {QUEUE_MAX} waiting entries"
            ));
        }
        entry.at = now;
        entry.state = State::Waiting;
        let digest = nostr::contracts::digest_bytes(
            format!("{}:{key}:{now}:{}", self.entries.len(), entry.text).as_bytes(),
        );
        entry.id = format!("q-{}-{}", now, &digest.trim_start_matches("sha256:")[..8]);
        self.entries.push(entry);
        Ok(())
    }

    /// The oldest waiting entry as `running`, or none. Every `running`
    /// entry left by a crash — she is idle when this is called, so a
    /// running mark means the run that held it ended with the host — is
    /// waiting again first.
    #[must_use]
    pub fn take(&mut self) -> Option<Entry> {
        for entry in &mut self.entries {
            if entry.state == State::Running {
                entry.state = State::Waiting;
            }
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.state == State::Waiting)?;
        entry.state = State::Running;
        Some(entry.clone())
    }

    /// Mark `id`'s entry `done` or `failed` and drop the finished past
    /// [`DONE_KEEP`].
    pub fn finish(&mut self, id: &str, done: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.state = if done { State::Done } else { State::Failed };
        }
        let finished = self
            .entries
            .iter()
            .filter(|entry| matches!(entry.state, State::Done | State::Failed))
            .count();
        let over = finished.saturating_sub(DONE_KEEP);
        if over > 0 {
            let mut dropped = 0;
            self.entries.retain(|entry| {
                if dropped < over && matches!(entry.state, State::Done | State::Failed) {
                    dropped += 1;
                    false
                } else {
                    true
                }
            });
        }
    }

    /// The waiting and running entries, oldest first.
    #[must_use]
    pub fn waiting(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.state == State::Waiting || entry.state == State::Running)
            .collect()
    }

    /// Remove `id`'s entry when it waits; running entries finish.
    ///
    /// # Errors
    /// No waiting entry has `id`.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !(entry.id == id && entry.state == State::Waiting));
        if self.entries.len() == before {
            return Err(format!("no waiting entry `{id}`"));
        }
        Ok(())
    }
}

fn queue_path(store: &super::agent::Store) -> std::path::PathBuf {
    store.dir().join(QUEUE_FILE)
}

/// The oldest waiting entry marked running and saved, for her next
/// request; `None` when the queue is empty or cannot be written.
#[must_use]
pub fn take(store: &super::agent::Store) -> Option<Entry> {
    let mut queue = Queue::load(store);
    let entry = queue.take()?;
    queue.save(store).ok()?;
    Some(entry)
}

/// Mark `id`'s entry ended; a save failure loses the mark, so the entry
/// waits again on the next reconcile instead of doubling.
pub fn finish(store: &super::agent::Store, id: &str, done: bool) {
    let mut queue = Queue::load(store);
    queue.finish(id, done);
    let _ = queue.save(store);
}

/// Empty the queue, for `agent stop`; waiting entries stop being work,
/// running ones the stop already cancelled close as cancelled.
pub fn clear(store: &super::agent::Store) -> usize {
    let mut queue = Queue::load(store);
    let count = queue.waiting().len();
    queue.entries.clear();
    let _ = queue.save(store);
    count
}

/// The queue file's path, for `queue` CLI reads without the host.
#[must_use]
pub fn file(store: &super::agent::Store) -> std::path::PathBuf {
    queue_path(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::agent::Store;

    fn store(dir: &tempfile::TempDir) -> Store {
        let store = Store::new(dir.path(), "alice").unwrap();
        store.open(dir.path(), 1).unwrap();
        store
    }

    fn entry(text: &str, computer: Option<&str>) -> Entry {
        Entry {
            id: String::new(),
            text: text.into(),
            workspace: None,
            computer: computer.map(str::to_owned),
            at: 0,
            state: State::Waiting,
        }
    }

    #[test]
    fn the_queue_keeps_order_and_survives_a_crash_mid_run() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        let mut queue = Queue::load(&store);
        queue.add(entry("first", None), 100, "alice").unwrap();
        queue
            .add(entry("second", Some("coderos-4080")), 101, "alice")
            .unwrap();
        queue.save(&store).unwrap();
        // `take` runs the oldest first.
        let first = take(&store).unwrap();
        assert_eq!(first.text, "first");
        assert_eq!(Queue::load(&store).entries[0].state, State::Running);
        // A crash before `finish` leaves it running; the next take —
        // called only while she is idle — returns it to waiting and hands
        // it out again.
        let again = take(&store).unwrap();
        assert_eq!(again.id, first.id);
        finish(&store, &first.id, true);
        assert_eq!(Queue::load(&store).entries[0].state, State::Done);
        let second = take(&store).unwrap();
        assert_eq!(second.text, "second");
        assert_eq!(second.computer.as_deref(), Some("coderos-4080"));
        finish(&store, &second.id, false);
        assert_eq!(Queue::load(&store).entries[1].state, State::Failed);
        // `remove` takes a waiting entry back; a finished one stays.
        let mut queue = Queue::load(&store);
        queue.add(entry("third", None), 102, "alice").unwrap();
        queue.save(&store).unwrap();
        let third = Queue::load(&store).entries[2].id.clone();
        let mut queue = Queue::load(&store);
        queue.remove(&third).unwrap();
        assert!(queue.remove(&third).is_err());
        // `clear` empties the queue and counts what waited.
        queue.add(entry("fourth", None), 103, "alice").unwrap();
        queue.save(&store).unwrap();
        assert_eq!(clear(&store), 1);
        assert!(Queue::load(&store).waiting().is_empty());
    }
}
