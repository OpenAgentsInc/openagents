//! Metrics filesystem service.

use crate::fs::{BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// APM metric payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApmMetric {
    /// Actions per minute value.
    pub value: f64,
    /// Window size in seconds.
    pub window_secs: u64,
}

/// Queue metric payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueMetric {
    /// Queue depth.
    pub depth: u64,
    /// Oldest issue summary.
    pub oldest_issue: Option<String>,
}

/// Last PR metric payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LastPrMetric {
    /// URL to the PR.
    pub url: Option<String>,
    /// Title of the PR.
    pub title: Option<String>,
    /// Whether it was merged.
    pub merged: Option<bool>,
}

/// Metrics snapshot exposed via /metrics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// APM metric.
    pub apm: Option<ApmMetric>,
    /// Queue metric.
    pub queue: Option<QueueMetric>,
    /// Last PR metric.
    pub last_pr: Option<LastPrMetric>,
}

/// Metrics filesystem service.
#[derive(Clone)]
pub struct MetricsFs {
    snapshot: Arc<RwLock<MetricsSnapshot>>,
}

impl MetricsFs {
    /// Create a metrics service with an empty snapshot.
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(MetricsSnapshot::default())),
        }
    }

    /// Replace the current snapshot.
    pub fn set_snapshot(&self, snapshot: MetricsSnapshot) {
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = snapshot;
        }
    }

    fn apm_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.snapshot.read().map_err(|_| FsError::Other("metrics lock poisoned".into()))?;
        let payload = guard.apm.clone().unwrap_or(ApmMetric {
            value: 0.0,
            window_secs: 60,
        });
        serde_json::to_vec_pretty(&payload).map_err(|err| FsError::Other(err.to_string()))
    }

    fn queue_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.snapshot.read().map_err(|_| FsError::Other("metrics lock poisoned".into()))?;
        let payload = guard.queue.clone().unwrap_or(QueueMetric {
            depth: 0,
            oldest_issue: None,
        });
        serde_json::to_vec_pretty(&payload).map_err(|err| FsError::Other(err.to_string()))
    }

    fn last_pr_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.snapshot.read().map_err(|_| FsError::Other("metrics lock poisoned".into()))?;
        let payload = guard.last_pr.clone().unwrap_or(LastPrMetric {
            url: None,
            title: None,
            merged: None,
        });
        serde_json::to_vec_pretty(&payload).map_err(|err| FsError::Other(err.to_string()))
    }
}

impl Default for MetricsFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for MetricsFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "apm" => {
                if flags.write || flags.create {
                    Ok(Box::new(MetricsWriteHandle::new(
                        MetricKind::Apm,
                        self.snapshot.clone(),
                    )))
                } else {
                    Ok(Box::new(BytesHandle::new(self.apm_json()?)))
                }
            }
            "queue" => {
                if flags.write || flags.create {
                    Ok(Box::new(MetricsWriteHandle::new(
                        MetricKind::Queue,
                        self.snapshot.clone(),
                    )))
                } else {
                    Ok(Box::new(BytesHandle::new(self.queue_json()?)))
                }
            }
            "last_pr" => {
                if flags.write || flags.create {
                    Ok(Box::new(MetricsWriteHandle::new(
                        MetricKind::LastPr,
                        self.snapshot.clone(),
                    )))
                } else {
                    Ok(Box::new(BytesHandle::new(self.last_pr_json()?)))
                }
            }
            "" => Err(FsError::IsDirectory),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![
                DirEntry::file("apm", self.apm_json()?.len() as u64),
                DirEntry::file("queue", self.queue_json()?.len() as u64),
                DirEntry::file("last_pr", self.last_pr_json()?.len() as u64),
            ]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "apm" => Ok(Stat::file(self.apm_json()?.len() as u64)),
            "queue" => Ok(Stat::file(self.queue_json()?.len() as u64)),
            "last_pr" => Ok(Stat::file(self.last_pr_json()?.len() as u64)),
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
        "metrics"
    }
}

enum MetricKind {
    Apm,
    Queue,
    LastPr,
}

struct MetricsWriteHandle {
    kind: MetricKind,
    snapshot: Arc<RwLock<MetricsSnapshot>>,
    buffer: Vec<u8>,
}

impl MetricsWriteHandle {
    fn new(kind: MetricKind, snapshot: Arc<RwLock<MetricsSnapshot>>) -> Self {
        Self {
            kind,
            snapshot,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for MetricsWriteHandle {
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
        let mut guard = self
            .snapshot
            .write()
            .map_err(|_| FsError::Other("metrics lock poisoned".to_string()))?;
        match self.kind {
            MetricKind::Apm => {
                let metric: ApmMetric = serde_json::from_slice(&self.buffer)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                guard.apm = Some(metric);
            }
            MetricKind::Queue => {
                let metric: QueueMetric = serde_json::from_slice(&self.buffer)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                guard.queue = Some(metric);
            }
            MetricKind::LastPr => {
                let metric: LastPrMetric = serde_json::from_slice(&self.buffer)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                guard.last_pr = Some(metric);
            }
        }
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}
