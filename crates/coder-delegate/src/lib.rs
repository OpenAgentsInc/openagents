//! What Coder's terminal turn runs, split out of Coder One so Coder builds
//! without Microluna (issue #9889): the CLI adapters for Claude Code and
//! Codex ([`delegate`], [`adapter`]), the probe battery and Jev's judge
//! ([`judge`], [`probes`]), the briefing ([`delegate::Briefing`]), the
//! policy sections a turn reads ([`policy`]), the terminal turn itself
//! ([`terminal`]), and the records they leave ([`record`], [`usage`]).
//!
//! Coder One (`crates/coder-one`) re-exports each module under its old
//! path and adds what only its episodes run, including the deprecated
//! Microluna loop. This crate does not depend on `crates/microluna`.

pub mod action;
pub mod adapter;
pub mod agent;
pub mod briefing_jev;
pub mod briefing_knowledge;
pub mod collect;
pub mod component;
pub mod credentials;
pub mod data_profile;
pub mod deadline;
pub mod decision;
pub mod delegate;
pub mod environment;
pub mod files;
pub mod guests;
pub mod judge;
pub mod limit;
pub mod monitor;
pub mod ops;
pub mod pack;
pub mod policy;
pub mod probes;
pub mod record;
pub mod requirements;
pub mod say;
pub mod scripted;
pub mod session;
pub mod shell;
pub mod state;
pub mod steering;
pub mod stream;
pub mod system;
pub mod tail;
pub mod terminal;
pub mod transport;
pub mod usage;

pub use action::Action;
pub use agent::{Bounds, Ended, Generate, Judge, Judgments, Shell, run};
pub use state::{Environment, Issue, Observation, State, Surveyed, Turn};
