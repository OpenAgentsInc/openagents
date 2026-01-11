//! Project configuration for OpenAgents
//!
//! Implements user stories CONF-001 through CONF-033:
//! - Basic config loading (CONF-001..005)
//! - Safety configuration (CONF-010..013)
//! - Codex Code configuration (CONF-020..024)
//! - Sandbox configuration (CONF-030..033)
//!
//! Configuration is stored in `.openagents/project.json`.
//!
//! # Example
//!
//! ```no_run
//! use config::{ProjectConfig, load_config, save_config};
//!
//! // Load config from project root
//! let config = load_config("/path/to/project").unwrap();
//! println!("Project: {}", config.project_id);
//!
//! // Create with defaults
//! let config = ProjectConfig::new("my-project");
//! save_config("/path/to/project", &config).unwrap();
//! ```

mod loader;
mod types;

pub use loader::*;
pub use types::*;
