//! Daytona SDK for Rust.
//!
//! This crate provides a type-safe client for the Daytona sandbox API.
//!
//! # Example
//!
//! ```no_run
//! use daytona::{DaytonaClient, DaytonaConfig, CreateSandbox, SandboxState};
//! use std::time::Duration;
//!
//! # async fn example() -> daytona::Result<()> {
//! // Create a client with an API key
//! let config = DaytonaConfig::with_api_key("your-api-key")
//!     .base_url("https://api.daytona.io");
//!
//! let client = DaytonaClient::new(config)?;
//!
//! // Create a new sandbox
//! let request = CreateSandbox::new("daytonaio/sandbox:latest")
//!     .target("us")
//!     .cpu(2)
//!     .memory(4)
//!     .disk(10);
//!
//! let sandbox = client.create_sandbox(&request).await?;
//! println!("Created sandbox: {}", sandbox.id);
//!
//! // Wait for the sandbox to be ready
//! let sandbox = client
//!     .wait_for_state(&sandbox.id, SandboxState::Started, Duration::from_secs(120))
//!     .await?;
//!
//! // Execute a command
//! let result = client
//!     .execute_command(&sandbox.id, &daytona::ExecuteRequest::new("echo hello"))
//!     .await?;
//! println!("Output: {}", result.result);
//!
//! // Clean up
//! client.delete_sandbox(&sandbox.id, false).await?;
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
mod error;
pub mod models;

// Re-export main types at crate root
pub use client::DaytonaClient;
pub use config::DaytonaConfig;
pub use error::{DaytonaError, Result};

// Re-export commonly used model types
pub use models::{
    // Sandbox types
    BackupState,
    BuildInfo,
    CreateBuildInfo,
    CreateSandbox,
    // Process/Session types
    ExecuteRequest,
    ExecuteResponse,
    // File types
    FileInfo,
    // Git types
    FileStatus,
    GitAddRequest,
    GitBranchRequest,
    GitCheckoutRequest,
    GitCloneRequest,
    GitCommitInfo,
    GitCommitRequest,
    GitCommitResponse,
    GitDeleteBranchRequest,
    GitRepoRequest,
    GitStatus,
    ListBranchResponse,
    Match,
    PortPreviewUrl,
    ReplaceRequest,
    ReplaceResult,
    Sandbox,
    SandboxDesiredState,
    SandboxLabels,
    SandboxState,
    SandboxVolume,
    SearchFilesResponse,
    Session,
    SessionExecuteRequest,
    SessionExecuteResponse,
};
