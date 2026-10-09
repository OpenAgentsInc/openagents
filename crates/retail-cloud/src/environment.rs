//! Saved customer environments (ENV-10): the
//! `openagents.cloud.retail-environment.v1` contract
//! (`docs/cloud/retail-environment-contract.md`) as data, its price book and
//! quote, and the funded lifecycle over the central ledger.
//!
//! A customer buys one environment setup: a setup machine, a clean builder,
//! and an independent verifier with its idempotence fork (at most
//! [`MACHINES`] machines, [`SECONDS_MAX`] wall time), and, when the checked
//! version is saved, its image kept for the retention days they chose.
//! The offer's maximum (every machine for the whole wall time, the
//! coordination charge, and the largest image for every retention day) is
//! held before anything starts. Settlement charges the measured machine
//! seconds, the coordination charge, and storage for the saved image's
//! real size; the rest of the hold is released, which is not a refund.
//! More retention is a prepaid renewal; when it runs out the version can no
//! longer be selected and its image is due for deletion.
//!
//! Nothing here sells anything. [`Gate::open`] stays shut until the owner
//! has reviewed this contract, published (not proposed) its price book, and
//! recorded a funded qualification; the checked-in book is proposed.
//! Every step is journaled before or after its one ledger effect so a
//! restart repeats nothing it cannot prove did not happen ([`recover`]).

use pay_ledger::Ledger;
use pay_ledger::compute::{HoldRequest, HoldState};
use route_contract::price_book::{CreditUnit, ModelPayer, QuotePayer, Settlement};
use route_contract::{Digest, digest_of};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::Source;
use crate::journal::Journal;
use crate::{Error, Result};

pub const CONTRACT: &str = "openagents.cloud.retail-environment.v1";
pub const COMPUTER_CLASS: &str = "retail-env-boat-large-v1";
pub const TASK_CLASS: &str = "retail-environment-setup-v1";
pub const BOOK_SCHEMA: &str = "openagents.cloud.environment-price-book.v1";
pub const QUOTE_SCHEMA: &str = "openagents.cloud.environment-quote.v1";
pub const ADMISSION_SCHEMA: &str = "openagents.cloud.environment-admission.v1";
/// Setup, builder, verifier, and the verifier's idempotence fork.
pub const MACHINES: u64 = 4;
/// Wall time for the whole setup, build, and check.
pub const SECONDS_MAX: u64 = 2 * 3600;
pub const OBJECTIVE_MAX: usize = 8 * 1024;
pub const PROFILE_MAX: usize = 64;
pub const CHECKS_MIN: usize = 1;
pub const CHECKS_MAX: usize = 8;
pub const CHECK_COMMAND_MAX: usize = 1024;
pub const RETENTION_DAYS_MIN: u64 = 1;
pub const RETENTION_DAYS_MAX: u64 = 90;
const DAY: i64 = 86_400;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS environment_purchase (
    id TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    bytes TEXT NOT NULL
);
";

// ---------------------------------------------------------------------------
// The request.

/// What a customer asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentRequest {
    pub source: Source,
    /// What the environment should be able to do, for the setup agent.
    pub objective: String,
    /// The qualification profile, for example `rust-library`.
    pub profile: String,
    /// Behavior checks the verifier runs, frozen before any build.
    pub checks: Vec<String>,
    /// Wall time for setup, build, and check, at most two hours.
    pub max_seconds: u64,
    /// Days the saved image is kept, prepaid.
    pub retention_days: u64,
    pub ceiling_sats: Option<u64>,
}

/// Why a request is outside the v1 class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsupported {
    /// Not a public `github.com` repository at a 40-digit commit.
    Source,
    Objective,
    Profile,
    Checks,
    WallTime,
    Retention,
}

impl EnvironmentRequest {
    /// # Errors
    ///
    /// The first [`Unsupported`] part.
    pub fn check(&self) -> std::result::Result<(), Unsupported> {
        if !self.source.supported() {
            return Err(Unsupported::Source);
        }
        if self.objective.trim().is_empty() || self.objective.len() > OBJECTIVE_MAX {
            return Err(Unsupported::Objective);
        }
        if self.profile.is_empty()
            || self.profile.len() > PROFILE_MAX
            || !self
                .profile
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(Unsupported::Profile);
        }
        if self.checks.len() < CHECKS_MIN
            || self.checks.len() > CHECKS_MAX
            || self
                .checks
                .iter()
                .any(|c| c.trim().is_empty() || c.len() > CHECK_COMMAND_MAX)
        {
            return Err(Unsupported::Checks);
        }
        if self.max_seconds == 0 || self.max_seconds > SECONDS_MAX {
            return Err(Unsupported::WallTime);
        }
        if !(RETENTION_DAYS_MIN..=RETENTION_DAYS_MAX).contains(&self.retention_days) {
            return Err(Unsupported::Retention);
        }
        Ok(())
    }
    #[must_use]
    pub fn digest(&self) -> String {
        digest_of(&(&self.objective, &self.profile, &self.checks)).to_string()
    }
}

// ---------------------------------------------------------------------------
// The price book and quote.

/// Whether the owner has reviewed a book. Only a published book sells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookStatus {
    Proposed,
    Published,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassPrice {
    pub computer: String,
    pub task: String,
    /// Each machine's metered rate.
    pub compute_msats_per_machine_second: u64,
    pub machines: u64,
    pub max_seconds: u64,
    /// Once per purchase whose first machine started.
    pub coordination_sats: u64,
    /// The saved image's storage, per started GB and day.
    pub storage_msats_per_gb_day: u64,
    /// The largest image this class saves.
    pub image_gb_max: u64,
    pub retention_days_max: u64,
    pub recipient: String,
    pub model: ModelPayer,
}

/// One environment price book. A change is a new `version`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceBook {
    pub schema: String,
    /// `retail-env-YYYY-MM-DD.N`.
    pub version: String,
    pub status: BookStatus,
    pub effective_at: u64,
    pub credit: CreditUnit,
    pub class: ClassPrice,
}

/// Why a book or a quote was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Refusal {
    /// Environment purchases are not open.
    Closed,
    Unsupported {
        part: Unsupported,
    },
    Malformed {
        detail: String,
    },
    AboveCeiling {
        max_sats: u64,
    },
    /// The quote no longer matches the book.
    Changed,
    /// The purchase belongs to another account.
    NotYours,
    /// The account's balance is shared with another service and cannot
    /// hold for an environment.
    SharedBalance,
    /// The saved image is larger than the class keeps.
    ImageTooLarge,
    /// The retention ran out; the version is no longer kept.
    Lapsed,
    /// The step does not fit the purchase's current state.
    Phase,
    /// The principal may not spend this account's balance.
    NoSpendRight,
}

fn ceil_msats(msats: u128) -> Option<u64> {
    u64::try_from(msats.div_ceil(1000)).ok()
}

impl PriceBook {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
    /// # Errors
    ///
    /// A malformed or ambiguous book.
    pub fn check(&self) -> std::result::Result<(), Refusal> {
        let bad = |d: &str| Refusal::Malformed { detail: d.into() };
        if self.schema != BOOK_SCHEMA {
            return Err(bad("expected openagents.cloud.environment-price-book.v1"));
        }
        if !self.version.starts_with("retail-env-") {
            return Err(bad("a version is retail-env-YYYY-MM-DD.N"));
        }
        if self.credit.sats != 1 || self.credit.name.is_empty() {
            return Err(bad("one credit is one sat"));
        }
        let c = &self.class;
        if c.computer != COMPUTER_CLASS || c.task != TASK_CLASS {
            return Err(bad("the book prices the v1 classes"));
        }
        if c.compute_msats_per_machine_second == 0
            || c.storage_msats_per_gb_day == 0
            || c.machines == 0
            || c.machines > MACHINES
            || c.max_seconds == 0
            || c.max_seconds > SECONDS_MAX
            || c.image_gb_max == 0
            || c.retention_days_max == 0
            || c.retention_days_max > RETENTION_DAYS_MAX
            || c.recipient.is_empty()
        {
            return Err(bad("every rate and bound is set and within the contract"));
        }
        if self.maximum(c.max_seconds, c.retention_days_max).is_none() {
            return Err(bad("the maximum does not fit 64 bits"));
        }
        Ok(())
    }
    fn compute(&self, machine_seconds: u64) -> Option<u64> {
        ceil_msats(
            u128::from(machine_seconds) * u128::from(self.class.compute_msats_per_machine_second),
        )
    }
    fn storage(&self, gb: u64, days: u64) -> Option<u64> {
        ceil_msats(
            u128::from(gb) * u128::from(days) * u128::from(self.class.storage_msats_per_gb_day),
        )
    }
    fn maximum(&self, seconds: u64, days: u64) -> Option<u64> {
        self.compute(seconds.checked_mul(self.class.machines)?)?
            .checked_add(self.class.coordination_sats)?
            .checked_add(self.storage(self.class.image_gb_max, days)?)
    }

    /// The quote for `request`.
    ///
    /// # Errors
    ///
    /// A malformed book, an unsupported request, or a maximum above the
    /// customer's ceiling.
    pub fn quote(&self, request: &EnvironmentRequest) -> std::result::Result<Quote, Refusal> {
        self.check()?;
        request
            .check()
            .map_err(|part| Refusal::Unsupported { part })?;
        let c = &self.class;
        if request.max_seconds > c.max_seconds || request.retention_days > c.retention_days_max {
            return Err(Refusal::Unsupported {
                part: if request.max_seconds > c.max_seconds {
                    Unsupported::WallTime
                } else {
                    Unsupported::Retention
                },
            });
        }
        let overflow = || Refusal::Malformed {
            detail: "the maximum does not fit 64 bits".into(),
        };
        let compute = self
            .compute(request.max_seconds * c.machines)
            .ok_or_else(overflow)?;
        let storage = self
            .storage(c.image_gb_max, request.retention_days)
            .ok_or_else(overflow)?;
        let max_sats = self
            .maximum(request.max_seconds, request.retention_days)
            .ok_or_else(overflow)?;
        if let Some(ceiling) = request.ceiling_sats
            && max_sats > ceiling
        {
            return Err(Refusal::AboveCeiling { max_sats });
        }
        let ModelPayer::CallerKey { provider } = &c.model;
        let to = Some(c.recipient.clone());
        Ok(Quote {
            schema: QUOTE_SCHEMA.into(),
            book: self.digest(),
            version: self.version.clone(),
            computer: c.computer.clone(),
            task: c.task.clone(),
            max_seconds: request.max_seconds,
            machines: c.machines,
            retention_days: request.retention_days,
            image_gb_max: c.image_gb_max,
            lines: vec![
                Line {
                    resource: Charge::Compute,
                    payer: QuotePayer::CallerBalance,
                    basis: LineBasis::PerMachineSecond {
                        msats: c.compute_msats_per_machine_second,
                    },
                    max_sats: compute,
                    recipient: to.clone(),
                },
                Line {
                    resource: Charge::Coordination,
                    payer: QuotePayer::CallerBalance,
                    basis: LineBasis::Fixed,
                    max_sats: c.coordination_sats,
                    recipient: to.clone(),
                },
                Line {
                    resource: Charge::Storage,
                    payer: QuotePayer::CallerBalance,
                    basis: LineBasis::PerGbDay {
                        msats: c.storage_msats_per_gb_day,
                    },
                    max_sats: storage,
                    recipient: to,
                },
                Line {
                    resource: Charge::Model,
                    payer: QuotePayer::CallerKey {
                        provider: provider.clone(),
                    },
                    basis: LineBasis::CallerKey,
                    max_sats: 0,
                    recipient: None,
                },
            ],
            max_sats,
            max_credits: max_sats,
        })
    }

    /// The prepaid charge for keeping a `gb` image `days` more days.
    ///
    /// # Errors
    ///
    /// Days out of bounds or an image above the class's largest.
    pub fn renewal(&self, gb: u64, days: u64) -> std::result::Result<u64, Refusal> {
        self.check()?;
        if !(RETENTION_DAYS_MIN..=self.class.retention_days_max).contains(&days) {
            return Err(Refusal::Unsupported {
                part: Unsupported::Retention,
            });
        }
        if gb == 0 || gb > self.class.image_gb_max {
            return Err(Refusal::ImageTooLarge);
        }
        self.storage(gb, days).ok_or(Refusal::Malformed {
            detail: "the renewal does not fit 64 bits".into(),
        })
    }
}

/// The checked-in book: proposed, so it sells nothing until the owner
/// publishes a reviewed one.
///
/// # Panics
///
/// Never: the fixture is checked in and tested.
#[must_use]
pub fn price_book() -> PriceBook {
    serde_json::from_str(include_str!("../fixtures/environment-price-book.json"))
        .expect("the checked-in environment price book parses")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Charge {
    Compute,
    Coordination,
    Storage,
    Model,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LineBasis {
    PerMachineSecond { msats: u64 },
    PerGbDay { msats: u64 },
    Fixed,
    CallerKey,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub resource: Charge,
    pub payer: QuotePayer,
    pub basis: LineBasis,
    pub max_sats: u64,
    pub recipient: Option<String>,
}

/// What an offer shows and the hold reserves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub schema: String,
    pub book: Digest,
    pub version: String,
    pub computer: String,
    pub task: String,
    pub max_seconds: u64,
    pub machines: u64,
    pub retention_days: u64,
    pub image_gb_max: u64,
    pub lines: Vec<Line>,
    pub max_sats: u64,
    pub max_credits: u64,
}

/// How a purchase ended, as settlement sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// No machine started.
    NotStarted,
    /// No machine became reachable.
    ProviderUnavailable,
    /// Machines ran; no version was saved (setup or checks failed, or the
    /// customer did not save).
    Ended,
    /// The customer cancelled after a machine started.
    Cancelled,
    /// The checked version was saved and its image is kept.
    Saved,
    /// Not known yet; the whole hold stays reserved.
    Unknown,
}

/// What the machines measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// The sum of every machine's billed seconds.
    pub machine_seconds: u64,
    /// The saved image's size, rounded up to a whole GB.
    pub image_gb: Option<u64>,
}

impl Quote {
    /// # Errors
    ///
    /// [`Refusal::Changed`] when the book moved.
    pub fn check(
        &self,
        book: &PriceBook,
        request: &EnvironmentRequest,
    ) -> std::result::Result<(), Refusal> {
        match book.quote(request) {
            Ok(fresh) if &fresh == self => Ok(()),
            _ => Err(Refusal::Changed),
        }
    }
    fn line(&self, resource: Charge) -> Option<&Line> {
        self.lines.iter().find(|l| l.resource == resource)
    }
    /// Settle the hold for `ending`, given what was measured.
    #[must_use]
    pub fn settle(&self, ending: Ending, usage: Option<Usage>) -> Settlement {
        let release = Settlement {
            charge_sats: Some(0),
            released_sats: self.max_sats,
            held_sats: 0,
        };
        let hold = Settlement {
            charge_sats: None,
            released_sats: 0,
            held_sats: self.max_sats,
        };
        let (Some(compute), Some(coordination), Some(storage)) = (
            self.line(Charge::Compute),
            self.line(Charge::Coordination),
            self.line(Charge::Storage),
        ) else {
            return hold;
        };
        match ending {
            Ending::NotStarted | Ending::ProviderUnavailable => release,
            Ending::Unknown => hold,
            Ending::Ended | Ending::Cancelled | Ending::Saved => {
                let Some(usage) = usage else {
                    return hold;
                };
                let LineBasis::PerMachineSecond { msats } = compute.basis else {
                    return hold;
                };
                let seconds = usage
                    .machine_seconds
                    .min(self.max_seconds.saturating_mul(self.machines));
                let compute = ceil_msats(u128::from(seconds) * u128::from(msats))
                    .unwrap_or(u64::MAX)
                    .min(compute.max_sats);
                let kept = if ending == Ending::Saved {
                    let (Some(gb), LineBasis::PerGbDay { msats }) =
                        (usage.image_gb, &storage.basis)
                    else {
                        return hold;
                    };
                    ceil_msats(
                        u128::from(gb.min(self.image_gb_max))
                            * u128::from(self.retention_days)
                            * u128::from(*msats),
                    )
                    .unwrap_or(u64::MAX)
                    .min(storage.max_sats)
                } else {
                    0
                };
                let charge = compute
                    .saturating_add(coordination.max_sats)
                    .saturating_add(kept)
                    .min(self.max_sats);
                Settlement {
                    charge_sats: Some(charge),
                    released_sats: self.max_sats - charge,
                    held_sats: 0,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Availability.

/// What must hold before anyone can buy: the owner reviewed this contract,
/// the book is the published one they reviewed (by digest), and a funded
/// qualification receipt is recorded. A fixture never opens it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub contract_reviewed: bool,
    /// The digest of the published book the owner reviewed.
    #[serde(default)]
    pub book: Option<Digest>,
    /// The funded qualification receipt's digest.
    #[serde(default)]
    pub qualification: Option<String>,
}
impl Gate {
    /// # Errors
    ///
    /// [`Refusal::Closed`] unless every condition holds for `book`.
    pub fn open(&self, book: &PriceBook) -> std::result::Result<(), Refusal> {
        let ok = self.contract_reviewed
            && book.status == BookStatus::Published
            && self.book.as_ref() == Some(&book.digest())
            && self.qualification.as_ref().is_some_and(|q| !q.is_empty())
            && book.check().is_ok();
        if ok { Ok(()) } else { Err(Refusal::Closed) }
    }
}

// ---------------------------------------------------------------------------
// Admission and the authority class.

/// Who may use a saved version: only the account that bought it, and only
/// by selecting it for that account's own tasks. No shell or terminal on
/// any machine, no publication, and no credential but the customer's own
/// OpenAI key for the setup agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub schema: String,
    pub contract: String,
    pub account: String,
    pub purchase: String,
    pub source: Source,
    pub request: String,
    pub computer_class: String,
    pub task_class: String,
    /// `customer:openai` only.
    pub credentials: Vec<String>,
    pub terminal: bool,
    pub publication: Vec<String>,
    /// The only account whose tasks may select the saved version.
    pub selectable_by: String,
    pub retention_days: u64,
    pub price_book: String,
    pub price_book_digest: Digest,
    pub max_charge_sats: u64,
}
impl Admission {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
}

// ---------------------------------------------------------------------------
// The funded lifecycle.

/// The image a saved version keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub environment: String,
    pub version: String,
    /// The provider's image identity.
    pub image_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub purchase: String,
    pub ending: Ending,
    pub usage: Option<Usage>,
    pub settlement: Settlement,
    pub charge_msat: Option<i64>,
    pub released_msat: i64,
    pub held_msat: i64,
    pub settled_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Renewal {
    pub id: String,
    pub days: u64,
    pub charge_msat: i64,
    pub at: i64,
}

/// How long a saved image is paid for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    pub saved: Saved,
    pub image_gb: u64,
    pub paid_until: i64,
    #[serde(default)]
    pub renewals: Vec<Renewal>,
    #[serde(default)]
    pub lapsed_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Phase {
    Offered,
    Confirmed {
        hold: String,
        at: i64,
    },
    Ended {
        hold: String,
        ending: Ending,
        usage: Option<Usage>,
        saved: Option<Saved>,
        at: i64,
    },
    Settled {
        hold: String,
        receipt: Receipt,
        saved: Option<Saved>,
    },
}

/// One purchase as the journal keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Purchase {
    pub id: String,
    pub account: String,
    pub request: EnvironmentRequest,
    pub quote: Quote,
    pub admission: Admission,
    /// What the customer confirms: the request, quote, and admission.
    pub digest: Digest,
    pub made_at: i64,
    pub phase: Phase,
    #[serde(default)]
    pub retention: Option<Retention>,
}

fn hold_id(purchase: &str) -> String {
    format!("env:{purchase}")
}
fn renewal_hold(purchase: &str, renewal: &str) -> String {
    format!("env:{purchase}:renew:{renewal}")
}
fn refused(r: Refusal) -> Error {
    Error::Environment(r)
}

fn read(journal: &Journal, id: &str) -> Result<Option<Purchase>> {
    let bytes: Option<String> = journal
        .connection
        .query_row(
            "SELECT bytes FROM environment_purchase WHERE id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(bytes.map(|b| serde_json::from_str(&b)).transpose()?)
}
fn write(journal: &mut Journal, p: &Purchase) -> Result<()> {
    let bytes = serde_json::to_string(p)?;
    let tx = journal.immediate()?;
    tx.execute(
        "INSERT INTO environment_purchase(id,account,bytes) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET bytes=excluded.bytes",
        params![p.id, p.account, bytes],
    )?;
    tx.commit()?;
    Ok(())
}

/// One purchase, only for its own account.
///
/// # Errors
///
/// [`Refusal::NotYours`] for another account's purchase.
pub fn purchase(journal: &Journal, account: &str, id: &str) -> Result<Option<Purchase>> {
    match read(journal, id)? {
        Some(p) if p.account != account => Err(refused(Refusal::NotYours)),
        other => Ok(other),
    }
}

/// Every purchase, oldest first.
///
/// # Errors
///
/// A journal failure.
pub fn purchases(journal: &Journal) -> Result<Vec<Purchase>> {
    let mut q = journal
        .connection
        .prepare("SELECT bytes FROM environment_purchase ORDER BY rowid")?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.iter().map(|b| Ok(serde_json::from_str(b)?)).collect()
}

/// Make (or return) the offer `id` for `request`.
///
/// # Errors
///
/// A closed gate, an unsupported request, a ceiling, or a retry with other
/// terms.
#[allow(clippy::too_many_arguments)]
pub fn offer(
    journal: &mut Journal,
    book: &PriceBook,
    gate: &Gate,
    account: &str,
    id: &str,
    request: &EnvironmentRequest,
    now: i64,
) -> Result<Purchase> {
    if account.is_empty() || id.is_empty() || id.len() > 128 {
        return Err(Error::Invalid("purchase identity"));
    }
    if let Some(existing) = read(journal, id)? {
        if existing.account != account || existing.request != *request {
            return Err(Error::Conflict(
                "the offer retry changed its identity or terms",
            ));
        }
        return Ok(existing);
    }
    gate.open(book).map_err(refused)?;
    let quote = book.quote(request).map_err(refused)?;
    let admission = Admission {
        schema: ADMISSION_SCHEMA.into(),
        contract: CONTRACT.into(),
        account: account.into(),
        purchase: id.into(),
        source: request.source.clone(),
        request: request.digest(),
        computer_class: quote.computer.clone(),
        task_class: quote.task.clone(),
        credentials: vec!["customer:openai".into()],
        terminal: false,
        publication: vec![],
        selectable_by: account.into(),
        retention_days: request.retention_days,
        price_book: quote.version.clone(),
        price_book_digest: quote.book.clone(),
        max_charge_sats: quote.max_sats,
    };
    let digest = digest_of(&(id, account, request, &quote, &admission));
    let p = Purchase {
        id: id.into(),
        account: account.into(),
        request: request.clone(),
        quote,
        admission,
        digest,
        made_at: now,
        phase: Phase::Offered,
        retention: None,
    };
    write(journal, &p)?;
    Ok(p)
}

fn hold_request(p: &Purchase, at: i64) -> HoldRequest {
    HoldRequest {
        id: hold_id(&p.id),
        account: p.account.clone(),
        quote: digest_of(&p.quote).to_string(),
        execution: hold_id(&p.id),
        terms: p.digest.to_string(),
        amount_msat: i64::try_from(p.quote.max_sats.saturating_mul(1000)).unwrap_or(i64::MAX),
        at,
    }
}

/// Confirm the displayed offer: hold its maximum before anything starts.
/// A retry returns the same hold; an insufficient balance holds nothing.
///
/// # Errors
///
/// Another account, a changed offer or book, a closed gate, no spend
/// right, a shared balance, or the ledger's refusal.
#[allow(clippy::too_many_arguments)]
pub fn confirm(
    journal: &mut Journal,
    ledger: &mut Ledger,
    book: &PriceBook,
    gate: &Gate,
    account: &str,
    id: &str,
    displayed: &Digest,
    spend: bool,
    now: i64,
) -> Result<Purchase> {
    let mut p = purchase(journal, account, id)?.ok_or(Error::Invalid("no such offer"))?;
    if &p.digest != displayed {
        return Err(Error::Conflict(
            "confirmation differs from the displayed offer",
        ));
    }
    if !matches!(p.phase, Phase::Offered) {
        return Ok(p);
    }
    if !spend {
        return Err(refused(Refusal::NoSpendRight));
    }
    gate.open(book).map_err(refused)?;
    p.quote.check(book, &p.request).map_err(refused)?;
    if ledger.shared_retail_binding(account)?.is_some() {
        return Err(refused(Refusal::SharedBalance));
    }
    let hold = ledger.reserve(&hold_request(&p, now))?;
    p.phase = Phase::Confirmed {
        hold: hold.request.id,
        at: now,
    };
    write(journal, &p)?;
    Ok(p)
}

/// Record how the purchase ended. An unknown ending keeps the whole hold;
/// a later known ending replaces it. A known ending never changes.
///
/// # Errors
///
/// A purchase that was never confirmed, a changed ending, or a saved
/// version without its image size or above the class's largest.
pub fn end(
    journal: &mut Journal,
    ledger: &mut Ledger,
    id: &str,
    ending: Ending,
    usage: Option<Usage>,
    saved: Option<Saved>,
    now: i64,
) -> Result<Purchase> {
    let mut p = read(journal, id)?.ok_or(Error::Invalid("no such purchase"))?;
    if (ending == Ending::Saved) != saved.is_some() {
        return Err(Error::Invalid("a saved ending names its version"));
    }
    if ending == Ending::Saved {
        match usage.and_then(|u| u.image_gb) {
            None => return Err(Error::Invalid("a saved ending measures its image")),
            Some(gb) if gb == 0 || gb > p.quote.image_gb_max => {
                return Err(refused(Refusal::ImageTooLarge));
            }
            Some(_) => {}
        }
    }
    let hold = match &p.phase {
        Phase::Confirmed { hold, .. } => hold.clone(),
        Phase::Ended {
            hold,
            ending: Ending::Unknown,
            ..
        } => hold.clone(),
        Phase::Ended {
            ending: e,
            usage: u,
            saved: s,
            ..
        } => {
            if *e == ending && *u == usage && *s == saved {
                return Ok(p);
            }
            return Err(Error::Conflict("the purchase already ended otherwise"));
        }
        Phase::Offered | Phase::Settled { .. } => return Err(refused(Refusal::Phase)),
    };
    if ending == Ending::Unknown {
        ledger.mark_hold_unknown(&hold)?;
    }
    p.phase = Phase::Ended {
        hold,
        ending,
        usage,
        saved,
        at: now,
    };
    write(journal, &p)?;
    Ok(p)
}

/// Settle a known ending once: charge what was measured, release the rest,
/// and start the saved image's paid retention. An unknown ending stays
/// held.
///
/// # Errors
///
/// A purchase that has not ended, or a ledger refusal.
pub fn settle(journal: &mut Journal, ledger: &mut Ledger, id: &str, now: i64) -> Result<Receipt> {
    let mut p = read(journal, id)?.ok_or(Error::Invalid("no such purchase"))?;
    let (hold, ending, usage, saved) = match &p.phase {
        Phase::Settled { receipt, .. } => return Ok(receipt.clone()),
        Phase::Ended {
            hold,
            ending,
            usage,
            saved,
            ..
        } => (hold.clone(), *ending, *usage, saved.clone()),
        _ => return Err(refused(Refusal::Phase)),
    };
    let settlement = p.quote.settle(ending, usage);
    let max_msat = i64::try_from(p.quote.max_sats * 1000).unwrap_or(i64::MAX);
    let Some(charge) = settlement.charge_sats else {
        return Ok(Receipt {
            purchase: p.id.clone(),
            ending,
            usage,
            settlement,
            charge_msat: None,
            released_msat: 0,
            held_msat: max_msat,
            settled_at: None,
        });
    };
    let charge_msat = i64::try_from(charge * 1000).unwrap_or(i64::MAX);
    let (settled, _) = ledger.settle_environment_hold(&hold, charge_msat, now)?;
    if settled.state != HoldState::Settled {
        return Err(Error::Invalid("the hold did not settle"));
    }
    let receipt = Receipt {
        purchase: p.id.clone(),
        ending,
        usage,
        settlement,
        charge_msat: Some(charge_msat),
        released_msat: max_msat - charge_msat,
        held_msat: 0,
        settled_at: settled.settled_at,
    };
    if let (Some(s), Some(gb)) = (&saved, usage.and_then(|u| u.image_gb)) {
        p.retention = Some(Retention {
            saved: s.clone(),
            image_gb: gb,
            paid_until: now + i64::try_from(p.quote.retention_days).unwrap_or(0) * DAY,
            renewals: vec![],
            lapsed_at: None,
        });
    }
    p.phase = Phase::Settled {
        hold,
        receipt: receipt.clone(),
        saved,
    };
    write(journal, &p)?;
    Ok(receipt)
}

/// Keep a saved image `days` more days, paid now from the balance.
/// The same renewal identity returns its first result.
///
/// # Errors
///
/// Another account, a lapsed or unsaved purchase, a closed gate, no spend
/// right, a changed renewal, or the ledger's refusal.
#[allow(clippy::too_many_arguments)]
pub fn renew(
    journal: &mut Journal,
    ledger: &mut Ledger,
    book: &PriceBook,
    gate: &Gate,
    account: &str,
    id: &str,
    renewal: &str,
    days: u64,
    spend: bool,
    now: i64,
) -> Result<Purchase> {
    let mut p = purchase(journal, account, id)?.ok_or(Error::Invalid("no such purchase"))?;
    let Some(retention) = p.retention.clone() else {
        return Err(refused(Refusal::Phase));
    };
    if let Some(r) = retention.renewals.iter().find(|r| r.id == renewal) {
        if r.days != days {
            return Err(Error::Conflict("the renewal retry changed its terms"));
        }
        return Ok(p);
    }
    if retention.lapsed_at.is_some() || retention.paid_until <= now {
        return Err(refused(Refusal::Lapsed));
    }
    if !spend {
        return Err(refused(Refusal::NoSpendRight));
    }
    gate.open(book).map_err(refused)?;
    if ledger.shared_retail_binding(account)?.is_some() {
        return Err(refused(Refusal::SharedBalance));
    }
    let sats = book.renewal(retention.image_gb, days).map_err(refused)?;
    let charge_msat = i64::try_from(sats * 1000).unwrap_or(i64::MAX);
    let hold = renewal_hold(id, renewal);
    let terms = digest_of(&(id, renewal, days, retention.image_gb, book.digest()));
    ledger.reserve(&HoldRequest {
        id: hold.clone(),
        account: account.into(),
        quote: book.digest().to_string(),
        execution: hold.clone(),
        terms: terms.to_string(),
        amount_msat: charge_msat,
        at: now,
    })?;
    ledger.settle_environment_hold(&hold, charge_msat, now)?;
    let r = p.retention.as_mut().expect("retention");
    r.paid_until += i64::try_from(days).unwrap_or(0) * DAY;
    r.renewals.push(Renewal {
        id: renewal.into(),
        days,
        charge_msat,
        at: now,
    });
    write(journal, &p)?;
    Ok(p)
}

/// A saved image whose retention ran out: the version is no longer
/// selectable and the image is due for deletion by its provider owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetireDue {
    pub purchase: String,
    pub account: String,
    pub saved: Saved,
}

/// Mark every saved image whose paid retention ended by `now` as lapsed.
///
/// # Errors
///
/// A journal failure.
pub fn lapse(journal: &mut Journal, now: i64) -> Result<Vec<RetireDue>> {
    let mut out = vec![];
    for mut p in purchases(journal)? {
        let Some(r) = p.retention.as_mut() else {
            continue;
        };
        if r.lapsed_at.is_none() && r.paid_until <= now {
            r.lapsed_at = Some(now);
            out.push(RetireDue {
                purchase: p.id.clone(),
                account: p.account.clone(),
                saved: r.saved.clone(),
            });
            write(journal, &p)?;
        }
    }
    Ok(out)
}

/// Whether `account`'s tasks may select the saved version now.
#[must_use]
pub fn may_select(p: &Purchase, account: &str, now: i64) -> bool {
    p.admission.selectable_by == account
        && p.account == account
        && matches!(p.phase, Phase::Settled { .. })
        && p.retention
            .as_ref()
            .is_some_and(|r| r.lapsed_at.is_none() && r.paid_until > now)
}

/// What one recovery visit did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Recovery {
    /// Offers whose hold the ledger already has: confirmed.
    pub confirmed: Vec<String>,
    /// Known endings settled.
    pub settled: Vec<String>,
    /// Still held for an unknown ending.
    pub held: Vec<String>,
    pub lapsed: Vec<RetireDue>,
}

/// After a restart: finish a confirmation whose hold exists, settle every
/// known ending once, keep unknown endings held, and lapse expired
/// retention. Nothing is reserved or charged twice.
///
/// # Errors
///
/// A journal or ledger failure.
pub fn recover(journal: &mut Journal, ledger: &mut Ledger, now: i64) -> Result<Recovery> {
    let mut out = Recovery::default();
    for mut p in purchases(journal)? {
        match &p.phase {
            Phase::Offered => {
                if let Some(h) = ledger.hold(&hold_id(&p.id))?
                    && h.request.terms == p.digest.to_string()
                {
                    p.phase = Phase::Confirmed {
                        hold: h.request.id,
                        at: h.request.at,
                    };
                    write(journal, &p)?;
                    out.confirmed.push(p.id.clone());
                }
            }
            Phase::Ended {
                ending: Ending::Unknown,
                ..
            } => out.held.push(p.id.clone()),
            Phase::Ended { .. } => {
                settle(journal, ledger, &p.id, now)?;
                out.settled.push(p.id.clone());
            }
            Phase::Confirmed { .. } | Phase::Settled { .. } => {}
        }
    }
    out.lapsed = lapse(journal, now)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_book_is_proposed_and_closed() {
        let book = price_book();
        book.check().unwrap();
        assert_eq!(book.status, BookStatus::Proposed);
        let gate = Gate {
            contract_reviewed: true,
            book: Some(book.digest()),
            qualification: Some("receipt".into()),
        };
        assert_eq!(gate.open(&book), Err(Refusal::Closed));
        let mut published = book.clone();
        published.status = BookStatus::Published;
        assert_eq!(
            gate.open(&published),
            Err(Refusal::Closed),
            "another digest"
        );
        let gate = Gate {
            book: Some(published.digest()),
            ..gate
        };
        gate.open(&published).unwrap();
        assert_eq!(Gate::default().open(&published), Err(Refusal::Closed));
    }
}
