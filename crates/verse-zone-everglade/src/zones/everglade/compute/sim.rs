//! A labeled **DEMO** pool for captures (`verse --pylon-sim`): invented
//! pylons of every family and tier whose load rises and falls with the
//! clock. Its sample says `demo`, so the field draws the DEMO mark over the
//! basin and the inspect panel leads with it. Nothing else reads it.

use world_tree::{Family, PylonStatus, Tier};

use super::{ComputeSource, PylonSample, Sample, WellSample};

/// The demo pool's pylons: label, family, tier, memory, and slots.
const PYLONS: [(&str, Family, Tier, u32, u32); 8] = [
    ("Demo studio Mac", Family::UnifiedMemory, Tier::Large, 64, 2),
    ("Demo GPU tower", Family::Gpu, Tier::Large, 32, 4),
    ("Demo laptop", Family::UnifiedMemory, Tier::Small, 8, 1),
    ("Demo render node", Family::Gpu, Tier::Xl, 128, 8),
    ("Demo mini", Family::UnifiedMemory, Tier::Medium, 16, 1),
    ("Demo build box", Family::Cpu, Tier::Large, 64, 4),
    ("Demo Mac Studio", Family::UnifiedMemory, Tier::Xl, 256, 4),
    ("Demo old server", Family::Cpu, Tier::Small, 8, 1),
];

/// The demo pool.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sim;

impl ComputeSource for Sim {
    fn sample(&mut self, now: u64) -> Sample {
        let phase = now / 20;
        let pylons = PYLONS
            .iter()
            .enumerate()
            .map(|(i, &(label, family, tier, memory_gb, total))| {
                let k = i as u64;
                // One pylon is stale, so the demo shows unknown too.
                let stale = i == 7;
                let wave = (phase + k * 3) % 5;
                PylonSample {
                    id: format!("demo:pylon-{i}"),
                    label: label.into(),
                    family,
                    tier,
                    memory_gb,
                    status: if i == 5 {
                        PylonStatus::Draining
                    } else {
                        PylonStatus::Online
                    },
                    busy: (total as u64 * wave / 4).min(total as u64) as u32,
                    total,
                    jobs: [3, 48, 0, 12_400, 7, 860, 154_000, 2][i] + phase % 7,
                    uptime: Some(3600 * (1 + k * k * 30)),
                    observed_at: if stale { now.saturating_sub(900) } else { now },
                    owner: false,
                    // The busiest towers have passing checks.
                    sigil: !stale && matches!(i, 1 | 3 | 6),
                }
            })
            .collect();
        Sample {
            pylons,
            wells: vec![
                WellSample {
                    provider: "codex".into(),
                    capacity: true,
                    until: None,
                },
                WellSample {
                    provider: "claude".into(),
                    capacity: phase % 3 != 0,
                    until: (phase % 3 == 0).then_some(now + 1800),
                },
            ],
            rate: 60 + (phase % 4) as u32 * 90,
            pool: "demo".into(),
            demo: true,
            // One of the demo jobs is this computer's, on the GPU tower, and the
            // demo aggregate recomputes, so the beam and the rim both show.
            in_flight: vec!["demo:pylon-1".into()],
            verified: true,
        }
    }
}
