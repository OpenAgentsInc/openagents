//! A labeled **DEMO** pool for captures (`verse --pylon-sim`): invented
//! pylons of every family and tier whose load rises and falls with the
//! clock. Its sample says `demo`, so the field draws the DEMO mark over the
//! basin and the inspect panel leads with it. Nothing else reads it.

use std::collections::BTreeMap;

use world_tree::{Family, PylonStatus, Tier};

use super::{ComputeSource, Market, PylonSample, Sample, ServiceSample, ThreadSample, WellSample};

/// The demo market's services: seller, slug, summary, and price in sats.
const SERVICES: [(&str, &str, &str, u64); 3] = [
    (
        "demo-victor",
        "plan-review",
        "Victor reviews a day plan",
        25,
    ),
    ("demo-erin", "lead-brief", "Erin briefs a sales lead", 40),
    ("demo-paul", "call-notes", "Paul summarizes a call", 15),
];

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
                    // Demo sats are never real: the paying towers show
                    // TEST coins.
                    paid_msat: if matches!(i, 1 | 3 | 6) {
                        BTreeMap::from([("regtest".to_string(), 2_000 * (phase % 50 + 1))])
                    } else {
                        BTreeMap::new()
                    },
                    coin: (!stale && matches!(i, 1 | 3 | 6) && (phase + k) % 2 == 0)
                        .then_some(super::Coin { test: true }),
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
            market: Market {
                services: SERVICES
                    .iter()
                    .map(|&(seller, offer, summary, sats)| ServiceSample {
                        seller: seller.into(),
                        offer: offer.into(),
                        summary: summary.into(),
                        price_msat: Some(sats * 1_000),
                        test: true,
                    })
                    .collect(),
                jobs: 1_240 + phase % 90,
                sales: 310 + phase % 40,
                paid_msat: BTreeMap::from([(
                    "regtest".to_string(),
                    4_000_000 + 9_000 * (phase % 50),
                )]),
                // One settlement thread at a time, to a paying tower.
                threads: vec![ThreadSample {
                    pylon: format!("demo:pylon-{}", [1, 3, 6][(phase % 3) as usize]),
                    mainnet: false,
                }],
            },
        }
    }
}
