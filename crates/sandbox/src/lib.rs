//! Container sandbox execution for OpenAgents
//!
//! Provides a unified interface for running commands in containers,
//! with support for Docker and macOS Container backends.
//!
//! # Example
//!
//! ```ignore
//! use sandbox::{detect_backend, ContainerConfig};
//!
//! // Auto-detect the best backend
//! let backend = detect_backend().await;
//!
//! // Configure the container
//! let config = ContainerConfig::new("ubuntu:latest", "/tmp/workspace")
//!     .workdir("/app")
//!     .memory_limit("4G")
//!     .timeout_ms(60000);
//!
//! // Run a command
//! let result = backend.run(
//!     &["bash".into(), "-c".into(), "echo hello".into()],
//!     &config,
//! ).await?;
//!
//! println!("Exit code: {}", result.exit_code);
//! println!("Output: {}", result.stdout);
//! ```

mod error;
mod config;
mod backend;
mod docker;
mod macos;
mod detect;
mod credentials;

pub use error::*;
pub use config::*;
pub use backend::*;
pub use docker::*;
pub use macos::*;
pub use detect::*;
pub use credentials::*;
