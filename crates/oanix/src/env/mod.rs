//! OANIX Environment - Complete agent execution environment
//!
//! OanixEnv wraps a Namespace with lifecycle management, status tracking,
//! and optional WASI runtime support.
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::{OanixEnv, EnvBuilder, TaskFs, LogsFs, MemFs};
//!
//! let env = EnvBuilder::new()
//!     .mount("/task", TaskFs::new(spec, meta))
//!     .mount("/logs", LogsFs::new())
//!     .mount("/tmp", MemFs::new())
//!     .build()?;
//!
//! // Check status
//! println!("Env ID: {}", env.id());
//! println!("Status: {:?}", env.status());
//!
//! // Run WASI module (requires `wasi` feature)
//! #[cfg(feature = "wasi")]
//! let result = env.run_wasi(&wasm_bytes, RunConfig::default())?;
//! ```

mod status;

pub use status::{EnvStatus, EnvStatusInfo};

use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::error::OanixError;
use crate::namespace::{Namespace, NamespaceBuilder};
use crate::service::FileService;

#[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
use crate::wasi::{RunConfig, RunResult, WasiRuntime};

/// Complete agent execution environment
///
/// OanixEnv provides:
/// - Unique environment ID
/// - Namespace with mounted services
/// - Status tracking (Created, Running, Completed, Failed)
/// - Optional WASI runtime for executing WebAssembly
///
/// # Lifecycle
///
/// ```text
/// Created → Running → Completed
///              ↘ Failed
/// ```
pub struct OanixEnv {
    /// Unique environment identifier
    id: Uuid,
    /// The namespace providing filesystem services
    namespace: Namespace,
    /// Current status
    status: Arc<RwLock<EnvStatus>>,
    /// Creation timestamp
    created_at: u64,
    /// Optional WASI runtime (native only)
    #[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
    wasi_runtime: Option<WasiRuntime>,
}

impl OanixEnv {
    /// Create a new environment from a namespace
    pub fn new(namespace: Namespace) -> Self {
        Self {
            id: Uuid::new_v4(),
            namespace,
            status: Arc::new(RwLock::new(EnvStatus::Created)),
            created_at: now(),
            #[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
            wasi_runtime: None,
        }
    }

    /// Create with a specific ID
    pub fn with_id(id: Uuid, namespace: Namespace) -> Self {
        Self {
            id,
            namespace,
            status: Arc::new(RwLock::new(EnvStatus::Created)),
            created_at: now(),
            #[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
            wasi_runtime: None,
        }
    }

    /// Get the environment ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get a reference to the namespace
    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    /// Get the current status
    pub fn status(&self) -> EnvStatus {
        self.status.read().unwrap().clone()
    }

    /// Get detailed status info
    pub fn status_info(&self) -> EnvStatusInfo {
        let status = self.status.read().unwrap().clone();
        EnvStatusInfo {
            id: self.id,
            status,
            created_at: self.created_at,
            mount_count: self.namespace.mounts().len(),
        }
    }

    /// Set status to Running
    pub fn set_running(&self) {
        let mut status = self.status.write().unwrap();
        if matches!(*status, EnvStatus::Created) {
            *status = EnvStatus::Running {
                started_at: now(),
            };
        }
    }

    /// Set status to Completed
    pub fn set_completed(&self, exit_code: i32) {
        let mut status = self.status.write().unwrap();
        if matches!(*status, EnvStatus::Running { .. }) {
            *status = EnvStatus::Completed {
                finished_at: now(),
                exit_code,
            };
        }
    }

    /// Set status to Failed
    pub fn set_failed(&self, error: impl Into<String>) {
        let mut status = self.status.write().unwrap();
        *status = EnvStatus::Failed {
            finished_at: now(),
            error: error.into(),
        };
    }

    /// Check if environment is finished (completed or failed)
    pub fn is_finished(&self) -> bool {
        let status = self.status.read().unwrap();
        matches!(*status, EnvStatus::Completed { .. } | EnvStatus::Failed { .. })
    }

    /// Resolve a path within the namespace
    pub fn resolve<'a>(&'a self, path: &'a str) -> Option<(&'a dyn FileService, &'a str)> {
        self.namespace.resolve(path)
    }

    /// Run a WASI module in this environment (requires `wasi` feature)
    #[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
    pub fn run_wasi(
        &mut self,
        wasm_bytes: &[u8],
        config: RunConfig,
    ) -> Result<RunResult, OanixError> {
        // Initialize runtime if needed
        if self.wasi_runtime.is_none() {
            self.wasi_runtime = Some(WasiRuntime::new()?);
        }

        // Set status to running
        self.set_running();

        // Execute
        let runtime = self.wasi_runtime.as_ref().unwrap();
        let result = runtime.run(&self.namespace, wasm_bytes, config);

        // Update status based on result
        match &result {
            Ok(run_result) => {
                if run_result.exit_code == 0 {
                    self.set_completed(0);
                } else {
                    self.set_completed(run_result.exit_code);
                }
            }
            Err(e) => {
                self.set_failed(e.to_string());
            }
        }

        result
    }
}

/// Builder for creating OanixEnv instances
///
/// # Example
///
/// ```rust,ignore
/// let env = EnvBuilder::new()
///     .id(my_uuid)
///     .mount("/task", task_fs)
///     .mount("/logs", logs_fs)
///     .mount("/tmp", MemFs::new())
///     .build()?;
/// ```
pub struct EnvBuilder {
    id: Option<Uuid>,
    namespace_builder: NamespaceBuilder,
}

impl EnvBuilder {
    /// Create a new environment builder
    pub fn new() -> Self {
        Self {
            id: None,
            namespace_builder: Namespace::builder(),
        }
    }

    /// Set a specific environment ID
    pub fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    /// Mount a service at a path
    pub fn mount<S: FileService + 'static>(mut self, path: &str, service: S) -> Self {
        self.namespace_builder = self.namespace_builder.mount(path, service);
        self
    }

    /// Build the environment
    pub fn build(self) -> Result<OanixEnv, OanixError> {
        let namespace = self.namespace_builder.build();

        let env = match self.id {
            Some(id) => OanixEnv::with_id(id, namespace),
            None => OanixEnv::new(namespace),
        };

        Ok(env)
    }
}

impl Default for EnvBuilder {
    fn default() -> Self {
        Self::new()
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
    use crate::services::{MemFs, TaskFs, TaskMeta, TaskSpec};

    #[test]
    fn test_env_creation() {
        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        assert_eq!(env.status(), EnvStatus::Created);
        assert!(!env.is_finished());
    }

    #[test]
    fn test_env_with_id() {
        let id = Uuid::new_v4();
        let env = EnvBuilder::new()
            .id(id)
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        assert_eq!(env.id(), id);
    }

    #[test]
    fn test_env_lifecycle() {
        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        // Initial state
        assert_eq!(env.status(), EnvStatus::Created);

        // Set running
        env.set_running();
        match env.status() {
            EnvStatus::Running { started_at } => assert!(started_at > 0),
            _ => panic!("Expected Running status"),
        }

        // Set completed
        env.set_completed(0);
        match env.status() {
            EnvStatus::Completed { exit_code, .. } => assert_eq!(exit_code, 0),
            _ => panic!("Expected Completed status"),
        }

        assert!(env.is_finished());
    }

    #[test]
    fn test_env_failure() {
        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        env.set_running();
        env.set_failed("Something went wrong");

        match env.status() {
            EnvStatus::Failed { error, .. } => {
                assert!(error.contains("went wrong"));
            }
            _ => panic!("Expected Failed status"),
        }

        assert!(env.is_finished());
    }

    #[test]
    fn test_env_resolve() {
        let env = EnvBuilder::new()
            .mount("/task", TaskFs::new(
                TaskSpec {
                    id: "test-001".into(),
                    task_type: "test".into(),
                    description: "Test task".into(),
                    input: serde_json::json!({}),
                },
                TaskMeta::default(),
            ))
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        // Can resolve paths
        assert!(env.resolve("/task/spec.json").is_some());
        assert!(env.resolve("/tmp").is_some());
        assert!(env.resolve("/nonexistent").is_none());
    }

    #[test]
    fn test_env_status_info() {
        let id = Uuid::new_v4();
        let env = EnvBuilder::new()
            .id(id)
            .mount("/task", MemFs::new())
            .mount("/logs", MemFs::new())
            .build()
            .unwrap();

        let info = env.status_info();
        assert_eq!(info.id, id);
        assert_eq!(info.mount_count, 2);
        assert!(info.created_at > 0);
    }
}
