//! Federated Recursive Language Models (FRLM)
//!
//! FRLM extends RLM with distributed execution across a heterogeneous compute network.
//! The conductor orchestrates parallel sub-queries, aggregates results, and emits
//! trace-native execution records.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                    FRLM CONDUCTOR                           │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
//! │  │ Environment │  │  Scheduler  │  │   Budget/Policy     │ │
//! │  │ (fragments) │  │  (fanout)   │  │   (caps, verify)    │ │
//! │  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
//! │         └────────────────┼────────────────────┘            │
//! │                          │                                  │
//! │                    ┌─────▼─────┐                            │
//! │                    │   Trace   │                            │
//! │                    │  Emitter  │                            │
//! │                    └───────────┘                            │
//! └─────────────────────────┬──────────────────────────────────┘
//!                           │
//!           ┌───────────────┼───────────────┐
//!           ▼               ▼               ▼
//!     ┌──────────┐    ┌──────────┐    ┌──────────┐
//!     │  Local   │    │  Swarm   │    │ Datacenter│
//!     │ (FM/RLM) │    │ (NIP-90) │    │  (API)   │
//!     └──────────┘    └──────────┘    └──────────┘
//! ```
//!
//! # Key Components
//!
//! - [`FrlmConductor`]: Main orchestrator that manages sub-queries
//! - [`SubQueryScheduler`]: Handles async fanout and result collection
//! - [`TraceEmitter`]: Emits structured trace events for observability
//! - [`FrlmPolicy`]: Budget, timeout, and verification policies

pub mod conductor;
pub mod error;
pub mod policy;
pub mod scheduler;
pub mod trace;
pub mod types;
pub mod verification;

#[cfg(test)]
mod bench_stats;

// Re-exports
pub use conductor::FrlmConductor;
pub use error::{FrlmError, Result};
pub use policy::{FrlmPolicy, Quorum, QuorumPolicy};
pub use scheduler::SubQueryScheduler;
pub use trace::{TraceEmitter, TraceEvent};
pub use types::*;
