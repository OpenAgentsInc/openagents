//! What the runner keeps on disk under its state directory:
//!
//! - `service.json`: the execution ledger (`nostr::execution::Service`),
//!   written before each answer that depends on it, so a retransmitted
//!   request gets the recorded answer and never a second run;
//! - `roots/<key>.json`: each claim's NIP-RUN root, persisted before the
//!   `accepted` answer;
//! - `jobs/<key>/job.json`: each job's request, what it ran, where its
//!   results are, and what was published from them;
//! - `jobs/<key>/results/`: the results directory the engine wrote.

use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::execution::Service;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One job: a run, or a publish of a run's report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    /// The ledger's idempotency key.
    pub key: String,
    /// `run` or `publish`.
    pub action: String,
    /// The signer: the trainer.
    pub principal: String,
    /// The signed request, exactly as it arrived.
    pub request: Event,
    /// `running`, `completed`, `failed`, `cancelled`, or `unknown`.
    pub status: String,
    /// The publication a run checks, when it's a check.
    pub check: Option<String>,
    /// The publication a run externally validates on a second suite, when
    /// it's a validation. Absent in jobs written before the field.
    #[serde(default)]
    pub validates: Option<String>,
    /// The results directory, once written.
    pub results: Option<PathBuf>,
    /// The report's ArtifactRef, as the run's result names it.
    pub report: Option<Value>,
    /// The `3188` holding the report, sealed to the trainer.
    pub sealed: Option<Value>,
    /// What a publish sent: the suite's release and the `3189`.
    pub published: Option<Value>,
}

/// The state directory.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

/// Serializes ledger writes and keeps them in order.
///
/// Callers take a snapshot of the ledger and a generation number together
/// under the runner's state lock, then write outside it. Writes run one at
/// a time under this writer's lock, and a snapshot older than one already
/// on disk is skipped: the newer one already holds everything it had, so
/// the file never goes back in time.
#[derive(Debug, Default)]
pub struct LedgerWriter {
    written: std::sync::Mutex<u64>,
}

impl LedgerWriter {
    /// Writes `service`, taken at `generation`, unless a later generation
    /// is already on disk.
    ///
    /// # Errors
    ///
    /// The I/O error.
    pub fn persist(
        &self,
        store: &Store,
        generation: u64,
        service: &Service,
    ) -> std::io::Result<()> {
        self.persist_with(generation, || store.save_service(service))
    }

    /// The ordering of [`LedgerWriter::persist`] around any write.
    ///
    /// # Errors
    ///
    /// The error `write` returns.
    pub fn persist_with(
        &self,
        generation: u64,
        write: impl FnOnce() -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut written = self
            .written
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if generation <= *written {
            return Ok(());
        }
        write()?;
        *written = generation;
        Ok(())
    }
}

/// A file-safe name for an idempotency key.
#[must_use]
pub fn key_name(key: &str) -> String {
    nostr::contracts::digest_bytes(key.as_bytes())
        .trim_start_matches("sha256:")
        .chars()
        .take(32)
        .collect()
}

impl Store {
    /// The store under `dir`.
    ///
    /// # Errors
    ///
    /// When the directory can't be made private.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::create_dir_all(dir.join("jobs"))?;
        std::fs::create_dir_all(dir.join("roots"))?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// The state directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The ledger, or a new one for `worker`.
    #[must_use]
    pub fn service(&self, worker: &str, capacity: u32, horizon: u64) -> Service {
        std::fs::read(self.dir.join("service.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Service>(&bytes).ok())
            .filter(|service| service.worker == worker)
            .map(|mut service| {
                service.capacity = capacity;
                service.horizon = horizon;
                service
            })
            .unwrap_or_else(|| Service::new(worker, capacity, horizon))
    }

    /// Writes the ledger.
    ///
    /// # Errors
    ///
    /// The I/O error.
    pub fn save_service(&self, service: &Service) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(service).map_err(std::io::Error::other)?;
        crate::write_atomic(&self.dir.join("service.json"), &bytes)
    }

    /// Writes a claim's root.
    ///
    /// # Errors
    ///
    /// The I/O error.
    pub fn save_root(&self, key: &str, root: &[u8]) -> std::io::Result<()> {
        crate::write_atomic(
            &self
                .dir
                .join("roots")
                .join(format!("{}.json", key_name(key))),
            root,
        )
    }

    /// A job's directory.
    #[must_use]
    pub fn job_dir(&self, key: &str) -> PathBuf {
        self.dir.join("jobs").join(key_name(key))
    }

    /// Writes a job's record.
    ///
    /// # Errors
    ///
    /// The I/O error.
    pub fn save_job(&self, job: &Job) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(job).map_err(std::io::Error::other)?;
        crate::write_atomic(&self.job_dir(&job.key).join("job.json"), &bytes)
    }

    /// Every job on disk.
    #[must_use]
    pub fn jobs(&self) -> Vec<Job> {
        let Ok(entries) = std::fs::read_dir(self.dir.join("jobs")) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read(entry.path().join("job.json")).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect()
    }

    /// The completed run `principal` asked for whose report has `digest`.
    #[must_use]
    pub fn run_with_report(&self, principal: &str, digest: &str) -> Option<Job> {
        self.jobs().into_iter().find(|job| {
            job.action == "run"
                && job.principal == principal
                && job.status == "completed"
                && job.report.as_ref().and_then(|r| r["digest"].as_str()) == Some(digest)
        })
    }
}
