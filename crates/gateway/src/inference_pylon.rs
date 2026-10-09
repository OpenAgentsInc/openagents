//! Pylon providers and local Psionic as inference upstreams
//! (`docs/inference/gateway.md`, sections 4 and 8, P2; #11070).
//!
//! - Local Psionic: `psionic-serve` at `PSIONIC_BASE_URL` serving
//!   `PSIONIC_MODELS`, as `local/<id>`, for requests that ask for
//!   `route.only: ["local"]`. Unset, it stays out of routing.
//! - Pylon providers: each `inference.pylons` registration becomes the
//!   upstream `pylon:<pylon>`. Jobs go over `inference.pylon_relay` (the
//!   OpenAgents relay by default) as NIP-CJ conversation jobs signed by
//!   the gateway's buyer key (`<registry>/inference/pylon-buyer.key`, made
//!   on first use; the provider allowlists it). What a provider earns is
//!   recorded in the split ledger `earnings.ledger` as the provider's
//!   share, converted to millisats at `inference.sats_rate`; the payout
//!   worker pays shares out over its rails, as for every other earning.
//!   Without the earnings ledger or the sats rate there is no way to pay a
//!   provider, so no Pylon upstream is mounted.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use inference::rates::SatsRate;
use inference::upstream::psionic::LocalPsionic;
use inference::upstream::pylon::{Done, Earning, Earnings, Job, Jobs, PylonUpstream, Registration};
use inference::upstream::{AttemptError, BoxFuture, ErrorClass, Upstream};

/// What a Pylon earning is recorded under in the split ledger.
pub const RESOURCE: &str = "openagents.inference.pylon.v1";

/// How long the gateway waits for a provider's result.
const JOB_WAIT: Duration = Duration::from_secs(120);

/// Local Psionic and the configured Pylon providers.
#[must_use]
pub fn upstreams(config: &crate::config::Config) -> Vec<Arc<dyn Upstream>> {
    let mut upstreams: Vec<Arc<dyn Upstream>> = vec![Arc::new(LocalPsionic::from_env())];
    let Some(inference) = &config.inference else {
        return upstreams;
    };
    if inference.pylons.is_empty() {
        return upstreams;
    }
    let (Some(earnings), Some(rate)) = (&config.earnings, &inference.sats_rate) else {
        eprintln!(
            "inference: Pylon providers are configured but not mounted: they need `earnings.ledger` \
             and `inference.sats_rate` to be paid"
        );
        return upstreams;
    };
    let ledger = match LedgerEarnings::open(&earnings.ledger, rate.clone()) {
        Ok(ledger) => Arc::new(ledger) as Arc<dyn Earnings>,
        Err(why) => {
            eprintln!("inference: Pylon providers not mounted: {why}");
            return upstreams;
        }
    };
    let key = config.registry.join("inference").join("pylon-buyer.key");
    let jobs = match NostrJobs::open(
        inference
            .pylon_relay
            .clone()
            .unwrap_or_else(|| pylon::DEFAULT_RELAY.to_owned()),
        &key,
        config.registry.join("inference").join("pylon"),
    ) {
        Ok(jobs) => Arc::new(jobs) as Arc<dyn Jobs>,
        Err(why) => {
            eprintln!("inference: Pylon providers not mounted: {why}");
            return upstreams;
        }
    };
    for registration in &inference.pylons {
        match PylonUpstream::new(registration.clone(), jobs.clone(), ledger.clone()) {
            Ok(upstream) => upstreams.push(Arc::new(upstream)),
            Err(why) => eprintln!("inference: Pylon provider not mounted: {why}"),
        }
    }
    upstreams
}

/// Millisats for `micros` of a dollar at `rate`, rounded to the nearest.
#[must_use]
pub fn msat(micros: u64, rate: &SatsRate) -> Option<i64> {
    // msat = micros / 1e6 USD * 1e11 msat per BTC / usd_per_btc.
    let by = u128::from(rate.usd_per_btc);
    if by == 0 {
        return None;
    }
    let scaled = u128::from(micros) * 100_000;
    i64::try_from((scaled + by / 2) / by).ok()
}

/// Provider earnings in the split ledger: one settlement per job, the
/// caller's price received from their balance (`debit:` key), the
/// provider's price as their share, OpenAgents keeping the margin.
pub struct LedgerEarnings {
    ledger: Mutex<pay_ledger::Ledger>,
    rate: SatsRate,
}

impl LedgerEarnings {
    /// The ledger at `path` (opened beside the gateway's own handle).
    ///
    /// # Errors
    ///
    /// A sentence when the ledger cannot be opened.
    pub fn open(path: &Path, rate: SatsRate) -> Result<Self, String> {
        let ledger = pay_ledger::Ledger::open(path)
            .map_err(|_| "the earnings ledger can't be opened".to_owned())?;
        Ok(Self {
            ledger: Mutex::new(ledger),
            rate,
        })
    }

    /// Over an open ledger, for tests.
    #[must_use]
    pub fn new(ledger: pay_ledger::Ledger, rate: SatsRate) -> Self {
        Self {
            ledger: Mutex::new(ledger),
            rate,
        }
    }

    /// The ledger, for reads.
    pub fn with<T>(&self, read: impl FnOnce(&mut pay_ledger::Ledger) -> T) -> Option<T> {
        self.ledger.lock().ok().map(|mut ledger| read(&mut ledger))
    }
}

impl Earnings for LedgerEarnings {
    fn earned(&self, earning: &Earning) -> Result<(), String> {
        let share = msat(earning.provider_micros, &self.rate).ok_or("no sats rate")?;
        let price = msat(earning.price_micros, &self.rate)
            .ok_or("no sats rate")?
            .max(share);
        if share <= 0 {
            // Less than a millisat: nothing to pay.
            return Ok(());
        }
        let settled_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| i64::try_from(span.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        let mut ledger = self
            .ledger
            .lock()
            .map_err(|_| "the earnings ledger is unavailable".to_owned())?;
        ledger
            .record_settlement(pay_ledger::SettlementInput {
                key: format!("debit:inference:{}", earning.request),
                resource: RESOURCE.to_owned(),
                plugin_id: None,
                release_id: None,
                price_msat: price,
                received_msat: price,
                rail: pay_ledger::Rail::Balance,
                payer_alias: None,
                settled_at,
                split: pay_ledger::Split::Earned {
                    beneficiary: earning.party.clone(),
                    amount_msat: share,
                    kind: pay_ledger::EarnedKind::Worker,
                },
            })
            .map(|_| ())
            .map_err(|error| format!("the earning was refused: {error}"))
    }
}

/// Jobs sent over a relay with the `pylon` crate's buyer.
pub struct NostrJobs {
    relay: String,
    buyer: pylon::identity::Identity,
    home: PathBuf,
}

impl NostrJobs {
    /// Jobs over `relay`, signed by the key at `key` (made if missing),
    /// with receipts kept under `home`.
    ///
    /// # Errors
    ///
    /// A sentence when the key cannot be read or made.
    pub fn open(relay: String, key: &Path, home: PathBuf) -> Result<Self, String> {
        if let Some(parent) = key.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| "the inference folder can't be made".to_owned())?;
        }
        std::fs::create_dir_all(&home)
            .map_err(|_| "the receipts folder can't be made".to_owned())?;
        let buyer = pylon::identity::Identity::load_or_create(key)?;
        Ok(Self { relay, buyer, home })
    }
}

impl Jobs for NostrJobs {
    fn run<'a>(
        &'a self,
        provider: &'a Registration,
        job: Job,
    ) -> BoxFuture<'a, Result<Done, AttemptError>> {
        Box::pin(async move {
            let ask = pylon::client::Ask {
                relay: self.relay.clone(),
                pylon: Some(provider.provider.clone()),
                prompt: job.task,
                wait: JOB_WAIT,
                publish_receipt: true,
                home: self.home.clone(),
                checkers: std::collections::BTreeSet::new(),
                pay: None,
            };
            let history: Vec<pylon::engine::Turn> = job
                .history
                .into_iter()
                .map(|turn| pylon::engine::Turn {
                    role: turn.role,
                    content: turn.content,
                })
                .collect();
            let answer = pylon::client::ask_conversation(
                &self.buyer,
                &ask,
                &history,
                job.instructions.as_deref(),
            )
            .await
            .map_err(|why| AttemptError::new(ErrorClass::Connection, why))?;
            let Some(text) = answer.text else {
                let error = answer.error.unwrap_or_else(|| "no answer".to_owned());
                let class = if error.starts_with("no answer within") {
                    ErrorClass::Timeout
                } else {
                    ErrorClass::Upstream
                };
                return Err(AttemptError::new(class, error));
            };
            let count = |field: &str| {
                answer
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.get(field))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
            };
            Ok(Done {
                text,
                input_tokens: count("input"),
                output_tokens: count("output"),
                request: answer.request,
                receipt: answer.receipt,
            })
        })
    }
}
