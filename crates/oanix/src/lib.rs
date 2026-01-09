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
pub mod dspy_lifecycle;
pub mod dspy_pipelines;
pub mod dspy_situation;
pub mod manifest;
pub mod situation;
pub mod state;
pub mod tick;

pub use boot::{boot, boot_with_config};
pub use manifest::{
    BootConfig, ComputeManifest, DirectiveSummary, GpuDevice, HardwareManifest, IdentityManifest,
    InferenceBackend, IssueSummary, NetworkManifest, OanixManifest, RelayStatus,
    WorkspaceManifest,
};
pub use situation::{
    Connectivity, ComputePower, Environment, RecommendedAction, SituationAssessment,
};
pub use state::{ActiveTask, OanixMode, OanixState, PersistedState};
pub use tick::{
    run_tick_loop, oanix_tick, HousekeepingTask, IssueWork, JobWork, TickConfig, TickResult,
    UserAction, WorkItem,
};

// DSPy signatures for learned decision making
pub use dspy_lifecycle::{
    Complexity, IssueSelectionSignature, LifecycleDecisionSignature, LifecycleState,
    WorkPrioritizationSignature,
};
pub use dspy_situation::{PriorityAction, SituationAssessmentSignature, Urgency};

// DSPy pipeline wrappers (for tick loop and state management)
pub use dspy_pipelines::{
    IssueSelectionInput, IssueSelectionPipeline, IssueSelectionResult, SituationInput,
    SituationPipeline, SituationResult,
};
