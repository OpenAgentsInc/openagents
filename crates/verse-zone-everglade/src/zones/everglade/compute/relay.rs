//! The Pylon Field from a relay (P1, `docs/compute/pylon.md`): every pylon
//! whose NIP-PYLON beacon (`30200`) verifies, with its jobs from verified
//! receipts (`3201`) and the pool's rate from its newest aggregate (`30201`)
//! when one is valid, through `pylon::field`. A background thread holds
//! one subscription and reconnects when it ends, so a frame never waits on
//! the network; [`ComputeSource::sample`] only reads what the thread has
//! seen. Memory is bounded by `pylon::field::Live`'s limits.
//!
//! A beacon past its validity draws as unknown. Nothing connects until the
//! field first asks, which is when the player enters Everglade, and the
//! thread stops when the source drops.
//!
//! The jobs in flight come from this computer's own client: the marks
//! `openagents pylon ask` leaves under the pylon home while a job runs
//! (`pylon::inflight`). They draw the beam to Alice's station.
//!
//! Native and feature-gated (`pylon-relay`, on with Verse's `desktop`
//! feature): the web and the phones leave the field dormant.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use pylon::field::{Live, RelayField};
use world_tree::{Family, PylonStatus, Tier};

use super::{ComputeSource, Market, PylonSample, Sample, ServiceSample, ThreadSample};

/// The pool the field draws.
pub const POOL: &str = "everglade";

/// The relay source.
pub struct RelaySource {
    /// Taken by the thread when it starts.
    field: Option<RelayField>,
    live: Arc<Mutex<Live>>,
    stop: Arc<AtomicBool>,
    /// The pylon home whose in-flight marks are this computer's jobs.
    home: Option<PathBuf>,
}

impl RelaySource {
    /// A field over `relay` for the [`POOL`] pool, reading jobs in flight
    /// from the pylon home `home` and verdicts from the checkers it trusts
    /// (`pylon::check::trusted`). The reader's key is a fresh one: reading
    /// beacons needs no identity of the owner's.
    #[must_use]
    pub fn new(relay: &str, home: Option<PathBuf>) -> Self {
        let checkers = home
            .as_deref()
            .map(pylon::check::trusted)
            .unwrap_or_default();
        Self {
            field: Some(
                RelayField::new(relay, Some(POOL), pylon::identity::Identity::generate())
                    .trusting(checkers.clone()),
            ),
            live: Arc::new(Mutex::new(
                Live::new(Some(POOL))
                    .trusting(checkers)
                    .trusting_brokers(pylon::market::brokers()),
            )),
            stop: Arc::new(AtomicBool::new(false)),
            home,
        }
    }

    /// The source as the environment names it: `VERSE_PYLON_RELAY`, else
    /// the production relay, and the pylon home (`OPENAGENTS_PYLON_HOME` or
    /// `~/.openagents/compute`). `None` when `VERSE_PYLON_RELAY` is `off`.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let relay = std::env::var("VERSE_PYLON_RELAY")
            .ok()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| pylon::DEFAULT_RELAY.to_string());
        (relay != "off").then(|| Self::new(&relay, Some(pylon::home())))
    }

    /// Whether the subscription has caught up with the relay.
    #[must_use]
    pub fn synced(&self) -> bool {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .synced
    }

    /// Starts the subscription thread the first time the field asks.
    fn start(&mut self) {
        let Some(field) = self.field.take() else {
            return;
        };
        let (live, stop) = (Arc::clone(&self.live), Arc::clone(&self.stop));
        let _ = std::thread::Builder::new()
            .name("pylon-field".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(field.watch(&live, &stop));
            });
    }
}

impl Drop for RelaySource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl ComputeSource for RelaySource {
    fn sample(&mut self, now: u64) -> Sample {
        self.start();
        let (pylons, rate, verified, market) = {
            let live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
            (
                live.pylons(now),
                live.rate(now),
                live.aggregate(now).is_some_and(|(_, verified)| verified),
                live.market(now),
            )
        };
        let mut pylons: Vec<PylonSample> = pylons.into_iter().map(|p| sample(p, now)).collect();
        // By address, so a pylon keeps its site as its beacon updates.
        pylons.sort_by(|a, b| a.id.cmp(&b.id));
        let in_flight = self
            .home
            .as_deref()
            .map(|home| pylon::inflight::read(home, now))
            .unwrap_or_default()
            .into_iter()
            .map(|job| job.pylon)
            .collect();
        Sample {
            pylons,
            wells: Vec::new(),
            rate,
            pool: POOL.into(),
            demo: false,
            in_flight,
            verified,
            market: market_sample(market),
        }
    }
}

/// The agent market as the Agora samples it.
#[must_use]
pub fn market_sample(market: pylon::field::Market) -> Market {
    let paid = &market.paid_msat;
    Market {
        services: market
            .listings
            .iter()
            .map(|l| ServiceSample {
                seller: pylon::identity::npub(&l.seller).chars().take(12).collect(),
                offer: l.offer.clone(),
                summary: l.summary.clone(),
                price_msat: l.price_msat,
                test: l.test(),
            })
            .collect(),
        jobs: market.jobs,
        sales: market.sales,
        paid_msat: [
            ("bitcoin", paid.bitcoin),
            ("testnet", paid.testnet),
            ("signet", paid.signet),
            ("regtest", paid.regtest),
        ]
        .into_iter()
        .filter(|(_, msat)| *msat > 0)
        .map(|(network, msat)| (network.to_string(), msat))
        .collect(),
        threads: market
            .threads
            .into_iter()
            .map(|t| ThreadSample {
                pylon: t.pylon,
                mainnet: t.mainnet,
            })
            .collect(),
    }
}

/// One verified pylon as the field samples it at `now`.
#[must_use]
pub fn sample(pylon: pylon::field::Pylon, now: u64) -> PylonSample {
    let state = pylon.state;
    let paid = &state.paid_msat;
    let paid_msat = [
        ("bitcoin", paid.bitcoin),
        ("testnet", paid.testnet),
        ("signet", paid.signet),
        ("regtest", paid.regtest),
    ]
    .into_iter()
    .filter(|(_, msat)| *msat > 0)
    .map(|(network, msat)| (network.to_string(), msat))
    .collect();
    let status = match state.status.as_str() {
        "online" => PylonStatus::Online,
        "draining" => PylonStatus::Draining,
        "offline" => PylonStatus::Offline,
        _ => PylonStatus::Unknown,
    };
    PylonSample {
        id: state.pylon,
        label: state.label,
        family: match state.family {
            nostr::pylon::Family::UnifiedMemory => Family::UnifiedMemory,
            nostr::pylon::Family::Gpu => Family::Gpu,
            nostr::pylon::Family::Cpu => Family::Cpu,
        },
        tier: match state.tier {
            nostr::pylon::Tier::Small => Tier::Small,
            nostr::pylon::Tier::Medium => Tier::Medium,
            nostr::pylon::Tier::Large => Tier::Large,
            nostr::pylon::Tier::Xl => Tier::Xl,
        },
        memory_gb: pylon.memory_gb,
        status,
        busy: state.busy,
        total: state.total,
        jobs: state.jobs,
        uptime: (status != PylonStatus::Unknown).then_some(state.uptime),
        observed_at: pylon.observed_at,
        owner: false,
        sigil: pylon.standing == nostr::pylon::Standing::Passing,
        paid_msat,
        coin: pylon
            .coin
            .and_then(|c| super::coin(c.finished_at, c.test, now)),
    }
}
