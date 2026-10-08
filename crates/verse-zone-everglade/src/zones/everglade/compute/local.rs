//! This computer as the field's one local pylon (P0): its `build` slots
//! busy and free from the lease table, its jobs from the lease receipts,
//! and the pool's rate from the receipts of the last minute, all read with
//! `coder_lease::observe`, which never changes the table. The provider
//! capacity book (`capacity.json` in the task store) gives a well per
//! provider it names. Nothing here touches the network or spends.
//!
//! The pylon carries the **OWNER** mark: it serves only the owner's own
//! work until P1 publishes beacons. When the lease root can't be read, the
//! pylon's status is unknown, never online; a source that stops being
//! asked goes stale and turns unknown too (`super::project`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder_lease::Machine;
use serde_json::Value;
use world_tree::{Family, PylonStatus, Tier};

use super::{ComputeSource, PylonSample, Sample, WellSample};

/// The capacity book's file name and format
/// (`microcoder_loop::capacity`).
const BOOK: &str = "capacity.json";
const BOOK_SCHEMA: &str = "openagents.coder.provider-capacity.v1";
/// The window a receipt counts toward the pool's rate in.
const RATE_WINDOW: Duration = Duration::from_secs(60);
/// The memory steps NIP-PYLON's `class.memory_gb` takes, GB.
const MEMORY_STEPS: [u32; 7] = [8, 16, 32, 64, 128, 256, 512];

/// This computer's hardware class: family, tier, and memory in GB.
#[must_use]
pub fn class(machine: Machine, unified: bool) -> (Family, Tier, u32) {
    let gb = (machine.memory_bytes / 1_000_000_000) as u32;
    let memory_gb = MEMORY_STEPS
        .iter()
        .copied()
        .rfind(|&step| step <= gb)
        .unwrap_or(MEMORY_STEPS[0]);
    if unified {
        let tier = match gb {
            0..16 => Tier::Small,
            16..32 => Tier::Medium,
            32..96 => Tier::Large,
            _ => Tier::Xl,
        };
        (Family::UnifiedMemory, tier, memory_gb)
    } else {
        let tier = match machine.cores {
            0..8 => Tier::Small,
            8..16 => Tier::Medium,
            16..64 => Tier::Large,
            _ => Tier::Xl,
        };
        (Family::Cpu, tier, memory_gb)
    }
}

/// Whether this build runs on unified memory: an Apple silicon Mac.
#[must_use]
pub const fn unified_memory() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

/// The local source.
#[derive(Clone, Debug)]
pub struct LocalSource {
    leases: PathBuf,
    tasks: PathBuf,
    slots: u32,
    class: (Family, Tier, u32),
    /// The newest good observation, whose job count a failed read keeps.
    last: Option<PylonSample>,
}

impl LocalSource {
    /// This computer from the lease root `leases` and the task store
    /// `tasks`, with `slots` build slots and hardware `class`.
    #[must_use]
    pub fn new(leases: PathBuf, tasks: PathBuf, slots: u32, class: (Family, Tier, u32)) -> Self {
        Self {
            leases,
            tasks,
            slots: slots.max(1),
            class,
            last: None,
        }
    }

    /// This computer as its environment names it: the lease root
    /// (`OPENAGENTS_LEASE_ROOT` or `~/.openagents/leases`), the task store
    /// beside it, and the build slots its limits allow.
    ///
    /// # Errors
    /// When neither `HOME` nor the lease root variable is set.
    pub fn from_env() -> Result<Self, String> {
        let leases = coder_lease::root_from_env()?;
        let tasks = std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".openagents/tasks"))
            .ok_or("set HOME to find the task store")?;
        let slots = coder_lease::Limits::from_env().map_or(1, |l| l.build);
        let class = class(Machine::detect(), unified_memory());
        Ok(Self::new(
            leases,
            tasks,
            u32::try_from(slots).unwrap_or(64).min(64),
            class,
        ))
    }
}

/// The wells the capacity book in `dir` names, at `now`: one per provider
/// with a recorded refusal, dry while the latest one holds. No book, or
/// one that isn't the book's format, names none.
#[must_use]
pub fn wells(dir: &Path, now: u64) -> Vec<WellSample> {
    let Ok(bytes) = std::fs::read(dir.join(BOOK)) else {
        return Vec::new();
    };
    let Ok(book) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    if book.get("schema").and_then(Value::as_str) != Some(BOOK_SCHEMA) {
        return Vec::new();
    }
    let mut until: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for refusal in book
        .get("refusals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(provider) = refusal.get("provider").and_then(Value::as_str) else {
            continue;
        };
        let at = refusal.get("until").and_then(Value::as_u64).unwrap_or(0);
        let held = until.entry(provider.to_owned()).or_default();
        *held = (*held).max(at);
    }
    until
        .into_iter()
        .map(|(provider, until)| WellSample {
            provider,
            capacity: until <= now,
            until: (until > now).then_some(until),
        })
        .collect()
}

impl ComputeSource for LocalSource {
    fn sample(&mut self, now: u64) -> Sample {
        let (family, tier, memory_gb) = self.class;
        let mut rate = 0;
        let pylon = match coder_lease::observe(&self.leases, RATE_WINDOW) {
            Ok(seen) => {
                rate = u32::try_from(seen.recent_receipts).unwrap_or(u32::MAX);
                let busy = u32::try_from(seen.held_amount("build")).unwrap_or(u32::MAX);
                let pylon = PylonSample {
                    id: "local:this-computer".into(),
                    label: "This computer".into(),
                    family,
                    tier,
                    memory_gb,
                    status: PylonStatus::Online,
                    busy: busy.min(self.slots),
                    total: self.slots,
                    jobs: seen.receipts,
                    uptime: None,
                    observed_at: now,
                    owner: true,
                };
                self.last = Some(pylon.clone());
                pylon
            }
            // The table can't be read: the pylon is unknown at once, with
            // the jobs the last good read counted.
            Err(_) => PylonSample {
                status: PylonStatus::Unknown,
                busy: 0,
                observed_at: now,
                ..self.last.clone().unwrap_or(PylonSample {
                    id: "local:this-computer".into(),
                    label: "This computer".into(),
                    family,
                    tier,
                    memory_gb,
                    status: PylonStatus::Unknown,
                    busy: 0,
                    total: self.slots,
                    jobs: 0,
                    uptime: None,
                    observed_at: now,
                    owner: true,
                })
            },
        };
        Sample {
            pylons: vec![pylon],
            wells: wells(&self.tasks, now),
            rate,
            pool: "local".into(),
            demo: false,
        }
    }
}
