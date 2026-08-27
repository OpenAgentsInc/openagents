//! Coder: the OpenAgents coder TUI.
//!
//! The frame, the palette, the composer wiring, the session's own commands,
//! and the ACP path are here. The runtime beneath — tools, threads, grants,
//! lanes, metering, revocation — is `openagents_cli`, used as a library so
//! there is one implementation of each and this crate cannot drift from it.

pub mod acp;
pub mod acp_harness;
pub mod agents;
pub mod commands;
pub mod credit;
pub mod export;
pub mod goal;
pub mod image;
pub mod interactive;
pub mod markdown;
pub mod osc8;
pub mod recall;
pub mod runtime;
pub mod transcript;
pub mod tui;
pub mod turn;
