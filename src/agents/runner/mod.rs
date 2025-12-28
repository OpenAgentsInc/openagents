//! Agent Runner Module
//!
//! Executes tick cycles for sovereign agents. Each tick:
//! 1. Fetches + decrypts agent state from Nostr
//! 2. Gathers observations (mentions, DMs, zaps)
//! 3. Requests compute from providers and PAYS for it
//! 4. Parses LLM response into actions
//! 5. Executes actions (post, DM, zap, etc.)
//! 6. Encrypts + publishes updated state
//! 7. Publishes trajectory for transparency

pub mod compute;
pub mod scheduler;
pub mod state;
pub mod tick;

pub use compute::ComputeClient;
pub use scheduler::Scheduler;
pub use state::StateManager;
pub use tick::{TickExecutor, TickResult, TickTrigger};
