//! The `pylon-release.v1` descriptor: one pinned document per CLI release,
//! compiled into the binary and printed by `openagents pylon version`.
//!
//! It names the `pylon/1` contract version; per lane the model repository,
//! revision, file sha256, model id, and hardware floors; per platform the
//! Psionic server's digest and build commit ([`psionic_pins`]); the
//! self-check reference set's digest and the [`Limits`] per model; and the
//! default relay and pool. [`crate::setup`] reads its model pins from here,
//! so there is one copy of each. The descriptor ships inside the signed
//! CLI: there is no separate download to trust.

pub mod psionic_pins;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::verify::Limits;

pub use psionic_pins::{PSIONIC_PINS, PsionicPin};

/// The descriptor's schema.
pub const SCHEMA: &str = "pylon-release.v1";
/// The agent contract this release speaks ([`crate::envelope`]).
pub const CONTRACT: &str = "pylon/1";
/// The pool a pylon asks to join unless told otherwise.
pub const DEFAULT_POOL: &str = "everglade";

/// The smallest machine a lane admits. A lower floor cannot serve wrong
/// answers: the self-check still decides per machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Floors {
    /// Apple Silicon unified memory, in GB (packed weights).
    pub apple_memory_gb: f64,
    /// NVIDIA GPU memory, in GB.
    pub nvidia_memory_gb: f64,
    /// Memory the model must leave free beside it, in GiB.
    pub headroom_gib: f64,
}

/// One served lane and the model it pins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lane {
    /// The lane (`decision`).
    pub lane: &'static str,
    /// The served name (`clef-flash`).
    pub name: &'static str,
    /// The Hugging Face repository.
    pub model_repo: &'static str,
    /// The pinned revision of that repository.
    pub revision: &'static str,
    /// The file within it.
    pub file: &'static str,
    /// Where it downloads from: the repository at the revision.
    pub url: &'static str,
    /// The downloaded file's sha256.
    pub file_sha256: &'static str,
    /// Its size in bytes.
    pub bytes: u64,
    /// The model's identity: [`crate::model_id`]'s digest of its tensors.
    pub model_id: &'static str,
    /// The smallest machine this lane admits (decision 9, 2026-10-19).
    pub floors: Floors,
    /// The checkers' round rule for this model.
    pub limits: Limits,
    /// The self-check reference set that ships for it.
    pub reference: &'static str,
}

/// Clef-Flash Q4_K_M from `ggml-org/Clef-Flash-GGUF` (Apache-2.0), served
/// by Psionic on the decision lane.
pub const CLEF_FLASH: Lane = Lane {
    lane: "decision",
    name: "clef-flash",
    model_repo: "ggml-org/Clef-Flash-GGUF",
    revision: "6b4fe4c59098479652d94f0fe11a40facbebd1ec",
    file: "Clef-Flash-Q4_K_M.gguf",
    url: "https://huggingface.co/ggml-org/Clef-Flash-GGUF/resolve/6b4fe4c59098479652d94f0fe11a40facbebd1ec/Clef-Flash-Q4_K_M.gguf",
    file_sha256: "3626116623ca8d89641952471fac9ce12af84fb3517520029a2ac6f5146868a2",
    bytes: 6_486_448_288,
    model_id: "gguf-tensors:sha256:2e417b2214d9a0b8736d850a1d80ffa4709be32056673b2f201f59cf8e6d07a3",
    floors: Floors {
        apple_memory_gb: 16.0,
        nvidia_memory_gb: 8.0,
        headroom_gib: 1.0,
    },
    limits: crate::verify::CLEF_FLASH,
    reference: crate::calibrate::CLEF_FLASH_REFERENCE,
};

/// Every lane this release serves.
pub const LANES: &[Lane] = &[CLEF_FLASH];

/// The sha256 of a lane's shipped self-check reference set.
#[must_use]
pub fn reference_digest(lane: &Lane) -> String {
    format!("sha256:{:x}", Sha256::digest(lane.reference.as_bytes()))
}

fn limits_json(limits: &Limits) -> Value {
    json!({
        "mean": limits.mean,
        "strike": limits.strike,
        "item": limits.item,
        "items": limits.items,
        "verified": limits.verified,
    })
}

/// The descriptor as JSON.
#[must_use]
pub fn descriptor() -> Value {
    let lanes: Vec<Value> = LANES
        .iter()
        .map(|lane| {
            json!({
                "lane": lane.lane,
                "name": lane.name,
                "model_repo": lane.model_repo,
                "revision": lane.revision,
                "file": lane.file,
                "url": lane.url,
                "file_sha256": lane.file_sha256,
                "bytes": lane.bytes,
                "model_id": lane.model_id,
                "floors": {
                    "apple_memory_gb": lane.floors.apple_memory_gb,
                    "nvidia_memory_gb": lane.floors.nvidia_memory_gb,
                    "headroom_gib": lane.floors.headroom_gib,
                },
                "selfcheck_reference": reference_digest(lane),
                "limits": limits_json(&lane.limits),
            })
        })
        .collect();
    let psionic: serde_json::Map<String, Value> = PSIONIC_PINS
        .iter()
        .map(|pin| {
            (
                pin.platform.to_string(),
                json!({"sha256": pin.sha256, "build_commit": pin.build_commit}),
            )
        })
        .collect();
    json!({
        "schema": SCHEMA,
        "contract": CONTRACT,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "lanes": lanes,
        "psionic": psionic,
        "default_relay": crate::DEFAULT_RELAY,
        "default_pool": DEFAULT_POOL,
    })
}

/// `version`: the contract, the CLI version, and the descriptor.
#[must_use]
pub fn version() -> (Value, String) {
    let descriptor = descriptor();
    let mut text = format!(
        "{CONTRACT} · openagents {} · {SCHEMA}\n",
        env!("CARGO_PKG_VERSION")
    );
    for lane in LANES {
        text.push_str(&format!(
            "{} ({} lane): {}@{} {}\n  file sha256 {}\n  model id {}\n  self-check reference {}\n",
            lane.name,
            lane.lane,
            lane.model_repo,
            &lane.revision[..12],
            lane.file,
            lane.file_sha256,
            lane.model_id,
            reference_digest(lane),
        ));
    }
    for pin in PSIONIC_PINS {
        text.push_str(&format!(
            "psionic {}: {}\n",
            pin.platform,
            pin.sha256.unwrap_or("not published in this release")
        ));
    }
    text.push_str(&format!(
        "relay {} · pool {DEFAULT_POOL}",
        crate::DEFAULT_RELAY
    ));
    (
        json!({
            "contract": CONTRACT,
            "cli_version": env!("CARGO_PKG_VERSION"),
            "release": descriptor,
        }),
        text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_is_the_repository_at_the_pinned_revision() {
        for lane in LANES {
            assert_eq!(
                lane.url,
                format!(
                    "https://huggingface.co/{}/resolve/{}/{}",
                    lane.model_repo, lane.revision, lane.file
                )
            );
            assert_eq!(lane.file_sha256.len(), 64);
        }
    }

    #[test]
    fn setup_reads_its_pins_from_the_descriptor() {
        assert_eq!(crate::setup::CLEF_FLASH_URL, CLEF_FLASH.url);
        assert_eq!(crate::setup::CLEF_FLASH_SHA256, CLEF_FLASH.file_sha256);
        assert_eq!(crate::setup::CLEF_FLASH_MODEL_ID, CLEF_FLASH.model_id);
        assert_eq!(crate::setup::CLEF_FLASH_BYTES, CLEF_FLASH.bytes);
        assert_eq!(crate::setup::CLEF_FLASH_FILE, CLEF_FLASH.file);
    }

    #[test]
    fn the_descriptor_names_every_pin() {
        let d = descriptor();
        assert_eq!(d["schema"], SCHEMA);
        assert_eq!(d["contract"], "pylon/1");
        assert_eq!(d["default_relay"], crate::DEFAULT_RELAY);
        assert_eq!(d["default_pool"], DEFAULT_POOL);
        let lane = &d["lanes"][0];
        assert_eq!(lane["model_id"], CLEF_FLASH.model_id);
        assert_eq!(lane["floors"]["apple_memory_gb"], 16.0);
        assert_eq!(lane["floors"]["nvidia_memory_gb"], 8.0);
        assert_eq!(lane["limits"]["items"], 8);
        assert!(
            lane["selfcheck_reference"]
                .as_str()
                .is_some_and(|s| s.starts_with("sha256:") && s.len() == 71)
        );
        for platform in ["macos-arm64", "linux-x86_64-cuda", "linux-x86_64"] {
            assert!(d["psionic"][platform].is_object(), "{platform}");
        }
    }
}
