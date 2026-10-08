//! A read-only look at the lease table for a viewer, such as Verse's
//! Pylon Field (`docs/compute/verse-compute.md`, P0): which leases are held
//! now and how many receipts the broker has written.
//!
//! [`observe`] never takes the table lock, never prunes, and never removes
//! a holder lock, so a viewer that polls it can't change what the broker
//! decides. The table is written by an atomic rename, so an unlocked read
//! sees a whole table. A held lease counts only while its holder lock is
//! still held by a live process; a dead holder's lease, which the next
//! broker call drops, isn't counted.

use std::fs::OpenOptions;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::table::{Table, held_lock};
use crate::{Entry, Error, State, TABLE_SCHEMA};

/// What the lease root holds now, as a viewer sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Observed {
    /// The held leases whose holders still live.
    pub held: Vec<Entry>,
    /// How many leases wait in a queue.
    pub waiting: usize,
    /// How many receipts the broker has written: one per released lease.
    pub receipts: u64,
    /// How many of those were written within the recent window.
    pub recent_receipts: u64,
}

impl Observed {
    /// The amount held of `resource`, such as the `build` slots in use.
    #[must_use]
    pub fn held_amount(&self, resource: &str) -> u64 {
        self.held
            .iter()
            .filter(|entry| entry.resource == resource)
            .map(|entry| entry.amount)
            .sum()
    }
}

/// Reads the lease root at `root` without changing it. Receipts written
/// within `recent` of now count as recent.
///
/// # Errors
/// The root doesn't exist, or its table can't be read or isn't the
/// broker's.
pub fn observe(root: &Path, recent: Duration) -> Result<Observed, Error> {
    if !root.is_dir() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a lease root", root.display()),
        )));
    }
    let path = root.join("table.json");
    let table = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Table>(&bytes)
            .map_err(|error| Error::Corrupt(format!("{}: {error}", path.display())))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Table::default(),
        Err(error) => return Err(error.into()),
    };
    if table.schema != TABLE_SCHEMA {
        return Err(Error::Corrupt(format!(
            "{} has schema {}, not {TABLE_SCHEMA}",
            path.display(),
            table.schema
        )));
    }
    let mut out = Observed::default();
    for entry in table.leases {
        match entry.state {
            State::Held if holder_lives(root, &entry.id) => out.held.push(entry),
            State::Held => {}
            State::Waiting => out.waiting += 1,
        }
    }
    let now = SystemTime::now();
    if let Ok(files) = std::fs::read_dir(root.join("receipts")) {
        for file in files.flatten() {
            if !file.file_name().to_string_lossy().ends_with(".json") {
                continue;
            }
            out.receipts += 1;
            let fresh = file
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|at| now.duration_since(at).ok())
                .is_some_and(|age| age <= recent);
            if fresh {
                out.recent_receipts += 1;
            }
        }
    }
    Ok(out)
}

/// Whether some process holds lease `id`'s holder lock. Unlike the
/// broker's own check, it leaves the file in place either way.
fn holder_lives(root: &Path, id: &str) -> bool {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = match options.open(held_lock(root, id)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        // Something is there that can't be opened: count the lease rather
        // than guess.
        Err(_) => return true,
    };
    match file.try_lock() {
        // Nobody holds it; dropping the file releases the lock at once.
        Ok(()) => false,
        Err(std::fs::TryLockError::WouldBlock | std::fs::TryLockError::Error(_)) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Broker, Holder, Limits, Request, Resource, Wait};

    fn limits() -> Limits {
        Limits {
            build: 2,
            memory_gib: 8,
            disk_floor_gb: 0,
            build_disk_gb: 0,
        }
    }

    fn holder() -> Holder {
        Holder {
            session: "test".into(),
            agent: "none".into(),
            pid: std::process::id(),
            command: "cargo".into(),
        }
    }

    #[test]
    fn a_missing_root_is_an_error_and_an_empty_root_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(observe(&dir.path().join("absent"), Duration::from_secs(60)).is_err());
        let seen = observe(dir.path(), Duration::from_secs(60)).unwrap();
        assert_eq!(seen, Observed::default());
    }

    #[test]
    fn a_held_build_lease_counts_while_held_and_leaves_a_receipt_after() {
        let dir = tempfile::tempdir().unwrap();
        let broker = Broker::new(dir.path().to_path_buf(), limits());
        let lease = broker
            .acquire(Request::new(Resource::parse("build").unwrap(), holder()).wait(Wait::No))
            .unwrap();
        let table = std::fs::read(dir.path().join("table.json")).unwrap();
        let seen = observe(dir.path(), Duration::from_secs(60)).unwrap();
        assert_eq!(seen.held_amount("build"), 1);
        assert_eq!(seen.receipts, 0);
        // Observing changed nothing on disk.
        assert_eq!(std::fs::read(dir.path().join("table.json")).unwrap(), table);
        lease.release(Some(0)).unwrap();
        let seen = observe(dir.path(), Duration::from_secs(60)).unwrap();
        assert_eq!(seen.held_amount("build"), 0);
        assert_eq!((seen.receipts, seen.recent_receipts), (1, 1));
    }
}
