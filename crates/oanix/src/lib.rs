//! OANIX: OpenAgents Agent Operating Environment
//!
//! A Rust-native agent OS inspired by [WANIX](https://github.com/tractordev/wanix) by Jeff Lindsay and Plan 9 from Bell Labs.
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

pub mod env;
pub mod error;
pub mod namespace;
pub mod scheduler;
pub mod service;
pub mod services;

#[cfg(feature = "browser")]
pub mod web;

#[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
pub mod wasi;

#[cfg(all(feature = "net-executor", not(target_arch = "wasm32")))]
pub mod executor;

// Re-exports
pub use error::OanixError;
pub use namespace::{Mount, Namespace, NamespaceBuilder};
pub use service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};
pub use services::{
    ConnectionInfo, CowFs, FuncFs, HttpFs, HttpMethod, HttpRequest, HttpResponse, LogEvent,
    LogLevel, LogsFs, MapFs, MapFsBuilder, MemFs, RequestState, TaskFs, TaskMeta, TaskSpec,
    TaskStatus, WsConnection, WsFs, WsState,
};

// Environment exports
pub use env::{EnvBuilder, EnvStatus, EnvStatusInfo, OanixEnv};

// Scheduler exports
pub use scheduler::{JobKind, JobResult, JobSpec, JobStatus, Scheduler, SchedulerStatus};

#[cfg(not(target_arch = "wasm32"))]
pub use services::WorkspaceFs;

#[cfg(feature = "nostr")]
pub use services::{Filter, NostrFs};

#[cfg(all(feature = "wasi", not(target_arch = "wasm32")))]
pub use wasi::{RunConfig, RunResult, WasiRuntime};
