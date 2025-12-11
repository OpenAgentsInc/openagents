//! OpenAgents Vibe: AI-native development platform
//!
//! Vibe is an AI-powered IDE for "vibe coding" full-stack applications.
//! Describe what you want, watch the agent build it, deploy with one click.
//!
//! # Features
//!
//! - **Projects**: Browse, create, and manage projects with templates
//! - **Editor**: Full IDE with file tree, code editor, preview, terminal
//! - **Database**: Visual database browser, SQL editor, schema management
//! - **Deploy**: One-click deployment, custom domains, analytics
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
//! use vibe::{VibeConfig, VibeScreen};
//! use gpui_oa::*;
//!
//! // Create a new Vibe screen in your GPUI app
//! let screen = cx.new(|cx| VibeScreen::new(cx));
//! ```

// Core configuration and traits
pub mod config;
pub mod error;
pub mod traits;

// UI types and mock data
pub mod types;

// Main screen component
pub mod screen;

// UI modules
pub mod projects;
pub mod editor;
pub mod database;
pub mod deploy;

// Re-exports
pub use config::VibeConfig;
pub use error::VibeError;
pub use traits::{IdeFs, JobBackend, TerminalBackend};
pub use screen::VibeScreen;
pub use types::*;
