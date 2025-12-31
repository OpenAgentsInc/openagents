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
//! # Design Documents
//!
//! See the `docs/` folder for design considerations:
//! - `DESIGN.md` — Core architecture and principles
//! - `TRAITS.md` — Trait definitions and interfaces
//! - `BACKENDS.md` — Backend implementations
//! - `AGENT-SPECIFIC.md` — What makes this agent-specific
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

// TODO: Implement core modules
// pub mod agent;      // Agent trait and context
// pub mod backend;    // Backend trait and implementations
// pub mod identity;   // Cryptographic identity
// pub mod memory;     // Structured agent memory
// pub mod message;    // Message types and transport
// pub mod budget;     // Resource budgets
// pub mod trigger;    // Tick triggers
// pub mod error;      // Error types
