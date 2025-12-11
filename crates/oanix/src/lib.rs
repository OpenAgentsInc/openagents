//! OANIX: OpenAgents Agent Operating Environment
//!
//! A Rust-native agent OS inspired by Plan 9 from Bell Labs and WANIX.
//!
//! OANIX provides:
//! - Plan 9-style namespaces where capabilities are granted by mounting services
//! - WASI-first execution for portable, deterministic sandboxed workloads
//! - Per-environment isolation with controlled capabilities
//!
//! # Architecture
//!
//! ```text
//! /task         - Task specification & metadata
//! /workspace    - Project files (git repo, snapshot, or ephemeral FS)
//! /logs         - Structured logs, ATIF trajectories
//! /cap/nostr    - Nostr/NIP-90 capability
//! /cap/ws       - WebSocket capability
//! /cap/payments - Lightning payment capability
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::{Namespace, OanixEnv, MemFs, TaskFs, WorkspaceFs};
//!
//! let ns = Namespace::builder()
//!     .mount("/task", TaskFs::new("regex-log"))
//!     .mount("/workspace", WorkspaceFs::new("/path/to/project"))
//!     .mount("/logs", MemFs::new())
//!     .build();
//!
//! let env = OanixEnv::new(ns, WasiRuntime::default());
//! let result = env.run_wasi(wasm_bytes, RunConfig::default()).await?;
//! ```

pub mod error;
pub mod namespace;
pub mod service;

// Re-exports
pub use error::OanixError;
pub use namespace::{Mount, Namespace, NamespaceBuilder};
pub use service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};
