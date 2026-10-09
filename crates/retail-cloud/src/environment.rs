//! Saved customer environments (ENV-10): the
//! `openagents.cloud.retail-environment.v2` contract
//! (`docs/cloud/retail-environment-contract.md`) as data, the subscription
//! plan that pays for it, and the metered lifecycle.
//!
//! Environments come with a monthly subscription ([`EnvironmentPlan`],
//! "Pro"): each subscription month includes a number of machine-hours on
//! the standard machine, a number of machines at once, and saved image
//! storage. A setup's measured machine-seconds count against the month's
//! included hours. When they run out, more hours are charged only if the
//! person turned on extra hours ([`ExtraHours`]), from their credits, and
//! never past the monthly cap they set. Unused hours do not roll over.
//! Model usage stays on the person's own key or subscription.
//!
//! There is no per-run time limit from us: a run stops when the person's
//! own limit ([`EnvironmentRequest::max_seconds`]), their included hours, or
//! their extra-hours cap runs out ([`budget`]), never at a limit we chose.
//!
//! Nothing here sells anything. [`Gate::open`] stays shut until the owner
//! has reviewed the contract, published (not proposed) the plan, and
//! recorded a funded qualification; the checked-in plan is proposed.
//! Subscription months arrive through [`record_period`] (the billing
//! side's job), and extra-hour charges leave through an outbox of
//! [`Debit`]s that [`post_debits`] hands to a [`Credits`] adapter once
//! each, so a restart never charges twice.

use route_contract::{Digest, digest_of};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::Source;
use crate::journal::Journal;
use crate::{Error, Result};

pub const CONTRACT: &str = "openagents.cloud.retail-environment.v2";
pub const COMPUTER_CLASS: &str = "retail-env-standard-v1";
pub const TASK_CLASS: &str = "retail-environment-setup-v1";
pub const PLAN_SCHEMA: &str = "openagents.cloud.environment-plan.v1";
pub const TERMS_SCHEMA: &str = "openagents.cloud.environment-terms.v1";
pub const ADMISSION_SCHEMA: &str = "openagents.cloud.environment-admission.v2";
pub const OBJECTIVE_MAX: usize = 8 * 1024;
pub const PROFILE_MAX: usize = 64;
pub const CHECKS_MIN: usize = 1;
pub const CHECKS_MAX: usize = 8;
pub const CHECK_COMMAND_MAX: usize = 1024;
/// Saved versions stay this many days after a subscription ends, so a
/// person who renews late finds them again.
pub const KEEP_AFTER_END_DAYS: i64 = 30;
const DAY: i64 = 86_400;
const HOUR: u128 = 3600;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS environment_purchase (
    id TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    bytes TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS environment_record (
    kind TEXT NOT NULL,
    key TEXT NOT NULL,
    account TEXT NOT NULL,
    bytes TEXT NOT NULL,
    PRIMARY KEY (kind, key)
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
    /// The person's own limit on machine-seconds for this setup. `None`
    /// means no limit of theirs; we set none.
    #[serde(default)]
    pub max_seconds: Option<u64>,
}

/// Why a request is outside the class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsupported {
    /// Not a public `github.com` repository at a 40-digit commit.
    Source,
    Objective,
    Profile,
    Checks,
    /// A limit of zero seconds.
    WallTime,
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
        if self.max_seconds == Some(0) {
            return Err(Unsupported::WallTime);
        }
        Ok(())
    }
    #[must_use]
    pub fn digest(&self) -> String {
        digest_of(&(&self.objective, &self.profile, &self.checks)).to_string()
    }
}

// ---------------------------------------------------------------------------
// The plan.

/// Whether the owner has published a plan. Only a published plan sells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Proposed,
    Published,
}

/// The machine every environment runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    pub class: String,
    pub vcpus: u32,
    pub memory_gb: u32,
}

/// Who pays for model use: always the person, on their own key or
/// subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    BringYourOwn,
}

/// The subscription plan environments come with. A change is a new
/// `version`; a purchase keeps the terms it was offered under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentPlan {
    pub schema: String,
    /// `env-plan-YYYY-MM-DD.N`.
    pub version: String,
    pub status: PlanStatus,
    /// The billing catalog's plan id (`GET /v1/plans`).
    pub plan: String,
    pub name: String,
    /// What one month costs, in millionths of a US dollar.
    pub price_usd_micros: u64,
    pub machine: Machine,
    /// Machine-hours each subscription month includes.
    pub included_machine_hours: u64,
    /// Machines that may run at once.
    pub machines_at_once: u64,
    /// Saved environment images, in GB.
    pub storage_gb: u64,
    /// Saved versions kept at once.
    pub saved_versions: u64,
    /// Each machine-hour past the month's included hours, in millionths of
    /// a US dollar, charged only when the person turned extra hours on.
    pub extra_hour_usd_micros: u64,
    /// Whether unused hours carry into the next month.
    pub rollover: bool,
    pub model: ModelSource,
}

impl EnvironmentPlan {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
    /// # Errors
    ///
    /// A malformed plan.
    pub fn check(&self) -> std::result::Result<(), Refusal> {
        let bad = |d: &str| Refusal::Malformed { detail: d.into() };
        if self.schema != PLAN_SCHEMA {
            return Err(bad("expected openagents.cloud.environment-plan.v1"));
        }
        if !self.version.starts_with("env-plan-") {
            return Err(bad("a version is env-plan-YYYY-MM-DD.N"));
        }
        if self.machine.class != COMPUTER_CLASS {
            return Err(bad("the plan names the standard machine class"));
        }
        if self.plan.is_empty()
            || self.name.is_empty()
            || self.price_usd_micros == 0
            || self.machine.vcpus == 0
            || self.machine.memory_gb == 0
            || self.included_machine_hours == 0
            || self.machines_at_once == 0
            || self.storage_gb == 0
            || self.saved_versions == 0
            || self.extra_hour_usd_micros == 0
        {
            return Err(bad("every price and allowance is set"));
        }
        if self.rollover {
            return Err(bad("unused hours do not roll over in this contract"));
        }
        Ok(())
    }
    #[must_use]
    pub fn included_seconds(&self) -> u64 {
        self.included_machine_hours.saturating_mul(3600)
    }
}

/// The checked-in plan: proposed, so it sells nothing until the owner
/// publishes it.
///
/// # Panics
///
/// Never: the fixture is checked in and tested.
#[must_use]
pub fn plan() -> EnvironmentPlan {
    serde_json::from_str(include_str!("../fixtures/environment-plan.json"))
        .expect("the checked-in environment plan parses")
}

/// Why a step was refused. [`Refusal::message`] is what a person reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Refusal {
    /// Environments are not open on this service.
    Closed,
    /// The account has no current subscription.
    NoPlan,
    Unsupported {
        part: Unsupported,
    },
    Malformed {
        detail: String,
    },
    /// The offer no longer matches the plan.
    Changed,
    /// The purchase belongs to another account.
    NotYours,
    /// The month's included hours are used and extra hours are off.
    AllowanceUsed {
        hours: u64,
        resets_at: i64,
    },
    /// Extra hours are on, and this month's cap is spent.
    CapReached {
        cap_usd_micros: u64,
        resets_at: i64,
    },
    /// Every machine the plan allows at once is in use.
    MachinesBusy {
        machines: u64,
    },
    /// Saving this image would pass the plan's storage.
    StorageFull {
        gb: u64,
    },
    /// The plan's saved versions are all in use.
    TooManyVersions {
        versions: u64,
    },
    /// The step does not fit the purchase's current state.
    Phase,
    /// The principal may not spend for this account.
    NoSpendRight,
}

impl Refusal {
    /// The plain sentence a person sees.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Closed => "Saved environments aren't available yet.".into(),
            Self::NoPlan => "Saved environments come with the Pro plan.".into(),
            Self::Unsupported { .. } | Self::Malformed { .. } => {
                "That setup request isn't one we can run.".into()
            }
            Self::Changed => "The plan changed since you looked. Check it again.".into(),
            Self::NotYours => "That environment isn't yours.".into(),
            Self::AllowanceUsed { hours, resets_at } => format!(
                "You've used this month's {hours} hours. Turn on extra hours in Settings, or wait until {}.",
                day_label(*resets_at)
            ),
            Self::CapReached {
                cap_usd_micros,
                resets_at,
            } => format!(
                "You've reached the {} you set for extra hours this month. Raise it in Settings, or wait until {}.",
                usd(*cap_usd_micros),
                day_label(*resets_at)
            ),
            Self::MachinesBusy { machines } => format!(
                "Your {machines} machines are busy. Wait for a setup to finish, then try again."
            ),
            Self::StorageFull { gb } => format!(
                "Your saved environments would use more than {gb} GB. Delete one to save this one."
            ),
            Self::TooManyVersions { versions } => {
                format!("You have {versions} saved versions. Delete one to save another.")
            }
            Self::Phase => "That can't be done at this step.".into(),
            Self::NoSpendRight => "This sign-in can't start paid work.".into(),
        }
    }
}

/// `$20`, `$0.18`.
#[must_use]
pub fn usd(micros: u64) -> String {
    let cents = micros.div_ceil(10_000);
    if cents % 100 == 0 {
        format!("${}", cents / 100)
    } else {
        format!("${}.{:02}", cents / 100, cents % 100)
    }
}

/// `November 9`, in UTC.
#[must_use]
pub fn day_label(at: i64) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    // Howard Hinnant's civil_from_days.
    let z = at.div_euclid(DAY) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let index = usize::try_from(month - 1).unwrap_or(0).min(11);
    format!("{} {day}", MONTHS[index])
}

// ---------------------------------------------------------------------------
// Availability.

/// What must hold before anyone can buy: the owner reviewed the contract,
/// the plan is the published one they reviewed (by digest), and a funded
/// qualification receipt is recorded. A fixture never opens it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub contract_reviewed: bool,
    /// The digest of the published plan the owner reviewed.
    #[serde(default)]
    pub plan: Option<Digest>,
    /// The funded qualification receipt's digest.
    #[serde(default)]
    pub qualification: Option<String>,
}
impl Gate {
    /// # Errors
    ///
    /// [`Refusal::Closed`] unless every condition holds for `plan`.
    pub fn open(&self, plan: &EnvironmentPlan) -> std::result::Result<(), Refusal> {
        let ok = self.contract_reviewed
            && plan.status == PlanStatus::Published
            && self.plan.as_ref() == Some(&plan.digest())
            && self.qualification.as_ref().is_some_and(|q| !q.is_empty())
            && plan.check().is_ok();
        if ok { Ok(()) } else { Err(Refusal::Closed) }
    }
}

// ---------------------------------------------------------------------------
// Records beside the purchases: subscription months, settings, monthly
// meters, and the debit outbox.

const PERIOD: &str = "period";
const SETTINGS: &str = "settings";
const MONTH: &str = "month";
const DEBIT: &str = "debit";
const CREDITS: &str = "credits";
const NOTICE: &str = "notice";

fn get<T: for<'de> Deserialize<'de>>(
    journal: &Journal,
    kind: &str,
    key: &str,
) -> Result<Option<T>> {
    let bytes: Option<String> = journal
        .connection
        .query_row(
            "SELECT bytes FROM environment_record WHERE kind=? AND key=?",
            [kind, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(bytes.map(|b| serde_json::from_str(&b)).transpose()?)
}
fn all<T: for<'de> Deserialize<'de>>(
    journal: &Journal,
    kind: &str,
    account: Option<&str>,
) -> Result<Vec<T>> {
    let mut q = journal.connection.prepare(
        "SELECT bytes FROM environment_record WHERE kind=?1 AND (?2 IS NULL OR account=?2) ORDER BY rowid",
    )?;
    let rows = q
        .query_map(params![kind, account], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.iter().map(|b| Ok(serde_json::from_str(b)?)).collect()
}
fn put<T: Serialize>(
    journal: &mut Journal,
    kind: &str,
    key: &str,
    account: &str,
    value: &T,
) -> Result<()> {
    let bytes = serde_json::to_string(value)?;
    let tx = journal.immediate()?;
    tx.execute(
        "INSERT INTO environment_record(kind,key,account,bytes) VALUES(?,?,?,?) ON CONFLICT(kind,key) DO UPDATE SET bytes=excluded.bytes",
        params![kind, key, account, bytes],
    )?;
    tx.commit()?;
    Ok(())
}

/// One paid subscription month for an account, as billing reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub account: String,
    /// The plan version the month was paid under.
    pub plan: String,
    pub start: i64,
    pub end: i64,
}

/// Record a paid month (idempotent on account and start).
///
/// # Errors
///
/// An empty or backward period, or a journal failure.
pub fn record_period(journal: &mut Journal, period: &Period) -> Result<()> {
    if period.account.is_empty() || period.end <= period.start {
        return Err(Error::Invalid("a period has an account and a length"));
    }
    let key = format!("{}:{}", period.account, period.start);
    put(journal, PERIOD, &key, &period.account, period)
}

/// Cut the account's month that holds `at` short at `at`: the
/// subscription ended before the month did (cancelled now, not at the
/// month's end). Returns whether a month was shortened. Seconds already
/// counted stay counted.
///
/// # Errors
///
/// A journal failure.
pub fn end_period(journal: &mut Journal, account: &str, at: i64) -> Result<bool> {
    let periods: Vec<Period> = all(journal, PERIOD, Some(account))?;
    let Some(mut p) = periods.into_iter().find(|p| p.start <= at && at < p.end) else {
        return Ok(false);
    };
    let key = format!("{}:{}", p.account, p.start);
    p.end = at.max(p.start + 1);
    put(journal, PERIOD, &key, account, &p)?;
    Ok(true)
}

/// Name the credits account (the billing workspace) that pays this
/// account's extra hours. Billing records it with each paid month.
///
/// # Errors
///
/// An empty name, or a journal failure.
pub fn set_credits_account(journal: &mut Journal, account: &str, credits: &str) -> Result<()> {
    if account.is_empty() || credits.is_empty() {
        return Err(Error::Invalid("an account and its credits account"));
    }
    put(journal, CREDITS, account, account, &credits)
}

/// The credits account that pays this account's extra hours, if billing
/// named one.
///
/// # Errors
///
/// A journal failure.
pub fn credits_account(journal: &Journal, account: &str) -> Result<Option<String>> {
    get(journal, CREDITS, account)
}

/// A plain fact about the account's Stripe payments that Settings says
/// out loud (refunds and disputes, #11074).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notice {
    /// The bank opened a dispute on a payment: the paid month runs on, and
    /// no new month starts until it closes.
    Dispute,
    /// The last payment was refunded in full, so the month ended.
    Refunded,
    /// A dispute closed against the payment, so the month ended.
    DisputeLost,
}

/// Say (or, with `None`, clear) the account's payment notice.
///
/// # Errors
///
/// An empty account, or a journal failure.
pub fn set_notice(journal: &mut Journal, account: &str, notice: Option<Notice>) -> Result<()> {
    if account.is_empty() {
        return Err(Error::Invalid("an account"));
    }
    put(journal, NOTICE, account, account, &notice)
}

/// The account's payment notice, if one stands.
///
/// # Errors
///
/// A journal failure.
pub fn notice(journal: &Journal, account: &str) -> Result<Option<Notice>> {
    Ok(get::<Option<Notice>>(journal, NOTICE, account)?.flatten())
}

/// The account whose extra hours the credits account pays (the reverse of
/// [`credits_account`]); billing knows the workspace, the meter the account.
///
/// # Errors
///
/// A journal failure.
pub fn account_paid_by(journal: &Journal, credits: &str) -> Result<Option<String>> {
    let bytes = serde_json::to_string(credits)?;
    Ok(journal
        .connection
        .query_row(
            "SELECT account FROM environment_record WHERE kind=? AND bytes=? ORDER BY rowid DESC LIMIT 1",
            [CREDITS, bytes.as_str()],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

/// Where an account's subscription stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "standing", rename_all = "snake_case")]
pub enum Standing {
    Active { period: Period },
    Ended { at: i64 },
    Never,
}

/// The account's subscription at `now`.
///
/// # Errors
///
/// A journal failure.
pub fn standing(journal: &Journal, account: &str, now: i64) -> Result<Standing> {
    let periods: Vec<Period> = all(journal, PERIOD, Some(account))?;
    if let Some(p) = periods.iter().find(|p| p.start <= now && now < p.end) {
        return Ok(Standing::Active { period: p.clone() });
    }
    Ok(periods
        .iter()
        .filter(|p| p.end <= now)
        .map(|p| p.end)
        .max()
        .map_or(Standing::Never, |at| Standing::Ended { at }))
}

fn active(journal: &Journal, account: &str, now: i64) -> Result<Period> {
    match standing(journal, account, now)? {
        Standing::Active { period } => Ok(period),
        _ => Err(refused(Refusal::NoPlan)),
    }
}

/// The person's extra-hours choice. Off until they turn it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraHours {
    pub enabled: bool,
    /// The most extra hours may cost in one month, in millionths of a US
    /// dollar.
    pub cap_usd_micros: u64,
}

/// The account's extra-hours choice.
///
/// # Errors
///
/// A journal failure.
pub fn extra_hours(journal: &Journal, account: &str) -> Result<ExtraHours> {
    Ok(get(journal, SETTINGS, account)?.unwrap_or_default())
}

/// Save the account's extra-hours choice.
///
/// # Errors
///
/// An empty account, or a journal failure.
pub fn set_extra_hours(journal: &mut Journal, account: &str, choice: ExtraHours) -> Result<()> {
    if account.is_empty() {
        return Err(Error::Invalid("an account"));
    }
    put(journal, SETTINGS, account, account, &choice)
}

/// One account's meter for one subscription month.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Month {
    pub start: i64,
    pub end: i64,
    /// Machine-seconds counted against the included hours.
    pub included_seconds: u64,
    /// Machine-seconds past the included hours.
    pub extra_seconds: u64,
    /// What extra hours cost this month, never past the cap.
    pub extra_usd_micros: u64,
    /// Purchases already counted, so a replay counts nothing twice.
    pub purchases: Vec<String>,
}

fn month_key(account: &str, start: i64) -> String {
    format!("{account}:{start}")
}

fn month_of(journal: &Journal, period: &Period) -> Result<Month> {
    Ok(
        get(journal, MONTH, &month_key(&period.account, period.start))?.unwrap_or(Month {
            start: period.start,
            end: period.end,
            ..Month::default()
        }),
    )
}

/// One extra-hours charge waiting for, or already given to, the credits
/// ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Debit {
    /// `env:<purchase>`: the ledger's idempotency key.
    pub key: String,
    pub account: String,
    pub usd_micros: u64,
    /// The machine-seconds past the included hours this charge pays for.
    #[serde(default)]
    pub extra_seconds: u64,
    pub at: i64,
    #[serde(default)]
    pub posted_at: Option<i64>,
}

/// The account's credits. `debit` must be idempotent on `debit.key`: the
/// same key twice is one charge.
pub trait Credits {
    /// # Errors
    ///
    /// The ledger could not take the debit now; it is offered again later.
    fn debit(&mut self, debit: &Debit) -> std::result::Result<(), String>;
}

/// Hand every waiting debit to `credits` once. A failed debit stays waiting.
///
/// # Errors
///
/// A journal failure.
pub fn post_debits(
    journal: &mut Journal,
    credits: &mut dyn Credits,
    now: i64,
) -> Result<Vec<String>> {
    let mut posted = vec![];
    for mut d in all::<Debit>(journal, DEBIT, None)? {
        if d.posted_at.is_some() {
            continue;
        }
        if credits.debit(&d).is_ok() {
            d.posted_at = Some(now);
            put(journal, DEBIT, &d.key.clone(), &d.account.clone(), &d)?;
            posted.push(d.key);
        }
    }
    Ok(posted)
}

/// Every debit, oldest first.
///
/// # Errors
///
/// A journal failure.
pub fn debits(journal: &Journal) -> Result<Vec<Debit>> {
    all(journal, DEBIT, None)
}

/// How much more the account may run this month.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Budget {
    pub included_left_seconds: u64,
    /// Seconds the person's extra-hours cap still pays for; zero when off.
    pub extra_left_seconds: u64,
    pub resets_at: i64,
}

impl Budget {
    #[must_use]
    pub fn seconds(&self) -> u64 {
        self.included_left_seconds
            .saturating_add(self.extra_left_seconds)
    }
}

fn budget_of(
    included_seconds: u64,
    extra_hour_usd_micros: u64,
    month: &Month,
    extra: ExtraHours,
) -> Budget {
    let included_left_seconds = included_seconds.saturating_sub(month.included_seconds);
    let extra_left_seconds = if extra.enabled && extra_hour_usd_micros > 0 {
        let left = u128::from(extra.cap_usd_micros.saturating_sub(month.extra_usd_micros));
        u64::try_from(left * HOUR / u128::from(extra_hour_usd_micros)).unwrap_or(u64::MAX)
    } else {
        0
    };
    Budget {
        included_left_seconds,
        extra_left_seconds,
        resets_at: month.end,
    }
}

/// The account's remaining machine-seconds this month: the runner stops
/// machines when this reaches zero.
///
/// # Errors
///
/// [`Refusal::NoPlan`], or a journal failure.
pub fn budget(
    journal: &Journal,
    plan: &EnvironmentPlan,
    account: &str,
    now: i64,
) -> Result<Budget> {
    let period = active(journal, account, now)?;
    let month = month_of(journal, &period)?;
    Ok(budget_of(
        plan.included_seconds(),
        plan.extra_hour_usd_micros,
        &month,
        extra_hours(journal, account)?,
    ))
}

fn spent(plan: &EnvironmentPlan, b: &Budget, extra: ExtraHours) -> Refusal {
    if extra.enabled {
        Refusal::CapReached {
            cap_usd_micros: extra.cap_usd_micros,
            resets_at: b.resets_at,
        }
    } else {
        Refusal::AllowanceUsed {
            hours: plan.included_machine_hours,
            resets_at: b.resets_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Terms, admission, and the purchase.

/// What an offer shows: the plan terms the setup runs under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub schema: String,
    pub plan: Digest,
    pub version: String,
    pub computer: String,
    pub task: String,
    pub machine: Machine,
    pub machines_at_once: u64,
    pub included_machine_hours: u64,
    pub extra_hour_usd_micros: u64,
    /// The person's own limit, if they set one.
    pub max_seconds: Option<u64>,
}

impl Terms {
    fn of(plan: &EnvironmentPlan, request: &EnvironmentRequest) -> Self {
        Self {
            schema: TERMS_SCHEMA.into(),
            plan: plan.digest(),
            version: plan.version.clone(),
            computer: plan.machine.class.clone(),
            task: TASK_CLASS.into(),
            machine: plan.machine.clone(),
            machines_at_once: plan.machines_at_once,
            included_machine_hours: plan.included_machine_hours,
            extra_hour_usd_micros: plan.extra_hour_usd_micros,
            max_seconds: request.max_seconds,
        }
    }
}

/// Who may use a saved version: only the account that made it, and only
/// by selecting it for that account's own tasks. No shell or terminal on
/// any machine, no publication, and no credential but the person's own
/// model key.
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
    /// `customer:model` only.
    pub credentials: Vec<String>,
    pub terminal: bool,
    pub publication: Vec<String>,
    /// The only account whose tasks may select the saved version.
    pub selectable_by: String,
    pub plan: String,
    pub plan_digest: Digest,
}
impl Admission {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
}

/// The image a saved version keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub environment: String,
    pub version: String,
    /// The provider's image identity.
    pub image_id: String,
}

/// How a purchase ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// No machine started.
    NotStarted,
    /// No machine became reachable.
    ProviderUnavailable,
    /// Machines ran; no version was saved.
    Ended,
    /// The person cancelled after a machine started.
    Cancelled,
    /// The checked version was saved and its image is kept.
    Saved,
    /// Not known yet; nothing is counted until it is.
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

/// What settlement counted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub purchase: String,
    pub ending: Ending,
    pub usage: Option<Usage>,
    /// Seconds taken from the month's included hours.
    pub included_seconds: u64,
    /// Seconds past them.
    pub extra_seconds: u64,
    /// What those extra seconds cost from credits, never past the cap.
    pub extra_usd_micros: u64,
    pub settled_at: i64,
}

/// A saved version and its image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
    pub saved: Saved,
    pub image_gb: u64,
    pub kept_at: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Phase {
    Offered,
    Confirmed {
        at: i64,
        /// The subscription month the setup counts against.
        period: Period,
    },
    Ended {
        period: Period,
        ending: Ending,
        usage: Option<Usage>,
        saved: Option<Saved>,
        at: i64,
    },
    Settled {
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
    pub terms: Terms,
    pub admission: Admission,
    /// What the customer confirms: the request, terms, and admission.
    pub digest: Digest,
    pub made_at: i64,
    pub phase: Phase,
    #[serde(default)]
    pub kept: Option<Kept>,
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

fn of_account(journal: &Journal, account: &str) -> Result<Vec<Purchase>> {
    Ok(purchases(journal)?
        .into_iter()
        .filter(|p| p.account == account)
        .collect())
}

/// The account's saved images now: total GB and versions.
///
/// # Errors
///
/// A journal failure.
pub fn storage(journal: &Journal, account: &str) -> Result<(u64, u64)> {
    let kept: Vec<Kept> = of_account(journal, account)?
        .into_iter()
        .filter_map(|p| p.kept)
        .filter(|k| k.deleted_at.is_none())
        .collect();
    Ok((kept.iter().map(|k| k.image_gb).sum(), kept.len() as u64))
}

/// Make (or return) the offer `id` for `request`.
///
/// # Errors
///
/// A closed gate, no subscription, a used-up month, an unsupported
/// request, or a retry with other terms.
#[allow(clippy::too_many_arguments)]
pub fn offer(
    journal: &mut Journal,
    plan: &EnvironmentPlan,
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
    gate.open(plan).map_err(refused)?;
    plan.check().map_err(refused)?;
    request
        .check()
        .map_err(|part| refused(Refusal::Unsupported { part }))?;
    let left = budget(journal, plan, account, now)?;
    if left.seconds() == 0 {
        return Err(refused(spent(plan, &left, extra_hours(journal, account)?)));
    }
    let terms = Terms::of(plan, request);
    let admission = Admission {
        schema: ADMISSION_SCHEMA.into(),
        contract: CONTRACT.into(),
        account: account.into(),
        purchase: id.into(),
        source: request.source.clone(),
        request: request.digest(),
        computer_class: terms.computer.clone(),
        task_class: terms.task.clone(),
        credentials: vec!["customer:model".into()],
        terminal: false,
        publication: vec![],
        selectable_by: account.into(),
        plan: terms.version.clone(),
        plan_digest: terms.plan.clone(),
    };
    let digest = digest_of(&(id, account, request, &terms, &admission));
    let p = Purchase {
        id: id.into(),
        account: account.into(),
        request: request.clone(),
        terms,
        admission,
        digest,
        made_at: now,
        phase: Phase::Offered,
        kept: None,
    };
    write(journal, &p)?;
    Ok(p)
}

/// Confirm the displayed offer: the setup may start. Nothing is held: the
/// setup runs on the month's included hours, then on extra hours only if
/// the person turned them on.
///
/// # Errors
///
/// Another account, a changed offer or plan, a closed gate, no spend
/// right, no subscription, a used-up month, or busy machines.
#[allow(clippy::too_many_arguments)]
pub fn confirm(
    journal: &mut Journal,
    plan: &EnvironmentPlan,
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
    gate.open(plan).map_err(refused)?;
    if p.terms != Terms::of(plan, &p.request) {
        return Err(refused(Refusal::Changed));
    }
    let period = active(journal, account, now)?;
    let left = budget(journal, plan, account, now)?;
    if left.seconds() == 0 {
        return Err(refused(spent(plan, &left, extra_hours(journal, account)?)));
    }
    let running = of_account(journal, account)?
        .iter()
        .filter(|o| {
            o.id != p.id
                && matches!(
                    o.phase,
                    Phase::Confirmed { .. }
                        | Phase::Ended {
                            ending: Ending::Unknown,
                            ..
                        }
                )
        })
        .count() as u64;
    // A setup runs at most the plan's machines at once, so a second setup
    // waits for the first.
    if running > 0 {
        return Err(refused(Refusal::MachinesBusy {
            machines: plan.machines_at_once,
        }));
    }
    p.phase = Phase::Confirmed { at: now, period };
    write(journal, &p)?;
    Ok(p)
}

/// Record how the purchase ended. An unknown ending counts nothing yet; a
/// later known ending replaces it. A known ending never changes.
///
/// # Errors
///
/// A purchase that was never confirmed, a changed ending, or a saved
/// version without its size or past the plan's storage or versions.
pub fn end(
    journal: &mut Journal,
    plan: &EnvironmentPlan,
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
    let period = match &p.phase {
        Phase::Confirmed { period, .. } => period.clone(),
        Phase::Ended {
            period,
            ending: Ending::Unknown,
            ..
        } => period.clone(),
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
    if ending == Ending::Saved {
        let Some(gb) = usage.and_then(|u| u.image_gb).filter(|gb| *gb > 0) else {
            return Err(Error::Invalid("a saved ending measures its image"));
        };
        let (used_gb, versions) = storage(journal, &p.account)?;
        if versions >= plan.saved_versions {
            return Err(refused(Refusal::TooManyVersions {
                versions: plan.saved_versions,
            }));
        }
        if used_gb.saturating_add(gb) > plan.storage_gb {
            return Err(refused(Refusal::StorageFull {
                gb: plan.storage_gb,
            }));
        }
    }
    p.phase = Phase::Ended {
        period,
        ending,
        usage,
        saved,
        at: now,
    };
    write(journal, &p)?;
    Ok(p)
}

/// Settle a known ending once: count its machine-seconds against the
/// month's included hours, charge any past them as extra hours (only when
/// turned on, never past the cap), and keep a saved image. An unknown
/// ending waits.
///
/// # Errors
///
/// A purchase that has not ended or is still unknown, or a journal failure.
pub fn settle(journal: &mut Journal, id: &str, now: i64) -> Result<Receipt> {
    let mut p = read(journal, id)?.ok_or(Error::Invalid("no such purchase"))?;
    let (period, ending, usage, saved) = match &p.phase {
        Phase::Settled { receipt, .. } => return Ok(receipt.clone()),
        Phase::Ended {
            ending: Ending::Unknown,
            ..
        } => return Err(refused(Refusal::Phase)),
        Phase::Ended {
            period,
            ending,
            usage,
            saved,
            ..
        } => (period.clone(), *ending, *usage, saved.clone()),
        _ => return Err(refused(Refusal::Phase)),
    };
    let seconds = match ending {
        Ending::NotStarted | Ending::ProviderUnavailable => 0,
        _ => usage
            .map(|u| u.machine_seconds)
            .ok_or(Error::Invalid("a run that started measures its seconds"))?,
    };
    let mut month = month_of(journal, &period)?;
    let extra = extra_hours(journal, &p.account)?;
    let left = budget_of(
        p.terms.included_machine_hours.saturating_mul(3600),
        p.terms.extra_hour_usd_micros,
        &month,
        extra,
    );
    let included = seconds.min(left.included_left_seconds);
    let past = seconds - included;
    let extra_usd_micros = if extra.enabled {
        let full = u128::from(past) * u128::from(p.terms.extra_hour_usd_micros);
        u64::try_from(full.div_ceil(HOUR))
            .unwrap_or(u64::MAX)
            .min(extra.cap_usd_micros.saturating_sub(month.extra_usd_micros))
    } else {
        0
    };
    if !month.purchases.contains(&p.id) {
        month.included_seconds += included;
        month.extra_seconds += past;
        month.extra_usd_micros += extra_usd_micros;
        month.purchases.push(p.id.clone());
        put(
            journal,
            MONTH,
            &month_key(&p.account, period.start),
            &p.account.clone(),
            &month,
        )?;
    }
    if extra_usd_micros > 0 {
        let key = format!("env:{}", p.id);
        if get::<Debit>(journal, DEBIT, &key)?.is_none() {
            put(
                journal,
                DEBIT,
                &key,
                &p.account.clone(),
                &Debit {
                    key: key.clone(),
                    account: p.account.clone(),
                    usd_micros: extra_usd_micros,
                    extra_seconds: past,
                    at: now,
                    posted_at: None,
                },
            )?;
        }
    }
    let receipt = Receipt {
        purchase: p.id.clone(),
        ending,
        usage,
        included_seconds: included,
        extra_seconds: past,
        extra_usd_micros,
        settled_at: now,
    };
    if let (Some(s), Some(gb)) = (&saved, usage.and_then(|u| u.image_gb)) {
        p.kept = Some(Kept {
            saved: s.clone(),
            image_gb: gb,
            kept_at: now,
            deleted_at: None,
        });
    }
    p.phase = Phase::Settled {
        receipt: receipt.clone(),
        saved,
    };
    write(journal, &p)?;
    Ok(receipt)
}

/// A saved image that is no longer kept: its provider owner deletes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetireDue {
    pub purchase: String,
    pub account: String,
    pub saved: Saved,
}

/// The person deletes a saved version to free its storage.
///
/// # Errors
///
/// Another account's purchase, nothing saved, or a journal failure.
pub fn delete(journal: &mut Journal, account: &str, id: &str, now: i64) -> Result<RetireDue> {
    let mut p = purchase(journal, account, id)?.ok_or(Error::Invalid("no such purchase"))?;
    let Some(kept) = p.kept.as_mut() else {
        return Err(refused(Refusal::Phase));
    };
    if kept.deleted_at.is_none() {
        kept.deleted_at = Some(now);
    }
    let due = RetireDue {
        purchase: p.id.clone(),
        account: p.account.clone(),
        saved: kept.saved.clone(),
    };
    write(journal, &p)?;
    Ok(due)
}

/// Retire every saved image whose account's subscription ended at least
/// [`KEEP_AFTER_END_DAYS`] ago.
///
/// # Errors
///
/// A journal failure.
pub fn retire(journal: &mut Journal, now: i64) -> Result<Vec<RetireDue>> {
    let mut out = vec![];
    for mut p in purchases(journal)? {
        let Some(kept) = p.kept.as_ref() else {
            continue;
        };
        if kept.deleted_at.is_some() {
            continue;
        }
        let ended = match standing(journal, &p.account, now)? {
            Standing::Active { .. } => continue,
            Standing::Ended { at } => at,
            Standing::Never => kept.kept_at,
        };
        if ended + KEEP_AFTER_END_DAYS * DAY > now {
            continue;
        }
        let kept = p.kept.as_mut().expect("kept");
        kept.deleted_at = Some(now);
        out.push(RetireDue {
            purchase: p.id.clone(),
            account: p.account.clone(),
            saved: kept.saved.clone(),
        });
        write(journal, &p)?;
    }
    Ok(out)
}

/// Whether `account`'s tasks may select the saved version now: it is
/// theirs, still kept, and their subscription is current.
///
/// # Errors
///
/// A journal failure.
pub fn may_select(journal: &Journal, p: &Purchase, account: &str, now: i64) -> Result<bool> {
    Ok(p.admission.selectable_by == account
        && p.account == account
        && matches!(p.phase, Phase::Settled { .. })
        && p.kept.as_ref().is_some_and(|k| k.deleted_at.is_none())
        && matches!(standing(journal, account, now)?, Standing::Active { .. }))
}

/// What one recovery visit did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Recovery {
    /// Known endings settled.
    pub settled: Vec<String>,
    /// Still waiting for an unknown ending.
    pub held: Vec<String>,
    pub retired: Vec<RetireDue>,
}

/// After a restart: settle every known ending once, leave unknown endings
/// waiting, and retire images past their keep. Nothing is counted or
/// charged twice.
///
/// # Errors
///
/// A journal failure.
pub fn recover(journal: &mut Journal, now: i64) -> Result<Recovery> {
    let mut out = Recovery::default();
    for p in purchases(journal)? {
        match &p.phase {
            Phase::Ended {
                ending: Ending::Unknown,
                ..
            } => out.held.push(p.id.clone()),
            Phase::Ended { .. } => {
                settle(journal, &p.id, now)?;
                out.settled.push(p.id.clone());
            }
            Phase::Offered | Phase::Confirmed { .. } | Phase::Settled { .. } => {}
        }
    }
    out.retired = retire(journal, now)?;
    Ok(out)
}

/// What Settings shows for one account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub standing: Standing,
    /// Machine-seconds used this month, included and extra.
    pub used_seconds: u64,
    pub included_seconds: u64,
    pub extra: ExtraHours,
    pub extra_spent_usd_micros: u64,
    pub storage_gb: u64,
    pub versions: u64,
    /// A refund or dispute Settings says plainly.
    pub notice: Option<Notice>,
}

/// The account's plan, month, and storage at `now`.
///
/// # Errors
///
/// A journal failure.
pub fn summary(
    journal: &Journal,
    plan: &EnvironmentPlan,
    account: &str,
    now: i64,
) -> Result<Summary> {
    let standing = standing(journal, account, now)?;
    let month = match &standing {
        Standing::Active { period } => month_of(journal, period)?,
        _ => Month::default(),
    };
    let (storage_gb, versions) = storage(journal, account)?;
    Ok(Summary {
        standing,
        used_seconds: month.included_seconds + month.extra_seconds,
        included_seconds: plan.included_seconds(),
        extra: extra_hours(journal, account)?,
        extra_spent_usd_micros: month.extra_usd_micros,
        storage_gb,
        versions,
        notice: notice(journal, account)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_plan_is_proposed_and_closed() {
        let plan = plan();
        plan.check().unwrap();
        assert_eq!(plan.status, PlanStatus::Proposed);
        assert_eq!(plan.price_usd_micros, 20_000_000);
        assert_eq!(plan.included_machine_hours, 100);
        assert_eq!(plan.machines_at_once, 2);
        assert_eq!((plan.machine.vcpus, plan.machine.memory_gb), (2, 8));
        assert_eq!((plan.storage_gb, plan.saved_versions), (20, 10));
        assert_eq!(plan.extra_hour_usd_micros, 180_000);
        assert!(!plan.rollover);
        let gate = Gate {
            contract_reviewed: true,
            plan: Some(plan.digest()),
            qualification: Some("receipt".into()),
        };
        assert_eq!(gate.open(&plan), Err(Refusal::Closed));
        let mut published = plan.clone();
        published.status = PlanStatus::Published;
        assert_eq!(
            gate.open(&published),
            Err(Refusal::Closed),
            "another digest"
        );
        let gate = Gate {
            plan: Some(published.digest()),
            ..gate
        };
        gate.open(&published).unwrap();
        assert_eq!(Gate::default().open(&published), Err(Refusal::Closed));
    }

    #[test]
    fn messages_are_plain() {
        // 2026-11-09T00:00:00Z.
        let nov9 = 1_794_182_400;
        assert_eq!(day_label(nov9), "November 9");
        assert_eq!(usd(180_000), "$0.18");
        assert_eq!(usd(20_000_000), "$20");
        assert_eq!(
            Refusal::AllowanceUsed {
                hours: 100,
                resets_at: nov9
            }
            .message(),
            "You've used this month's 100 hours. Turn on extra hours in Settings, or wait until November 9."
        );
        assert_eq!(
            Refusal::CapReached {
                cap_usd_micros: 10_000_000,
                resets_at: nov9
            }
            .message(),
            "You've reached the $10 you set for extra hours this month. Raise it in Settings, or wait until November 9."
        );
    }
}
