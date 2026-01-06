//! Multi-backend LM routing with swarm simulation.
//!
//! This crate provides a unified interface for routing LLM calls to multiple backends:
//! - Apple FM Bridge (local)
//! - Simulated NIP-90 swarm (for testing distributed scenarios)
//! - Mock backend (for testing)
//!
//! # Example
//!
//! ```rust,ignore
//! use lm_router::{LmRouter, LmRouterBuilder};
//!
//! let router = LmRouterBuilder::new()
//!     .add_fm_bridge("http://localhost:3030")
//!     .default_model("apple-fm")
//!     .build()?;
//!
//! let response = router.complete("apple-fm", "What is 2+2?", 100).await?;
//! println!("Answer: {}", response.text);
//! println!("Tokens: {}", response.usage.total_tokens);
//! ```

mod backend;
mod error;
mod router;
mod usage;

pub mod backends;

pub use backend::{LmBackend, LmResponse};
pub use error::{Error, Result};
pub use router::{LmRouter, LmRouterBuilder};
pub use usage::{LmUsage, UsageReport, UsageTracker};

// Re-export swarm simulation types
pub use backends::swarm_sim::{LatencyDist, SwarmSimConfig, SwarmSimulator};
