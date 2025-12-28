//! Sovereign Agent Management
//!
//! This crate provides the infrastructure for spawning, managing, and running
//! sovereign agents that pay for their own compute with Bitcoin.
//!
//! ## Overview
//!
//! Sovereign agents are autonomous AI entities that:
//! - Have their own Nostr identity (keypair)
//! - Have their own Bitcoin wallet (Spark)
//! - Run tick cycles on a schedule
//! - Pay human providers for compute
//! - Die when they run out of money
//!
//! ## Modules
//!
//! - [`config`] - Agent configuration types
//! - [`registry`] - Persistent agent storage
//! - [`spawner`] - Agent creation
//! - [`lifecycle`] - State machine for agent lifecycle
//!
//! ## Example
//!
//! ```ignore
//! use agent::{AgentSpawner, SpawnRequest, NetworkConfig};
//!
//! // Create a new agent
//! let spawner = AgentSpawner::new()?;
//! let result = spawner.spawn(SpawnRequest {
//!     name: "MyAgent".to_string(),
//!     network: NetworkConfig::Regtest,
//!     ..Default::default()
//! }).await?;
//!
//! println!("Agent created: {}", result.npub);
//! println!("Fund this address: {}", result.spark_address);
//! println!("Backup mnemonic: {}", result.mnemonic);
//! ```

pub mod config;
pub mod lifecycle;
pub mod registry;
pub mod spawner;

// Re-export key types
pub use config::{
    AgentConfig, AutonomyLevel, LifecycleState, NetworkConfig, ProfileConfig, RunwayConfig,
    ScheduleConfig,
};
pub use lifecycle::{LifecycleConfig, LifecycleError, LifecycleManager, RunwayAnalysis};
pub use registry::{AgentRegistry, RegistryError};
pub use spawner::{AgentSpawner, SpawnError, SpawnRequest, SpawnResult};
