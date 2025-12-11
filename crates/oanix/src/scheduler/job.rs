//! Job specification and status types

use uuid::Uuid;

/// Job specification
#[derive(Debug, Clone)]
pub struct JobSpec {
    /// Unique job identifier
    pub id: Uuid,
    /// Environment to run in
    pub env_id: Uuid,
    /// Type of job
    pub kind: JobKind,
    /// Priority (higher = more important, default = 0)
    pub priority: i32,
    /// Current status
    pub status: JobStatus,
    /// Environment variables
    pub env_vars: Vec<(String, String)>,
    /// Working directory within namespace
    pub working_dir: Option<String>,
    /// Created timestamp
    pub created_at: u64,
    /// Optional timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Optional tags for filtering/organization
    pub tags: Vec<String>,
}

impl JobSpec {
    /// Create a new job specification
    pub fn new(env_id: Uuid, kind: JobKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            env_id,
            kind,
            priority: 0,
            status: JobStatus::Pending,
            env_vars: Vec::new(),
            working_dir: None,
            created_at: now(),
            timeout_secs: None,
            tags: Vec::new(),
        }
    }

    /// Create with a specific job ID
    pub fn with_id(id: Uuid, env_id: Uuid, kind: JobKind) -> Self {
        Self {
            id,
            env_id,
            kind,
            priority: 0,
            status: JobStatus::Pending,
            env_vars: Vec::new(),
            working_dir: None,
            created_at: now(),
            timeout_secs: None,
            tags: Vec::new(),
        }
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set environment variables
    pub fn with_env(mut self, env_vars: Vec<(String, String)>) -> Self {
        self.env_vars = env_vars;
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = Some(timeout_secs);
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Type of job to execute
#[derive(Debug, Clone)]
pub enum JobKind {
    /// Execute a WASI WebAssembly module
    Wasi {
        /// WebAssembly bytecode
        wasm_bytes: Vec<u8>,
        /// Command-line arguments
        args: Vec<String>,
    },

    /// Execute a script (future: shell, Python, etc.)
    Script {
        /// Script content
        script: String,
    },

    /// Custom job type for extensibility
    Custom {
        /// Job type name
        name: String,
        /// Arbitrary data
        data: serde_json::Value,
    },
}

impl JobKind {
    /// Create a WASI job from bytes
    pub fn wasi(wasm_bytes: Vec<u8>) -> Self {
        JobKind::Wasi {
            wasm_bytes,
            args: Vec::new(),
        }
    }

    /// Create a WASI job with arguments
    pub fn wasi_with_args(wasm_bytes: Vec<u8>, args: Vec<String>) -> Self {
        JobKind::Wasi { wasm_bytes, args }
    }

    /// Create a script job
    pub fn script(script: impl Into<String>) -> Self {
        JobKind::Script {
            script: script.into(),
        }
    }

    /// Create a custom job
    pub fn custom(name: impl Into<String>, data: serde_json::Value) -> Self {
        JobKind::Custom {
            name: name.into(),
            data,
        }
    }

    /// Get the job kind as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::Wasi { .. } => "wasi",
            JobKind::Script { .. } => "script",
            JobKind::Custom { .. } => "custom",
        }
    }
}

/// Job execution status
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is waiting to be executed
    Pending,

    /// Job is currently running
    Running {
        /// When execution started
        started_at: u64,
    },

    /// Job completed successfully
    Completed {
        /// When execution finished
        finished_at: u64,
        /// Exit code (0 = success)
        exit_code: i32,
    },

    /// Job execution failed
    Failed {
        /// When execution failed
        finished_at: u64,
        /// Error message
        error: String,
    },

    /// Job was cancelled
    Cancelled {
        /// When job was cancelled
        cancelled_at: u64,
    },
}

impl JobStatus {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed { .. } | JobStatus::Failed { .. } | JobStatus::Cancelled { .. }
        )
    }

    /// Get the status as a simple string
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running { .. } => "running",
            JobStatus::Completed { .. } => "completed",
            JobStatus::Failed { .. } => "failed",
            JobStatus::Cancelled { .. } => "cancelled",
        }
    }
}

impl Default for JobStatus {
    fn default() -> Self {
        JobStatus::Pending
    }
}

/// Get current Unix timestamp
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_spec_creation() {
        let env_id = Uuid::new_v4();
        let job = JobSpec::new(env_id, JobKind::script("echo hello"));

        assert_eq!(job.env_id, env_id);
        assert_eq!(job.priority, 0);
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.created_at > 0);
    }

    #[test]
    fn test_job_builder_pattern() {
        let env_id = Uuid::new_v4();
        let job = JobSpec::new(env_id, JobKind::script("test"))
            .with_priority(10)
            .env("KEY1", "value1")
            .env("KEY2", "value2")
            .with_working_dir("/workspace")
            .with_timeout(300)
            .tag("urgent")
            .tag("test");

        assert_eq!(job.priority, 10);
        assert_eq!(job.env_vars.len(), 2);
        assert_eq!(job.working_dir, Some("/workspace".to_string()));
        assert_eq!(job.timeout_secs, Some(300));
        assert_eq!(job.tags, vec!["urgent", "test"]);
    }

    #[test]
    fn test_job_kind_wasi() {
        let kind = JobKind::wasi(vec![0, 1, 2, 3]);
        assert_eq!(kind.as_str(), "wasi");

        let kind = JobKind::wasi_with_args(vec![0, 1, 2, 3], vec!["arg1".into(), "arg2".into()]);
        match kind {
            JobKind::Wasi { args, .. } => {
                assert_eq!(args, vec!["arg1", "arg2"]);
            }
            _ => panic!("Expected Wasi"),
        }
    }

    #[test]
    fn test_job_kind_script() {
        let kind = JobKind::script("echo hello world");
        match kind {
            JobKind::Script { script } => assert_eq!(script, "echo hello world"),
            _ => panic!("Expected Script"),
        }
    }

    #[test]
    fn test_job_kind_custom() {
        let kind = JobKind::custom("my-job-type", serde_json::json!({"key": "value"}));
        match kind {
            JobKind::Custom { name, data } => {
                assert_eq!(name, "my-job-type");
                assert_eq!(data["key"], "value");
            }
            _ => panic!("Expected Custom"),
        }
    }

    #[test]
    fn test_job_status_terminal() {
        assert!(!JobStatus::Pending.is_terminal());
        assert!(!JobStatus::Running { started_at: 0 }.is_terminal());
        assert!(JobStatus::Completed { finished_at: 0, exit_code: 0 }.is_terminal());
        assert!(JobStatus::Failed { finished_at: 0, error: "".into() }.is_terminal());
        assert!(JobStatus::Cancelled { cancelled_at: 0 }.is_terminal());
    }

    #[test]
    fn test_job_status_serialization() {
        let status = JobStatus::Running { started_at: 12345 };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"running\""));

        let status = JobStatus::Completed {
            finished_at: 99999,
            exit_code: 42,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"exit_code\":42"));
    }
}
