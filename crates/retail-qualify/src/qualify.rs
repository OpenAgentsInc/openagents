//! The bounded funded-qualification plan and runner (#10723).
//!
//! A [`Plan`] pins everything one qualification may touch: the customer
//! account and principal, the top-up invoice, the computer and task class,
//! the source, the provider disclosure, the spending ceiling, the cleanup
//! deadline, and the independent check. [`Plan::check`] refuses anything
//! outside the v1 contract. [`run_fake`] runs the plan end to end on fakes
//! and must pass before real funding is enabled. [`run_funded`] is the
//! owner's step: without configured live bindings it refuses and says what
//! is missing rather than claim a funded outcome; with them it runs the
//! plan on the resident receiver wallet and the live Boat binding (#10748).
//! [`run_simulated`] runs those same adapters against simulated backends.

use std::time::Duration;

use retail_cloud::authority::Source;
use retail_cloud::boat::{BoatAdapter, BoatConfig};
use retail_cloud::contract::{self, TaskRequest};
use retail_cloud::dispatch::{self, CheckRun, ExecutorEnd, TaskStatus};
use retail_cloud::{Error, Result, recover, retain, settle};
use route_contract::price_book::Ending;
use route_contract::snapshot::Recipient;
use serde::{Deserialize, Serialize};

use crate::bindings::{BindingRefusal, Bindings};
use crate::harness::{World, rights};
use crate::{bound, sim};

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

    pub(crate) fn request(&self) -> TaskRequest {
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
    /// The live adapters against simulated backends: the resident wallet
    /// socket over a simulated Lightning network, and the Boat binding over
    /// a fake Boat API. It proves the bindings, not a funded outcome, and
    /// the launch gate never accepts it.
    Simulated,
}

/// Actual native bindings used by a funded run. This is attributable runner
/// evidence; deployment additionally verifies its retained ledger and receiver.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentBinding {
    pub receiver: String,
    pub boat_api_base: String,
    pub boat_org: Option<String>,
    pub boat_key_digest: String,
    pub template: String,
    pub model_provider: String,
    pub price_book: route_contract::Digest,
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
    /// The receiver's record of the paid top-up carries a preimage whose
    /// SHA-256 is the payment hash.
    #[serde(default)]
    pub preimage_verified: bool,
    /// What the run was bound to, in plain words.
    #[serde(default)]
    pub bindings: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DeploymentBinding>,
    /// What the simulated backends observed; only on a simulated run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation: Option<SimulationReport>,
}

impl QualificationReceipt {
    pub(crate) fn empty(plan: &Plan, mode: Mode, label: &str) -> Self {
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
            preimage_verified: false,
            bindings: None,
            deployment: None,
            simulation: None,
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
    /// No live bindings were configured (`--bindings`); the owner runs the
    /// funded step from the runbook.
    NoLiveBinding,
    /// The configured bindings are refused.
    Bindings { refusal: BindingRefusal },
    /// The Boat binding could not be built.
    BoatUnavailable,
}

/// What a funded receipt says about itself.
pub const FUNDED_LABEL: &str = "FUNDED QUALIFICATION: real sats paid to the resident receiver wallet and one real Boat sandbox under the separate retail account.";
/// What a simulated receipt says about itself.
pub const SIMULATED_LABEL: &str = "SIMULATION: the live adapters (the resident wallet socket client and the Boat binding) against a simulated Lightning network and a loopback fake Boat API. No real payment, credential, machine, or owner account was used. It proves the bindings, not a funded outcome.";
/// The simulated customer's model key; never a real key.
pub const SIMULATED_MODEL_KEY: &str = "sk-sim-retail-qualification-0000";
const SIMULATED_BOAT_KEY: &str = "sim-retail-boat-key-0000";

/// The owner's funded step. It checks the plan, the owner's confirmation of
/// its exact digest, and a passing fake run. Without `bindings` it refuses
/// with [`FundedRefusal::NoLiveBinding`]. With them, it checks them for a
/// funded run, resolves the retail Boat key and the test customer's model
/// key through `env`, and runs the plan on the resident receiver wallet and
/// the live Boat binding. The owner pays the printed invoice.
///
/// # Errors
///
/// A [`FundedRefusal`]; a run that started returns its receipt instead,
/// qualified or not.
pub fn run_funded(
    plan: &Plan,
    confirmed_digest: &str,
    bindings: Option<&Bindings>,
    env: &dyn Fn(&str) -> Option<String>,
) -> std::result::Result<QualificationReceipt, FundedRefusal> {
    plan.check()
        .map_err(|refusal| FundedRefusal::Plan { refusal })?;
    if confirmed_digest != plan.digest() {
        return Err(FundedRefusal::NotConfirmed);
    }
    if !run_fake(plan).qualified {
        return Err(FundedRefusal::FakeNotPassing);
    }
    let Some(bindings) = bindings else {
        return Err(FundedRefusal::NoLiveBinding);
    };
    let secrets = bindings
        .check(true, env)
        .map_err(|refusal| FundedRefusal::Bindings { refusal })?;
    let wallet = openagents_wallet::resident::RemoteWallet::probe(&bindings.wallet_home).ok_or(
        FundedRefusal::Bindings {
            refusal: BindingRefusal::WalletUnreachable,
        },
    )?;
    bound::prepare_state_dir(&bindings.state_dir).map_err(|_| FundedRefusal::BoatUnavailable)?;
    bound::prepare_state_dir(&bindings.state_dir.join("boat"))
        .map_err(|_| FundedRefusal::BoatUnavailable)?;
    let boat = BoatAdapter::new(
        secrets.boat_key,
        &BoatConfig {
            base_url: bindings.boat_api_base.clone(),
            org: bindings.boat_org.clone(),
            state_dir: bindings.state_dir.join("boat"),
            retry: None,
        },
    )
    .map_err(|_| FundedRefusal::BoatUnavailable)?;
    let mut receipt = bound::run(
        &bound::Run {
            plan,
            wallet: &wallet,
            boat: &boat,
            payer: &bound::OwnerPays,
            model_provider: bindings.model_provider.clone(),
            model_key: secrets.model_key,
            template: bindings.template.clone(),
            state_dir: &bindings.state_dir,
            poll: Duration::from_millis(bindings.poll_millis),
            payment_wait: Duration::from_secs(bindings.payment_wait_seconds),
        },
        Mode::Funded,
        FUNDED_LABEL,
    );
    receipt.deployment = Some(DeploymentBinding {
        receiver: openagents_wallet::LightningWallet::node_id(&wallet),
        boat_api_base: bindings.boat_api_base.clone(),
        boat_org: bindings.boat_org.clone(),
        boat_key_digest: secrets.boat_key_digest,
        template: bindings.template.clone(),
        model_provider: bindings.model_provider.clone(),
        price_book: route_contract::digest_of(&retail_cloud::contract::price_book()),
    });
    receipt.bindings = Some(format!(
        "openagents-wallet resident receiver at its control socket; Boat {}{}",
        bindings.boat_api_base,
        bindings
            .boat_org
            .as_deref()
            .map(|org| format!(" organization {org}"))
            .unwrap_or_default()
    ));
    Ok(receipt)
}

/// What the simulated backends observed, retained beside the receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationReport {
    pub invoices_issued: u64,
    pub payer_proofs: usize,
    /// Create requests Boat received, including the retry after a lost reply.
    pub boat_create_requests: u64,
    pub boat_sandboxes_created: usize,
    pub boat_sandboxes_left: usize,
    pub executors_started: u64,
    pub owner_commands: usize,
    /// No command line carried the customer's key.
    pub key_kept_off_command_lines: bool,
    /// No sandbox file still held the key at the end.
    pub key_removed: bool,
    pub unauthorized_requests: u64,
}

/// Run `plan` on the live adapters against the simulated backends: the
/// resident receiver wallet's socket over a simulated Lightning network,
/// and the Boat binding over a loopback fake Boat API whose first create
/// reply is lost. Nothing real is paid, started, or read.
#[must_use]
pub fn run_simulated(plan: &Plan) -> QualificationReceipt {
    let receipt = QualificationReceipt::empty(plan, Mode::Simulated, SIMULATED_LABEL);
    let fail = |mut receipt: QualificationReceipt, why: String| {
        receipt.failure = Some(why);
        receipt
    };
    let Ok(dir) = tempfile::tempdir() else {
        return fail(receipt, "temporary directory".into());
    };
    let root = match dir.path().canonicalize() {
        Ok(root) => root,
        Err(_) => return fail(receipt, "temporary directory unavailable".into()),
    };
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).is_err() {
            return fail(receipt, "private simulation directory unavailable".into());
        }
    }
    let network = sim::SimNetwork::new();
    let wallet_home = root.join("wallet");
    let resident = match sim::Resident::serve(&wallet_home, network.receiver()) {
        Ok(resident) => resident,
        Err(error) => return fail(receipt, error.to_string()),
    };
    let fake = match sim::FakeBoat::start(SIMULATED_BOAT_KEY) {
        Ok(fake) => fake,
        Err(error) => return fail(receipt, error.to_string()),
    };
    fake.set_usage_seconds(90);
    fake.lose_next_create_reply();
    let bindings = Bindings {
        schema: crate::bindings::SCHEMA.into(),
        simulation: true,
        wallet_home: wallet_home.clone(),
        boat_api_base: fake.base().to_owned(),
        boat_org: None,
        template: retail_cloud::provision::template("20261006"),
        state_dir: root.join("state"),
        model_provider: "openai".into(),
        payment_wait_seconds: 30,
        poll_millis: 10,
    };
    let env = |name: &str| match name {
        crate::bindings::BOAT_KEY_ENV => Some(SIMULATED_BOAT_KEY.to_owned()),
        crate::bindings::MODEL_KEY_ENV => Some(SIMULATED_MODEL_KEY.to_owned()),
        _ => None,
    };
    let secrets = match bindings.check(false, &env) {
        Ok(secrets) => secrets,
        Err(refusal) => return fail(receipt, format!("bindings refused: {refusal:?}")),
    };
    let Some(wallet) = openagents_wallet::resident::RemoteWallet::probe(&wallet_home) else {
        return fail(receipt, "the resident wallet did not answer".into());
    };
    if let Err(error) = bound::prepare_state_dir(&bindings.state_dir) {
        return fail(receipt, error.to_string());
    }
    if let Err(error) = bound::prepare_state_dir(&bindings.state_dir.join("boat")) {
        return fail(receipt, error.to_string());
    }
    let boat = match BoatAdapter::new(
        secrets.boat_key,
        &BoatConfig {
            base_url: bindings.boat_api_base.clone(),
            org: None,
            state_dir: bindings.state_dir.join("boat"),
            retry: Some(boat::RetryPolicy {
                max_retries: 2,
                base_delay: Duration::from_millis(5),
                max_delay: Duration::from_millis(50),
            }),
        },
    ) {
        Ok(boat) => boat,
        Err(error) => return fail(receipt, error.to_string()),
    };
    let payer = network.payer();
    let mut receipt = bound::run(
        &bound::Run {
            plan,
            wallet: &wallet,
            boat: &boat,
            payer: &payer,
            model_provider: bindings.model_provider.clone(),
            model_key: secrets.model_key,
            template: bindings.template.clone(),
            state_dir: &bindings.state_dir,
            poll: Duration::from_millis(bindings.poll_millis),
            payment_wait: Duration::from_secs(bindings.payment_wait_seconds),
        },
        Mode::Simulated,
        SIMULATED_LABEL,
    );
    let commands = fake.commands();
    let report = SimulationReport {
        invoices_issued: network.issued(),
        payer_proofs: payer.proofs.lock().map_or(0, |proofs| proofs.len()),
        boat_create_requests: fake.create_requests(),
        boat_sandboxes_created: fake.sandboxes_created(),
        boat_sandboxes_left: fake.active(),
        executors_started: fake.executors_started(),
        owner_commands: commands.len(),
        key_kept_off_command_lines: commands.iter().all(|c| !c.contains(SIMULATED_MODEL_KEY)),
        key_removed: fake.files_containing(SIMULATED_MODEL_KEY).is_empty(),
        unauthorized_requests: fake.unauthorized(),
    };
    drop(resident);
    receipt.bindings = Some(
        "SIMULATED: openagents-wallet RemoteWallet over the resident socket, served by a simulated Lightning receiver; retail_cloud::boat::BoatAdapter over a loopback fake Boat API".into(),
    );
    let effects_hold = report.invoices_issued == 1
        && report.payer_proofs == 1
        && report.boat_sandboxes_created == 1
        && report.boat_sandboxes_left == 0
        && report.executors_started == 1
        && report.key_kept_off_command_lines
        && report.key_removed
        && report.unauthorized_requests == 0;
    if !effects_hold {
        receipt.qualified = false;
        receipt
            .failure
            .get_or_insert_with(|| format!("simulated effects did not hold: {report:?}"));
    }
    receipt.simulation = Some(report);
    receipt
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
