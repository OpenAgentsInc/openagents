use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, sync_channel, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
};

#[cfg(test)]
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// One file persisted by the async checkpoint writeback worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncCheckpointWritebackFile {
    /// Stable relative path inside the checkpoint directory.
    pub relative_path: PathBuf,
    /// Stable caller-owned artifact digest for the file payload.
    pub artifact_digest: String,
    /// Immutable bytes to persist.
    pub bytes: Vec<u8>,
}

impl AsyncCheckpointWritebackFile {
    /// Creates one checkpoint file descriptor and validates its relative path.
    pub fn new(
        relative_path: impl Into<PathBuf>,
        artifact_digest: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, AsyncCheckpointWritebackError> {
        let relative_path = relative_path.into();
        validate_relative_path(relative_path.as_path())?;
        Ok(Self {
            relative_path,
            artifact_digest: artifact_digest.into(),
            bytes,
        })
    }
}

/// Immutable payload handed off from the train loop to the writeback worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncCheckpointWritebackPayload {
    /// Stable write identifier.
    pub write_id: String,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Final checkpoint directory that should appear atomically when durable.
    pub final_directory: PathBuf,
    /// Files to persist under the checkpoint directory.
    pub files: Vec<AsyncCheckpointWritebackFile>,
    /// Stable digest over the payload metadata and file digests.
    pub payload_digest: String,
}

impl AsyncCheckpointWritebackPayload {
    /// Creates one validated immutable checkpoint payload.
    pub fn new(
        write_id: impl Into<String>,
        checkpoint_ref: impl Into<String>,
        checkpoint_family: impl Into<String>,
        final_directory: impl Into<PathBuf>,
        files: Vec<AsyncCheckpointWritebackFile>,
    ) -> Result<Self, AsyncCheckpointWritebackError> {
        let write_id = write_id.into();
        let checkpoint_ref = checkpoint_ref.into();
        let checkpoint_family = checkpoint_family.into();
        let final_directory = final_directory.into();
        if files.is_empty() {
            return Err(AsyncCheckpointWritebackError::EmptyPayload {
                write_id: write_id.clone(),
            });
        }
        let mut seen = BTreeSet::new();
        for file in &files {
            validate_relative_path(file.relative_path.as_path())?;
            if !seen.insert(file.relative_path.clone()) {
                return Err(AsyncCheckpointWritebackError::DuplicateRelativePath {
                    write_id: write_id.clone(),
                    relative_path: file.relative_path.clone(),
                });
            }
        }
        let payload_digest = stable_payload_digest(
            write_id.as_str(),
            checkpoint_ref.as_str(),
            checkpoint_family.as_str(),
            final_directory.as_path(),
            files.as_slice(),
        );
        Ok(Self {
            write_id,
            checkpoint_ref,
            checkpoint_family,
            final_directory,
            files,
            payload_digest,
        })
    }

    /// Returns the total byte count across all files.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum::<u64>()
    }
}

/// One durable async checkpoint write outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncCheckpointWritebackReceipt {
    /// Stable write identifier.
    pub write_id: String,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Final durable checkpoint directory.
    pub final_directory: PathBuf,
    /// Stable payload digest.
    pub payload_digest: String,
    /// Number of files persisted into the checkpoint directory.
    pub file_count: usize,
    /// Total persisted byte count.
    pub total_bytes: u64,
    /// Plain-language durability detail.
    pub detail: String,
}

/// Bounded worker configuration for async checkpoint writeback.
#[derive(Clone, Debug)]
pub struct AsyncCheckpointWritebackOptions {
    queue_capacity: usize,
    #[cfg(test)]
    injected_write_delay: Duration,
    #[cfg(test)]
    fail_before_finalize: bool,
}

impl AsyncCheckpointWritebackOptions {
    /// Creates one bounded worker configuration.
    pub fn bounded(queue_capacity: usize) -> Result<Self, AsyncCheckpointWritebackError> {
        if queue_capacity == 0 {
            return Err(AsyncCheckpointWritebackError::InvalidQueueCapacity);
        }
        Ok(Self {
            queue_capacity,
            #[cfg(test)]
            injected_write_delay: Duration::ZERO,
            #[cfg(test)]
            fail_before_finalize: false,
        })
    }

    /// Returns the configured bounded queue capacity.
    #[must_use]
    pub const fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    /// Returns the maximum number of writes that may be accepted at once,
    /// including the one currently executing on the worker.
    #[must_use]
    pub const fn max_in_flight_writes(&self) -> usize {
        self.queue_capacity.saturating_add(1)
    }

    #[cfg(test)]
    pub(crate) fn with_test_injected_write_delay(mut self, delay: Duration) -> Self {
        self.injected_write_delay = delay;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_test_fail_before_finalize(mut self) -> Self {
        self.fail_before_finalize = true;
        self
    }
}

/// Failure emitted by the bounded async checkpoint writeback worker.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AsyncCheckpointWritebackError {
    /// The worker was configured with an impossible queue size.
    #[error("async checkpoint writeback requires queue_capacity > 0")]
    InvalidQueueCapacity,
    /// One file path tried to escape the checkpoint directory.
    #[error("async checkpoint writeback requires a relative file path, found `{path}`")]
    InvalidRelativePath {
        /// Invalid relative path.
        path: PathBuf,
    },
    /// One payload carried no files.
    #[error("async checkpoint write `{write_id}` must contain at least one file")]
    EmptyPayload {
        /// Stable write identifier.
        write_id: String,
    },
    /// One payload listed the same relative path more than once.
    #[error("async checkpoint write `{write_id}` duplicates relative path `{relative_path}`")]
    DuplicateRelativePath {
        /// Stable write identifier.
        write_id: String,
        /// Duplicated relative path.
        relative_path: PathBuf,
    },
    /// The bounded queue refused one write because it was already full.
    #[error("async checkpoint write `{write_id}` was refused because the bounded queue is full")]
    QueueFull {
        /// Stable write identifier.
        write_id: String,
    },
    /// The worker was already shut down when the caller submitted or awaited a write.
    #[error("async checkpoint write `{write_id}` could not reach a live worker")]
    WorkerClosed {
        /// Stable write identifier.
        write_id: String,
    },
    /// The final checkpoint directory already existed.
    #[error(
        "async checkpoint write `{write_id}` cannot overwrite existing checkpoint directory `{path}`"
    )]
    FinalDirectoryAlreadyExists {
        /// Stable write identifier.
        write_id: String,
        /// Existing final checkpoint directory.
        path: PathBuf,
    },
    /// One shutdown path explicitly refused a queued write before it started.
    #[error("async checkpoint write `{write_id}` was refused during shutdown before it started")]
    ShutdownRefusedPending {
        /// Stable write identifier.
        write_id: String,
    },
    /// One staged write was interrupted before final atomic publication.
    #[error(
        "async checkpoint write `{write_id}` was interrupted before atomic finalization; `{path}` remains uncommitted"
    )]
    WriteInterruptedUncommitted {
        /// Stable write identifier.
        write_id: String,
        /// Final durable checkpoint directory that was intentionally left absent.
        path: PathBuf,
    },
    /// One write failed while materializing the payload.
    #[error("async checkpoint write `{write_id}` failed: {detail}")]
    WriteFailed {
        /// Stable write identifier.
        write_id: String,
        /// Plain-language failure detail.
        detail: String,
    },
    /// The background worker thread did not exit cleanly.
    #[error("async checkpoint worker join failed: {detail}")]
    WorkerJoin {
        /// Plain-language join detail.
        detail: String,
    },
}

/// One in-flight async checkpoint write handle.
#[derive(Debug)]
pub struct AsyncCheckpointWritebackTicket {
    write_id: String,
    receiver: Receiver<Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError>>,
}

impl AsyncCheckpointWritebackTicket {
    /// Returns the stable write identifier for this ticket.
    #[must_use]
    pub fn write_id(&self) -> &str {
        self.write_id.as_str()
    }

    /// Waits until the background worker either seals or refuses the write.
    pub fn wait(self) -> Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError> {
        self.receiver.recv().unwrap_or_else(|_| {
            Err(AsyncCheckpointWritebackError::WorkerClosed {
                write_id: self.write_id,
            })
        })
    }
}

/// Bounded background worker that persists immutable checkpoint payloads.
pub struct AsyncCheckpointWritebackWorker {
    sender: Option<SyncSender<WorkerCommand>>,
    refuse_pending: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl AsyncCheckpointWritebackWorker {
    /// Starts one bounded async checkpoint writeback worker.
    pub fn new(
        options: AsyncCheckpointWritebackOptions,
    ) -> Result<Self, AsyncCheckpointWritebackError> {
        if options.queue_capacity == 0 {
            return Err(AsyncCheckpointWritebackError::InvalidQueueCapacity);
        }
        let (sender, receiver) = sync_channel(options.queue_capacity);
        let refuse_pending = Arc::new(AtomicBool::new(false));
        let join_handle = Some(spawn_worker(
            receiver,
            refuse_pending.clone(),
            WorkerControl::from_options(&options),
        ));
        Ok(Self {
            sender: Some(sender),
            refuse_pending,
            join_handle,
        })
    }

    /// Attempts to enqueue one immutable checkpoint payload without blocking.
    pub fn submit(
        &self,
        payload: AsyncCheckpointWritebackPayload,
    ) -> Result<AsyncCheckpointWritebackTicket, AsyncCheckpointWritebackError> {
        let Some(sender) = &self.sender else {
            return Err(AsyncCheckpointWritebackError::WorkerClosed {
                write_id: payload.write_id.clone(),
            });
        };
        let (result_tx, result_rx) = mpsc::channel();
        let write_id = payload.write_id.clone();
        match sender.try_send(WorkerCommand::Write { payload, result_tx }) {
            Ok(()) => Ok(AsyncCheckpointWritebackTicket {
                write_id,
                receiver: result_rx,
            }),
            Err(TrySendError::Full(WorkerCommand::Write { payload, .. })) => {
                Err(AsyncCheckpointWritebackError::QueueFull {
                    write_id: payload.write_id,
                })
            }
            Err(TrySendError::Disconnected(WorkerCommand::Write { payload, .. })) => {
                Err(AsyncCheckpointWritebackError::WorkerClosed {
                    write_id: payload.write_id,
                })
            }
            Err(_) => Err(AsyncCheckpointWritebackError::WorkerClosed { write_id }),
        }
    }

    /// Flushes all accepted writes and joins the worker.
    pub fn shutdown_flush(
        &mut self,
    ) -> Result<Vec<AsyncCheckpointWritebackReceipt>, AsyncCheckpointWritebackError> {
        self.shutdown(false)
    }

    /// Refuses queued writes that have not started yet and joins the worker.
    pub fn shutdown_refuse_pending(
        &mut self,
    ) -> Result<Vec<AsyncCheckpointWritebackReceipt>, AsyncCheckpointWritebackError> {
        self.shutdown(true)
    }

    fn shutdown(
        &mut self,
        refuse_pending: bool,
    ) -> Result<Vec<AsyncCheckpointWritebackReceipt>, AsyncCheckpointWritebackError> {
        if refuse_pending {
            self.refuse_pending.store(true, Ordering::SeqCst);
        }
        let Some(sender) = self.sender.take() else {
            if let Some(handle) = self.join_handle.take() {
                return join_worker(handle);
            }
            return Ok(Vec::new());
        };
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        sender
            .send(WorkerCommand::Shutdown {
                receipts_tx: shutdown_tx,
            })
            .map_err(|_| AsyncCheckpointWritebackError::WorkerJoin {
                detail: String::from("worker stopped before receiving shutdown"),
            })?;
        let receipts = shutdown_rx.recv().unwrap_or_else(|_| {
            Err(AsyncCheckpointWritebackError::WorkerJoin {
                detail: String::from("worker stopped before replying to shutdown"),
            })
        })?;
        let Some(handle) = self.join_handle.take() else {
            return Ok(receipts);
        };
        let _ = join_worker(handle)?;
        Ok(receipts)
    }
}

impl Drop for AsyncCheckpointWritebackWorker {
    fn drop(&mut self) {
        let _ = self.shutdown_refuse_pending();
    }
}

/// Writes one immutable checkpoint payload synchronously with the same atomic-finalization semantics as the worker.
pub fn write_checkpoint_payload_sync(
    payload: &AsyncCheckpointWritebackPayload,
) -> Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError> {
    write_checkpoint_payload_sync_with_control(payload, &WorkerControl::default())
}

#[cfg(test)]
pub(crate) fn write_checkpoint_payload_sync_with_options(
    payload: &AsyncCheckpointWritebackPayload,
    options: &AsyncCheckpointWritebackOptions,
) -> Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError> {
    write_checkpoint_payload_sync_with_control(payload, &WorkerControl::from_options(options))
}

#[derive(Clone, Default)]
struct WorkerControl {
    #[cfg(test)]
    injected_write_delay: Duration,
    #[cfg(test)]
    fail_before_finalize: bool,
}

impl WorkerControl {
    fn from_options(options: &AsyncCheckpointWritebackOptions) -> Self {
        let _ = options;
        Self {
            #[cfg(test)]
            injected_write_delay: options.injected_write_delay,
            #[cfg(test)]
            fail_before_finalize: options.fail_before_finalize,
        }
    }
}

enum WorkerCommand {
    Write {
        payload: AsyncCheckpointWritebackPayload,
        result_tx:
            mpsc::Sender<Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError>>,
    },
    Shutdown {
        receipts_tx: mpsc::Sender<
            Result<Vec<AsyncCheckpointWritebackReceipt>, AsyncCheckpointWritebackError>,
        >,
    },
}

fn spawn_worker(
    receiver: Receiver<WorkerCommand>,
    refuse_pending: Arc<AtomicBool>,
    control: WorkerControl,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut receipts = Vec::new();
        while let Ok(command) = receiver.recv() {
            match command {
                WorkerCommand::Write { payload, result_tx } => {
                    let result = if refuse_pending.load(Ordering::SeqCst) {
                        Err(AsyncCheckpointWritebackError::ShutdownRefusedPending {
                            write_id: payload.write_id.clone(),
                        })
                    } else {
                        write_checkpoint_payload_sync_with_control(&payload, &control)
                    };
                    if let Ok(receipt) = &result {
                        receipts.push(receipt.clone());
                    }
                    let _ = result_tx.send(result);
                }
                WorkerCommand::Shutdown { receipts_tx } => {
                    let _ = receipts_tx.send(Ok(receipts));
                    break;
                }
            }
        }
    })
}

fn join_worker(
    handle: JoinHandle<()>,
) -> Result<Vec<AsyncCheckpointWritebackReceipt>, AsyncCheckpointWritebackError> {
    handle
        .join()
        .map_err(|_| AsyncCheckpointWritebackError::WorkerJoin {
            detail: String::from("worker thread panicked"),
        })?;
    Ok(Vec::new())
}

fn write_checkpoint_payload_sync_with_control(
    payload: &AsyncCheckpointWritebackPayload,
    control: &WorkerControl,
) -> Result<AsyncCheckpointWritebackReceipt, AsyncCheckpointWritebackError> {
    let _ = control;
    let final_directory = payload.final_directory.as_path();
    if final_directory.exists() {
        return Err(AsyncCheckpointWritebackError::FinalDirectoryAlreadyExists {
            write_id: payload.write_id.clone(),
            path: payload.final_directory.clone(),
        });
    }
    let parent_dir = final_directory.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent_dir).map_err(|error| {
        AsyncCheckpointWritebackError::WriteFailed {
            write_id: payload.write_id.clone(),
            detail: error.to_string(),
        }
    })?;

    let temp_directory = parent_dir.join(format!(
        ".{}.partial-{}",
        final_directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("checkpoint"),
        payload.write_id
    ));
    if temp_directory.exists() {
        std::fs::remove_dir_all(temp_directory.as_path()).map_err(|error| {
            AsyncCheckpointWritebackError::WriteFailed {
                write_id: payload.write_id.clone(),
                detail: error.to_string(),
            }
        })?;
    }
    std::fs::create_dir_all(temp_directory.as_path()).map_err(|error| {
        AsyncCheckpointWritebackError::WriteFailed {
            write_id: payload.write_id.clone(),
            detail: error.to_string(),
        }
    })?;

    let write_result = (|| -> Result<(), AsyncCheckpointWritebackError> {
        for file in &payload.files {
            let destination = temp_directory.join(&file.relative_path);
            if let Some(file_parent) = destination.parent() {
                std::fs::create_dir_all(file_parent).map_err(|error| {
                    AsyncCheckpointWritebackError::WriteFailed {
                        write_id: payload.write_id.clone(),
                        detail: error.to_string(),
                    }
                })?;
            }
            std::fs::write(destination.as_path(), file.bytes.as_slice()).map_err(|error| {
                AsyncCheckpointWritebackError::WriteFailed {
                    write_id: payload.write_id.clone(),
                    detail: error.to_string(),
                }
            })?;
        }
        #[cfg(test)]
        if !control.injected_write_delay.is_zero() {
            thread::sleep(control.injected_write_delay);
        }
        #[cfg(test)]
        if control.fail_before_finalize {
            return Err(AsyncCheckpointWritebackError::WriteInterruptedUncommitted {
                write_id: payload.write_id.clone(),
                path: payload.final_directory.clone(),
            });
        }
        std::fs::rename(temp_directory.as_path(), final_directory).map_err(|error| {
            AsyncCheckpointWritebackError::WriteFailed {
                write_id: payload.write_id.clone(),
                detail: error.to_string(),
            }
        })?;
        Ok(())
    })();

    if let Err(error) = &write_result {
        let _ = std::fs::remove_dir_all(temp_directory.as_path());
        return Err(error.clone());
    }

    Ok(AsyncCheckpointWritebackReceipt {
        write_id: payload.write_id.clone(),
        checkpoint_ref: payload.checkpoint_ref.clone(),
        checkpoint_family: payload.checkpoint_family.clone(),
        final_directory: payload.final_directory.clone(),
        payload_digest: payload.payload_digest.clone(),
        file_count: payload.files.len(),
        total_bytes: payload.total_bytes(),
        detail: String::from("checkpoint directory was staged and atomically finalized"),
    })
}

fn validate_relative_path(path: &Path) -> Result<(), AsyncCheckpointWritebackError> {
    if path.is_absolute() {
        return Err(AsyncCheckpointWritebackError::InvalidRelativePath {
            path: path.to_path_buf(),
        });
    }
    let mut has_component = false;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(_) => has_component = true,
            _ => {
                return Err(AsyncCheckpointWritebackError::InvalidRelativePath {
                    path: path.to_path_buf(),
                });
            }
        }
    }
    if !has_component {
        return Err(AsyncCheckpointWritebackError::InvalidRelativePath {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn stable_payload_digest(
    write_id: &str,
    checkpoint_ref: &str,
    checkpoint_family: &str,
    final_directory: &Path,
    files: &[AsyncCheckpointWritebackFile],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_async_checkpoint_writeback_payload|");
    hasher.update(write_id.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(final_directory.to_string_lossy().as_bytes());
    for file in files {
        hasher.update(b"|");
        hasher.update(file.relative_path.to_string_lossy().as_bytes());
        hasher.update(b"|");
        hasher.update(file.artifact_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        thread,
        time::Duration,
        time::Instant,
    };

    use tempfile::tempdir;

    use super::{
        write_checkpoint_payload_sync, AsyncCheckpointWritebackError, AsyncCheckpointWritebackFile,
        AsyncCheckpointWritebackOptions, AsyncCheckpointWritebackPayload,
        AsyncCheckpointWritebackWorker,
    };

    fn payload(
        root: &Path,
        name: &str,
    ) -> Result<AsyncCheckpointWritebackPayload, AsyncCheckpointWritebackError> {
        AsyncCheckpointWritebackPayload::new(
            format!("{name}-write"),
            format!("checkpoint://{name}"),
            String::from("train.test.async_checkpoint"),
            root.join(name),
            vec![
                AsyncCheckpointWritebackFile::new(
                    PathBuf::from("checkpoint_manifest.json"),
                    String::from("manifest-digest"),
                    br#"{"checkpoint":"manifest"}"#.to_vec(),
                )?,
                AsyncCheckpointWritebackFile::new(
                    PathBuf::from("checkpoint_model.safetensors"),
                    String::from("weights-digest"),
                    vec![1_u8, 2, 3, 4],
                )?,
            ],
        )
    }

    fn wait_for_started_write(root: &Path, name: &str, write_id: &str) {
        let started_directory = root.join(format!(".{name}.partial-{write_id}"));
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if started_directory.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!(
            "expected in-progress checkpoint directory `{}` to appear",
            started_directory.display()
        );
    }

    #[test]
    fn sync_checkpoint_writeback_publishes_atomically() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let payload = payload(directory.path(), "step-00001")?;
        let receipt = write_checkpoint_payload_sync(&payload)?;
        assert_eq!(receipt.file_count, 2);
        assert!(receipt.final_directory.exists());
        assert!(receipt
            .final_directory
            .join("checkpoint_manifest.json")
            .exists());
        assert!(receipt
            .final_directory
            .join("checkpoint_model.safetensors")
            .exists());
        Ok(())
    }

    #[test]
    fn async_checkpoint_writeback_refuses_queue_overload() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempdir()?;
        let options = AsyncCheckpointWritebackOptions::bounded(1)?
            .with_test_injected_write_delay(Duration::from_millis(100));
        let mut worker = AsyncCheckpointWritebackWorker::new(options)?;
        let first = worker.submit(payload(directory.path(), "step-00001")?)?;
        wait_for_started_write(directory.path(), "step-00001", "step-00001-write");
        let second = worker.submit(payload(directory.path(), "step-00002")?)?;
        let error = worker
            .submit(payload(directory.path(), "step-00003")?)
            .expect_err("third queued write should refuse");
        assert!(matches!(
            error,
            AsyncCheckpointWritebackError::QueueFull { .. }
        ));
        let _ = first.wait()?;
        let _ = second.wait()?;
        let _ = worker.shutdown_flush()?;
        Ok(())
    }

    #[test]
    fn async_checkpoint_writeback_shutdown_refuses_pending_writes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let options = AsyncCheckpointWritebackOptions::bounded(1)?
            .with_test_injected_write_delay(Duration::from_millis(100));
        let mut worker = AsyncCheckpointWritebackWorker::new(options)?;
        let first = worker.submit(payload(directory.path(), "step-00001")?)?;
        wait_for_started_write(directory.path(), "step-00001", "step-00001-write");
        let second = worker.submit(payload(directory.path(), "step-00002")?)?;
        let _ = worker.shutdown_refuse_pending()?;
        let first_receipt = first.wait()?;
        assert!(first_receipt.final_directory.exists());
        let second_error = second.wait().expect_err("pending write should refuse");
        assert!(matches!(
            second_error,
            AsyncCheckpointWritebackError::ShutdownRefusedPending { .. }
        ));
        assert!(!directory.path().join("step-00002").exists());
        Ok(())
    }

    #[test]
    fn async_checkpoint_writeback_interrupted_write_stays_uncommitted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let options = AsyncCheckpointWritebackOptions::bounded(1)?.with_test_fail_before_finalize();
        let mut worker = AsyncCheckpointWritebackWorker::new(options)?;
        let ticket = worker.submit(payload(directory.path(), "step-00001")?)?;
        let error = ticket
            .wait()
            .expect_err("worker should refuse interrupted write");
        assert!(matches!(
            error,
            AsyncCheckpointWritebackError::WriteInterruptedUncommitted { .. }
        ));
        assert!(!directory.path().join("step-00001").exists());
        let _ = worker.shutdown_flush()?;
        Ok(())
    }
}
