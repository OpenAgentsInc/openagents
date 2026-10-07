//! Reclaim the kache compile cache through kache's own collector
//! (docs/background/kache.md, #10758).
//!
//! kache serializes its collectors with `gc.lock` in the store directory, an
//! OS advisory lock held through an open file. The kernel drops it when the
//! holder exits, so a dead holder never blocks a collection; the pid written
//! into the file is only a note of the last holder. This module therefore
//! never deletes or rewrites the lock file (kache documents that unlinking it
//! can let two collectors run at once) and never deletes anything under the
//! store: it reads who holds the lock, waits while a live collector runs, then
//! runs `kache gc --json` and reports what the collection freed.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

/// The lock file kache's collectors share, inside the store directory.
pub const GC_LOCK: &str = "gc.lock";

/// Who holds `gc.lock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Lock {
    /// The store has no lock file yet.
    Absent,
    /// No process holds the lock. `last_pid` is the last holder the file
    /// names; `last_alive` says whether that process still runs. A dead last
    /// holder is a stale note, not a held lock, so there is nothing to clear.
    Free {
        last_pid: Option<u32>,
        last_alive: bool,
    },
    /// A process holds the lock: a collection is running now.
    Held { pid: Option<u32>, alive: bool },
}

/// Read who holds `gc.lock` in `store_dir`.
///
/// The probe takes the lock for an instant when no one holds it, which is
/// how an advisory lock is tested. It never changes or removes the file.
pub fn probe(store_dir: &Path) -> Lock {
    let path = store_dir.join(GC_LOCK);
    let Ok(file) = File::open(&path) else {
        return Lock::Absent;
    };
    let pid = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok());
    let alive = pid.is_some_and(pid_alive);
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            Lock::Free {
                last_pid: pid,
                last_alive: alive,
            }
        }
        Err(_) => Lock::Held { pid, alive },
    }
}

/// Whether a process with this id runs (signal 0 checks without sending).
pub fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill with signal 0 only checks that the process exists.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// The store's size as kache reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Disk {
    /// Bytes of every blob the store names, the figure `local_max_size` caps.
    pub store_bytes: u64,
    /// The cap, `local_max_size`.
    pub store_limit_bytes: u64,
    /// Blob bytes no other file shares: what deleting the store would free.
    pub disk_private_bytes: u64,
    /// Blob bytes target directories also hold as APFS clones or hard links.
    pub cloned_into_targets_bytes: u64,
}

impl Disk {
    pub fn under_cap(&self) -> bool {
        self.store_bytes <= self.store_limit_bytes
    }
}

/// What one reclaim did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reclaim {
    /// The store directory whose lock was read.
    pub store_dir: PathBuf,
    /// The lock as found before the first collection attempt.
    pub lock: Lock,
    pub before: Disk,
    pub after: Disk,
    /// Collections kache skipped because another collector held the lock.
    pub skipped: u32,
    /// Whether a collection ran to completion.
    pub collected: bool,
    pub entries_dropped: u64,
    /// Store bytes the run removed, and the private disk bytes that returned.
    pub store_bytes_removed: u64,
    pub disk_bytes_reclaimed: u64,
}

/// How to reach kache and how long to wait for a running collector.
#[derive(Debug, Clone)]
pub struct Kache {
    /// The `kache` program.
    pub program: PathBuf,
    /// Collection attempts before giving up on a busy lock.
    pub attempts: u32,
    /// Wait between attempts while a live collector holds the lock.
    pub wait: Duration,
}

impl Default for Kache {
    fn default() -> Self {
        Self {
            program: PathBuf::from("kache"),
            attempts: 10,
            wait: Duration::from_secs(30),
        }
    }
}

#[derive(serde::Deserialize)]
struct Stats {
    disk: Disk,
    #[serde(default)]
    stores: Vec<StoreRow>,
}

#[derive(serde::Deserialize)]
struct StoreRow {
    path: PathBuf,
}

#[derive(serde::Deserialize)]
struct Gc {
    skipped: bool,
    disk: Disk,
    #[serde(default)]
    entries_dropped: u64,
    #[serde(default)]
    store_bytes_removed: u64,
    #[serde(default)]
    disk_bytes_reclaimed: u64,
}

impl Kache {
    fn json<T: serde::de::DeserializeOwned>(&self, args: &[&str]) -> Result<T, String> {
        let out = Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|e| format!("could not run {}: {e}", self.program.display()))?;
        if !out.status.success() {
            return Err(format!(
                "`kache {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        serde_json::from_slice(&out.stdout)
            .map_err(|e| format!("`kache {}` printed unreadable JSON: {e}", args.join(" ")))
    }

    /// The store's size and directory, from `kache stats --json`.
    pub fn status(&self) -> Result<(PathBuf, Disk, Lock), String> {
        let stats: Stats = self.json(&["stats", "--json"])?;
        let cache = stats
            .stores
            .first()
            .map(|row| row.path.clone())
            .ok_or("`kache stats` named no store")?;
        let store_dir = cache.join("store");
        let lock = probe(&store_dir);
        Ok((store_dir, stats.disk, lock))
    }

    /// Run kache's collector until it completes or `attempts` run out.
    pub fn reclaim(&self) -> Result<Reclaim, String> {
        let (store_dir, before, lock) = self.status()?;
        let mut report = Reclaim {
            store_dir,
            lock,
            before,
            after: before,
            skipped: 0,
            collected: false,
            entries_dropped: 0,
            store_bytes_removed: 0,
            disk_bytes_reclaimed: 0,
        };
        for attempt in 0..self.attempts.max(1) {
            if attempt > 0 {
                std::thread::sleep(self.wait);
            }
            // A live collector is doing the work already; wait for it rather
            // than racing it. A dead or absent holder cannot block kache.
            if matches!(probe(&report.store_dir), Lock::Held { alive: true, .. }) {
                report.skipped += 1;
                continue;
            }
            let gc: Gc = self.json(&["gc", "--json"])?;
            report.after = gc.disk;
            if gc.skipped {
                report.skipped += 1;
                continue;
            }
            report.collected = true;
            report.entries_dropped = gc.entries_dropped;
            report.store_bytes_removed = gc.store_bytes_removed;
            report.disk_bytes_reclaimed = gc.disk_bytes_reclaimed;
            break;
        }
        if !report.collected {
            // Another collector may have done the work; report where it left
            // the store.
            if let Ok((_, disk, _)) = self.status() {
                report.after = disk;
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    /// A pid that ran and exited, so no process has it now.
    fn dead_pid() -> u32 {
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    fn write_lock(store: &Path, pid: u32) -> File {
        std::fs::create_dir_all(store).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(store.join(GC_LOCK))
            .unwrap();
        write!(file, "{pid}").unwrap();
        file
    }

    #[test]
    fn a_dead_holder_leaves_the_lock_free_and_the_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let pid = dead_pid();
        drop(write_lock(&store, pid));
        assert!(!pid_alive(pid));
        assert_eq!(
            probe(&store),
            Lock::Free {
                last_pid: Some(pid),
                last_alive: false
            }
        );
        // The probe never removes or rewrites kache's lock file.
        assert_eq!(
            std::fs::read_to_string(store.join(GC_LOCK)).unwrap(),
            pid.to_string()
        );
    }

    #[test]
    fn a_live_holder_holds_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let me = std::process::id();
        let held = write_lock(&store, me);
        held.lock().unwrap();
        assert_eq!(
            probe(&store),
            Lock::Held {
                pid: Some(me),
                alive: true
            }
        );
        drop(held);
        // A process another test forks inherits the descriptor until it
        // execs, so the release can trail the drop by a moment.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while matches!(probe(&store), Lock::Held { .. }) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            probe(&store),
            Lock::Free {
                last_pid: Some(me),
                last_alive: true
            }
        );
    }

    #[test]
    fn a_store_without_a_lock_file_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(probe(dir.path()), Lock::Absent);
    }

    /// A stand-in `kache` that prints canned JSON for `stats` and `gc`.
    fn fake_kache(dir: &Path, cache: &Path, gc_skipped: bool) -> PathBuf {
        let disk = |store: u64| {
            format!(
                r#"{{"store_bytes":{store},"store_limit_bytes":100,"disk_private_bytes":{store},"cloned_into_targets_bytes":0}}"#
            )
        };
        let stats = format!(
            r#"{{"disk":{},"stores":[{{"path":"{}"}}]}}"#,
            disk(500),
            cache.display()
        );
        let gc = format!(
            r#"{{"skipped":{gc_skipped},"disk":{},"entries_dropped":7,"store_bytes_removed":420,"disk_bytes_reclaimed":410}}"#,
            disk(80)
        );
        let script = dir.join("kache");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ncase \"$1\" in\n  stats) echo '{stats}' ;;\n  gc) echo '{gc}' ;;\n  *) exit 2 ;;\nesac\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[test]
    fn reclaim_runs_the_collector_past_a_dead_holder() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        drop(write_lock(&cache.join("store"), dead_pid()));
        let kache = Kache {
            program: fake_kache(dir.path(), &cache, false),
            attempts: 2,
            wait: Duration::ZERO,
        };
        let report = kache.reclaim().unwrap();
        assert!(report.collected);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.before.store_bytes, 500);
        assert_eq!(report.after.store_bytes, 80);
        assert!(report.after.under_cap());
        assert_eq!(report.disk_bytes_reclaimed, 410);
    }

    #[test]
    fn reclaim_waits_for_a_live_holder_and_does_not_collect() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let held = write_lock(&cache.join("store"), std::process::id());
        held.lock().unwrap();
        let kache = Kache {
            program: fake_kache(dir.path(), &cache, false),
            attempts: 3,
            wait: Duration::ZERO,
        };
        let report = kache.reclaim().unwrap();
        assert!(!report.collected);
        assert_eq!(report.skipped, 3);
        assert!(matches!(report.lock, Lock::Held { alive: true, .. }));
    }

    #[test]
    fn reclaim_counts_a_collection_kache_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let kache = Kache {
            program: fake_kache(dir.path(), &cache, true),
            attempts: 2,
            wait: Duration::ZERO,
        };
        let report = kache.reclaim().unwrap();
        assert!(!report.collected);
        assert_eq!(report.skipped, 2);
    }
}
