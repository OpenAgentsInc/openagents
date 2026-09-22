//! Open-ended agent episodes in a Minecraft world.
//!
//! The shape follows the Voyager paper (arXiv:2305.16291): an agent lives
//! in an open-ended environment, proposes its own next task, acts, checks
//! whether the task finished, and banks what worked as a reusable skill.
//! Each half of the loop is a module with an honest mechanical arm and a
//! model-driven arm:
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
//!   Minecraft version, seed, difficulty, gamerules, deposits, economy,
//!   guild and quest sections, a `scenario` (`quest` by default, `war`
//!   for the combat arms), a `curriculum` section, and the episode
//!   bounds — digested by file so two runs are comparable.
//! - **Curriculum.** [`curriculum::Curriculum`] proposes tasks: the
//!   manifest's declared list first, then an Open Responses door's
//!   proposals behind a warm-up schedule. A world with no section runs
//!   the built-in starter tasks.
//! - **Programs.** Tasks act through [`interpret`], a bounded Lua
//!   engine over the bridge's typed ops — the paper's code-as-action
//!   claim with the host owning the vocabulary. A task's program is its
//!   own script, a banked [`skills::Skill`], a retrieval the decision
//!   door picks, or a program the `act` door writes; a fault feeds back
//!   for repair across [`episode`]'s four-round refinement loop.
//! - **Critic.** [`critic::Spec`] checks a task: mechanical specs first,
//!   a `noul` call to the decision door ([`decide::Door`]) where a spec
//!   names one. A passing task marked `bank` lands in the digested
//!   [`skills::SkillStore`].
//! - **The record.** Every exchange and every event lands in an `atif`
//!   log inside the run directory, beside `server.log`, the ledger, and
//!   the decisions. [`evidence::render`] turns a finished run into the
//!   demo's coverage matrix, metrics, and readable chain.

pub mod bridge;
pub mod critic;
pub mod curriculum;
pub mod decide;
pub mod ensemble;
pub mod episode;
pub mod error;
pub mod evidence;
pub mod guild;
pub mod interpret;
pub mod keys;
pub mod ledger;
pub mod quest;
pub mod relay;
pub mod server;
pub mod skills;
pub mod state;
pub mod world;

pub use error::{Error, Result};
