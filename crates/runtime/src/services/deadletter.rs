//! Deadletter filesystem service.

use crate::envelope::Envelope;
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
};
use std::sync::{Arc, Mutex};

/// Deadletter queue view.
#[derive(Clone)]
pub struct DeadletterFs {
    queue: Arc<Mutex<Vec<Envelope>>>,
}

impl DeadletterFs {
    /// Create a deadletter service backed by the given queue.
    pub fn new(queue: Arc<Mutex<Vec<Envelope>>>) -> Self {
        Self { queue }
    }
}

impl FileService for DeadletterFs {
    fn open(&self, path: &str, _flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "" | "deadletter" => {
                let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                let json = serde_json::to_vec_pretty(&*queue)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => {
                let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                let entries = queue
                    .iter()
                    .enumerate()
                    .map(|(idx, env)| DirEntry {
                        name: format!("{}.json", idx),
                        is_dir: false,
                        size: serde_json::to_vec(env)
                            .map(|bytes| bytes.len() as u64)
                            .unwrap_or(0),
                        modified: Some(env.timestamp),
                    })
                    .collect();
                Ok(entries)
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "deadletter" => {
                let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                Ok(Stat::file(queue.len() as u64))
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, _path: &str) -> FsResult<Option<Box<dyn crate::fs::WatchHandle>>> {
        Ok(None)
    }

    fn name(&self) -> &str {
        "deadletter"
    }
}
