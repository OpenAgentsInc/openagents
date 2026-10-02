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
//! The **playtest log** ([`session`]) is the on-device list of structural
//! events (tab, screen, event code, time) the app keeps while **playtest
//! logging** is on: for everyone, unless the build turned it off for a
//! release. Its types hold no text, so it
//! can't record a message, prompt, key, invoice, address, or amount. It
//! leaves the device only inside a report whose preview showed it in full.
//!
//! **Give feedback** ([`feedback`]) is a report about text the tester
//! selected: the selection, where it came from, and their comment.
//!
//! **Triage** ([`triage`]) turns opened reports into GitHub issue drafts,
//! deduplicating by exact identity only, and keeps the append-only triage
//! log that records every acceptance with the tester's key.
//!
//! **Awards** ([`award`]) join an accepted contribution in the triage log
//! to the tester's public playtest report and a `playtest` quest, and are
//! signed only with the playtest referee key.
//!
//! **TestFlight feedback** ([`testflight`]) that App Store Connect holds
//! joins the same inbox as drafts and log entries, without the tester's
//! Apple identity; it backs no award.
//!
//! This crate has no network and no storage: the app and the triage tool
//! carry the events.

pub mod award;
pub mod feedback;
pub mod report;
pub mod session;
pub mod testflight;
pub mod triage;

/// The OpenAgents triage key, in hex, that reports are sealed to.
///
/// `None` until the owner creates the key and publishes its public half
/// (the workspace's `NEEDS_OWNER.md` has the steps). Until then the app
/// keeps each report on the phone as waiting and sends it once a build
/// carries the key; nothing is sent to any other key.
pub const TRIAGE_KEY: Option<&str> = None;

/// The environment variable that points a desktop build at another triage
/// key: the hex or `npub` public key an operator reads with `openagents
/// playtest inbox --triage-key`, for proving the path end to end before the
/// owner's key exists. Unset, [`TRIAGE_KEY`] decides.
pub const TRIAGE_KEY_ENV: &str = "OPENAGENTS_PLAYTEST_TRIAGE_KEY";

/// The triage key a desktop build seals to: [`TRIAGE_KEY_ENV`] when it is
/// set to a valid public key, otherwise [`TRIAGE_KEY`].
#[must_use]
pub fn triage_key(env: Option<&str>) -> Option<secp256k1::XOnlyPublicKey> {
    if let Some(value) = env.map(str::trim).filter(|v| !v.is_empty()) {
        if let Ok(key) = value.parse() {
            return Some(key);
        }
        if let Ok(bytes) = nostr::nip19::decode_npub(value) {
            return secp256k1::XOnlyPublicKey::from_byte_array(bytes).ok();
        }
        return None;
    }
    TRIAGE_KEY.and_then(|key| key.parse().ok())
}

/// The relay reports go to. It serves a gift wrap only to the
/// authenticated reader its `p` tag names.
pub const RELAY: &str = "wss://relay.openagents.com";
