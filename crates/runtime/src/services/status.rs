//! Status filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
};
use crate::types::AgentId;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Status snapshot exposed via /status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusSnapshot {
    /// Agent id.
    pub agent_id: AgentId,
    /// Arbitrary status payload.
    pub payload: serde_json::Value,
}

impl StatusSnapshot {
    /// Create an empty snapshot for an agent.
    pub fn empty(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            payload: serde_json::json!({}),
        }
    }
}

/// Agent status as a file service.
#[derive(Clone)]
pub struct StatusFs {
    snapshot: Arc<RwLock<StatusSnapshot>>,
}

impl StatusFs {
    /// Create a status service from a shared snapshot.
    pub fn new(snapshot: Arc<RwLock<StatusSnapshot>>) -> Self {
        Self { snapshot }
    }

    /// Replace the current snapshot.
    pub fn set_snapshot(&self, snapshot: StatusSnapshot) {
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = snapshot;
        }
    }

    fn snapshot_json(&self) -> FsResult<Vec<u8>> {
        let guard = self
            .snapshot
            .read()
            .map_err(|_| FsError::Other("status lock poisoned".into()))?;
        let json =
            serde_json::to_vec_pretty(&*guard).map_err(|err| FsError::Other(err.to_string()))?;
        Ok(json)
    }
}

impl FileService for StatusFs {
    fn open(&self, path: &str, _flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "" | "status" => Ok(Box::new(BytesHandle::new(self.snapshot_json()?))),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![DirEntry::file(
                "status",
                self.snapshot_json()?.len() as u64,
            )]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "status" => Ok(Stat::file(self.snapshot_json()?.len() as u64)),
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
        "status"
    }
}
