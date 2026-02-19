//! Logs filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
    StreamHandle, WatchEvent, WatchHandle,
};
use crate::types::Timestamp;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEFAULT_LOG_CAPACITY: usize = 512;

/// Trace event stored in logs.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TraceEvent {
    /// Timestamp when event was recorded.
    pub timestamp: Timestamp,
    /// Event payload as UTF-8 or hex.
    pub data: String,
}

/// Logs service with streaming trace support.
#[derive(Clone)]
pub struct LogsFs {
    traces: Arc<WatchHub>,
    recent: Arc<Mutex<VecDeque<TraceEvent>>>,
    capacity: usize,
}

impl LogsFs {
    /// Create a logs service with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_LOG_CAPACITY)
    }

    /// Create a logs service with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            traces: Arc::new(WatchHub::new()),
            recent: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity: capacity.max(1),
        }
    }

    /// Record a trace event and notify watchers.
    pub fn emit_trace(&self, data: impl AsRef<[u8]>) {
        let raw = data.as_ref();
        let payload = match std::str::from_utf8(raw) {
            Ok(text) => text.to_string(),
            Err(_) => hex::encode(raw),
        };
        let event = TraceEvent {
            timestamp: Timestamp::now(),
            data: payload.clone(),
        };

        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        if recent.len() >= self.capacity {
            recent.pop_front();
        }
        recent.push_back(event);
        drop(recent);

        self.traces
            .broadcast(WatchEvent::Data(payload.into_bytes()));
    }

    fn recent_json(&self) -> FsResult<Vec<u8>> {
        let recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&*recent).map_err(|err| FsError::Other(err.to_string()))
    }

    fn trajectory_jsonl(&self) -> FsResult<Vec<u8>> {
        let recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = String::new();
        for event in recent.iter() {
            let line =
                serde_json::to_string(event).map_err(|err| FsError::Other(err.to_string()))?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out.into_bytes())
    }

    fn subscribe(&self) -> std::sync::mpsc::Receiver<Vec<u8>> {
        self.traces.subscribe_stream()
    }

    fn subscribe_watch(&self) -> std::sync::mpsc::Receiver<WatchEvent> {
        self.traces.subscribe()
    }
}

impl Default for LogsFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for LogsFs {
    fn open(&self, path: &str, _flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "trace" => Ok(Box::new(StreamHandle::new(self.subscribe()))),
            "recent" => Ok(Box::new(BytesHandle::new(self.recent_json()?))),
            "trajectory" => Ok(Box::new(BytesHandle::new(self.trajectory_jsonl()?))),
            "" => Err(FsError::IsDirectory),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![
                DirEntry::file("trace", 0),
                DirEntry::file("recent", self.recent_json()?.len() as u64),
                DirEntry::file("trajectory", self.trajectory_jsonl()?.len() as u64),
            ]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "trace" => Ok(Stat::file(0)),
            "recent" => Ok(Stat::file(self.recent_json()?.len() as u64)),
            "trajectory" => Ok(Stat::file(self.trajectory_jsonl()?.len() as u64)),
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

    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        match path {
            "trace" => Ok(Some(Box::new(MpscWatchHandle::new(self.subscribe_watch())))),
            _ => Ok(None),
        }
    }

    fn name(&self) -> &str {
        "logs"
    }
}

struct WatchHub {
    senders: Mutex<Vec<std::sync::mpsc::Sender<WatchEvent>>>,
}

impl WatchHub {
    fn new() -> Self {
        Self {
            senders: Mutex::new(Vec::new()),
        }
    }

    fn subscribe(&self) -> std::sync::mpsc::Receiver<WatchEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut senders = self.senders.lock().unwrap_or_else(|e| e.into_inner());
        senders.push(tx);
        rx
    }

    fn subscribe_stream(&self) -> std::sync::mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut senders = self.senders.lock().unwrap_or_else(|e| e.into_inner());
        senders.push(Self::wrap_stream_sender(tx));
        rx
    }

    fn wrap_stream_sender(
        sender: std::sync::mpsc::Sender<Vec<u8>>,
    ) -> std::sync::mpsc::Sender<WatchEvent> {
        let (proxy_tx, proxy_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(event) = proxy_rx.recv() {
                if let WatchEvent::Data(data) = event {
                    if sender.send(data).is_err() {
                        break;
                    }
                }
            }
        });
        proxy_tx
    }

    fn broadcast(&self, event: WatchEvent) {
        let mut senders = self.senders.lock().unwrap_or_else(|e| e.into_inner());
        senders.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

struct MpscWatchHandle {
    receiver: std::sync::mpsc::Receiver<WatchEvent>,
}

impl MpscWatchHandle {
    fn new(receiver: std::sync::mpsc::Receiver<WatchEvent>) -> Self {
        Self { receiver }
    }
}

impl WatchHandle for MpscWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        match timeout {
            Some(duration) => match self.receiver.recv_timeout(duration) {
                Ok(event) => Ok(Some(event)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(None),
            },
            None => match self.receiver.recv() {
                Ok(event) => Ok(Some(event)),
                Err(_) => Ok(None),
            },
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}
