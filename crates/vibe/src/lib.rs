//! OpenAgents Vibe: In-browser agentic development environment
//!
//! Vibe is a browser-based IDE for "vibe coding" full-stack applications.
//! Built on OANIX namespaces with:
//! - React/JS/TS frontends with HMR
//! - Rust backends compiled to WASM
//! - Zed-inspired editor compiled to WASM
//! - Agent-native workflows (scaffolding, refactoring, testing)
//!
//! # Architecture
//!
//! ```text
//! +-------------------------------------------------------------+
//! | Vibe Shell (Rust->WASM + JS bootstrap)                      |
//! |-------------------------------------------------------------|
//! |  Zed-Style IDE Core (Rust->WASM)                            |
//! |    - Editor, tabs, tree, palette                            |
//! |    - Terminal UI + ATIF viewer                              |
//! |                                                             |
//! |  Dev Runtime Layer                                          |
//! |    - Bundler & DevServer Engine (WASM)                      |
//! |    - Preview router (iframe)                                |
//! |                                                             |
//! |  OANIX Kernel (Rust->WASM)                                  |
//! |    - Namespace (/workspace, /logs, /cap/*)                  |
//! |    - WASI job runner                                        |
//! +-------------------------------------------------------------+
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use vibe::{VibeConfig, VibeSession};
//! use oanix::Namespace;
//!
//! let config = VibeConfig::fullstack_template();
//! let session = VibeSession::create(config).await?;
//!
//! // User edits files, dev runtime rebuilds, preview updates
//! session.run().await?;
//! ```

pub mod config;
pub mod error;
pub mod traits;

// Re-exports
pub use config::VibeConfig;
pub use error::VibeError;
pub use traits::{IdeFs, JobBackend, TerminalBackend};
