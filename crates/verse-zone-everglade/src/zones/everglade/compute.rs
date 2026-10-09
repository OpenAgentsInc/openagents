//! Compute in the Verse, P0 (`docs/compute/verse-compute.md`, issue
//! #10920): the Pylon Field and the Wellspring drawn from a
//! [`ComputeSource`].
//!
//! A source answers one question, [`ComputeSource::sample`]: which pylons
//! it knows now, each as a [`PylonSample`] (an ID, a label, a hardware
//! class, a status, busy and total slots, jobs served, and when it was
//! observed), which model providers' wells it knows ([`WellSample`]), and
//! the pool's recent job rate. Everything the field shows comes from that
//! sample through pure functions ([`project`], [`wellspring`], and
//! [`look`]), so a P1 source that reads NIP-PYLON beacons and receipts
//! plugs in beside the local one without touching the drawing.
//!
//! The sources here:
//!
//! - [`local::LocalSource`] (native only): this computer, from its lease
//!   table and receipts (`coder_lease::observe`) and the provider capacity
//!   book. Verse's desktop installs it; the web and the phones install
//!   nothing, so their field shows no pylons and a dim, still basin.
//! - [`relay::RelaySource`] (native, feature `pylon-relay`): every pylon
//!   whose NIP-PYLON beacon verifies on the relay, and this computer's jobs
//!   in flight on them (P1). The desktop shows it beside the local source
//!   through [`Merged`].
//! - [`sim::Sim`]: a labeled **DEMO** pool for captures (`verse
//!   --pylon-sim`).
//!
//! Honest by construction: a pylon whose sample is stale or whose source
//! failed shows **unknown**, never online ([`project`]); with no sample
//! there is no pylon; with no pylons the Wellspring is dim and still; and
//! a beam runs to Alice's workstation only while her studio seat is
//! working or one of this computer's pylon jobs is in flight.

#[cfg(test)]
mod agora_tests;
pub mod draw;
#[cfg(not(target_arch = "wasm32"))]
pub mod local;
pub mod look;
#[cfg(all(feature = "pylon-relay", not(target_arch = "wasm32")))]
pub mod relay;
pub mod sim;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use world_tree::{Family, PylonStatus, State, Tier, Tree};

use super::layout::pylon_field::{self, Field, MAX_WELLS};

/// How long a sample stays fresh after it was observed, s: NIP-PYLON's
/// beacon bound.
pub const FRESH: u64 = 300;
/// How far ahead of the reader's clock an observation may be, s.
pub const AHEAD: u64 = 30;
/// How often the field asks its source, s: a pylon's state changes at most
/// once every 5 seconds.
pub const POLL: f32 = 5.0;
/// How far from the field's middle a player inspects a pylon or the
/// basin, m.
pub const INSPECT_REACH: f32 = 2.8;

/// One pylon as a source knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct PylonSample {
    /// The pylon's address: `local:<slug>` for this machine, a `30200`
    /// address for a beacon.
    pub id: String,
    /// Display text, such as `This computer`.
    pub label: String,
    pub family: Family,
    pub tier: Tier,
    /// Memory, GB, rounded down to a NIP-PYLON step.
    pub memory_gb: u32,
    /// What the source said; [`project`] turns a stale one unknown.
    pub status: PylonStatus,
    pub busy: u32,
    pub total: u32,
    /// Jobs served, from receipts.
    pub jobs: u64,
    /// Seconds online, when the source knows.
    pub uptime: Option<u64>,
    /// When the source observed it, Unix seconds.
    pub observed_at: u64,
    /// It serves only its owner's own work (the **OWNER** mark): true for
    /// this machine until P1.
    pub owner: bool,
    /// A trusted checker's verdicts pass it and none fails it: it carries
    /// the sigil (P2).
    pub sigil: bool,
    /// Paid on its receipts of the last day, msat by network (P3); test
    /// networks never sum with `bitcoin`.
    pub paid_msat: BTreeMap<String, u64>,
    /// The coin-light: a receipt whose preimage hashes to its payment hash
    /// paid this pylon in the last [`COIN_SECS`] (P3).
    pub coin: Option<Coin>,
}

/// A pylon's coin-light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coin {
    /// Paid in test sats: the coin is pale and marked **TEST**.
    pub test: bool,
}

/// How long a paid receipt lights its pylon's coin, s.
pub const COIN_SECS: u64 = 15;

/// The coin-light for a paid receipt that finished at `finished_at`, at
/// `now`: lit for [`COIN_SECS`] after it, never from the future.
#[must_use]
pub fn coin(finished_at: u64, test: bool, now: u64) -> Option<Coin> {
    (finished_at <= now.saturating_add(AHEAD) && now.saturating_sub(finished_at) <= COIN_SECS)
        .then_some(Coin { test })
}

/// A model provider's well round the basin, from the capacity book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WellSample {
    /// The provider, such as `codex`.
    pub provider: String,
    /// Whether it has capacity now; false while a refusal holds.
    pub capacity: bool,
    /// When a holding refusal lifts, Unix seconds.
    pub until: Option<u64>,
}

/// What a source knows now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub pylons: Vec<PylonSample>,
    pub wells: Vec<WellSample>,
    /// Jobs finished in the last minute across the pool.
    pub rate: u32,
    /// The pool's address: `local` for this machine's.
    pub pool: String,
    /// Demonstration data, drawn with the **DEMO** mark.
    pub demo: bool,
    /// The pylons running one of this computer's jobs now, by
    /// [`PylonSample::id`], one entry a job: what draws the beam to
    /// Alice's station.
    pub in_flight: Vec<String>,
    /// The pool's newest aggregate recomputed from the records the source
    /// holds, which lights the basin's rim.
    pub verified: bool,
    /// The agent market the Agora shows (P4).
    pub market: Market,
}

/// The agent market as a source knows it (P4): what the Agora's compute
/// counter, services wall, and settlement threads draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Market {
    /// Agent services on offer, newest first, from verified NIP-MKT
    /// offerings.
    pub services: Vec<ServiceSample>,
    /// Accepted jobs in the pool over the last day.
    pub jobs: u64,
    /// Of those, the jobs a trusted broker bought: sales and agent orders.
    pub sales: u64,
    /// Paid on the pool's receipts over the last day, msat by network;
    /// test networks never sum with `bitcoin`.
    pub paid_msat: BTreeMap<String, u64>,
    /// Broker jobs that just finished, by [`PylonSample::id`]: each draws a
    /// settlement thread from the Agora to its pylon.
    pub threads: Vec<ThreadSample>,
}

/// One agent service on the Agora's wall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceSample {
    /// The seller, shortened for display.
    pub seller: String,
    /// The offering's slug, such as `plan-review`.
    pub offer: String,
    pub summary: String,
    pub price_msat: Option<u64>,
    /// Sold in test sats only: marked **TEST**.
    pub test: bool,
}

/// One settlement thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadSample {
    /// The pylon's [`PylonSample::id`].
    pub pylon: String,
    /// Gold for mainnet sats; pale for test sats and for an agent order's
    /// job, which carries no payment of its own.
    pub mainnet: bool,
}

/// Where a field's pylons and wells come from.
pub trait ComputeSource: Send {
    /// What the source knows at `now`, Unix seconds.
    fn sample(&mut self, now: u64) -> Sample;
}

/// Several sources as one field, such as this computer beside the relay's
/// pylons: each source's pylons in order, so the first source's take the
/// first sites; every well, job in flight, and job a minute; the first
/// pool that isn't this computer's own; and a rim lit when any source
/// recomputed its pool's aggregate.
pub struct Merged(pub Vec<Box<dyn ComputeSource>>);

impl ComputeSource for Merged {
    fn sample(&mut self, now: u64) -> Sample {
        let mut out = Sample::default();
        for source in &mut self.0 {
            let one = source.sample(now);
            out.pylons.extend(one.pylons);
            out.wells.extend(one.wells);
            out.rate = out.rate.saturating_add(one.rate);
            out.in_flight.extend(one.in_flight);
            out.verified |= one.verified;
            out.demo |= one.demo;
            out.market.services.extend(one.market.services);
            out.market.jobs += one.market.jobs;
            out.market.sales += one.market.sales;
            for (network, msat) in one.market.paid_msat {
                *out.market.paid_msat.entry(network).or_default() += msat;
            }
            out.market.threads.extend(one.market.threads);
            if out.pool.is_empty() || (out.pool == "local" && !one.pool.is_empty()) {
                out.pool = one.pool;
            }
        }
        out
    }
}

/// `pylon`'s state at `now`, Unix seconds: its status, unless its sample
/// is stale or from the future, which is unknown, never online.
#[must_use]
pub fn project(pylon: &PylonSample, now: u64) -> State {
    let fresh = pylon.observed_at <= now.saturating_add(AHEAD)
        && now.saturating_sub(pylon.observed_at) <= FRESH;
    let status = if fresh {
        pylon.status
    } else {
        PylonStatus::Unknown
    };
    let known = status != PylonStatus::Unknown;
    State::Pylon {
        pylon: pylon.id.clone(),
        status,
        family: pylon.family,
        tier: pylon.tier,
        busy: if known {
            pylon.busy.min(pylon.total)
        } else {
            0
        },
        total: pylon.total,
        jobs: pylon.jobs,
        paid_msat: pylon.paid_msat.clone(),
        uptime: pylon.uptime.filter(|_| known),
    }
}

/// The Wellspring's state from the pylons' projected states: `None` when
/// the source knows no pylon. Only online and draining pylons count, and
/// the pool is verified only when the source recomputed its aggregate,
/// which the local pool never has.
#[must_use]
pub fn wellspring(sample: &Sample, pylons: &[State]) -> Option<State> {
    if pylons.is_empty() {
        return None;
    }
    let (mut online, mut busy, mut total) = (0, 0, 0);
    for state in pylons {
        if let State::Pylon {
            status: PylonStatus::Online | PylonStatus::Draining,
            busy: b,
            total: t,
            ..
        } = state
        {
            online += 1;
            busy += b;
            total += t;
        }
    }
    Some(State::Wellspring {
        pool: sample.pool.clone(),
        online,
        busy,
        total,
        rate: if online > 0 { sample.rate } else { 0 },
        verified: sample.verified,
    })
}

/// The field's live state: its source, the newest sample, and the clock
/// its light animates on.
#[derive(Default)]
pub struct Compute {
    source: Option<Box<dyn ComputeSource>>,
    sample: Sample,
    /// The newest sample's projection: each pylon's state, in site order.
    pylons: Vec<State>,
    well: Option<State>,
    /// When the source was last asked, s of the field's clock.
    asked: Option<f32>,
    clock: f32,
    now: u64,
}

impl Compute {
    /// A field fed by `source`.
    #[must_use]
    pub fn with_source(source: Box<dyn ComputeSource>) -> Self {
        Self {
            source: Some(source),
            ..Self::default()
        }
    }

    /// Feeds the field from `source`, or from nothing.
    pub fn set_source(&mut self, source: Option<Box<dyn ComputeSource>>) {
        self.source = source;
        self.asked = None;
        if self.source.is_none() {
            self.sample = Sample::default();
            self.pylons.clear();
            self.well = None;
        }
    }

    /// Whether a source feeds the field.
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Advances the light by `dt` s, and at most every [`POLL`] seconds
    /// asks the source what it knows at `now`, Unix seconds.
    pub fn tick(&mut self, dt: f32, now: u64) {
        self.clock = (self.clock + dt.max(0.0)) % 3600.0;
        self.now = now;
        let due = self.asked.is_none_or(|at| {
            let since = (self.clock - at).rem_euclid(3600.0);
            since >= POLL
        });
        if due && let Some(source) = self.source.as_mut() {
            self.asked = Some(self.clock);
            self.sample = source.sample(now);
            self.sample.wells.truncate(MAX_WELLS);
        }
        // Freshness runs on the reader's clock, so a source that stops
        // answering turns its pylons unknown.
        self.pylons = self.sample.pylons.iter().map(|p| project(p, now)).collect();
        self.well = wellspring(&self.sample, &self.pylons);
    }

    /// The newest sample.
    #[must_use]
    pub fn sample(&self) -> &Sample {
        &self.sample
    }

    /// Each sampled pylon's state, in site order.
    #[must_use]
    pub fn pylon_states(&self) -> &[State] {
        &self.pylons
    }

    /// The Wellspring's state, when the source knows any pylon.
    #[must_use]
    pub fn wellspring_state(&self) -> Option<&State> {
        self.well.as_ref()
    }

    /// The pool's news for town day `day`, which a villager passes on when
    /// the player talks to it (P4): the Wellspring's online pylons and busy
    /// slots, the jobs a minute, and the day's sales at the Agora, all
    /// from the newest sample. `None` while the field knows no pylon. It
    /// names no quest step and earns no XP; its ID changes each day, so a
    /// villager tells each player once a day.
    #[must_use]
    pub fn rumor(&self, day: i64) -> Option<::townsfolk::rumor::Rumor> {
        let Some(State::Wellspring {
            online,
            busy,
            total,
            rate,
            ..
        }) = &self.well
        else {
            return None;
        };
        let demo = if self.sample.demo {
            "In the demo pool, the "
        } else {
            "The "
        };
        let plural =
            |n: u64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut fact = format!(
            "{demo}Wellspring runs on {} with {busy} of {total} slots busy, and {} finished in \
             the last minute.",
            plural(u64::from(*online), "pylon", "pylons"),
            plural(u64::from(*rate), "job", "jobs"),
        );
        if self.sample.market.sales > 0 {
            fact.push_str(&format!(
                " The Agora sold {} today.",
                plural(self.sample.market.sales, "job", "jobs")
            ));
        }
        Some(::townsfolk::rumor::Rumor {
            schema: ::townsfolk::rumor::RUMOR_SCHEMA.into(),
            id: format!("pylon-pool-news-{day}"),
            fact: fact.chars().take(::townsfolk::rumor::MAX_FACT).collect(),
            source: String::new(),
            node: WELLSPRING_SOURCE.into(),
            day,
            at: "00:00".into(),
            days: 1,
            step: String::new(),
            repeat: None,
        })
    }

    /// The field's clock, s.
    #[must_use]
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The pylons' and the Wellspring's states by world-tree node ID, for
    /// [`world_tree::Conditions::compute`]. Pylons past the field's sites
    /// have no node and aren't drawn.
    #[must_use]
    pub fn conditions(&self, tree: &Tree) -> BTreeMap<String, State> {
        let mut out = BTreeMap::new();
        for (i, state) in self.pylons.iter().enumerate() {
            if let Some(node) = tree.by_source(&site_source(i)) {
                out.insert(node.id.clone(), state.clone());
            }
        }
        if let (Some(state), Some(node)) = (&self.well, tree.by_source(WELLSPRING_SOURCE)) {
            out.insert(node.id.clone(), state.clone());
        }
        out
    }

    /// What a player at `at` inspects up close: the pylon or the basin in
    /// reach, or the Agora's compute counter or services wall, as caption
    /// lines.
    #[must_use]
    pub fn inspect(&self, at: [f32; 2]) -> Option<String> {
        if let Some(text) = draw::agora::inspect(&self.sample, &self.pylons, at) {
            return Some(text);
        }
        let field = pylon_field::site()?;
        inspect(
            field,
            &self.sample,
            &self.pylons,
            self.well.as_ref(),
            at,
            self.now,
        )
    }
}

/// Whether studio seat `seat` is working in `view`: running, editing,
/// testing, or anything else but idle, paused, done, or failed. The beam
/// to its station is drawn only then.
#[must_use]
pub fn seat_working(view: Option<&coder_access::studio::View>, seat: &str) -> bool {
    use coder_access::studio::Activity;
    view.is_some_and(|view| {
        view.seats.iter().any(|s| {
            s.seat == seat
                && !matches!(
                    s.activity,
                    Activity::Idle | Activity::Paused | Activity::Done | Activity::Failed
                )
        })
    })
}

/// The world-tree source of the Wellspring's node.
pub const WELLSPRING_SOURCE: &str = "pylon-field:wellspring";

/// The world-tree source of pylon site `i`'s node.
#[must_use]
pub fn site_source(i: usize) -> String {
    format!("pylon-field:site:{i}")
}

/// [`Compute::inspect`] over explicit parts.
#[must_use]
pub fn inspect(
    field: &Field,
    sample: &Sample,
    pylons: &[State],
    well: Option<&State>,
    at: [f32; 2],
    now: u64,
) -> Option<String> {
    let near = |p: [f32; 2]| (p[0] - at[0]).hypot(p[1] - at[1]) <= INSPECT_REACH;
    let demo = if sample.demo { "DEMO · " } else { "" };
    for (i, site) in field.sites.iter().enumerate() {
        if !near(*site) {
            continue;
        }
        let (Some(p), Some(state)) = (sample.pylons.get(i), pylons.get(i)) else {
            return Some("Pylon Field\nAn empty pylon site: no machine here".into());
        };
        let jobs = sample.in_flight.iter().filter(|id| **id == p.id).count();
        let mine = match jobs {
            0 => String::new(),
            1 => "\nRunning a job from this computer".into(),
            n => format!("\nRunning {n} jobs from this computer"),
        };
        return Some(format!("{demo}{}{mine}", look::describe_pylon(p, state)));
    }
    for (i, w) in sample.wells.iter().enumerate() {
        if (field.well(i)[0] - at[0]).hypot(field.well(i)[1] - at[1]) <= 1.0 {
            return Some(format!("{demo}{}", look::describe_well(w, now)));
        }
    }
    if (field.center[0] - at[0]).hypot(field.center[1] - at[1]) <= pylon_field::BASIN + 1.6 {
        return Some(format!("{demo}{}", look::describe_wellspring(well)));
    }
    None
}
