//! Goals filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
};
use crate::storage::{AgentStorage, block_on_storage};
use crate::types::AgentId;
use std::sync::Arc;

const GOAL_PREFIX: &str = "goal:";

/// Goals service backed by agent storage.
#[derive(Clone)]
pub struct GoalsFs {
    storage: Arc<dyn AgentStorage>,
    agent_id: AgentId,
}

impl GoalsFs {
    /// Create a goals service.
    pub fn new(agent_id: AgentId, storage: Arc<dyn AgentStorage>) -> Self {
        Self { storage, agent_id }
    }

    fn storage_key(goal_id: &str) -> String {
        format!("{GOAL_PREFIX}{goal_id}")
    }

    fn list_goal_ids(&self) -> FsResult<Vec<String>> {
        let keys = block_on_storage(self.storage.list(&self.agent_id, GOAL_PREFIX))
            .map_err(|err| FsError::Other(err.to_string()))?;
        Ok(keys
            .into_iter()
            .map(|key| key.trim_start_matches(GOAL_PREFIX).to_string())
            .collect())
    }

    fn get_goal(&self, goal_id: &str) -> FsResult<Vec<u8>> {
        let key = Self::storage_key(goal_id);
        let value = block_on_storage(self.storage.get(&self.agent_id, &key))
            .map_err(|err| FsError::Other(err.to_string()))?;
        value.ok_or(FsError::NotFound)
    }

    fn set_goal(&self, goal_id: &str, value: &[u8]) -> FsResult<()> {
        let key = Self::storage_key(goal_id);
        block_on_storage(self.storage.set(&self.agent_id, &key, value))
            .map_err(|err| FsError::Other(err.to_string()))
    }

    fn delete_goal(&self, goal_id: &str) -> FsResult<()> {
        let key = Self::storage_key(goal_id);
        block_on_storage(self.storage.delete(&self.agent_id, &key))
            .map_err(|err| FsError::Other(err.to_string()))
    }
}

impl FileService for GoalsFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return Err(FsError::IsDirectory);
        }

        let goal_id = path.trim_end_matches(".json");
        if flags.write || flags.create {
            Ok(Box::new(GoalWriter::new(self.clone(), goal_id.to_string())))
        } else {
            let data = self.get_goal(goal_id)?;
            Ok(Box::new(BytesHandle::new(data)))
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => {
                let mut entries = Vec::new();
                for goal_id in self.list_goal_ids()? {
                    let size = self
                        .get_goal(&goal_id)
                        .map(|data| data.len() as u64)
                        .unwrap_or(0);
                    entries.push(DirEntry::file(format!("{goal_id}.json"), size));
                }
                Ok(entries)
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            _ => {
                let goal_id = path.trim_end_matches(".json");
                let size = self.get_goal(goal_id)?.len() as u64;
                Ok(Stat::file(size))
            }
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, path: &str) -> FsResult<()> {
        let goal_id = path.trim_end_matches(".json");
        self.delete_goal(goal_id)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, _path: &str) -> FsResult<Option<Box<dyn crate::fs::WatchHandle>>> {
        Ok(None)
    }

    fn name(&self) -> &str {
        "goals"
    }
}

struct GoalWriter {
    goals: GoalsFs,
    goal_id: String,
    buffer: Vec<u8>,
}

impl GoalWriter {
    fn new(goals: GoalsFs, goal_id: String) -> Self {
        Self {
            goals,
            goal_id,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for GoalWriter {
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
        self.goals.set_goal(&self.goal_id, &self.buffer)
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}
