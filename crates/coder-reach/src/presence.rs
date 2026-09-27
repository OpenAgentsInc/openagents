//! Host presence: a host-signed, device-encrypted report of protocol version,
//! capabilities, compatibility, host generation, and bounded telemetry.
//!
//! Presence grants nothing. A reader judges freshness by when it received the
//! sample, refuses samples from the future, and reads the host's advertised
//! capabilities instead of assuming its own version's.

use std::collections::BTreeMap;

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Refusal, Result, artifact, fail, parse_pubkey, pubkey, requires, slug};

/// Schema of a presence body.
pub const SCHEMA: &str = "openagents.host-presence.v1";
/// Most capability flags one presence lists.
pub const MAX_CAPABILITIES: usize = 64;
/// Largest CPU count a host may report.
pub const MAX_CPU_COUNT: u32 = 4096;

/// Inclusive protocol version range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRange {
    pub min: u32,
    pub max: u32,
}

impl VersionRange {
    #[must_use]
    pub const fn contains(self, version: u32) -> bool {
        self.min <= version && version <= self.max
    }
}

/// Coarse resource telemetry. Whole percentages limit what a sample reveals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telemetry {
    /// Logical CPUs available to the host, 1 to 4,096.
    pub cpu_count: u32,
    /// Recent CPU utilization, 0 to 100 percent.
    pub cpu_utilization_pct: u8,
    /// Memory available to new work, 0 to 100 percent of the total.
    pub memory_available_pct: u8,
}

/// The presence body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    pub v: String,
    pub requires: Vec<String>,
    pub host: String,
    /// The owner whose directory lists this host.
    pub owner: String,
    /// Increases whenever the host restarts or changes its binary or state.
    pub generation: u64,
    /// The protocol version the host speaks.
    pub protocol: u32,
    /// Client protocol versions the host accepts.
    pub compatibility: VersionRange,
    /// Capability flags the host advertises. Unknown flags are ignored.
    pub capabilities: Vec<String>,
    /// Host clock when the host took this sample.
    pub observed_at: u64,
    /// Absent when the host withholds telemetry.
    pub telemetry: Option<Telemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// What a client speaks, used for the compatibility rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientProfile {
    /// The client's own protocol version.
    pub protocol: u32,
    /// Host protocol versions the client can drive.
    pub accepts: VersionRange,
}

/// Reader freshness policy, in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Freshness {
    /// Oldest acceptable sample, measured from the reader's receipt time.
    pub max_age: u64,
    /// Largest amount a sample's `observed_at` may lead its receipt time.
    pub max_future_skew: u64,
}

impl Default for Freshness {
    fn default() -> Self {
        Self {
            max_age: 180,
            max_future_skew: 30,
        }
    }
}

impl Presence {
    /// Check the closed schema and bounds.
    ///
    /// # Errors
    /// Refuses unknown versions or required features, bad keys, invalid
    /// ranges, malformed or duplicate capabilities, and out-of-range telemetry.
    pub fn validate(&self) -> Result<()> {
        if self.v != SCHEMA {
            return fail(Refusal::UnsupportedVersion, "presence version");
        }
        requires(&self.requires)?;
        parse_pubkey(&self.host)?;
        parse_pubkey(&self.owner)?;
        if self.host == self.owner {
            return fail(Refusal::Malformed, "host and owner keys must differ");
        }
        if self.protocol == 0
            || self.compatibility.min == 0
            || self.compatibility.min > self.compatibility.max
        {
            return fail(Refusal::Malformed, "protocol version range");
        }
        if self.capabilities.len() > MAX_CAPABILITIES {
            return fail(Refusal::LimitExceeded, "too many capabilities");
        }
        for (index, capability) in self.capabilities.iter().enumerate() {
            if !slug(capability) || self.capabilities[..index].contains(capability) {
                return fail(Refusal::Malformed, "capability flag");
            }
        }
        if let Some(t) = self.telemetry
            && (t.cpu_count == 0
                || t.cpu_count > MAX_CPU_COUNT
                || t.cpu_utilization_pct > 100
                || t.memory_available_pct > 100)
        {
            return fail(Refusal::Malformed, "telemetry out of range");
        }
        Ok(())
    }

    /// Sign with the host key and encrypt to one enrolled device.
    ///
    /// # Errors
    /// Refuses an invalid body or a key that does not match `host`.
    pub fn seal(
        &self,
        host: &SecretKey,
        device: &str,
        mailbox: &str,
        retain_until: u64,
    ) -> Result<Event> {
        self.validate()?;
        if pubkey(host) != self.host {
            return fail(
                Refusal::IdentityMismatch,
                "only the host seals its presence",
            );
        }
        parse_pubkey(device)?;
        artifact::seal(
            self,
            SCHEMA,
            host,
            device,
            mailbox,
            self.observed_at,
            retain_until,
        )
    }

    /// Open a presence sample for a host the owner's directory lists.
    ///
    /// # Errors
    /// Refuses another signer, another owner, mismatched times, or an invalid body.
    pub fn open(event: &Event, device: &SecretKey, host: &str, owner: &str) -> Result<Self> {
        let reader = pubkey(device);
        let (body, sealed): (Self, _) = artifact::open(event, device, host, &reader, SCHEMA)?;
        if body.host != host || body.owner != owner {
            return fail(
                Refusal::IdentityMismatch,
                "presence names another host or owner",
            );
        }
        if body.observed_at != sealed.issued_at {
            return fail(Refusal::Malformed, "body and envelope times differ");
        }
        body.validate()?;
        Ok(body)
    }

    /// Whether the host advertises `capability`. A client asks this rather
    /// than assuming a capability from its own version.
    #[must_use]
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// Advertised flags this client does not interpret. They are ignored,
    /// never treated as supported.
    #[must_use]
    pub fn unknown_capabilities<'a>(&'a self, known: &[&str]) -> Vec<&'a str> {
        self.capabilities
            .iter()
            .map(String::as_str)
            .filter(|c| !known.contains(c))
            .collect()
    }

    /// The compatibility rule: the host accepts the client's version, and the
    /// client accepts the host's version.
    ///
    /// # Errors
    /// Refuses as `incompatible` when either side is out of range.
    pub fn compatible(&self, client: &ClientProfile) -> Result<()> {
        if self.compatibility.contains(client.protocol) && client.accepts.contains(self.protocol) {
            Ok(())
        } else {
            fail(Refusal::Incompatible, "protocol versions do not overlap")
        }
    }
}

/// A presence sample and the reader's clock when it arrived.
#[derive(Clone, Debug, PartialEq)]
pub struct Received {
    pub presence: Presence,
    pub received_at: u64,
}

impl Received {
    /// Judge freshness at `now`. Receipt time, not the host's clock, bounds
    /// the age; a sample observed after its receipt plus the allowed skew is
    /// from the future and refuses.
    ///
    /// # Errors
    /// Refuses future samples as `malformed` and old samples as `stale`.
    pub fn judge(&self, now: u64, policy: Freshness) -> Result<()> {
        let observed = self.presence.observed_at;
        if observed > self.received_at.saturating_add(policy.max_future_skew) {
            return fail(Refusal::Malformed, "presence sample is from the future");
        }
        if self.received_at > now {
            return fail(Refusal::Malformed, "receipt time is after now");
        }
        if now - self.received_at > policy.max_age
            || self.received_at.saturating_sub(observed) > policy.max_age
        {
            return fail(Refusal::Stale, "presence sample is stale");
        }
        Ok(())
    }
}

/// The newest accepted presence per host. A lower generation than one
/// already seen refuses, so a replayed old sample cannot roll a host back.
#[derive(Clone, Debug, Default)]
pub struct PresenceBook {
    latest: BTreeMap<String, Received>,
}

impl PresenceBook {
    /// Record a sample after checking freshness at its receipt time.
    ///
    /// # Errors
    /// Refuses future samples, older generations, and samples older than the
    /// one already held for the same generation.
    pub fn record(&mut self, sample: Received, policy: Freshness) -> Result<()> {
        sample.presence.validate()?;
        sample.judge(sample.received_at, policy)?;
        if let Some(held) = self.latest.get(&sample.presence.host) {
            let old = (held.presence.generation, held.presence.observed_at);
            let new = (sample.presence.generation, sample.presence.observed_at);
            if sample.presence.generation < held.presence.generation {
                return fail(Refusal::Stale, "presence generation went backward");
            }
            if new <= old {
                return fail(Refusal::Stale, "presence is not newer than the held sample");
            }
        }
        self.latest.insert(sample.presence.host.clone(), sample);
        Ok(())
    }

    /// The held sample for `host`.
    #[must_use]
    pub fn get(&self, host: &str) -> Option<&Received> {
        self.latest.get(host)
    }

    /// Forget a host, for example after the owner removes it.
    pub fn forget(&mut self, host: &str) {
        self.latest.remove(host);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::new_id;

    pub(crate) fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }

    pub(crate) fn sample(host: u8, generation: u64, observed_at: u64) -> Presence {
        Presence {
            v: SCHEMA.into(),
            requires: vec![],
            host: pubkey(&key(host)),
            owner: pubkey(&key(1)),
            generation,
            protocol: 1,
            compatibility: VersionRange { min: 1, max: 1 },
            capabilities: vec!["direct-tcp".into(), "relay-control".into()],
            observed_at,
            telemetry: Some(Telemetry {
                cpu_count: 8,
                cpu_utilization_pct: 25,
                memory_available_pct: 50,
            }),
            meta: None,
        }
    }

    #[test]
    fn presence_round_trip_and_wrong_signer() {
        let presence = sample(2, 4, 1000);
        let device = key(9);
        let event = presence
            .seal(&key(2), &pubkey(&device), &new_id(), 2000)
            .unwrap();
        let opened = Presence::open(&event, &device, &pubkey(&key(2)), &pubkey(&key(1))).unwrap();
        assert_eq!(opened, presence);
        // Expecting a different host from the directory refuses.
        assert_eq!(
            Presence::open(&event, &device, &pubkey(&key(3)), &pubkey(&key(1)))
                .unwrap_err()
                .code,
            Refusal::IdentityMismatch
        );
        // A different owner's directory does not adopt this host.
        assert!(Presence::open(&event, &device, &pubkey(&key(2)), &pubkey(&key(7))).is_err());
        // Only the host key seals its presence.
        assert!(
            presence
                .seal(&key(3), &pubkey(&device), &new_id(), 2000)
                .is_err()
        );
    }

    #[test]
    fn freshness_uses_receipt_time_and_refuses_future_samples() {
        let policy = Freshness {
            max_age: 60,
            max_future_skew: 5,
        };
        let fresh = Received {
            presence: sample(2, 1, 1000),
            received_at: 1002,
        };
        fresh.judge(1030, policy).unwrap();
        // Old by receipt time.
        assert_eq!(fresh.judge(1063, policy).unwrap_err().code, Refusal::Stale);
        // Received now, but observed long ago: a replayed sample.
        let replayed = Received {
            presence: sample(2, 1, 900),
            received_at: 1000,
        };
        assert_eq!(
            replayed.judge(1000, policy).unwrap_err().code,
            Refusal::Stale
        );
        // Within skew is accepted; beyond skew is from the future.
        let ahead = Received {
            presence: sample(2, 1, 1005),
            received_at: 1000,
        };
        ahead.judge(1000, policy).unwrap();
        let future = Received {
            presence: sample(2, 1, 1006),
            received_at: 1000,
        };
        assert_eq!(
            future.judge(1000, policy).unwrap_err().code,
            Refusal::Malformed
        );
    }

    #[test]
    fn unknown_capabilities_and_required_features() {
        let mut presence = sample(2, 1, 1000);
        presence.capabilities.push("future-thing".into());
        presence.validate().unwrap();
        assert!(presence.supports("direct-tcp"));
        assert!(!presence.supports("terminal"));
        assert_eq!(
            presence.unknown_capabilities(&["direct-tcp", "relay-control"]),
            vec!["future-thing"]
        );
        presence.requires.push("oa-future".into());
        assert_eq!(
            presence.validate().unwrap_err().code,
            Refusal::UnsupportedFeature
        );
        let mut bad = sample(2, 1, 1000);
        bad.capabilities.push("Not A Slug".into());
        assert!(bad.validate().is_err());
        let mut dup = sample(2, 1, 1000);
        dup.capabilities.push("direct-tcp".into());
        assert!(dup.validate().is_err());
    }

    #[test]
    fn compatibility_reads_the_host_advertisement() {
        let mut presence = sample(2, 1, 1000);
        let client = ClientProfile {
            protocol: 1,
            accepts: VersionRange { min: 1, max: 2 },
        };
        presence.compatible(&client).unwrap();
        presence.compatibility = VersionRange { min: 2, max: 3 };
        assert_eq!(
            presence.compatible(&client).unwrap_err().code,
            Refusal::Incompatible
        );
        presence.compatibility = VersionRange { min: 1, max: 3 };
        presence.protocol = 3;
        assert_eq!(
            presence.compatible(&client).unwrap_err().code,
            Refusal::Incompatible
        );
    }

    #[test]
    fn telemetry_bounds_and_strict_fields() {
        let mut presence = sample(2, 1, 1000);
        presence.telemetry = Some(Telemetry {
            cpu_count: 0,
            cpu_utilization_pct: 0,
            memory_available_pct: 0,
        });
        assert!(presence.validate().is_err());
        presence.telemetry = Some(Telemetry {
            cpu_count: 4,
            cpu_utilization_pct: 101,
            memory_available_pct: 0,
        });
        assert!(presence.validate().is_err());
        let mut value = serde_json::to_value(sample(2, 1, 1000)).unwrap();
        value["telemetry"]["gpu_temp"] = 70.into();
        assert!(serde_json::from_value::<Presence>(value).is_err());
    }

    #[test]
    fn book_refuses_rollback() {
        let policy = Freshness::default();
        let mut book = PresenceBook::default();
        book.record(
            Received {
                presence: sample(2, 5, 1000),
                received_at: 1000,
            },
            policy,
        )
        .unwrap();
        let older = Received {
            presence: sample(2, 4, 1100),
            received_at: 1100,
        };
        assert_eq!(book.record(older, policy).unwrap_err().code, Refusal::Stale);
        let same = Received {
            presence: sample(2, 5, 1000),
            received_at: 1001,
        };
        assert!(book.record(same, policy).is_err());
        book.record(
            Received {
                presence: sample(2, 6, 1010),
                received_at: 1010,
            },
            policy,
        )
        .unwrap();
        assert_eq!(book.get(&pubkey(&key(2))).unwrap().presence.generation, 6);
    }
}
