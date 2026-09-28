//! The OpenAgents playtest program's records (`docs/game/playtesting.md`).
//!
//! A **report** ([`report`]) is what a tester files from the app: the build,
//! the tab and screen they were on, the device, and what happened, what they
//! expected, and the steps, with a kind (bug, confusing, idea, or felt
//! good). It travels privately: a NIP-17 message sealed with NIP-44 to the
//! OpenAgents triage key, signed by the tester's Verse world key
//! ([`report::wrap`]). The triage inbox opens it with the triage key
//! ([`report::open`]).
//!
//! The **session log** ([`session`]) is the opt-in, on-device list of
//! structural events (tab, screen, event code, time) the app keeps while the
//! tester has turned **Playtest session** on. Its types hold no text, so it
//! can't record a message, prompt, key, invoice, address, or amount. It
//! leaves the device only inside a report whose preview showed it in full.
//!
//! This crate has no network and no storage: the app and the triage tool
//! carry the events.

pub mod report;
pub mod session;

/// The OpenAgents triage key, in hex, that reports are sealed to.
///
/// `None` until the owner creates the key and publishes its public half
/// (the workspace's `NEEDS_OWNER.md` has the steps). Until then the app
/// keeps each report on the phone as waiting and sends it once a build
/// carries the key; nothing is sent to any other key.
pub const TRIAGE_KEY: Option<&str> = None;

/// The relay reports go to. It serves a gift wrap only to the
/// authenticated reader its `p` tag names.
pub const RELAY: &str = "wss://relay.openagents.com";
