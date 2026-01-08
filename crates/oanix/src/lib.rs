//! OANIX - OpenAgents NIX
//!
//! The agent operating system runtime. OANIX is the OS layer that wraps the
//! OpenAgents runtime, providing environment discovery, boot sequences, and
//! autonomous decision loops.
//!
//! ## Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │  OANIX = Operating System                   │
//! │  "What am I? What should I do?"             │
//! │  ├── Boot sequence                          │
//! │  ├── Hardware discovery                     │
//! │  ├── Situation assessment                   │
//! │  └── Autonomous decision loop               │
//! ├─────────────────────────────────────────────┤
//! │  Runtime = Execution Engine                 │
//! │  "How do agents run?"                       │
//! │  ├── Tick model                             │
//! │  ├── Filesystem abstraction                 │
//! │  └── /compute, /containers, /claude         │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use oanix::boot;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let manifest = boot().await?;
//!     println!("Discovered: {} CPU cores, {} backends",
//!         manifest.hardware.cpu_cores,
//!         manifest.compute.backends.len());
//!     Ok(())
//! }
//! ```

pub mod boot;
pub mod discovery;
pub mod display;
pub mod manifest;
pub mod situation;

pub use boot::boot;
pub use manifest::{
    ComputeManifest, GpuDevice, HardwareManifest, IdentityManifest, InferenceBackend,
    NetworkManifest, OanixManifest, RelayStatus,
};
pub use situation::{Connectivity, ComputePower, Environment, RecommendedAction, SituationAssessment};
