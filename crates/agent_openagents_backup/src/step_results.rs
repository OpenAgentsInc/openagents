//! Step Result Memoization
//!
//! Persists non-deterministic orchestrator step results to allow replay
//! after a crash. When the orchestrator restarts, previously recorded
//! results can be replayed instead of re-executing the step.

use crate::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Filename for step results storage
pub const STEP_RESULTS_FILENAME: &str = "step-results.json";

/// Result of a single orchestrator step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    /// Unique step identifier
    pub step_id: String,
    /// Session ID this step belongs to
    pub session_id: String,
    /// ISO timestamp when step was recorded
    pub timestamp: String,
    /// The actual result data
    pub result: Value,
    /// Optional hash of inputs (for invalidation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
}

/// Step result store persisted on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResultStore {
    /// Session ID associated with this store
    pub session_id: String,
    /// All recorded steps
    pub steps: Vec<StepResult>,
}

/// Manager for loading, replaying, and persisting step results
pub struct StepResultsManager {
    /// Session ID associated with the current store
    pub session_id: String,
    /// Whether we are replaying from a previous run
    pub replay_mode: bool,
    /// Path to the store file
    store_path: PathBuf,
    /// Cached steps by ID
    steps: HashMap<String, StepResult>,
}

impl StepResultsManager {
    /// Get the store path for a given openagents directory
    pub fn store_path(openagents_dir: &str) -> PathBuf {
        Path::new(openagents_dir).join(STEP_RESULTS_FILENAME)
    }

    /// Read an existing store from disk
    fn read_store(openagents_dir: &str) -> AgentResult<Option<StepResultStore>> {
        let store_path = Self::store_path(openagents_dir);

        if !store_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&store_path).map_err(|e| {
            AgentError::Session(format!("Failed to read step results: {}", e))
        })?;

        let store: StepResultStore = serde_json::from_str(&content).map_err(|e| {
            AgentError::Session(format!("Failed to parse step results: {}", e))
        })?;

        if store.session_id.is_empty() {
            return Ok(None);
        }

        Ok(Some(store))
    }

    /// Write store to disk
    fn write_store(&self) -> AgentResult<()> {
        let store = StepResultStore {
            session_id: self.session_id.clone(),
            steps: self.steps.values().cloned().collect(),
        };

        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AgentError::Session(format!("Failed to create step results directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(&store).map_err(|e| {
            AgentError::Session(format!("Failed to serialize step results: {}", e))
        })?;

        fs::write(&self.store_path, content).map_err(|e| {
            AgentError::Session(format!("Failed to write step results: {}", e))
        })?;

        Ok(())
    }

    /// Create a StepResultsManager for the current orchestrator run.
    /// If a previous store exists, replay_mode will be true and the existing
    /// session_id will be reused to continue the previous run.
    pub fn new(openagents_dir: &str, requested_session_id: &str) -> AgentResult<Self> {
        let existing = Self::read_store(openagents_dir)?;
        let store_path = Self::store_path(openagents_dir);

        let (session_id, steps, replay_mode) = match existing {
            Some(store) => {
                let steps: HashMap<String, StepResult> = store
                    .steps
                    .into_iter()
                    .map(|s| (s.step_id.clone(), s))
                    .collect();
                let replay_mode = !steps.is_empty();
                (store.session_id, steps, replay_mode)
            }
            None => (
                requested_session_id.to_string(),
                HashMap::new(),
                false,
            ),
        };

        Ok(Self {
            session_id,
            replay_mode,
            store_path,
            steps,
        })
    }

    /// Try to get a cached result for the given step
    pub fn get_result<T: for<'de> Deserialize<'de>>(
        &self,
        step_id: &str,
        input_hash: Option<&str>,
    ) -> Option<T> {
        let step = self.steps.get(step_id)?;

        // Check input hash matches if both are provided
        if let (Some(stored_hash), Some(requested_hash)) = (&step.input_hash, input_hash) {
            if stored_hash != requested_hash {
                return None;
            }
        }

        serde_json::from_value(step.result.clone()).ok()
    }

    /// Persist a new step result
    pub fn record_result<T: Serialize>(
        &mut self,
        step_id: &str,
        result: &T,
        input_hash: Option<&str>,
    ) -> AgentResult<()> {
        let value = serde_json::to_value(result).map_err(|e| {
            AgentError::Session(format!("Failed to serialize step result: {}", e))
        })?;

        let entry = StepResult {
            step_id: step_id.to_string(),
            session_id: self.session_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            result: value,
            input_hash: input_hash.map(|s| s.to_string()),
        };

        self.steps.insert(step_id.to_string(), entry);
        self.write_store()
    }

    /// Clear persisted step results
    pub fn clear(&mut self) -> AgentResult<()> {
        self.steps.clear();
        if self.store_path.exists() {
            fs::remove_file(&self.store_path).ok();
        }
        Ok(())
    }

    /// Check if a step result exists
    pub fn has_result(&self, step_id: &str) -> bool {
        self.steps.contains_key(step_id)
    }

    /// Get all recorded step IDs
    pub fn step_ids(&self) -> Vec<&str> {
        self.steps.keys().map(|s| s.as_str()).collect()
    }
}

/// Execute an operation with durable step semantics.
/// If a cached result exists, return it. Otherwise execute the operation
/// and record the result.
pub fn durable_step<T, F>(
    manager: &mut StepResultsManager,
    step_id: &str,
    input_hash: Option<&str>,
    operation: F,
) -> AgentResult<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
    F: FnOnce() -> AgentResult<T>,
{
    // Check for cached result
    if let Some(cached) = manager.get_result::<T>(step_id, input_hash) {
        return Ok(cached);
    }

    // Execute operation
    let result = operation()?;

    // Record result
    manager.record_result(step_id, &result, input_hash)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, String) {
        let temp = TempDir::new().unwrap();
        let path = temp.path().to_str().unwrap().to_string();
        (temp, path)
    }

    #[test]
    fn test_new_manager_no_existing() {
        let (_temp, path) = setup();
        let manager = StepResultsManager::new(&path, "session-1").unwrap();
        assert_eq!(manager.session_id, "session-1");
        assert!(!manager.replay_mode);
    }

    #[test]
    fn test_record_and_get_result() {
        let (_temp, path) = setup();
        let mut manager = StepResultsManager::new(&path, "session-1").unwrap();

        manager.record_result("step-1", &42i32, None).unwrap();

        let result: Option<i32> = manager.get_result("step-1", None);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_replay_mode() {
        let (_temp, path) = setup();

        // First manager records a result
        {
            let mut manager = StepResultsManager::new(&path, "session-1").unwrap();
            manager.record_result("step-1", &"hello", None).unwrap();
        }

        // Second manager should be in replay mode
        let manager = StepResultsManager::new(&path, "session-2").unwrap();
        assert!(manager.replay_mode);
        assert_eq!(manager.session_id, "session-1"); // Reuses original session

        let result: Option<String> = manager.get_result("step-1", None);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_input_hash_validation() {
        let (_temp, path) = setup();
        let mut manager = StepResultsManager::new(&path, "session-1").unwrap();

        manager
            .record_result("step-1", &100i32, Some("hash-abc"))
            .unwrap();

        // Same hash should return result
        let result: Option<i32> = manager.get_result("step-1", Some("hash-abc"));
        assert_eq!(result, Some(100));

        // Different hash should return None
        let result: Option<i32> = manager.get_result("step-1", Some("hash-xyz"));
        assert!(result.is_none());
    }

    #[test]
    fn test_clear() {
        let (_temp, path) = setup();
        let mut manager = StepResultsManager::new(&path, "session-1").unwrap();

        manager.record_result("step-1", &"data", None).unwrap();
        assert!(manager.has_result("step-1"));

        manager.clear().unwrap();
        assert!(!manager.has_result("step-1"));
    }

    #[test]
    fn test_durable_step() {
        let (_temp, path) = setup();
        let mut manager = StepResultsManager::new(&path, "session-1").unwrap();

        let mut call_count = 0;

        // First call executes operation
        let result = durable_step(&mut manager, "step-1", None, || {
            call_count += 1;
            Ok(42i32)
        })
        .unwrap();
        assert_eq!(result, 42);
        assert_eq!(call_count, 1);

        // Second call returns cached result
        let result = durable_step(&mut manager, "step-1", None, || {
            call_count += 1;
            Ok(99i32)
        })
        .unwrap();
        assert_eq!(result, 42); // Returns cached, not new value
        assert_eq!(call_count, 1); // Operation not called again
    }
}
