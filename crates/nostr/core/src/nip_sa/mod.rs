//! NIP-SA: Sovereign Agents
//!
//! This module implements the Sovereign Agents protocol for autonomous agents
//! that have their own Nostr identity, can take initiative without human prompting,
//! and can hold assets and skills under their own cryptographic identity.
//!
//! ## Overview
//!
//! NIP-SA defines a protocol for autonomous agents that:
//! - **Own their identity** - Nostr keypair with threshold protection
//! - **Own assets** - Skills, data, and funds cryptographically bound to the agent
//! - **Act autonomously** - Take initiative via scheduled ticks
//! - **Participate in markets** - Buy compute, sell services, transact with other agents
//!
//! ## Event Kinds
//!
//! | Kind  | Name | Type | Description |
//! |-------|------|------|-------------|
//! | 38000 | Agent Profile | Replaceable | Agent metadata with threshold config |
//! | 38001 | Agent State | Replaceable | NIP-44 encrypted goals, memory, wallet |
//! | 38002 | Agent Schedule | Replaceable | Heartbeat interval and triggers |
//! | 38003 | Agent Goals | Replaceable | Public goals (optional transparency) |
//! | 38010 | Tick Request | Ephemeral | Runner signals tick start |
//! | 38011 | Tick Result | Ephemeral | Runner reports tick outcome |
//! | 38020 | Skill License | Addressable | Marketplace-issued license |
//! | 38021 | Skill Delivery | Ephemeral | Gift-wrapped skill content |
//! | 38030 | Trajectory Session | Addressable | Run metadata and participants |
//! | 38031 | Trajectory Event | Regular | Individual trajectory step |
//!
//! ## Modules
//!
//! - [`profile`] - Agent Profile event (kind:38000)
//! - [`state`] - Agent State event (kind:38001) with NIP-44 encryption
//! - [`schedule`] - Agent Schedule event (kind:38002)
//! - [`goals`] - Agent Goals event (kind:38003)
//! - [`tick`] - Tick Request/Result events (kinds:38010, 38011)
//! - [`trajectory`] - Trajectory Session/Event (kinds:38030, 38031)
//! - [`skill`] - Skill License/Delivery events (kinds:38020, 38021)
//! - [`wallet_integration`] - Integration with Spark wallet for balance queries
//! - [`budget`] - Budget constraints and spending enforcement
//!
//! ## Specification
//!
//! See `crates/nostr/nips/SA.md` for the complete NIP-SA specification.

pub mod budget;
pub mod goals;
pub mod profile;
pub mod schedule;
pub mod skill;
pub mod state;
pub mod tick;
pub mod trajectory;
pub mod wallet_integration;

// Re-export key types for convenience
pub use budget::*;
pub use goals::*;
pub use profile::*;
pub use schedule::*;
pub use skill::*;
pub use state::*;
pub use tick::*;
pub use trajectory::*;
pub use wallet_integration::*;
