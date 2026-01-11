//! Agent backends for agentic compute jobs (Bazaar)
//!
//! Agent backends differ from inference backends in that they handle
//! complex, multi-step tasks that require:
//! - Repository checkout and file access
//! - Tool execution (shell commands, file edits)
//! - Multi-turn reasoning
//! - Sandboxed execution environments
//!
//! Examples: Codex Code, SWE-agent, Aider, Devin

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};

use crate::domain::{
    CodeReviewRequest, CodeReviewResult, PatchGenRequest, PatchGenResult,
    RepoIndexRequest, RepoIndexResult, SandboxRunRequest, SandboxRunResult,
};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("Execution failed: {0}")]
    ExecutionError(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Sandbox error: {0}")]
    SandboxError(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u32),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Job cancelled: {0}")]
    Cancelled(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Backend unavailable: {0}")]
    Unavailable(String),

    #[error("Model not supported: {0}")]
    ModelNotSupported(String),

    #[error("IO error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for AgentError {
    fn from(err: std::io::Error) -> Self {
        AgentError::IoError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;

// ============================================================================
// Progress Types
// ============================================================================

/// Progress update during job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobProgress {
    /// Job started
    Started {
        job_id: String,
        estimated_duration_secs: Option<u32>,
    },
    /// Repository cloning
    CloningRepo {
        repo: String,
        progress_pct: u8,
    },
    /// Agent is thinking/reasoning
    Thinking {
        message: String,
    },
    /// Agent is using a tool
    ToolUse {
        tool: String,
        input_preview: String,
    },
    /// Tool completed with result
    ToolResult {
        tool: String,
        success: bool,
        output_preview: Option<String>,
    },
    /// Partial result available
    PartialResult {
        content_preview: String,
        progress_pct: u8,
    },
    /// Running verification (tests, lint, etc.)
    Verifying {
        check: String,
    },
    /// Job completed successfully
    Completed {
        duration_ms: u64,
    },
    /// Job failed
    Failed {
        error: String,
    },
}

impl JobProgress {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobProgress::Completed { .. } | JobProgress::Failed { .. })
    }
}

// ============================================================================
// Capability Types
// ============================================================================

/// Capabilities advertised by an agent backend
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    /// Supports PatchGen jobs (kind 5932)
    pub patch_gen: bool,
    /// Supports CodeReview jobs (kind 5933)
    pub code_review: bool,
    /// Supports SandboxRun jobs (kind 5930)
    pub sandbox_run: bool,
    /// Supports RepoIndex jobs (kind 5931)
    pub repo_index: bool,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Supported models (e.g., ["codex-sonnet-4", "codex-opus-4"])
    pub supported_models: Vec<String>,
    /// Isolation mode (e.g., "container", "local", "gvisor")
    pub isolation_mode: String,
    /// Maximum time limit per job in seconds
    pub max_time_limit_secs: u32,
}

impl AgentCapabilities {
    /// Create capabilities for a Codex Code backend
    pub fn codex_code() -> Self {
        Self {
            patch_gen: true,
            code_review: true,
            sandbox_run: true,
            repo_index: false, // Typically done by different backend
            max_concurrent_jobs: 3,
            supported_models: vec![
                "codex-sonnet-4".to_string(),
                "codex-opus-4".to_string(),
            ],
            isolation_mode: "container".to_string(),
            max_time_limit_secs: 1800, // 30 minutes
        }
    }

    /// Check if a specific job kind is supported
    pub fn supports_kind(&self, kind: u16) -> bool {
        match kind {
            5930 => self.sandbox_run,
            5931 => self.repo_index,
            5932 => self.patch_gen,
            5933 => self.code_review,
            _ => false,
        }
    }

    /// Get list of supported job kinds
    pub fn supported_kinds(&self) -> Vec<u16> {
        let mut kinds = Vec::new();
        if self.sandbox_run {
            kinds.push(5930);
        }
        if self.repo_index {
            kinds.push(5931);
        }
        if self.patch_gen {
            kinds.push(5932);
        }
        if self.code_review {
            kinds.push(5933);
        }
        kinds
    }
}

// ============================================================================
// Agent Backend Trait
// ============================================================================

/// Core trait for agent backends that handle complex, multi-step tasks
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Backend identifier (e.g., "codex_code", "swe_agent")
    fn id(&self) -> &str;

    /// Check if the backend is ready to accept jobs
    async fn is_ready(&self) -> bool;

    /// Get backend capabilities
    fn capabilities(&self) -> AgentCapabilities;

    /// Execute a PatchGen job (kind 5932)
    async fn patch_gen(
        &self,
        request: PatchGenRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<PatchGenResult>;

    /// Execute a CodeReview job (kind 5933)
    async fn code_review(
        &self,
        request: CodeReviewRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<CodeReviewResult>;

    /// Execute a SandboxRun job (kind 5930)
    async fn sandbox_run(
        &self,
        request: SandboxRunRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<SandboxRunResult>;

    /// Execute a RepoIndex job (kind 5931)
    async fn repo_index(
        &self,
        request: RepoIndexRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<RepoIndexResult>;

    /// Cancel a running job
    async fn cancel(&self, job_id: &str) -> Result<()>;

    /// Initialize the backend (optional setup)
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Shutdown the backend (optional cleanup)
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// Get current number of running jobs
    fn active_jobs(&self) -> u32 {
        0
    }
}

// ============================================================================
// Agent Registry
// ============================================================================

/// Registry of available agent backends
pub struct AgentRegistry {
    backends: HashMap<String, Arc<RwLock<dyn AgentBackend>>>,
    default_backend: Option<String>,
}

impl AgentRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: None,
        }
    }

    /// Register an agent backend
    pub fn register(&mut self, id: &str, backend: Arc<RwLock<dyn AgentBackend>>) {
        if self.default_backend.is_none() {
            self.default_backend = Some(id.to_string());
        }
        self.backends.insert(id.to_string(), backend);
    }

    /// Get a backend by ID
    pub fn get(&self, id: &str) -> Option<Arc<RwLock<dyn AgentBackend>>> {
        self.backends.get(id).cloned()
    }

    /// Get the default backend
    pub fn default_backend(&self) -> Option<Arc<RwLock<dyn AgentBackend>>> {
        self.default_backend
            .as_ref()
            .and_then(|id| self.backends.get(id).cloned())
    }

    /// Get the default backend ID
    pub fn default_id(&self) -> Option<&str> {
        self.default_backend.as_deref()
    }

    /// Set the default backend
    pub fn set_default(&mut self, id: &str) -> bool {
        if self.backends.contains_key(id) {
            self.default_backend = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// List all available backend IDs
    pub fn available_backends(&self) -> Vec<&str> {
        self.backends.keys().map(|s| s.as_str()).collect()
    }

    /// Check if any backends are available
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }

    /// Find a backend that supports the given job kind
    pub async fn find_for_kind(&self, kind: u16) -> Option<Arc<RwLock<dyn AgentBackend>>> {
        for backend in self.backends.values() {
            let b = backend.read().await;
            if b.capabilities().supports_kind(kind) && b.is_ready().await {
                return Some(backend.clone());
            }
        }
        None
    }

    /// Get aggregated capabilities from all backends
    pub async fn aggregated_capabilities(&self) -> AgentCapabilities {
        let mut caps = AgentCapabilities::default();

        for backend in self.backends.values() {
            let b = backend.read().await;
            let bc = b.capabilities();
            caps.patch_gen |= bc.patch_gen;
            caps.code_review |= bc.code_review;
            caps.sandbox_run |= bc.sandbox_run;
            caps.repo_index |= bc.repo_index;
            caps.max_concurrent_jobs += bc.max_concurrent_jobs;
            for model in &bc.supported_models {
                if !caps.supported_models.contains(model) {
                    caps.supported_models.push(model.clone());
                }
            }
        }

        caps
    }

    /// Get status of all backends
    pub async fn status(&self) -> Vec<AgentBackendStatus> {
        let mut statuses = Vec::new();

        for (id, backend) in &self.backends {
            let b = backend.read().await;
            statuses.push(AgentBackendStatus {
                id: id.clone(),
                ready: b.is_ready().await,
                capabilities: b.capabilities(),
                active_jobs: b.active_jobs(),
            });
        }

        statuses
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an agent backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBackendStatus {
    pub id: String,
    pub ready: bool,
    pub capabilities: AgentCapabilities,
    pub active_jobs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_capabilities_codex_code() {
        let caps = AgentCapabilities::codex_code();
        assert!(caps.patch_gen);
        assert!(caps.code_review);
        assert!(caps.sandbox_run);
        assert!(!caps.repo_index);
        assert!(caps.supported_models.contains(&"codex-sonnet-4".to_string()));
    }

    #[test]
    fn test_agent_capabilities_supports_kind() {
        let caps = AgentCapabilities::codex_code();
        assert!(caps.supports_kind(5932)); // PatchGen
        assert!(caps.supports_kind(5933)); // CodeReview
        assert!(caps.supports_kind(5930)); // SandboxRun
        assert!(!caps.supports_kind(5931)); // RepoIndex - not supported
        assert!(!caps.supports_kind(5050)); // Text generation - not agent job
    }

    #[test]
    fn test_agent_capabilities_supported_kinds() {
        let caps = AgentCapabilities::codex_code();
        let kinds = caps.supported_kinds();
        assert!(kinds.contains(&5930));
        assert!(kinds.contains(&5932));
        assert!(kinds.contains(&5933));
        assert!(!kinds.contains(&5931));
    }

    #[test]
    fn test_job_progress_is_terminal() {
        assert!(!JobProgress::Started {
            job_id: "1".to_string(),
            estimated_duration_secs: None,
        }
        .is_terminal());

        assert!(!JobProgress::Thinking {
            message: "test".to_string(),
        }
        .is_terminal());

        assert!(JobProgress::Completed { duration_ms: 1000 }.is_terminal());

        assert!(JobProgress::Failed {
            error: "test".to_string(),
        }
        .is_terminal());
    }

    #[test]
    fn test_agent_registry() {
        let registry = AgentRegistry::new();
        assert!(!registry.has_backends());
        assert!(registry.default_backend().is_none());
    }
}
