//! Inbox filesystem service.

use crate::envelope::Envelope;
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const DEFAULT_INBOX_CAPACITY: usize = 1024;

/// Message inbox as a directory.
#[derive(Clone)]
pub struct InboxFs {
    queue: Arc<Mutex<VecDeque<Envelope>>>,
    deadletter: Arc<Mutex<Vec<Envelope>>>,
    capacity: usize,
}

impl InboxFs {
    /// Create an inbox with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_INBOX_CAPACITY)
    }

    /// Create an inbox with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            deadletter: Arc::new(Mutex::new(Vec::new())),
            capacity: capacity.max(1),
        }
    }

    /// Access the deadletter queue.
    pub fn deadletter(&self) -> Arc<Mutex<Vec<Envelope>>> {
        self.deadletter.clone()
    }

    /// Access the inbox queue.
    pub fn queue(&self) -> Arc<Mutex<VecDeque<Envelope>>> {
        self.queue.clone()
    }

    fn enqueue(&self, envelope: Envelope) {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= self.capacity {
            let mut dead = self.deadletter.lock().unwrap_or_else(|e| e.into_inner());
            dead.push(envelope);
        } else {
            queue.push_back(envelope);
        }
    }
}

impl Default for InboxFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for InboxFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "" | "inbox" => {
                if flags.write {
                    Ok(Box::new(InboxWriter::new(self.clone())))
                } else {
                    let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                    let items: Vec<Envelope> = queue.iter().cloned().collect();
                    let json = serde_json::to_vec_pretty(&items)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(json)))
                }
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
            "inbox" => {
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
        "inbox"
    }
}

struct InboxWriter {
    inbox: InboxFs,
    buffer: Vec<u8>,
}

impl InboxWriter {
    fn new(inbox: InboxFs) -> Self {
        Self {
            inbox,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for InboxWriter {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let envelope: Envelope =
            serde_json::from_slice(&self.buffer).map_err(|err| FsError::Other(err.to_string()))?;
        self.inbox.enqueue(envelope);
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}
