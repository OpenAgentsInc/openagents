//! The one NIP-REACH host generation counter for a host root.
//!
//! A host's generation increases whenever the host restarts, updates, or
//! restores state, and a client refuses a generation lower than one it
//! already holds. A standalone `coder host serve` and the service launcher
//! both start hosts from the same root, so both take generations from the
//! one counter this module owns: the `generation` record in the host root.
//! No other code computes a next generation.
//!
//! # Rules
//!
//! - [`reserve`] advances the counter and returns the new value, unclaimed.
//!   The launcher reserves a generation and passes it to the host it
//!   starts.
//! - [`claim`] marks a reserved or higher generation as used by one running
//!   host. A host started with an explicit generation claims it before it
//!   serves. Claiming a generation below the counter, or one already
//!   claimed, refuses: that value was or may have been used before.
//! - [`advance`] reserves and claims in one step, for a standalone host.
//!
//! The next value is one more than the largest of the recorded value, the
//! caller's floor, and the Unix time in seconds. The clock term keeps
//! generations increasing if the record is lost; the floor carries forward
//! a generation recorded elsewhere, such as an older launcher record.
//!
//! # Durability
//!
//! Every change holds an exclusive lock on `generation.lock` in the host
//! root, so concurrent starts serialize and each gets a distinct value. The
//! record is replaced atomically and synced before the new value is
//! returned. A crash after a reservation and before its use therefore
//! skips that value on the next start rather than reusing it. A record that
//! cannot be read refuses; it is never reset automatically, because a reset
//! could hand out a value a client already saw.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, fsx};

/// The record schema.
pub const SCHEMA: &str = "openagents.coder.host-generation.v1";
/// The record's file name in the host root.
pub const FILE: &str = "generation";
const LOCK: &str = "generation.lock";
/// The largest record this module reads.
const RECORD_MAX: u64 = 4096;

/// The counter record, `generation` in the host root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Always [`SCHEMA`].
    pub schema: String,
    /// The highest generation handed out.
    pub generation: u64,
    /// Whether a host started with that generation.
    pub claimed: bool,
}

impl Record {
    fn new(generation: u64, claimed: bool) -> Self {
        Record {
            schema: SCHEMA.into(),
            generation,
            claimed,
        }
    }
}

/// The counter's path under `root`.
#[must_use]
pub fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// Reads the counter without changing it. `None` means no host started
/// from this root yet.
pub fn current(root: &Path) -> Result<Option<Record>> {
    read(root)
}

/// Advances the counter for a host another process starts, such as the
/// launcher's host, and returns the new generation unclaimed. `floor` is a
/// generation the caller already recorded; the result exceeds it.
pub fn reserve(root: &Path, floor: u64) -> Result<u64> {
    change(root, |record| {
        let next = next(record.as_ref(), floor)?;
        Ok((Record::new(next, false), next))
    })
}

/// Claims `generation` for a host about to serve. It must be the reserved,
/// unclaimed value or higher than the counter; anything else refuses,
/// because a client may already have seen it.
pub fn claim(root: &Path, generation: u64) -> Result<()> {
    change(root, |record| {
        match &record {
            Some(r) if generation < r.generation => {
                return Err(Error::refused(format!(
                    "host generation {generation} is below the recorded generation {}; \
                     clients would refuse it",
                    r.generation
                )));
            }
            Some(r) if generation == r.generation && r.claimed => {
                return Err(Error::refused(format!(
                    "host generation {generation} was already used; start without a \
                     generation to take the next one"
                )));
            }
            _ => {}
        }
        Ok((Record::new(generation, true), ()))
    })
}

/// Advances and claims the counter in one step, for a host that starts
/// itself, and returns the new generation.
pub fn advance(root: &Path) -> Result<u64> {
    change(root, |record| {
        let next = next(record.as_ref(), 0)?;
        Ok((Record::new(next, true), next))
    })
}

fn next(record: Option<&Record>, floor: u64) -> Result<u64> {
    let recorded = record.map_or(0, |r| r.generation);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    recorded
        .max(floor)
        .max(now)
        .checked_add(1)
        .ok_or_else(|| Error::refused("the host generation counter is exhausted"))
}

/// Runs one read-modify-write under the counter lock. The new record is
/// durable before this returns.
fn change<T>(root: &Path, update: impl FnOnce(Option<Record>) -> Result<(Record, T)>) -> Result<T> {
    create_root(root)?;
    let _lock = lock(root)?;
    let (record, value) = update(read(root)?)?;
    // A crash during an earlier write by a process with this identifier
    // could leave its temporary file; the lock makes removing it safe.
    fsx::remove_file_if_present(&root.join(format!(".{FILE}.pending-{}", std::process::id())))?;
    fsx::atomic_write(&path(root), &serde_json::to_vec(&record)?, 0o600)?;
    Ok(value)
}

fn read(root: &Path) -> Result<Option<Record>> {
    let path = path(root);
    let bytes = match fsx::read_bounded(&path, RECORD_MAX) {
        Ok(bytes) => bytes,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if let Ok(record) = serde_json::from_slice::<Record>(&bytes)
        && record.schema == SCHEMA
    {
        return Ok(Some(record));
    }
    // A standalone host before this record kept a bare number, which the
    // host that wrote it used.
    std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|generation| Some(Record::new(generation, true)))
        .ok_or_else(|| {
            Error::refused(format!(
                "{} is not a host generation record; it is never reset automatically",
                path.display()
            ))
        })
}

/// Creates a missing host root privately. An existing root is used as it
/// is; the launcher checks its own root's permissions separately.
fn create_root(root: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    if !root.is_dir() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root)?;
    }
    Ok(())
}

/// Takes the counter lock, waiting while another starting host holds it.
fn lock(root: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(LOCK))?;
    loop {
        // SAFETY: `flock` reads a descriptor this function owns and one
        // integer flag.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}

#[cfg(test)]
mod tests;
