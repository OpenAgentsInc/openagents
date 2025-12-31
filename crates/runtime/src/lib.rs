//! OpenAgents Runtime
//!
//! A pluggable runtime for autonomous AI agents that works across cloud and local deployments.
//!
//! # Overview
//!
//! The runtime provides the execution environment for agents, handling:
//! - **Lifecycle** — Create, wake, hibernate, terminate agents
//! - **Storage** — Persistent state across ticks
//! - **Identity** — Cryptographic keypairs for every agent
//! - **Communication** — Message passing between agents
//! - **Resources** — Budget enforcement and limits
//!
//! # Core Concepts
//!
//! ## Tick Model
//!
//! Agents execute in discrete **ticks**:
//! 1. Wake (from hibernate or cold start)
//! 2. Load state from storage
//! 3. Receive trigger (message, alarm, event)
//! 4. Execute logic (may involve LLM, tools)
//! 5. Update state
//! 6. Schedule next wake or hibernate
//!
//! ## Drivers
//!
//! External events (HTTP, WebSocket, Nostr, etc.) are translated into
//! **envelopes** by drivers. Agents see one mailbox regardless of source.
//!
//! ## Backends
//!
//! The same agent code runs on any backend:
//! - Cloudflare Workers (Durable Objects)
//! - Local device (SQLite + tokio)
//! - Server (containers, Kubernetes)
//!
//! ## Agent Filesystem
//!
//! Inspired by Plan 9, every agent exposes a virtual filesystem:
//! ```text
//! /agents/<id>/
//! ├── status      # agent state
//! ├── inbox/      # incoming messages
//! ├── outbox/     # emitted events
//! ├── goals/      # active goals
//! ├── memory/     # conversations, patterns
//! ├── identity/   # pubkey, signing
//! └── wallet/     # balance, payments
//! ```
//!
//! # Design Documents
//!
//! See the `docs/` folder for design considerations:
//! - `DESIGN.md` — Core architecture and principles
//! - `TRAITS.md` — Trait definitions and interfaces
//! - `BACKENDS.md` — Backend implementations
//! - `AGENT-SPECIFIC.md` — What makes this agent-specific
//! - `DRIVERS.md` — Event drivers (HTTP, WS, Nostr)
//! - `CONTROL-PLANE.md` — Management API
//! - `PLAN9.md` — Plan 9 inspirations (filesystem, namespaces)
//! - `FILESYSTEM.md` — FileService trait and implementations
//! - `PRIOR-ART.md` — Related work (Plan 9, WANIX, OANIX)
//!
//! # Example
//!
//! ```rust,ignore
//! use openagents_runtime::{Agent, AgentContext, Trigger, TickResult};
//!
//! pub struct MyAgent;
//!
//! impl Agent for MyAgent {
//!     type State = MyState;
//!     type Config = MyConfig;
//!
//!     fn on_trigger(
//!         &self,
//!         ctx: &mut AgentContext<Self::State>,
//!         trigger: Trigger,
//!     ) -> Result<TickResult> {
//!         match trigger {
//!             Trigger::Message(msg) => {
//!                 // Handle incoming message
//!                 ctx.state.message_count += 1;
//!                 ctx.broadcast("message_received", &msg);
//!                 Ok(TickResult::success())
//!             }
//!             Trigger::Alarm(_) => {
//!                 // Handle scheduled alarm
//!                 ctx.schedule_alarm(Duration::from_secs(60), None);
//!                 Ok(TickResult::success())
//!             }
//!             _ => Ok(TickResult::default()),
//!         }
//!     }
//! }
//! ```
//!
//! # Feature Flags
//!
//! - `cloudflare` — Cloudflare Workers/Durable Objects backend
//! - `local` — Local device backend with SQLite
//! - `full` — Enable all optional features (tracing, metrics)

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod agent;
pub mod budget;
pub mod compute;
pub mod containers;
#[cfg(not(target_arch = "wasm32"))]
pub mod control_plane;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod dvm;
#[cfg(feature = "cloudflare")]
pub mod cloudflare;
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub mod browser;
#[cfg(not(target_arch = "wasm32"))]
pub mod drivers;
pub mod env;
pub mod engine;
pub mod envelope;
pub mod error;
pub mod fx;
pub mod fs;
pub mod idempotency;
pub mod identity;
pub mod namespace;
pub mod services;
pub mod storage;
pub mod tick;
pub mod trigger;
pub mod types;
pub mod wallet;
#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_http;

pub use agent::{Agent, AgentConfig, AgentContext, AgentState};
pub use budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetState, BudgetTracker};
pub use compute::{
    ComputeChunk, ComputeError, ComputeFs, ComputeKind, ComputePolicy, ComputeProvider,
    ComputeRequest, ComputeResponse, ComputeRouter, JobState, ModelInfo, Prefer, ProviderInfo,
    ProviderLatency, ProviderPricing, ProviderStatus, TokenUsage,
};
#[cfg(not(target_arch = "wasm32"))]
pub use compute::{DvmProvider, LocalProvider};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub use compute::OpenAgentsComputeProvider;
#[cfg(feature = "cloudflare")]
pub use compute::CloudflareProvider;
#[cfg(feature = "cloudflare")]
pub use cloudflare::{set_cloudflare_agent_factory, CloudflareAgent};
pub use containers::{
    ApiAuthResponse, ApiAuthState, ArtifactInfo, AuthMethod, CommandResult, ContainerCapabilities,
    ContainerError, ContainerFs, ContainerKind, ContainerLatency, ContainerLimits, ContainerPolicy,
    ContainerPricing, ContainerProvider, ContainerProviderInfo, ContainerRequest, ContainerResponse,
    ContainerRouter, ContainerStatus, ContainerUsage, ExecState,
    NostrAuthChallenge, NostrAuthResponse, OpenAgentsApiClient, OpenAgentsAuth,
    OpenAgentsContainerProvider, OutputChunk, OutputStream, RateLimitStatus, RepoAuth, RepoConfig,
    SessionState,
};
#[cfg(not(target_arch = "wasm32"))]
pub use containers::{DvmContainerProvider, LocalContainerProvider};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub use containers::WasmOpenAgentsContainerProvider;
pub use containers::ProviderStatus as ContainerProviderStatus;
#[cfg(not(target_arch = "wasm32"))]
pub use control_plane::{ControlPlane, LocalRuntime};
#[cfg(not(target_arch = "wasm32"))]
pub use drivers::{
    Driver, DriverHandle, EnvelopeSink, NostrDriver, NostrDriverConfig, NostrPublishRequest,
    RoutedEnvelope,
};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub use browser::{BrowserRuntime, BrowserRuntimeConfig};
pub use env::AgentEnv;
pub use engine::{manual_trigger, TickEngine};
pub use envelope::Envelope;
pub use error::{AgentError, Result};
pub use fs::{AccessLevel, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat, WatchEvent, WatchHandle};
pub use idempotency::{IdempotencyJournal, JournalEntry, JournalError, MemoryJournal};
#[cfg(feature = "cloudflare")]
pub use idempotency::DoJournal;
#[cfg(feature = "local")]
pub use idempotency::SqliteJournal;
pub use identity::{InMemorySigner, PublicKey, Signature, SigningService};
#[cfg(not(target_arch = "wasm32"))]
pub use identity::NostrSigner;
pub use namespace::Namespace;
pub use fx::{FxError, FxRateProvider, FxRateSnapshot, FxSource};
#[cfg(not(target_arch = "wasm32"))]
pub use fx::FxRateCache;
pub use services::{
    ApmMetric, DeadletterFs, GoalsFs, HudFs, HudSettings, IdentityFs, InboxFs, LastPrMetric,
    LogsFs, MetricsFs, MetricsSnapshot, QueueMetric, StatusFs, StatusSnapshot, TraceEvent,
    WalletFs,
};
pub use storage::{AgentStorage, InMemoryStorage, StorageOp};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub use storage::IndexedDbStorage;
#[cfg(feature = "cloudflare")]
pub use storage::CloudflareStorage;
pub use tick::{ResourceUsage, TickResult};
pub use trigger::{AlarmTrigger, EventTrigger, InitializeTrigger, ManualTrigger, MessageTrigger, Trigger, TriggerMeta};
pub use types::{AgentId, EnvelopeId, Timestamp};
pub use wallet::{WalletError, WalletFxProvider, WalletPayment, WalletService};

#[cfg(test)]
mod tests;
