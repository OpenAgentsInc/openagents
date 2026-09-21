//! Open-ended agent episodes in a Minecraft world.
//!
//! The shape follows the Voyager paper (arXiv:2305.16291): an agent lives
//! in an open-ended environment, proposes its own next task, acts, checks
//! whether the task finished, and banks what worked as a reusable skill.
//! Phase 1 builds the load-bearing pieces and leaves the model-driven
//! halves stubbed behind honest, mechanical stand-ins:
//!
//! - **Environment.** A local Minecraft server is a supervised child
//!   process ([`server::Server`]): its own process group, a deadline on
//!   becoming ready, a graceful `stop` on stdin and a group kill behind
//!   it. The bot that plays is `mc-bridge`, a separate crate under
//!   `mc-bridge/` built on nightly Rust — azalea needs `portable_simd`
//!   and the workspace pins stable — speaking one JSON object per line on
//!   stdin and stdout ([`bridge::Bridge`]). The split is the
//!   `swift/lev-bridge` precedent: the product crate orchestrates, the
//!   helper is a process, never a dependency.
//! - **Worlds.** A world is a manifest in `worlds/` ([`world::World`]):
//!   Minecraft version, seed, difficulty, gamerules, and the episode
//!   bounds, digested by file so two runs are comparable.
//! - **The agent.** [`episode::run`] walks a small curriculum of tasks —
//!   survey, explore, gather — and checks each against mechanical
//!   predicates (inventory deltas, position deltas) rather than claiming
//!   success. Where the paper's critic is a model call, phase 1 asks the
//!   world.
//! - **The record.** Every bridge exchange and every event the bot
//!   reports is a step in an `atif` log under the run directory, so an
//!   episode reads back the same way a Coder session does.
//!
//! What this crate deliberately does not do yet: generate code with a
//! model, retrieve skills from an index, or ask a decision model whether
//! a task succeeded. The bridge's action vocabulary is typed and bounded
//! — the host owns what may run — and the seams for the model halves are
//! where `docs/voyager/` says they are.

pub mod bridge;
pub mod decide;
pub mod ensemble;
pub mod episode;
pub mod error;
pub mod guild;
pub mod keys;
pub mod ledger;
pub mod relay;
pub mod server;
pub mod state;
pub mod world;

pub use error::{Error, Result};
