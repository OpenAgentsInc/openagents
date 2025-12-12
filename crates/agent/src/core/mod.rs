//! Core agent types for the OpenAgents ecosystem.
//!
//! This module provides the foundational types for defining, discovering,
//! and executing agents across local, cloud, and swarm compute environments.
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         Agent Ecosystem                              │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐          │
//! │  │   Identity   │    │   Manifest   │    │  Economics   │          │
//! │  │  (Nostr key) │───▶│ (Declarative)│───▶│  (Bitcoin)   │          │
//! │  └──────────────┘    └──────────────┘    └──────────────┘          │
//! │         │                   │                   │                   │
//! │         ▼                   ▼                   ▼                   │
//! │  ┌─────────────────────────────────────────────────────────┐       │
//! │  │                    Agent Runtime                         │       │
//! │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │       │
//! │  │  │  Local  │  │  Cloud  │  │  Swarm  │  │  Hybrid │    │       │
//! │  │  │Executor │  │Executor │  │Executor │  │Executor │    │       │
//! │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │       │
//! │  └─────────────────────────────────────────────────────────┘       │
//! │                              │                                      │
//! │                              ▼                                      │
//! │  ┌─────────────────────────────────────────────────────────┐       │
//! │  │                    NIP-90 Protocol                       │       │
//! │  │         (Job Requests → Results → Payments)              │       │
//! │  └─────────────────────────────────────────────────────────┘       │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **AgentId**: Universal identity based on Nostr public key (npub)
//! - **AgentManifest**: Declarative definition of agent capabilities
//! - **AgentExecutor**: Runtime trait for executing jobs
//! - **AgentEconomics**: Bitcoin wallet and payment configuration
//!
//! # Example
//!
//! ```rust,ignore
//! use agent::core::*;
//!
//! // Create an agent manifest
//! let manifest = AgentManifest::builder()
//!     .name("my-coding-agent")
//!     .version("1.0.0")
//!     .description("An autonomous coding agent")
//!     .add_skill("code-generation")
//!     .add_job_kind(KIND_JOB_TEXT_GENERATION)
//!     .environment(ExecutionEnvironment::Local { namespace: None })
//!     .build()?;
//!
//! // Create runtime agent
//! let agent = Agent::new(manifest, Some(keypair))?;
//! agent.go_online(&["wss://relay.damus.io"]).await?;
//! ```

mod capabilities;
mod economics;
mod events;
mod id;
mod manifest;
mod requirements;
mod state;
mod traits;

pub use capabilities::*;
pub use economics::*;
pub use events::*;
pub use id::*;
pub use manifest::*;
pub use requirements::*;
pub use state::*;
pub use traits::*;
