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
//!   Without the earnings ledger or the sats rate there is no way to pay
//!   such a provider, so it is not mounted.
//! - Merchant-settled Pylon providers (#11149): a registration with
//!   `settlement.mode = "merchant"` names the provider's own node key
//!   (`pay_to`) and the most one job may cost. The gateway buys each job
//!   before sending it, paying the provider's own invoice from
//!   `inference.pylon_payer` (the resident wallet, under the owner's
//!   standing grant on `bitcoin`), and pays no invoice that is not payable
//!   to that pinned key. The sats land at the provider's node: no ledger
//!   share is recorded and no payout follows. Without `pylon_payer` such a
//!   provider is not mounted.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use inference::rates::SatsRate;
use inference::upstream::psionic::LocalPsionic;
use inference::upstream::pylon::{
    Done, Earning, Earnings, Job, Jobs, PylonUpstream, Registration, Settlement,
};
use inference::upstream::{AttemptError, BoxFuture, ErrorClass, Upstream};
use pylon::paid::{Grant, Granted, Invoice, Network, Payer};

/// What a Pylon earning is recorded under in the split ledger.
pub const RESOURCE: &str = "openagents.inference.pylon.v1";

/// How long the gateway waits for a provider's result.
const JOB_WAIT: Duration = Duration::from_secs(120);

/// How long one direct payment to a provider may take.
const PAY_WAIT: Duration = Duration::from_secs(60);

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
    let ledger = match (&config.earnings, &inference.sats_rate) {
        (Some(earnings), Some(rate)) => {
            match LedgerEarnings::open(&earnings.ledger, rate.clone()) {
                Ok(ledger) => Some(Arc::new(ledger) as Arc<dyn Earnings>),
                Err(why) => {
                    eprintln!("inference: the earnings ledger for Pylon providers: {why}");
                    None
                }
            }
        }
        _ => None,
    };
    let home = config.registry.join("inference").join("pylon");
    let payer =
        inference
            .pylon_payer
            .as_ref()
            .and_then(|payer| match merchant_payer(payer, &home) {
                Ok(payer) => Some(payer),
                Err(why) => {
                    eprintln!("inference: the wallet for Pylon providers: {why}");
                    None
                }
            });
    let key = config.registry.join("inference").join("pylon-buyer.key");
    let mut jobs = match NostrJobs::open(
        inference
            .pylon_relay
            .clone()
            .unwrap_or_else(|| pylon::DEFAULT_RELAY.to_owned()),
        &key,
        home,
    ) {
        Ok(jobs) => jobs,
        Err(why) => {
            eprintln!("inference: Pylon providers not mounted: {why}");
            return upstreams;
        }
    };
    let mut mounted = Vec::new();
    for registration in &inference.pylons {
        let earnings = match &registration.settlement {
            Settlement::Ledger => {
                let Some(ledger) = &ledger else {
                    eprintln!(
                        "inference: Pylon provider {} not mounted: it is paid through the split \
                         ledger, which needs `earnings.ledger` and `inference.sats_rate`",
                        registration.pylon
                    );
                    continue;
                };
                ledger.clone()
            }
            Settlement::Merchant { pay_to, .. } => {
                let Some(payer) = &payer else {
                    eprintln!(
                        "inference: Pylon provider {} not mounted: it is paid at its own \
                         receiver, which needs `inference.pylon_payer`",
                        registration.pylon
                    );
                    continue;
                };
                jobs.pay(
                    &registration.pylon,
                    Arc::new(Pinned::new(payer.clone(), pay_to)),
                );
                Arc::new(PaidDirectly) as Arc<dyn Earnings>
            }
        };
        mounted.push((registration.clone(), earnings));
    }
    let jobs: Arc<dyn Jobs> = Arc::new(jobs);
    for (registration, earnings) in mounted {
        match PylonUpstream::new(registration, jobs.clone(), earnings) {
            Ok(upstream) => upstreams.push(Arc::new(upstream)),
            Err(why) => eprintln!("inference: Pylon provider not mounted: {why}"),
        }
    }
    upstreams
}

/// The payer for merchant-settled providers: the resident wallet, under
/// the owner's standing grant (`grant.json` in `home`) on `bitcoin`.
///
/// # Errors
///
/// A network x402 does not name, or a grant file that does not parse.
pub fn merchant_payer(
    config: &crate::config::PylonPayer,
    home: &Path,
) -> Result<Arc<dyn Payer>, String> {
    let network = Network::parse(&config.network)
        .filter(|network| network.x402().is_some())
        .ok_or("inference.pylon_payer.network must be bitcoin or testnet.")?;
    let grant = Grant::load(home)?;
    let wallet = WalletPayer {
        wallet_home: config.wallet_home.clone(),
        payer_node: config.payer_node.clone(),
        network,
        max_fee_msat: config.max_fee_msat,
    };
    Ok(Arc::new(Granted::new(Arc::new(wallet), grant, home)))
}

/// A merchant-settled provider has no ledger share: it was paid at its own
/// node before its job was sent. Recording one would pay it twice.
pub struct PaidDirectly;

impl Earnings for PaidDirectly {
    fn earned(&self, _earning: &Earning) -> Result<(), String> {
        Err("this provider is paid at its own receiver, not through the split ledger".into())
    }
}

/// A payer that pays only invoices payable to one provider's registered
/// node, for exactly the amount the challenge names.
pub struct Pinned {
    inner: Arc<dyn Payer>,
    pay_to: String,
}

impl Pinned {
    #[must_use]
    pub fn new(inner: Arc<dyn Payer>, pay_to: &str) -> Self {
        Self {
            inner,
            pay_to: pay_to.to_owned(),
        }
    }
}

impl Payer for Pinned {
    fn network(&self) -> Network {
        self.inner.network()
    }

    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        let decoded = nostr::x402::decode_invoice(&invoice.bolt11)
            .map_err(|_| "the provider's invoice does not decode; nothing was paid")?;
        if hex::encode(decoded.payee()) != self.pay_to {
            return Err(
                "the provider's invoice is not payable to its registered node; nothing was paid"
                    .into(),
            );
        }
        if decoded.amount_msat() != invoice.amount_msat
            || hex::encode(decoded.payment_hash()) != invoice.payment_hash
        {
            return Err(
                "the provider's invoice is not the one it offered; nothing was paid".into(),
            );
        }
        self.inner.pay(invoice)
    }
}

/// The resident wallet as a payer, reached on every payment the way the
/// x402 receiver reaches it, so a restarted wallet is picked up.
pub struct WalletPayer {
    pub wallet_home: PathBuf,
    pub payer_node: String,
    pub network: Network,
    pub max_fee_msat: u64,
}

impl Payer for WalletPayer {
    fn network(&self) -> Network {
        self.network
    }

    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        use openagents_wallet::LightningWallet;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = std::fs::symlink_metadata(&self.wallet_home)
            .map_err(|_| "The wallet is not reachable.".to_owned())?;
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 || metadata.uid() != uid
        {
            return Err("The wallet's home is not private to this user.".into());
        }
        let wallet = openagents_wallet::resident::RemoteWallet::probe(&self.wallet_home)
            .ok_or("The wallet is not running.")?;
        wallet
            .pay_from_node(
                &self.payer_node,
                &invoice.bolt11,
                self.max_fee_msat,
                PAY_WAIT,
            )
            .map(|proof| proof.preimage)
            .map_err(|error| error.to_string())
    }
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
    /// Who pays each merchant-settled provider, by pylon slug.
    payers: HashMap<String, Arc<dyn Payer>>,
}

impl NostrJobs {
    /// Pay `pylon`'s jobs with `payer`, buying each one before it is sent.
    pub fn pay(&mut self, pylon: &str, payer: Arc<dyn Payer>) {
        self.payers.insert(pylon.to_owned(), payer);
    }

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
        Ok(Self {
            relay,
            buyer,
            home,
            payers: HashMap::new(),
        })
    }
}

impl Jobs for NostrJobs {
    fn run<'a>(
        &'a self,
        provider: &'a Registration,
        job: Job,
    ) -> BoxFuture<'a, Result<Done, AttemptError>> {
        Box::pin(async move {
            // A merchant-settled provider's job is bought first, paid to
            // its own node; a ledger-paid one is sent without terms.
            let pay = match &provider.settlement {
                Settlement::Ledger => None,
                Settlement::Merchant { max_msat, .. } => {
                    let payer = self.payers.get(&provider.pylon).cloned().ok_or_else(|| {
                        AttemptError::new(
                            ErrorClass::Upstream,
                            "no wallet is set to pay this provider",
                        )
                    })?;
                    Some(pylon::client::Pay::Wallet {
                        payer,
                        max_msat: *max_msat,
                    })
                }
            };
            let ask = pylon::client::Ask {
                relay: self.relay.clone(),
                pylon: Some(provider.provider.clone()),
                prompt: job.task,
                wait: JOB_WAIT,
                publish_receipt: true,
                home: self.home.clone(),
                checkers: std::collections::BTreeSet::new(),
                pay,
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
                paid_msat: answer.paid_msat,
            })
        })
    }
}
