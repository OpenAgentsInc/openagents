//! The integrated fake-payment acceptance run (#10722).
//!
//! Each case drives the whole retail chain in its own isolated
//! [`World`]: account, top-up, offer, the four authorities, reservation,
//! provisioning, material, dispatch, checks, metering, cancellation,
//! settlement, retention, and teardown, with faults injected at its
//! boundaries. Every case checks balance conservation and counts effects:
//! executors started, sandboxes created, and sandboxes left running. The
//! receipt says plainly that it is a simulation.

use compute_workbench::host::read_observing;
use pay_ledger::compute::credential_digest;
use retail_cloud::dispatch::{self, CheckRun, ExecutorEnd, TaskStatus};
use retail_cloud::{Error, Result, cancel, contract, meter, recover, retain, settle};
use route_contract::price_book::Ending;
use serde::{Deserialize, Serialize};

use crate::harness::{World, principals, request, rights};

/// The acceptance receipt's schema.
pub const SCHEMA: &str = "openagents.cloud.retail-acceptance.v1";
/// What every receipt says about itself.
pub const LABEL: &str = "SIMULATION: fake wallet, fake Boat, fake sandbox, and fake task owner. No real payment, credential, machine, or owner chat was used.";

/// One case's outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub name: String,
    pub passed: bool,
    /// Why it failed, when it did.
    pub failure: Option<String>,
    pub executors_started: u64,
    pub sandboxes_created: u64,
    pub sandboxes_left_running: usize,
    pub charge_msat: Option<i64>,
    pub held_msat: i64,
    pub settled_msat: i64,
    pub conserved: bool,
    pub restarts: u32,
}

/// Simulated-clock measurements from the checked case.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measurements {
    pub readiness_seconds: i64,
    pub first_event_seconds: i64,
    pub checked_completion_seconds: i64,
    pub cancel_acknowledgment_seconds: Option<u64>,
    pub orphan_cleanup_seconds: i64,
}

/// The supported limits this run froze.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub contract: String,
    pub computer_class: String,
    pub task_class: String,
    pub price_book: String,
    pub task_seconds_max: u64,
    pub sandboxes_max: usize,
    pub checks_max: usize,
    pub retention_days: u64,
}

/// The acceptance receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    pub simulation: bool,
    pub label: String,
    pub limits: Limits,
    pub cases: Vec<Case>,
    pub measurements: Measurements,
    /// The account, funded execution, and receipt identities the standalone
    /// window and the Grid workshop both showed.
    pub views_agree: bool,
    pub passed: bool,
}

fn limits() -> Limits {
    Limits {
        contract: retail_cloud::authority::CONTRACT.into(),
        computer_class: contract::COMPUTER_CLASS.into(),
        task_class: contract::TASK_CLASS.into(),
        price_book: contract::price_book().version,
        task_seconds_max: contract::TASK_SECS_MAX,
        sandboxes_max: contract::SANDBOXES_MAX,
        checks_max: contract::CHECKS_MAX,
        retention_days: contract::RETENTION_DAYS,
    }
}

fn finish(
    world: &World,
    name: &str,
    account: &str,
    charge: Option<i64>,
    outcome: Result<()>,
) -> Case {
    let balance = world.ledger.compute_balance(account).unwrap_or_default();
    let conserved = balance.credited_msat
        == balance.available_msat + balance.held_msat + balance.settled_msat
        && balance.available_msat >= 0;
    let failure = outcome.err().map(|e| e.to_string());
    Case {
        name: name.into(),
        passed: failure.is_none() && conserved,
        failure,
        executors_started: world.owner.started(),
        sandboxes_created: world.provider.create_calls(),
        sandboxes_left_running: world.provider.active().len(),
        charge_msat: charge,
        held_msat: balance.held_msat,
        settled_msat: balance.settled_msat,
        conserved,
        restarts: world.restarts,
    }
}

fn ensure(condition: bool, what: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(what))
    }
}

/// End the task with a retained patch and its passing check, retain the
/// artifacts, tear the sandbox down, read the final usage, and settle.
fn complete(
    world: &mut World,
    funded: &retail_cloud::offer::FundedRequest,
    resource: &str,
    task: &str,
    seconds: u64,
) -> Result<settle::Receipt> {
    let patch = world.artifacts.declare(funded, task, resource);
    world.owner.set_status(
        resource,
        task,
        TaskStatus::Ended {
            end: ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![CheckRun {
                command: funded.task.checks[0].clone(),
                candidate: patch,
                exit_status: 0,
            }],
        },
    );
    world.provider.set_usage(resource, seconds);
    let now = world.tick(i64::try_from(seconds).unwrap_or(0));
    recover::step(
        &mut world.journal,
        &mut world.ledger,
        &world.provider,
        &world.owner,
        funded,
        now,
    )?;
    retain::request(&mut world.journal, funded, now)?;
    let retained = retain::advance(
        &mut world.journal,
        &world.provider,
        &world.artifacts,
        &funded.execution,
        world.now + 1,
    )?;
    ensure(
        retained.complete && retained.deleted(),
        "retention or teardown",
    )?;
    let now = world.tick(2);
    meter::poll(
        &mut world.journal,
        &world.provider,
        &funded.execution,
        "final",
        1,
        now,
    )?;
    let receipt = settle::settle(
        &mut world.journal,
        &mut world.ledger,
        funded,
        Ending::ExecutorEnded,
        now + 1,
    )?;
    ensure(
        receipt.charge_msat.is_some() && receipt.held_msat == 0,
        "a completed task settles at a known charge",
    )?;
    Ok(receipt)
}

/// The checked path, observed through both views.
fn checked(measurements: &mut Measurements, views_agree: &mut bool) -> Case {
    let mut world = match World::new() {
        Ok(w) => w,
        Err(e) => return failed("checked_patch", &e),
    };
    let mut charge = None;
    let outcome = (|| -> Result<()> {
        world.account("acct", 500)?;
        let confirmed_at = world.now;
        let funded = world.confirm("acct", "cf-checked", &request(600))?;
        let resource = world.ready(&funded)?;
        measurements.readiness_seconds = world.now - confirmed_at;
        world.deliver(&funded, &resource)?;
        let task = world.dispatch(&funded)?;
        world.tick(3);
        world.owner.emit(&resource, &task, "cloned and editing");
        let page = dispatch::observe(&world.journal, &world.owner, &funded, &rights(&funded), 0)?;
        ensure(page.events.len() == 1, "first event")?;
        measurements.first_event_seconds = world.now - confirmed_at;
        let receipt = complete(&mut world, &funded, &resource, &task, 61)?;
        measurements.checked_completion_seconds = world.now - confirmed_at;
        ensure(
            receipt.checks == Some(dispatch::Verdict::Verified),
            "the patch is verified",
        )?;
        charge = receipt.charge_msat;
        ensure(charge == Some(103_000), "61 seconds and coordination")?;
        // Both clients show the same account, funded run, and receipt.
        let mut seen = Vec::new();
        for (principal, _) in principals("acct").into_iter().take(2) {
            let account = read_observing(
                &world.ledger,
                &world.journal,
                &principal,
                &credential_digest(&principal),
                &[],
                world.now,
                |_| Some(rights(&funded)),
            )?;
            seen.push((
                account.account.clone(),
                account.balance.clone(),
                account
                    .receipts
                    .iter()
                    .map(|r| (r.execution.clone(), r.settled_msat))
                    .collect::<Vec<_>>(),
            ));
        }
        *views_agree = seen.len() == 2
            && seen[0] == seen[1]
            && seen[0].2 == vec![(funded.execution.clone(), Some(103_000))];
        ensure(*views_agree, "the window and the workshop agree")?;
        Ok(())
    })();
    finish(&world, "checked_patch", "acct", charge, outcome)
}

fn failed(name: &str, error: &Error) -> Case {
    Case {
        name: name.into(),
        passed: false,
        failure: Some(error.to_string()),
        executors_started: 0,
        sandboxes_created: 0,
        sandboxes_left_running: 0,
        charge_msat: None,
        held_msat: 0,
        settled_msat: 0,
        conserved: false,
        restarts: 0,
    }
}

/// Run one faulted case.
fn case(name: &str, run: impl FnOnce(&mut World) -> Result<Option<i64>>) -> Case {
    let mut world = match World::new() {
        Ok(w) => w,
        Err(e) => return failed(name, &e),
    };
    let outcome = run(&mut world);
    let charge = outcome.as_ref().ok().copied().flatten();
    finish(&world, name, "acct", charge, outcome.map(|_| ()))
}

/// Run every case and build the receipt.
#[must_use]
pub fn run() -> Receipt {
    let mut measurements = Measurements::default();
    let mut views_agree = false;
    let mut cases = vec![checked(&mut measurements, &mut views_agree)];

    cases.push(case(
        "duplicate_confirmation_and_crash_before_credit",
        |w| {
            // The wallet is paid, then the service crashes before any callback.
            w.ledger.create_compute_account("acct", w.now)?;
            w.account("acct", 300)?;
            w.restart()?;
            let funded = w.confirm("acct", "cf-dup", &request(600))?;
            let again = w.confirm("acct", "cf-dup", &request(600))?;
            ensure(funded == again, "one funded request per offer")?;
            let resource = w.ready(&funded)?;
            let hold_again =
                retail_cloud::reserve::reserve(&mut w.ledger, &funded, &rights(&funded), w.now)?;
            ensure(hold_again.request.amount_msat == 124_000, "one hold")?;
            w.deliver(&funded, &resource)?;
            let task = w.dispatch(&funded)?;
            let receipt = complete(w, &funded, &resource, &task, 30)?;
            ensure(w.wallet.issued() == 1, "one invoice")?;
            Ok(receipt.charge_msat)
        },
    ));

    cases.push(case(
        "lost_create_and_dispatch_acknowledgments_across_restarts",
        |w| {
            w.account("acct", 300)?;
            let funded = w.confirm("acct", "cf-lost", &request(600))?;
            w.provider.lose_next_ack();
            let resource = w.ready(&funded)?;
            w.restart()?;
            w.deliver(&funded, &resource)?;
            w.owner.lose_next_ack();
            let task = w.dispatch(&funded)?;
            w.restart()?;
            recover::step(
                &mut w.journal,
                &mut w.ledger,
                &w.provider,
                &w.owner,
                &funded,
                w.now + 1,
            )?;
            ensure(w.owner.started() == 1, "one executor")?;
            ensure(w.provider.create_calls() == 1, "one sandbox")?;
            let receipt = complete(w, &funded, &resource, &task, 61)?;
            let replay = settle::settle(
                &mut w.journal,
                &mut w.ledger,
                &funded,
                Ending::ExecutorEnded,
                w.now + 9,
            )?;
            ensure(replay == receipt, "a settlement replay changes nothing")?;
            ensure(w.ledger.totals()?.settlements == 1, "one debit")?;
            Ok(receipt.charge_msat)
        },
    ));

    let mut cancel_latency = None;
    cases.push(case("cancel_after_start_with_a_lost_stop_reply", |w| {
        w.account("acct", 300)?;
        let funded = w.confirm("acct", "cf-cancel", &request(600))?;
        let resource = w.ready(&funded)?;
        w.deliver(&funded, &resource)?;
        w.dispatch(&funded)?;
        w.provider.set_usage(&resource, 45);
        let requested = w.tick(40);
        cancel::request(&mut w.journal, &funded, &rights(&funded), requested)?;
        *w.stopper.at.borrow_mut() = requested + 2;
        *w.stopper.lose_next.borrow_mut() = true;
        let uncertain = cancel::advance(
            &mut w.journal,
            &w.ledger,
            &w.provider,
            &w.stopper,
            &w.artifacts,
            &funded.execution,
            requested + 3,
        )?;
        ensure(
            uncertain.charge.charge_sats.is_none(),
            "unknown until reconciled",
        )?;
        w.restart()?;
        let known = cancel::advance(
            &mut w.journal,
            &w.ledger,
            &w.provider,
            &w.stopper,
            &w.artifacts,
            &funded.execution,
            requested + 4,
        )?;
        cancel_latency = known.stop_latency_seconds;
        ensure(*w.stopper.calls.borrow() == 1, "the stop is not resent")?;
        ensure(known.provider_deleted, "the sandbox is gone")?;
        let receipt = settle::settle(
            &mut w.journal,
            &mut w.ledger,
            &funded,
            Ending::Cancelled,
            requested + 5,
        )?;
        Ok(receipt.charge_msat)
    }));
    measurements.cancel_acknowledgment_seconds = cancel_latency;

    cases.push(case(
        "provider_lost_after_start_keeps_the_hold_until_usage_reads",
        |w| {
            w.account("acct", 300)?;
            let funded = w.confirm("acct", "cf-loss", &request(600))?;
            let resource = w.ready(&funded)?;
            w.deliver(&funded, &resource)?;
            let task = w.dispatch(&funded)?;
            w.owner.set_status(&resource, &task, TaskStatus::Running);
            w.provider.set_usage_unreadable(true);
            w.provider.lose(&resource);
            let now = w.tick(90);
            recover::step(
                &mut w.journal,
                &mut w.ledger,
                &w.provider,
                &w.owner,
                &funded,
                now,
            )?;
            let held = w.ledger.compute_balance("acct")?.held_msat;
            ensure(held == 124_000, "unknown cost stays held")?;
            ensure(
                w.owner.started() == 1 && w.provider.create_calls() == 1,
                "no replacement execution",
            )?;
            w.provider.set_usage_unreadable(false);
            w.provider.set_usage(&resource, 1_200);
            let now = w.tick(5);
            recover::step(
                &mut w.journal,
                &mut w.ledger,
                &w.provider,
                &w.owner,
                &funded,
                now + 1,
            )?;
            let receipt = settle::settle(
                &mut w.journal,
                &mut w.ledger,
                &funded,
                Ending::ProviderLostAfterExecutor,
                now + 2,
            )?;
            // The lost sandbox is still a provider resource until deleted.
            retain::request(&mut w.journal, &funded, now + 3)?;
            let cleanup = retain::advance(
                &mut w.journal,
                &w.provider,
                &w.artifacts,
                &funded.execution,
                now + 4,
            )?;
            ensure(
                cleanup.deleted() && !cleanup.complete,
                "deleted, with delivery reported incomplete",
            )?;
            Ok(receipt.charge_msat)
        },
    ));

    let mut orphan = 0;
    cases.push(case("client_disappears_and_the_service_cleans_up", |w| {
        w.account("acct", 300)?;
        let funded = w.confirm("acct", "cf-orphan", &request(600))?;
        let resource = w.ready(&funded)?;
        w.deliver(&funded, &resource)?;
        let task = w.dispatch(&funded)?;
        let started = w.now;
        w.artifacts.declare(&funded, &task, &resource);
        w.owner.set_status(
            &resource,
            &task,
            TaskStatus::Ended {
                end: ExecutorEnd::Failed,
                patch: None,
                checks: vec![],
            },
        );
        // The client is gone; only the service worker runs.
        w.restart()?;
        let now = w.tick(30);
        retain::service_step(&mut w.journal, &w.provider, &w.owner, &w.artifacts, now)?;
        retain::service_step(&mut w.journal, &w.provider, &w.owner, &w.artifacts, now + 1)?;
        ensure(w.provider.active().is_empty(), "no orphaned sandbox")?;
        orphan = now + 1 - started;
        Ok(None)
    }));
    measurements.orphan_cleanup_seconds = orphan;

    let passed = views_agree
        && cases
            .iter()
            .all(|c| c.passed && c.executors_started <= 1 && c.sandboxes_left_running == 0);
    Receipt {
        schema: SCHEMA.into(),
        simulation: true,
        label: LABEL.into(),
        limits: limits(),
        cases,
        measurements,
        views_agree,
        passed,
    }
}
