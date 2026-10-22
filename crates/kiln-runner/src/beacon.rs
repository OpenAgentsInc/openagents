//! The runner's beacon (#12457): a NIP-PYLON `30200` record in the runner
//! lane ([`nostr::pylon::RUNNER_MARKER`]), so Kiln pages find runners the
//! way they find Pylons, and choose among many.
//!
//! The body is a v1 beacon with one service, `<key>:kiln/cargo-test` in the
//! conversation lane. Runs are free (`free-v1`). `meta` says what the page
//! needs to choose: the repository, the crates, and who is let in.

use nostr::pylon::{
    BEACON_V, Beacon, Class, Family, Lane, RUNNER_SERVICE, Service, Slots, Status, Tier,
};
use serde_json::json;

/// How long a beacon stands, in seconds (NIP-PYLON allows 300).
pub(crate) const VALID_SECS: u64 = 180;
/// How often the runner says so again, in seconds.
pub(crate) const EVERY_SECS: u64 = 60;

/// What the beacon describes.
pub(crate) struct About {
    pub provider: String,
    pub slug: String,
    pub label: String,
    pub since: u64,
    pub repository: String,
    pub crates: Vec<String>,
    /// `listed` or `open`.
    pub admission: &'static str,
    pub anonymous_jobs: u32,
    pub member_jobs: u32,
}

/// The listed memory size at or under this computer's, in GB.
pub(crate) fn memory_gb() -> u32 {
    let bytes = physical_memory().unwrap_or(8 << 30);
    let gb = u32::try_from(bytes >> 30).unwrap_or(u32::MAX);
    [512, 256, 128, 64, 32, 16, 8]
        .into_iter()
        .find(|size| *size <= gb)
        .unwrap_or(8)
}

fn physical_memory() -> Option<u64> {
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        return String::from_utf8_lossy(&out.stdout).trim().parse().ok();
    }
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text
        .lines()
        .find(|l| l.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

/// The beacon at `now`, with a free slot unless `busy`.
pub(crate) fn beacon(about: &About, now: u64, busy: bool, memory_gb: u32) -> Beacon {
    Beacon {
        v: BEACON_V.into(),
        requires: Vec::new(),
        meta: Some(json!({
            "repository": about.repository,
            "crates": about.crates,
            "admission": about.admission,
            "anonymous_jobs": about.anonymous_jobs,
            "member_jobs": about.member_jobs,
        })),
        provider: about.provider.clone(),
        pylon: about.slug.clone(),
        label: about.label.chars().take(64).collect(),
        status: Status::Online,
        generation: about.since,
        since: about.since,
        observed_at: now,
        valid_until: now + VALID_SECS,
        class: Class {
            family: Family::Cpu,
            tier: match memory_gb {
                0..=16 => Tier::Small,
                17..=32 => Tier::Medium,
                33..=64 => Tier::Large,
                _ => Tier::Xl,
            },
            memory_gb,
        },
        slots: Slots {
            total: 1,
            free: u32::from(!busy),
        },
        services: vec![Service {
            capability: format!("{}:{RUNNER_SERVICE}", about.provider),
            model: "cargo".into(),
            lanes: vec![Lane::CjConversation],
            offering: Some("cargo test and cargo check, offline, in a sandbox".into()),
            price_hint_msat: None,
        }],
        settlement: vec!["free-v1".into()],
        pools: Vec::new(),
        group: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_beacon_is_a_valid_runner_beacon() {
        let key = kiln::cj::Key::from_bytes([7; 32]).unwrap();
        let about = About {
            provider: key.pubkey().to_owned(),
            slug: "kiln-runner".into(),
            label: "Kiln runner".into(),
            since: 1_791_400_000,
            repository: "30617:ab:openagents".into(),
            crates: vec!["memory-stream".into()],
            admission: "listed",
            anonymous_jobs: 5,
            member_jobs: 30,
        };
        let body = beacon(&about, 1_791_400_100, true, memory_gb());
        assert_eq!(body.slots.free, 0);
        let event = nostr::pylon::runner_beacon_event(key.signer(), &body).unwrap();
        let read = nostr::pylon::parse_runner_beacon(&event).unwrap();
        assert_eq!(read.meta.unwrap()["crates"][0], "memory-stream");
        assert!(nostr::pylon::parse_beacon(&event).is_err());
    }
}
