pub mod agents;
pub mod background;
pub mod config;
pub mod error;
pub mod hooks;
pub mod registry;

pub use agents::builtin_agents;
pub use background::{BackgroundTask, BackgroundTaskManager, SessionId, TaskId, TaskStatus};
pub use config::{AgentConfig, AgentMode, AgentPermission, BashPermission, PermissionLevel};
pub use error::{Error, Result};
pub use hooks::{Hook, HookManager, HookResult};
pub use registry::AgentRegistry;
