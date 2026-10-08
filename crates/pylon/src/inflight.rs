//! The buyer's jobs in flight: one small file per job under
//! `<home>/inflight/`, written when [`crate::client::ask`] sends a request
//! and removed when the job ends, however it ends. Verse reads them to draw
//! the beam from the Wellspring to Alice's station while one of this
//! computer's jobs runs on a pylon. A file records the pylon's address and
//! when the job started, never the prompt.
//!
//! A buyer that dies mid-job leaves its file; a reader ignores any file
//! older than [`STALE_SECS`], the longest a job may wait.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The directory under the pylon home.
pub const DIR: &str = "inflight";
/// How long a job counts as in flight at most, s: the provider's 90-second
/// job bound and the relay's round trips.
pub const STALE_SECS: u64 = 120;
/// The most jobs a reader returns.
pub const MAX_READ: usize = 64;

/// One job in flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// The request event's ID.
    pub request: String,
    /// The serving pylon's `30200` address.
    pub pylon: String,
    /// When the request was sent, Unix seconds.
    pub started_at: u64,
}

/// A job's file, removed when this drops.
#[derive(Debug)]
pub struct Mark {
    path: Option<PathBuf>,
}

impl Mark {
    /// Records `job` under `home`. A file that can't be written leaves no
    /// mark and doesn't stop the job.
    #[must_use]
    pub fn new(home: &Path, job: &Job) -> Self {
        let dir = home.join(DIR);
        let path = dir.join(format!("{}.json", slug(&job.request)));
        let written = std::fs::create_dir_all(&dir).is_ok()
            && serde_json::to_vec(job)
                .ok()
                .is_some_and(|bytes| std::fs::write(&path, bytes).is_ok());
        Self {
            path: written.then_some(path),
        }
    }
}

impl Drop for Mark {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A file name from an event ID: hex only, at most 64 characters.
fn slug(id: &str) -> String {
    id.chars()
        .filter(char::is_ascii_hexdigit)
        .take(64)
        .collect()
}

/// The jobs in flight under `home` at `now`, Unix seconds: at most
/// [`MAX_READ`], oldest first, leaving out stale and unreadable files.
#[must_use]
pub fn read(home: &Path, now: u64) -> Vec<Job> {
    let Ok(entries) = std::fs::read_dir(home.join(DIR)) else {
        return Vec::new();
    };
    let mut jobs: Vec<Job> = entries
        .flatten()
        .take(MAX_READ * 4)
        .filter_map(|entry| {
            let bytes = std::fs::read(entry.path()).ok()?;
            if bytes.len() > 4096 {
                return None;
            }
            serde_json::from_slice::<Job>(&bytes).ok()
        })
        .filter(|job| {
            job.started_at <= now + 30 && now.saturating_sub(job.started_at) <= STALE_SECS
        })
        .collect();
    jobs.sort_by_key(|job| job.started_at);
    jobs.truncate(MAX_READ);
    jobs
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_shows_until_it_drops_and_a_stale_one_never_shows() {
        let home = tempfile::tempdir().unwrap();
        let job = Job {
            request: "ab".repeat(32),
            pylon: "30200:cd:studio-4080".into(),
            started_at: 1_000,
        };
        let mark = Mark::new(home.path(), &job);
        assert_eq!(read(home.path(), 1_010), vec![job.clone()]);
        // Past the longest a job may wait, a leftover file is ignored.
        assert!(read(home.path(), 1_000 + STALE_SECS + 1).is_empty());
        drop(mark);
        assert!(read(home.path(), 1_010).is_empty());
        // No directory reads as no jobs.
        assert!(read(&home.path().join("absent"), 1_010).is_empty());
    }
}
