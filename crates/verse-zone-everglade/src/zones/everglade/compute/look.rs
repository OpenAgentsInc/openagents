//! From data to look: pure functions from a pylon's or the Wellspring's
//! state to how the field draws it (`docs/compute/verse-compute.md`, The
//! Pylon Field and The Wellspring), and the inspect panel's words. Nothing
//! here reads a clock or a file, so the tests pin every rule.

use world_tree::{Family, PylonStatus, State, Tier};

use super::{PylonSample, WellSample};

/// A pylon's shape, from its hardware family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A slim crystal spire: a unified-memory machine such as a Mac.
    Spire,
    /// A broad obelisk: a GPU machine.
    Obelisk,
    /// A squat cairn: a CPU machine.
    Cairn,
}

/// How a pylon draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PylonLook {
    pub shape: Shape,
    /// Height, m: four steps by tier.
    pub height: f32,
    /// How bright its light is, 0 to 1: zero when it isn't online.
    pub glow: f32,
    /// The light breathes while it waits for work.
    pub breathes: bool,
    /// The light burns steady while it works.
    pub burns: bool,
    /// Grey and still, marked UNKNOWN, with a question mark in the inspect
    /// panel: a stale or missing source.
    pub unknown: bool,
    /// Bands of light round its base: one per tenfold step in jobs served.
    pub bands: u32,
    /// Moss up the base, 0 to 1, from uptime.
    pub moss: f32,
    /// Carved rune bands up the shaft: one per slot, at most
    /// [`MAX_RUNES`].
    pub runes: u32,
    /// The bands that burn: one per busy slot.
    pub lit_runes: u32,
    /// How bright the unlit bands glow while online, 0 to 1: a faint
    /// standby light, none when it isn't online.
    pub standby: f32,
    /// The stream of light along the ground to the basin, 0 to 1: the busy
    /// share of the slots while it serves, else zero.
    pub stream: f32,
}

/// The most rune bands a pylon wears.
pub const MAX_RUNES: u32 = 4;

/// The most bands a pylon wears: a million jobs.
pub const MAX_BANDS: u32 = 7;
/// Uptime that covers a pylon's base in moss, s: thirty days.
pub const MOSS_FULL: u64 = 30 * 24 * 3600;
/// How dim an online pylon waiting for work burns.
pub const IDLE_GLOW: f32 = 0.22;

/// A pylon's height by family and tier, m.
#[must_use]
pub fn height(family: Family, tier: Tier) -> f32 {
    let step = match tier {
        Tier::Small => 0,
        Tier::Medium => 1,
        Tier::Large => 2,
        Tier::Xl => 3,
    } as f32;
    match family {
        Family::UnifiedMemory => 1.8 + 0.6 * step,
        Family::Gpu => 1.6 + 0.55 * step,
        Family::Cpu => 0.9 + 0.3 * step,
    }
}

/// Bands for `jobs` served: none for none, then one per tenfold step.
#[must_use]
pub fn bands(jobs: u64) -> u32 {
    if jobs == 0 {
        0
    } else {
        (jobs.ilog10() + 1).min(MAX_BANDS)
    }
}

/// How `state` draws; `None` when it isn't a pylon's state.
#[must_use]
pub fn pylon(state: &State) -> Option<PylonLook> {
    let State::Pylon {
        status,
        family,
        tier,
        busy,
        total,
        jobs,
        uptime,
        ..
    } = state
    else {
        return None;
    };
    let shape = match family {
        Family::UnifiedMemory => Shape::Spire,
        Family::Gpu => Shape::Obelisk,
        Family::Cpu => Shape::Cairn,
    };
    let working = *busy > 0 && *total > 0;
    let (glow, breathes, burns) = match status {
        PylonStatus::Online if working => {
            (0.55 + 0.45 * (*busy as f32 / *total as f32), false, true)
        }
        PylonStatus::Online => (IDLE_GLOW, true, false),
        PylonStatus::Draining => (0.35, false, working),
        PylonStatus::Offline | PylonStatus::Unknown => (0.0, false, false),
    };
    Some(PylonLook {
        shape,
        height: height(*family, *tier),
        glow,
        breathes,
        burns,
        unknown: *status == PylonStatus::Unknown,
        bands: bands(*jobs),
        moss: uptime.map_or(0.0, |u| (u as f32 / MOSS_FULL as f32).min(1.0)),
        runes: (*total).clamp(1, MAX_RUNES),
        // A band per busy slot, spread over the bands when there are more
        // slots than bands.
        lit_runes: match status {
            PylonStatus::Online | PylonStatus::Draining if working => {
                let bands = (*total).clamp(1, MAX_RUNES);
                (busy * bands).div_ceil(*total).min(bands)
            }
            _ => 0,
        },
        standby: match status {
            PylonStatus::Online => 0.3,
            PylonStatus::Draining => 0.15,
            PylonStatus::Offline | PylonStatus::Unknown => 0.0,
        },
        stream: match status {
            PylonStatus::Online | PylonStatus::Draining if working => {
                (*busy as f32 / *total as f32).min(1.0)
            }
            _ => 0.0,
        },
    })
}

/// How the Wellspring draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WellLook {
    /// How bright the pool is, 0 to 1: zero with no capacity online.
    pub brightness: f32,
    /// How much its surface churns, 0 to 1: the busy share of the slots.
    pub churn: f32,
    /// Ripples a second, one per job, capped at 10.
    pub ripples: f32,
    /// The rim is lit only for a recomputed pool aggregate.
    pub rim: bool,
    /// Shafts of light rising from the pool: more with more capacity.
    pub shafts: u32,
    /// Motes rising from the pool: more with more busy slots.
    pub motes: u32,
}

/// The most shafts and motes the pool sends up.
pub const MAX_SHAFTS: u32 = 5;
pub const MAX_MOTES: u32 = 18;

/// Slots that light the pool fully.
pub const FULL_SLOTS: u32 = 16;
/// The most ripples a second.
pub const MAX_RIPPLES: f32 = 10.0;

/// The Wellspring's look; dim and still when there is no state.
#[must_use]
pub fn wellspring(state: Option<&State>) -> WellLook {
    let still = WellLook {
        brightness: 0.0,
        churn: 0.0,
        ripples: 0.0,
        rim: false,
        shafts: 0,
        motes: 0,
    };
    let Some(State::Wellspring {
        busy,
        total,
        rate,
        verified,
        ..
    }) = state
    else {
        return still;
    };
    if *total == 0 {
        return WellLook {
            rim: *verified,
            ..still
        };
    }
    let brightness = 0.3 + 0.7 * (*total as f32 / FULL_SLOTS as f32).min(1.0);
    let churn = (*busy as f32 / *total as f32).min(1.0);
    WellLook {
        brightness,
        churn,
        ripples: (*rate as f32 / 60.0).min(MAX_RIPPLES),
        rim: *verified,
        shafts: 1 + ((MAX_SHAFTS - 1) as f32 * (brightness - 0.3) / 0.7).round() as u32,
        motes: (MAX_MOTES as f32 * churn).round() as u32,
    }
}

/// A count with thousands separators, as the panel shows it.
#[must_use]
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A duration in words, such as `3 h` or `2 d`.
fn span(seconds: u64) -> String {
    match seconds {
        s if s < 3600 => format!("{} min", s / 60),
        s if s < 2 * 86_400 => format!("{} h", s / 3600),
        s => format!("{} d", s / 86_400),
    }
}

/// The inspect panel's lines for `pylon`, projected as `state`.
#[must_use]
pub fn describe_pylon(pylon: &PylonSample, state: &State) -> String {
    let State::Pylon {
        status,
        busy,
        total,
        jobs,
        uptime,
        ..
    } = state
    else {
        return pylon.label.clone();
    };
    let owner = if pylon.owner { " · OWNER" } else { "" };
    let class = format!(
        "{}, {}, {} GB",
        pylon.family.as_str(),
        pylon.tier.as_str(),
        pylon.memory_gb
    );
    if *status == PylonStatus::Unknown {
        return format!(
            "{}{owner} · ?\n{class} · unknown: no fresh sample from its source",
            pylon.label
        );
    }
    let up = uptime
        .map(|u| format!(" · up {}", span(u)))
        .unwrap_or_default();
    format!(
        "{}{owner}\n{class} · {}{up}\n{busy} of {total} slots busy · {} jobs served",
        pylon.label,
        status.as_str(),
        count(*jobs)
    )
}

/// The inspect panel's lines for the Wellspring.
#[must_use]
pub fn describe_wellspring(state: Option<&State>) -> String {
    match state {
        Some(State::Wellspring {
            pool,
            online,
            busy,
            total,
            rate,
            verified,
        }) => format!(
            "The Wellspring · {} pool\n{online} {} online · {busy} of {total} slots busy\n{rate} jobs in the last minute · {}",
            if pool == "local" {
                "this computer's"
            } else {
                pool
            },
            if *online == 1 { "pylon" } else { "pylons" },
            if *verified { "verified" } else { "unverified" }
        ),
        _ => "The Wellspring\nDormant: no compute source feeds it here".into(),
    }
}

/// The inspect panel's line for a provider's well.
#[must_use]
pub fn describe_well(well: &WellSample, now: u64) -> String {
    match (well.capacity, well.until) {
        (true, _) => format!(
            "{} well\nThe capacity book records no refusal holding now",
            well.provider
        ),
        (false, Some(until)) => format!(
            "{} well · dry\nRefused for a usage limit; lifts in {}",
            well.provider,
            span(until.saturating_sub(now))
        ),
        (false, None) => format!("{} well · dry\nRefused for a usage limit", well.provider),
    }
}
