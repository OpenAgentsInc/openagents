//! Container backend trait

use crate::{ContainerConfig, ContainerError, ContainerResult, ContainerRunResult};
use async_trait::async_trait;
use std::path::Path;

/// Options for running a command in a container
#[derive(Debug, Clone, Default)]
pub struct ContainerRunOptions {
    /// Whether to capture stdout (default: true)
    pub capture_stdout: bool,
    /// Whether to capture stderr (default: true)
    pub capture_stderr: bool,
}

impl ContainerRunOptions {
    /// Create default options (capture both streams)
    pub fn new() -> Self {
        Self {
            capture_stdout: true,
            capture_stderr: true,
        }
    }
}

/// Build options for container images
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Path to Dockerfile/Containerfile
    pub file: Option<String>,
    /// Builder memory limit
    pub memory_limit: Option<String>,
    /// Builder CPU limit
    pub cpu_limit: Option<f32>,
}

impl BuildOptions {
    /// Create new build options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the Dockerfile path
    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// Set memory limit
    pub fn memory_limit(mut self, limit: impl Into<String>) -> Self {
        self.memory_limit = Some(limit.into());
        self
    }

    /// Set CPU limit
    pub fn cpu_limit(mut self, cpus: f32) -> Self {
        self.cpu_limit = Some(cpus);
        self
    }
}

/// Container backend trait for different container runtimes
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Human-readable name for this backend
    fn name(&self) -> &'static str;

    /// Check if this backend is available on the current system
    async fn is_available(&self) -> bool;

    /// Run a command inside a container
    ///
    /// # Arguments
    /// * `command` - Command and arguments to run
    /// * `config` - Container configuration
    ///
    /// # Returns
    /// Result containing stdout, stderr, and exit code
    async fn run(
        &self,
        command: &[String],
        config: &ContainerConfig,
    ) -> ContainerResult<ContainerRunResult>;

    /// Build an image from a Dockerfile/Containerfile
    ///
    /// # Arguments
    /// * `context_dir` - Build context directory
    /// * `tag` - Tag for the built image
    /// * `options` - Optional build options
    async fn build(
        &self,
        context_dir: &Path,
        tag: &str,
        options: Option<BuildOptions>,
    ) -> ContainerResult<()>;
}

/// A no-op backend that always fails (used when no container runtime is available)
pub struct NoOpBackend;

#[async_trait]
impl ContainerBackend for NoOpBackend {
    fn name(&self) -> &'static str {
        "none"
    }

    async fn is_available(&self) -> bool {
        false
    }

    async fn run(
        &self,
        _command: &[String],
        _config: &ContainerConfig,
    ) -> ContainerResult<ContainerRunResult> {
        Err(ContainerError::not_available(
            "No container runtime available",
        ))
    }

    async fn build(
        &self,
        _context_dir: &Path,
        _tag: &str,
        _options: Option<BuildOptions>,
    ) -> ContainerResult<()> {
        Err(ContainerError::not_available(
            "No container runtime available",
        ))
    }
}
