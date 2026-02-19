//! Codex executor via the app-server.

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use crate::app_server_executor::AppServerExecutor;
use crate::autopilot_loop::AcpEventSender;
use crate::{AdjutantError, Task, TaskResult};

/// Executor that uses Codex via app-server.
pub struct CodexExecutor {
    workspace_root: PathBuf,
}

impl CodexExecutor {
    /// Create a new Codex executor.
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    /// Execute a task using Codex (non-streaming).
    pub async fn execute(&self, task: &Task) -> Result<TaskResult, AdjutantError> {
        let executor = AppServerExecutor::new(&self.workspace_root);
        executor.execute(task).await
    }

    /// Execute a task using Codex with streaming output.
    pub async fn execute_streaming(
        &self,
        task: &Task,
        token_tx: mpsc::UnboundedSender<String>,
        acp_sender: Option<AcpEventSender>,
    ) -> Result<TaskResult, AdjutantError> {
        let executor = AppServerExecutor::new(&self.workspace_root);
        executor.execute_streaming(task, token_tx, acp_sender).await
    }
}
