//! The bounded funded-qualification plan and runner (#10723).
//!
//! A [`Plan`] pins everything one qualification may touch: the customer
//! account and principal, the top-up invoice, the computer and task class,
//! the source, the provider disclosure, the spending ceiling, the cleanup
//! deadline, and the independent check. [`Plan::check`] refuses anything
//! outside the v1 contract. [`run_fake`] runs the plan end to end on fakes
//! and must pass before real funding is enabled. [`run_funded`] is the
//! owner's step: this build has no live wallet or Boat binding, so it
//! refuses and says what is missing rather than claim a funded outcome.

use retail_cloud::authority::Source;
use retail_cloud::contract::{self, TaskRequest};
use retail_cloud::dispatch::{self, CheckRun, ExecutorEnd, TaskStatus};
use retail_cloud::{Error, Result, recover, retain, settle};
use route_contract::price_book::Ending;
use route_contract::snapshot::Recipient;
use serde::{Deserialize, Serialize};

use crate::harness::{World, rights};

pub const PLAN_SCHEMA: &str = "openagents.cloud.retail-qualification-plan.v1";
pub const RECEIPT_SCHEMA: &str = "openagents.cloud.retail-qualification.v1";
/// The most a qualification may buy.
pub const TOP_UP_MAX_SATS: u64 = 1_000;

/// One bounded qualification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    /// A test account, never the owner's personal one.
    pub account: String,
    pub top_up_sats: u64,
    pub computer_class: String,
    pub task_class: String,
    pub price_book: String,
    pub source: Source,
    pub task: String,
    /// The independent check, run read-only on the exact candidate.
    pub check: String,
    pub max_seconds: u64,
    pub ceiling_sats: u64,
    /// Who sees the source and the task.
    pub disclosure: Vec<Recipient>,
    /// The sandbox must be gone this many seconds after confirmation.
    pub cleanup_deadline_seconds: i64,
}

/// Why a plan is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PlanRefusal {
    Schema,
    /// The top-up is zero or more than 1,000 sats.
    TopUp,
    /// Another class or price book than the v1 contract's.
    Class,
    /// The request is outside the v1 task class.
    Request,
    /// The quote's maximum is above the ceiling, or the ceiling is above
    /// the top-up.
    Ceiling,
    /// The disclosure differs from the contract's recipients.
    Disclosure,
    /// The cleanup deadline does not cover the task and its replacement.
    Cleanup,
}

impl Plan {
    /// The plan's digest, which the funded step must name.
    #[must_use]
    pub fn digest(&self) -> String {
        route_contract::digest_of(self).to_string()
    }

    fn request(&self) -> TaskRequest {
        TaskRequest {
            source: self.source.clone(),
            task: self.task.clone(),
            checks: vec![self.check.clone()],
            max_seconds: self.max_seconds,
            ceiling_sats: Some(self.ceiling_sats),
        }
    }

    /// Check the plan against the v1 contract and the price book.
    ///
    /// # Errors
    ///
    /// The first [`PlanRefusal`].
    pub fn check(&self) -> std::result::Result<(), PlanRefusal> {
        if self.schema != PLAN_SCHEMA {
            return Err(PlanRefusal::Schema);
        }
        if self.top_up_sats == 0 || self.top_up_sats > TOP_UP_MAX_SATS {
            return Err(PlanRefusal::TopUp);
        }
        let book = contract::price_book();
        if self.computer_class != contract::COMPUTER_CLASS
            || self.task_class != contract::TASK_CLASS
            || self.price_book != book.version
        {
            return Err(PlanRefusal::Class);
        }
        self.request().check().map_err(|_| PlanRefusal::Request)?;
        let placement = route_contract::price_book::Placement::Retail {
            computer: self.computer_class.clone(),
            task: self.task_class.clone(),
        };
        match book.quote(&placement, self.max_seconds, Some(self.ceiling_sats)) {
            Ok(Some(_)) if self.ceiling_sats <= self.top_up_sats => {}
            _ => return Err(PlanRefusal::Ceiling),
        }
        if self.disclosure != contract::recipients() {
            return Err(PlanRefusal::Disclosure);
        }
        let needed = i64::try_from(self.max_seconds).unwrap_or(i64::MAX)
            + retail_cloud::provision::READY_DEADLINE_SECS * 2;
        if self.cleanup_deadline_seconds < needed {
            return Err(PlanRefusal::Cleanup);
        }
        Ok(())
    }
}

/// How the qualification ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Fake wallet, Boat, sandbox, and task owner.
    Fake,
    /// Real funds and a real sandbox: only with the owner.
    Funded,
}

/// What the qualification retained.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationReceipt {
    pub schema: String,
    pub mode: Mode,
    /// Plain words about what this receipt does and does not prove.
    pub label: String,
    pub plan: String,
    pub qualified: bool,
    pub failure: Option<String>,
    pub account: String,
    pub invoice_payment_hash: Option<String>,
    pub execution: Option<String>,
    pub hold: Option<String>,
    pub sandbox: Option<String>,
    pub task: Option<String>,
    pub check: Option<String>,
    pub settlement_source: Option<String>,
    pub charge_msat: Option<i64>,
    pub released_msat: Option<i64>,
    /// Still held because a cost is unknown.
    pub unknown_held_msat: i64,
    pub provider_seconds: Option<u64>,
    pub teardown_acknowledged: bool,
    pub ledger_conserved: bool,
}

impl QualificationReceipt {
    fn empty(plan: &Plan, mode: Mode, label: &str) -> Self {
        Self {
            schema: RECEIPT_SCHEMA.into(),
            mode,
            label: label.into(),
            plan: plan.digest(),
            qualified: false,
            failure: None,
            account: plan.account.clone(),
            invoice_payment_hash: None,
            execution: None,
            hold: None,
            sandbox: None,
            task: None,
            check: None,
            settlement_source: None,
            charge_msat: None,
            released_msat: None,
            unknown_held_msat: 0,
            provider_seconds: None,
            teardown_acknowledged: false,
            ledger_conserved: false,
        }
    }
}

/// Run `plan` on fakes. This must pass before real funding is enabled.
#[must_use]
pub fn run_fake(plan: &Plan) -> QualificationReceipt {
    let mut receipt = QualificationReceipt::empty(
        plan,
        Mode::Fake,
        "FAKE QUALIFICATION: fake wallet, Boat, sandbox, and task owner. It proves the runner, not a funded outcome.",
    );
    if let Err(refusal) = plan.check() {
        receipt.failure = Some(format!("plan refused: {refusal:?}"));
        return receipt;
    }
    let mut world = match World::new() {
        Ok(world) => world,
        Err(error) => {
            receipt.failure = Some(error.to_string());
            return receipt;
        }
    };
    let outcome = fake_steps(&mut world, plan, &mut receipt);
    let balance = world
        .ledger
        .compute_balance(&plan.account)
        .unwrap_or_default();
    receipt.ledger_conserved =
        balance.credited_msat == balance.available_msat + balance.held_msat + balance.settled_msat;
    receipt.unknown_held_msat = balance.held_msat;
    match outcome {
        Ok(()) => {
            receipt.qualified = receipt.ledger_conserved
                && receipt.teardown_acknowledged
                && receipt.check.as_deref() == Some("verified")
                && receipt.unknown_held_msat == 0;
        }
        Err(error) => receipt.failure = Some(error.to_string()),
    }
    receipt
}

fn fake_steps(world: &mut World, plan: &Plan, receipt: &mut QualificationReceipt) -> Result<()> {
    world.account(&plan.account, plan.top_up_sats)?;
    receipt.invoice_payment_hash = world
        .ledger
        .top_ups(&plan.account)?
        .first()
        .map(|p| p.top_up.payment_hash.clone());
    let confirmed_at = world.now;
    let funded = world.confirm(&plan.account, "cf-qualification", &plan.request())?;
    receipt.execution = Some(funded.execution.clone());
    receipt.hold = Some(funded.request.clone());
    let resource = world.ready(&funded)?;
    receipt.sandbox = Some(resource.clone());
    world.deliver(&funded, &resource)?;
    let task = world.dispatch(&funded)?;
    receipt.task = Some(task.clone());
    let patch = world.artifacts.declare(&funded, &task, &resource);
    world.owner.set_status(
        &resource,
        &task,
        TaskStatus::Ended {
            end: ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![CheckRun {
                command: plan.check.clone(),
                candidate: patch,
                exit_status: 0,
            }],
        },
    );
    world.provider.set_usage(&resource, 90);
    let now = world.tick(90);
    recover::step(
        &mut world.journal,
        &mut world.ledger,
        &world.provider,
        &world.owner,
        &funded,
        now,
    )?;
    retain::request(&mut world.journal, &funded, now)?;
    let cleanup = retain::advance(
        &mut world.journal,
        &world.provider,
        &world.artifacts,
        &funded.execution,
        now + 1,
    )?;
    receipt.teardown_acknowledged =
        cleanup.deleted() && world.now + 1 - confirmed_at <= plan.cleanup_deadline_seconds;
    let now = world.tick(2);
    retail_cloud::meter::poll(
        &mut world.journal,
        &world.provider,
        &funded.execution,
        "final",
        1,
        now,
    )?;
    let settled = settle::settle(
        &mut world.journal,
        &mut world.ledger,
        &funded,
        Ending::ExecutorEnded,
        now + 1,
    )?;
    receipt.check = settled.checks.map(|v| match v {
        dispatch::Verdict::Verified => "verified".into(),
        dispatch::Verdict::CheckFailed => "check_failed".into(),
        dispatch::Verdict::Unchecked => "unchecked".into(),
    });
    receipt.settlement_source = settled.source.clone();
    receipt.charge_msat = settled.charge_msat;
    receipt.released_msat = Some(settled.released_msat);
    receipt.provider_seconds = world
        .journal
        .usage(&funded.execution)?
        .and_then(|u| u.events.last().and_then(|e| e.seconds));
    if settled
        .charge_msat
        .is_some_and(|c| c > i64::try_from(plan.ceiling_sats * 1000).unwrap_or(i64::MAX))
    {
        return Err(Error::Invalid("the charge passed the plan's ceiling"));
    }
    let _ = rights(&funded);
    Ok(())
}

/// Why the funded step did not run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum FundedRefusal {
    /// The plan does not pass [`Plan::check`].
    Plan { refusal: PlanRefusal },
    /// The confirmation did not name this exact plan's digest.
    NotConfirmed,
    /// The fake run of this plan did not pass.
    FakeNotPassing,
    /// This build has no live wallet or Boat binding; the owner runs the
    /// funded step from the runbook.
    NoLiveBinding,
}

/// The owner's funded step. It checks the plan, the owner's confirmation of
/// its exact digest, and a passing fake run, then refuses because this
/// build binds no live wallet or Boat account. It never moves money.
///
/// # Errors
///
/// Always a [`FundedRefusal`] in this build.
pub fn run_funded(plan: &Plan, confirmed_digest: &str) -> std::result::Result<(), FundedRefusal> {
    plan.check()
        .map_err(|refusal| FundedRefusal::Plan { refusal })?;
    if confirmed_digest != plan.digest() {
        return Err(FundedRefusal::NotConfirmed);
    }
    if !run_fake(plan).qualified {
        return Err(FundedRefusal::FakeNotPassing);
    }
    Err(FundedRefusal::NoLiveBinding)
}

/// The checked-in plan fixture.
///
/// # Panics
///
/// Never: the fixture is checked in and tested.
#[must_use]
pub fn fixture() -> Plan {
    serde_json::from_str(include_str!("../fixtures/qualification-plan-v1.json"))
        .expect("the checked-in plan parses")
}
