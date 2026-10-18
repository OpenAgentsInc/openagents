//! openagents-proto — wire types shared by engine, UI, and RPC.
//!
//! Ported from openagents's `packages/control/src/wire.ts` + `packages/harness/src/types.ts`.
//! Context occupancy is replicated per chat; billing `Usage` remains a harness passthrough.

pub mod agent;
pub mod agent_wakes;
pub mod attachment_mentions;
pub mod change_request_assessment;
pub mod chat_mentions;
pub mod device_paths;
pub mod entities;
pub mod file_mentions;
pub mod invocation;
pub mod motion;
pub mod preview;
pub mod projectless;
/// OpenAgents: saved-agent chat identity (#11966).
pub mod saved_agent;
pub mod sidebar_pins;
pub mod view;
pub mod voice;
pub mod workspace;

pub use agent::*;
pub use entities::*;
pub use preview::*;
pub use sidebar_pins::*;
pub use workspace::*;

/// Parse "0.2.12" (tolerating a `-suffix`/`+build` tail on the last part)
/// into a comparable triple — the fleet feature-gate primitive (device rows
/// stamp `Device::version` at boot). `None` for anything that doesn't lead
/// with three dotted integers, and gates treat `None` as "too old".
pub fn version_triple(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().splitn(3, '.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch = parts.next()?;
    let patch: u64 = patch
        .split(['-', '+'])
        .next()
        .unwrap_or(patch)
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod rename_compatibility_tests {
    #[test]
    fn saved_conversation_links_remain_readable() {
        let file = crate::file_mentions::local_file_link("src/lib.rs", false)
            .replace("openagents-file:", "zeron-file:");
        assert_eq!(
            crate::file_mentions::file_mention_links(&file)[0].path,
            "src/lib.rs"
        );
        let chat = crate::chat_mentions::chat_mention_link("chat-1", "Existing chat")
            .unwrap()
            .replace("openagents-chat:", "zeron-chat:");
        assert_eq!(
            crate::chat_mentions::chat_mention_links(&chat)[0].chat_id,
            "chat-1"
        );
        let image = crate::attachment_mentions::attachment_mention_link(2, None)
            .replace("openagents-image:", "zeron-image:");
        let attachment = crate::attachment_mentions::attachment_mention_link(3, Some("notes.md"))
            .replace("openagents-attachment:", "zeron-attachment:");
        assert_eq!(
            crate::attachment_mentions::attachment_mentions(&image)[0].index,
            2
        );
        assert_eq!(
            crate::attachment_mentions::attachment_mentions(&attachment)[0].index,
            3
        );
    }
}
