//! The append-only receipt file, `receipts.jsonl`.
//!
//! Each receipt is one line, written and fsynced before the response that
//! names it goes out, as before. The blocking write and fsync run on the
//! blocking pool, never on an async worker. A short lock covers only the
//! append so lines never interleave; the fsync runs outside it, so
//! concurrent writers' fsyncs overlap instead of queueing behind one lock.
//! A failed write is returned to the caller, logged as a
//! `receipt_unwritten` event, and counted for `/healthz`.

use std::fs::File;
use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The open receipt file and its failure count.
#[derive(Debug)]
pub(crate) struct ReceiptLog {
    file: File,
    append: std::sync::Mutex<()>,
    unwritten: AtomicU64,
}

impl ReceiptLog {
    pub(crate) fn new(file: File) -> Arc<Self> {
        Arc::new(Self {
            file,
            append: std::sync::Mutex::new(()),
            unwritten: AtomicU64::new(0),
        })
    }

    /// Appends `line` and makes it durable, off the async runtime.
    ///
    /// # Errors
    ///
    /// The I/O error, after it is logged and counted.
    pub(crate) async fn append(
        self: &Arc<Self>,
        line: String,
        digest: &str,
    ) -> std::io::Result<()> {
        let log = Arc::clone(self);
        let result = tokio::task::spawn_blocking(move || log.append_blocking(&line))
            .await
            .unwrap_or_else(|join| Err(std::io::Error::other(join.to_string())));
        if let Err(error) = &result {
            self.unwritten.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "{}",
                serde_json::json!({"event": "receipt_unwritten", "receipt": digest,
                                   "error": error.to_string()})
            );
        }
        result
    }

    fn append_blocking(&self, line: &str) -> std::io::Result<()> {
        let mut record = String::with_capacity(line.len() + 1);
        record.push_str(line);
        record.push('\n');
        {
            let _append = self
                .append
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (&self.file).write_all(record.as_bytes())?;
        }
        self.file.sync_all()
    }

    /// Receipts that could not be written since the process started.
    pub(crate) fn unwritten(&self) -> u64 {
        self.unwritten.load(Ordering::Relaxed)
    }

    /// A second handle on the file, for readers.
    pub(crate) fn try_clone(&self) -> std::io::Result<File> {
        self.file.try_clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(path: &std::path::Path) -> File {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_appends_land_as_whole_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("receipts.jsonl");
        let log = ReceiptLog::new(open(&path));
        let tasks: Vec<_> = (0..64)
            .map(|n| {
                let log = Arc::clone(&log);
                tokio::spawn(async move {
                    let line = format!("{{\"n\":{n},\"pad\":\"{}\"}}", "x".repeat(4096));
                    log.append(line, &n.to_string()).await.unwrap();
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let mut seen: Vec<u64> = text
            .lines()
            .map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).unwrap();
                value["n"].as_u64().unwrap()
            })
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..64).collect::<Vec<_>>());
        assert_eq!(log.unwritten(), 0);
    }

    #[tokio::test]
    async fn a_failed_write_is_returned_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("receipts.jsonl");
        std::fs::write(&path, "").unwrap();
        // A read-only handle stands in for a full or failing disk.
        let log = ReceiptLog::new(File::open(&path).unwrap());
        assert!(log.append("{}".into(), "sha256:fixture").await.is_err());
        assert_eq!(log.unwritten(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }
}
