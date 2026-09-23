//! Components: the steps of an episode with a fixed meaning.
//!
//! The evidence components' judgments, the briefing packer's parameters,
//! and the one Jev call every component makes live here, so the episode
//! and anything else that runs a component build the same requests and
//! record the same invocations.

pub mod evidence;
pub mod jev;
pub mod pack;
