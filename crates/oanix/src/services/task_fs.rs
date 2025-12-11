//! Task specification filesystem
//!
//! A composite FileService that exposes task specification, metadata,
//! live status, and results as files. Built on top of MapFs, FuncFs, and MemFs.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};
use crate::services::MemFs;

/// Task specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique task identifier
    pub id: String,
    /// Task type (e.g., "regex", "code-review", "refactor")
    pub task_type: String,
    /// Human-readable description
    pub description: String,
    /// Task-specific input data
    #[serde(default)]
    pub input: serde_json::Value,
}

/// Task metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta {
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional timeout in seconds
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Version of the task format
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_version() -> u32 {
    1
}

impl Default for TaskMeta {
    fn default() -> Self {
        TaskMeta {
            created_at: crate::services::mem_fs_now(),
            tags: Vec::new(),
            timeout_secs: None,
            version: 1,
        }
    }
}

/// Task execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is queued but not started
    Pending,
    /// Task is currently executing
    Running {
        /// When execution started (Unix epoch seconds)
        started_at: u64,
    },
    /// Task completed successfully
    Completed {
        /// When execution finished (Unix epoch seconds)
        finished_at: u64,
    },
    /// Task failed
    Failed {
        /// When execution finished (Unix epoch seconds)
        finished_at: u64,
        /// Error message
        error: String,
    },
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}

/// Task specification filesystem
///
/// Provides a file-based interface to task data:
///
/// ```text
/// /
/// ├── spec.json      # Task specification (read-only)
/// ├── meta.json      # Task metadata (read-only)
/// ├── status         # Live status (read-only, computed)
/// └── result.json    # Execution result (read-write)
/// ```
///
/// # Example
///
/// ```rust
/// use oanix::services::{TaskFs, TaskSpec, TaskMeta};
/// use oanix::service::{FileService, OpenFlags};
///
/// let spec = TaskSpec {
///     id: "task-001".to_string(),
///     task_type: "regex".to_string(),
///     description: "Extract dates from log files".to_string(),
///     input: serde_json::json!({"pattern": "date"}),
/// };
///
/// let task = TaskFs::new(spec, TaskMeta::default());
///
/// // Read spec
/// let mut handle = task.open("/spec.json", OpenFlags::read_only()).unwrap();
/// // ... read content
///
/// // Update status
/// task.set_running();
///
/// // Write result
/// let mut handle = task.open("/result.json", OpenFlags {
///     write: true,
///     create: true,
///     ..Default::default()
/// }).unwrap();
/// // ... write result
/// ```
pub struct TaskFs {
    spec_json: Vec<u8>,
    meta_json: Vec<u8>,
    status: Arc<RwLock<TaskStatus>>,
    result: MemFs,
}

impl TaskFs {
    /// Create a new TaskFs with the given specification and metadata
    pub fn new(spec: TaskSpec, meta: TaskMeta) -> Self {
        let spec_json = serde_json::to_vec_pretty(&spec).unwrap_or_default();
        let meta_json = serde_json::to_vec_pretty(&meta).unwrap_or_default();

        TaskFs {
            spec_json,
            meta_json,
            status: Arc::new(RwLock::new(TaskStatus::Pending)),
            result: MemFs::new(),
        }
    }

    /// Create a TaskFs from JSON strings (for loading from storage)
    pub fn from_json(spec_json: &str, meta_json: &str) -> Result<Self, serde_json::Error> {
        // Validate JSON by parsing
        let _spec: TaskSpec = serde_json::from_str(spec_json)?;
        let _meta: TaskMeta = serde_json::from_str(meta_json)?;

        Ok(TaskFs {
            spec_json: spec_json.as_bytes().to_vec(),
            meta_json: meta_json.as_bytes().to_vec(),
            status: Arc::new(RwLock::new(TaskStatus::Pending)),
            result: MemFs::new(),
        })
    }

    /// Get the current task status
    pub fn get_status(&self) -> TaskStatus {
        self.status.read().unwrap().clone()
    }

    /// Set the task status
    pub fn set_status(&self, status: TaskStatus) {
        *self.status.write().unwrap() = status;
    }

    /// Mark the task as running
    pub fn set_running(&self) {
        self.set_status(TaskStatus::Running {
            started_at: crate::services::mem_fs_now(),
        });
    }

    /// Mark the task as completed
    pub fn set_completed(&self) {
        self.set_status(TaskStatus::Completed {
            finished_at: crate::services::mem_fs_now(),
        });
    }

    /// Mark the task as failed
    pub fn set_failed(&self, error: impl Into<String>) {
        self.set_status(TaskStatus::Failed {
            finished_at: crate::services::mem_fs_now(),
            error: error.into(),
        });
    }

    /// Check if the task is in a terminal state (completed or failed)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.get_status(),
            TaskStatus::Completed { .. } | TaskStatus::Failed { .. }
        )
    }

    /// Get access to the result filesystem for programmatic access
    pub fn result_fs(&self) -> &MemFs {
        &self.result
    }
}

impl FileService for TaskFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "spec.json" => {
                if flags.write || flags.create || flags.truncate {
                    return Err(FsError::ReadOnly);
                }
                Ok(Box::new(StaticFileHandle::new(self.spec_json.clone())))
            }
            "meta.json" => {
                if flags.write || flags.create || flags.truncate {
                    return Err(FsError::ReadOnly);
                }
                Ok(Box::new(StaticFileHandle::new(self.meta_json.clone())))
            }
            "status" => {
                if flags.write || flags.create || flags.truncate {
                    return Err(FsError::ReadOnly);
                }
                // Compute status JSON on demand
                let status = self.status.read().unwrap();
                let content = serde_json::to_vec_pretty(&*status).unwrap_or_default();
                Ok(Box::new(StaticFileHandle::new(content)))
            }
            "result.json" => {
                // Delegate to MemFs for result
                self.result.open("/result.json", flags)
            }
            "" => Err(FsError::NotAFile("/".to_string())),
            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let path = path.trim_start_matches('/').trim_end_matches('/');

        if !path.is_empty() {
            return Err(FsError::NotFound(path.to_string()));
        }

        // Root directory
        let mut entries = vec![
            DirEntry {
                name: "spec.json".to_string(),
                is_dir: false,
                size: self.spec_json.len() as u64,
            },
            DirEntry {
                name: "meta.json".to_string(),
                is_dir: false,
                size: self.meta_json.len() as u64,
            },
            DirEntry {
                name: "status".to_string(),
                is_dir: false,
                size: 0, // Dynamic
            },
        ];

        // Add result.json if it exists
        if self.result.stat("/result.json").is_ok() {
            let meta = self.result.stat("/result.json").unwrap();
            entries.push(DirEntry {
                name: "result.json".to_string(),
                is_dir: false,
                size: meta.size,
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" => Ok(Metadata {
                is_dir: true,
                size: 4, // Number of entries
                modified: 0,
                readonly: false,
            }),
            "spec.json" => Ok(Metadata {
                is_dir: false,
                size: self.spec_json.len() as u64,
                modified: 0,
                readonly: true,
            }),
            "meta.json" => Ok(Metadata {
                is_dir: false,
                size: self.meta_json.len() as u64,
                modified: 0,
                readonly: true,
            }),
            "status" => {
                let status = self.status.read().unwrap();
                let content = serde_json::to_vec(&*status).unwrap_or_default();
                Ok(Metadata {
                    is_dir: false,
                    size: content.len() as u64,
                    modified: 0,
                    readonly: true,
                })
            }
            "result.json" => self.result.stat("/result.json"),
            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "TaskFs has fixed structure".into(),
        ))
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "TaskFs has fixed structure".into(),
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "TaskFs has fixed structure".into(),
        ))
    }
}

/// Simple read-only file handle for static content
struct StaticFileHandle {
    content: Vec<u8>,
    position: usize,
}

impl StaticFileHandle {
    fn new(content: Vec<u8>) -> Self {
        StaticFileHandle {
            content,
            position: 0,
        }
    }
}

impl FileHandle for StaticFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.position >= self.content.len() {
            return Ok(0);
        }

        let available = &self.content[self.position..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::ReadOnly)
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.position = pos as usize;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_spec() -> TaskSpec {
        TaskSpec {
            id: "test-001".to_string(),
            task_type: "regex".to_string(),
            description: "Test task".to_string(),
            input: serde_json::json!({"pattern": ".*"}),
        }
    }

    #[test]
    fn test_read_spec() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        let mut handle = task.open("/spec.json", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        let content = String::from_utf8_lossy(&buf[..n]);

        assert!(content.contains("test-001"));
        assert!(content.contains("regex"));
        assert!(content.contains("Test task"));
    }

    #[test]
    fn test_read_meta() {
        let mut meta = TaskMeta::default();
        meta.tags = vec!["test".to_string(), "example".to_string()];

        let task = TaskFs::new(test_spec(), meta);

        let mut handle = task.open("/meta.json", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        let content = String::from_utf8_lossy(&buf[..n]);

        assert!(content.contains("test"));
        assert!(content.contains("example"));
    }

    #[test]
    fn test_status_lifecycle() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        // Initial status
        assert_eq!(task.get_status(), TaskStatus::Pending);

        // Read status file
        let content = read_file(&task, "/status");
        assert!(content.contains("pending"));

        // Set running
        task.set_running();
        let status = task.get_status();
        assert!(matches!(status, TaskStatus::Running { .. }));

        let content = read_file(&task, "/status");
        assert!(content.contains("running"));
        assert!(content.contains("started_at"));

        // Set completed
        task.set_completed();
        assert!(matches!(task.get_status(), TaskStatus::Completed { .. }));

        let content = read_file(&task, "/status");
        assert!(content.contains("completed"));
    }

    #[test]
    fn test_status_failed() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        task.set_failed("Something went wrong");
        let status = task.get_status();

        match status {
            TaskStatus::Failed { error, .. } => {
                assert_eq!(error, "Something went wrong");
            }
            _ => panic!("Expected Failed status"),
        }

        let content = read_file(&task, "/status");
        assert!(content.contains("failed"));
        assert!(content.contains("Something went wrong"));
    }

    #[test]
    fn test_result_write_read() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        // Write result
        {
            let mut handle = task
                .open(
                    "/result.json",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle
                .write(br#"{"success": true, "output": "hello"}"#)
                .unwrap();
            handle.flush().unwrap();
        }

        // Read result
        let content = read_file(&task, "/result.json");
        assert!(content.contains("success"));
        assert!(content.contains("hello"));
    }

    #[test]
    fn test_spec_is_readonly() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        let result = task.open(
            "/spec.json",
            OpenFlags {
                write: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_readdir() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        let entries = task.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"spec.json"));
        assert!(names.contains(&"meta.json"));
        assert!(names.contains(&"status"));
        // result.json not yet created
        assert!(!names.contains(&"result.json"));

        // Create result
        {
            let mut h = task
                .open(
                    "/result.json",
                    OpenFlags {
                        write: true,
                        create: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            h.write(b"{}").unwrap();
            h.flush().unwrap();
        }

        let entries = task.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"result.json"));
    }

    #[test]
    fn test_stat() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        let meta = task.stat("/").unwrap();
        assert!(meta.is_dir);

        let meta = task.stat("/spec.json").unwrap();
        assert!(!meta.is_dir);
        assert!(meta.readonly);
        assert!(meta.size > 0);

        let meta = task.stat("/status").unwrap();
        assert!(!meta.is_dir);
        assert!(meta.readonly);
    }

    #[test]
    fn test_is_finished() {
        let task = TaskFs::new(test_spec(), TaskMeta::default());

        assert!(!task.is_finished());

        task.set_running();
        assert!(!task.is_finished());

        task.set_completed();
        assert!(task.is_finished());

        // Reset and test failed
        task.set_status(TaskStatus::Pending);
        task.set_failed("error");
        assert!(task.is_finished());
    }

    fn read_file(fs: &dyn FileService, path: &str) -> String {
        let mut handle = fs.open(path, OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        String::from_utf8_lossy(&buf[..n]).to_string()
    }
}
