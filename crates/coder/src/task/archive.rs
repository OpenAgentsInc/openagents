//! Archived tasks: finished work the owner took off every device's lists.
//!
//! Archiving hides a finished or cancelled task from the lists a device
//! reads: the host stops publishing its activity summary, and the history
//! observer marks its transcript's chat as archived, which chat lists leave
//! out. It deletes nothing. The task, its commands, its run evidence, and its
//! transcript stay exactly where they were, and restoring the task shows it
//! again.
//!
//! The record lives beside the task document, in `archive.json`, and is
//! written only under the task store's lock. It is a separate file so that an
//! older binary, which does not know it, still reads the task document
//! unchanged and simply shows every task.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Error, MAX_TASKS, Status, Store, private_open, regular_or_absent, text};

/// The archive record's schema.
pub const SCHEMA: &str = "openagents.coder.task-archive.v1";
/// The archive record's filename within the task store directory.
pub const FILE: &str = "archive.json";
const PENDING: &str = ".archive.pending";
/// The largest archive record, in bytes.
const MAX_BYTES: u64 = 1024 * 1024;
/// The longest archive reason, in bytes.
pub const MAX_REASON_BYTES: usize = 512;

/// Who archived a task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum By {
    /// The host's owner, with a command on the host.
    Owner,
    /// An enrolled device holding `operate`, by its key.
    Device { key: String },
}

/// One archived task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Unix seconds.
    pub at: u64,
    pub reason: String,
    pub by: By,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    tasks: BTreeMap<String, Entry>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            tasks: BTreeMap::new(),
        }
    }
}

/// Archive `task`, a finished or cancelled task in the store at `dir`.
/// Returns `false` when it was already archived, which leaves the first
/// entry unchanged.
///
/// # Errors
/// `NotFound` for an unknown task, `InvalidTransition` for a task that has
/// not ended, `InvalidCommand` for an empty or overlong reason, and store
/// I/O failures.
pub fn archive(dir: &Path, task: &str, reason: &str, by: By, at: u64) -> Result<bool, Error> {
    if !text(reason, MAX_REASON_BYTES, false) {
        return Err(Error::InvalidCommand(
            "an archive needs a nonempty single-line reason of at most 512 bytes",
        ));
    }
    if let By::Device { key } = &by
        && !(key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(Error::InvalidCommand(
            "a device key is 64 hexadecimal characters",
        ));
    }
    // The store's lock serializes archive writes with every task change.
    let store = Store::open(dir)?;
    let current = store.show(task)?;
    if !matches!(current.status, Status::Finished | Status::Cancelled) {
        return Err(Error::InvalidTransition);
    }
    let mut record = read(&store.dir)?;
    if record.tasks.contains_key(task) {
        return Ok(false);
    }
    if record.tasks.len() >= MAX_TASKS {
        return Err(Error::LimitExceeded);
    }
    record.tasks.insert(
        task.to_owned(),
        Entry {
            at,
            reason: reason.to_owned(),
            by,
        },
    );
    write(&store.dir, &record)?;
    Ok(true)
}

/// Show an archived task again. Returns `false` when it was not archived.
///
/// # Errors
/// Store I/O failures.
pub fn restore(dir: &Path, task: &str) -> Result<bool, Error> {
    let store = Store::open(dir)?;
    let mut record = read(&store.dir)?;
    if record.tasks.remove(task).is_none() {
        return Ok(false);
    }
    write(&store.dir, &record)?;
    Ok(true)
}

/// The archived tasks and their entries in the store at `dir`.
///
/// # Errors
/// A malformed or unsafe record, and I/O failures other than absence.
pub fn entries(dir: &Path) -> Result<BTreeMap<String, Entry>, Error> {
    Ok(read(dir)?.tasks)
}

/// The IDs of the archived tasks in the store at `dir`. An unreadable
/// record hides nothing: every task stays listed.
#[must_use]
pub fn archived(dir: &Path) -> BTreeSet<String> {
    entries(dir)
        .map(|tasks| tasks.into_keys().collect())
        .unwrap_or_default()
}

fn read(dir: &Path) -> Result<Record, Error> {
    let path = dir.join(FILE);
    if !regular_or_absent(&path)? {
        return Ok(Record::default());
    }
    let mut bytes = Vec::new();
    private_open(&path, false, false)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::LimitExceeded);
    }
    let record: Record = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Corrupt("the task archive record is malformed"))?;
    if record.schema != SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    Ok(record)
}

fn write(dir: &Path, record: &Record) -> Result<(), Error> {
    let bytes = serde_json::to_vec(record)
        .map_err(|_| Error::Corrupt("the task archive record could not be encoded"))?;
    let pending = dir.join(PENDING);
    if regular_or_absent(&pending)? {
        std::fs::remove_file(&pending)?;
    }
    let result = (|| {
        let mut file = private_open(&pending, true, true)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        regular_or_absent(&dir.join(FILE))?;
        std::fs::rename(&pending, dir.join(FILE))?;
        super::sync_directory(dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{
        Action, COMMAND_SCHEMA, Command, RequestedConfiguration, TaskIntent, Workspace,
    };

    fn submit(store: &mut Store, id: &str) {
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("submit-{id}"),
            task_id: id.into(),
            expected_revision: None,
            action: Action::Submit {
                intent: TaskIntent {
                    title: "Title".into(),
                    prompt: "Prompt".into(),
                    workspace: Workspace {
                        path: if cfg!(windows) {
                            r"C:\tmp\checkout"
                        } else {
                            "/tmp/checkout"
                        }
                        .into(),
                        source_revision: None,
                    },
                    configuration: RequestedConfiguration {
                        adapter: "bounded-command".into(),
                        model: None,
                    },
                },
            },
        };
        store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    }

    fn cancel(store: &mut Store, id: &str) {
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("cancel-{id}"),
            task_id: id.into(),
            expected_revision: Some(1),
            action: Action::Cancel {
                reason: "Not needed".into(),
            },
        };
        store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    }

    #[test]
    fn only_an_ended_task_is_archived_and_nothing_is_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("tasks");
        {
            let mut store = Store::open(&dir).unwrap();
            submit(&mut store, "queued");
            submit(&mut store, "ended");
            cancel(&mut store, "ended");
        }
        std::fs::write(dir.join("ended.1.atif.jsonl"), "{}\n").unwrap();
        let before = std::fs::read(dir.join("tasks.json")).unwrap();
        // A task that has not ended stays listed.
        assert!(matches!(
            archive(&dir, "queued", "Test chat", By::Owner, 5),
            Err(Error::InvalidTransition)
        ));
        assert!(matches!(
            archive(&dir, "missing", "Test chat", By::Owner, 5),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            archive(&dir, "ended", "two\nlines", By::Owner, 5),
            Err(Error::InvalidCommand(_))
        ));
        assert!(archived(&dir).is_empty());
        let device = By::Device {
            key: "ab".repeat(32),
        };
        assert!(archive(&dir, "ended", "Test chat", device.clone(), 5).unwrap());
        // A repeat keeps the first entry.
        assert!(!archive(&dir, "ended", "Again", By::Owner, 9).unwrap());
        let entries = entries(&dir).unwrap();
        assert_eq!(
            entries["ended"],
            Entry {
                at: 5,
                reason: "Test chat".into(),
                by: device
            }
        );
        assert_eq!(archived(&dir), BTreeSet::from(["ended".to_owned()]));
        // The task document and the transcript are untouched.
        assert_eq!(std::fs::read(dir.join("tasks.json")).unwrap(), before);
        assert!(dir.join("ended.1.atif.jsonl").exists());
        assert_eq!(Store::open(&dir).unwrap().list().unwrap().len(), 2);
        // Restoring shows it again.
        assert!(restore(&dir, "ended").unwrap());
        assert!(!restore(&dir, "ended").unwrap());
        assert!(archived(&dir).is_empty());
    }

    #[test]
    fn a_malformed_record_hides_nothing_and_refuses_writes() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("tasks");
        {
            let mut store = Store::open(&dir).unwrap();
            submit(&mut store, "ended");
            cancel(&mut store, "ended");
        }
        let path = dir.join(FILE);
        std::fs::write(&path, "not json").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(archived(&dir).is_empty());
        assert!(matches!(
            archive(&dir, "ended", "Test chat", By::Owner, 5),
            Err(Error::Corrupt(_))
        ));
    }
}
