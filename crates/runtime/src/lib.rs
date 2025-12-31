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
//
// Core abstractions:
// pub mod agent;      // Agent trait and context
// pub mod envelope;   // Message envelope types
// pub mod trigger;    // Tick triggers (Message, Alarm, Event, etc.)
// pub mod storage;    // AgentStorage trait
// pub mod transport;  // MessageTransport trait
// pub mod identity;   // SigningService trait
// pub mod budget;     // Resource budgets and limits
// pub mod backend;    // RuntimeBackend trait
// pub mod error;      // Error types
//
// Agent-specific:
// pub mod memory;     // Structured memory (conversations, goals, patterns)
// pub mod namespace;  // Mount tables and capabilities (Plan 9 inspired)
// pub mod plumber;    // Event routing rules
//
// Drivers (in separate crate):
// - HttpDriver, WebSocketDriver, NostrDriver, SchedulerDriver
