//! The qualification runner on bound adapters (#10748).
//!
//! [`run`] drives one plan through the retail chain on whatever implements
//! the seams: the live resident wallet and Boat for the owner's funded run,
//! or the same adapters against the simulated backends in [`crate::sim`].
//! It runs on the wall clock, polls instead of advancing a fake clock, and
//! verifies the top-up's preimage against its payment hash. It moves money
//! only as far as the payer pays the plan's one invoice, and it never pays
//! anything itself.

use std::path::Path;
use std::time::{Duration, Instant};

use openagents_wallet::{LightningWallet, Proof, parse_hash32};
use pay_ledger::Ledger;
use pay_ledger::compute::{Binding, PurchaseState, Rights, credential_digest};
use retail_cloud::cancel::StopOwner;
use retail_cloud::dispatch::{self, TaskOwner, TaskStatus};
use retail_cloud::journal::Journal;
use retail_cloud::material::{Credential, CustomerSecret, Sandbox};
use retail_cloud::offer::{self, Capacity, ConfirmedVia, FundedRequest};
use retail_cloud::provision::{self, Provider, ProvisionState};
use retail_cloud::retain::Artifacts;
use retail_cloud::{
    Error, Result, cancel, contract, material, meter, recover, retain, settle, sha256_hex, topup,
};
use route_contract::price_book::Ending;

use crate::harness::{principals, rights};
use crate::qualify::{Mode, Plan, QualificationReceipt};

/// Who pays the plan's top-up invoice.
pub trait Payer {
    /// Pay `bolt11`, or arrange for it to be paid. A simulated payer
    /// returns its preimage proof; the owner's returns `None` and pays from
    /// their own wallet.
    ///
    /// # Errors
    ///
    /// The payment failed.
    fn pay(
        &self,
        invoice: &openagents_wallet::IssuedInvoice,
    ) -> std::result::Result<Option<Proof>, String>;
}

/// The owner pays from a wallet they control: print the invoice and wait.
pub struct OwnerPays;

impl Payer for OwnerPays {
    fn pay(
        &self,
        invoice: &openagents_wallet::IssuedInvoice,
    ) -> std::result::Result<Option<Proof>, String> {
        eprintln!(
            "Pay this top-up invoice of {} sats from a wallet you control:\n{}",
            invoice.amount_msat / 1000,
            invoice.bolt11
        );
        Ok(None)
    }
}

/// Everything one bound run uses.
pub struct Run<'a, W, B> {
    pub plan: &'a Plan,
    pub wallet: &'a W,
    pub boat: &'a B,
    pub payer: &'a dyn Payer,
    pub model_provider: String,
    pub model_key: String,
    pub template: String,
    pub state_dir: &'a Path,
    pub poll: Duration,
    pub payment_wait: Duration,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn preimage_matches(preimage: &str, payment_hash: &str) -> bool {
    hex::decode(preimage).is_ok_and(|bytes| bytes.len() == 32 && sha256_hex(&bytes) == payment_hash)
}

/// Run the plan on bound adapters and build its receipt.
pub fn run<W, B>(run: &Run<'_, W, B>, mode: Mode, label: &str) -> QualificationReceipt
where
    W: LightningWallet,
    B: Provider + Sandbox + TaskOwner + StopOwner + Artifacts,
{
    let plan = run.plan;
    let mut receipt = QualificationReceipt::empty(plan, mode, label);
    if let Err(refusal) = plan.check() {
        receipt.failure = Some(format!("plan refused: {refusal:?}"));
        return receipt;
    }
    let opened = std::fs::create_dir_all(run.state_dir)
        .map_err(|_| Error::Invalid("cannot create the state directory"))
        .and_then(|()| {
            Ok((
                Ledger::open(run.state_dir.join("ledger.sqlite"))?,
                Journal::open(run.state_dir.join("journal.sqlite"))?,
            ))
        });
    let (mut ledger, mut journal) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            receipt.failure = Some(error.to_string());
            return receipt;
        }
    };
    let outcome = steps(run, &mut ledger, &mut journal, &mut receipt);
    let balance = ledger.compute_balance(&plan.account).unwrap_or_default();
    receipt.ledger_conserved =
        balance.credited_msat == balance.available_msat + balance.held_msat + balance.settled_msat;
    receipt.unknown_held_msat = balance.held_msat;
    match outcome {
        Ok(()) => {
            receipt.qualified = receipt.ledger_conserved
                && receipt.teardown_acknowledged
                && receipt.preimage_verified
                && receipt.check.as_deref() == Some("verified")
                && receipt.unknown_held_msat == 0;
        }
        Err(error) => receipt.failure = Some(error.to_string()),
    }
    receipt
}

fn pause(run: &Run<'_, impl LightningWallet, impl Provider>) {
    std::thread::sleep(run.poll);
}

fn top_up<W: LightningWallet, B: Provider>(
    run: &Run<'_, W, B>,
    ledger: &mut Ledger,
    receipt: &mut QualificationReceipt,
) -> Result<()> {
    let account = &run.plan.account;
    let start = now();
    ledger.create_compute_account(account, start)?;
    for (principal, kind) in principals(account) {
        ledger.bind_principal(&Binding {
            principal: principal.clone(),
            account: account.clone(),
            kind,
            credential: credential_digest(&principal),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: start,
        })?;
    }
    let cli = format!("cli:{account}");
    let purchase_id = format!("buy-{account}");
    let purchase = topup::request_top_up(
        ledger,
        run.wallet,
        &topup::TopUpRequest {
            principal: cli.clone(),
            credential: credential_digest(&cli),
            purchase: purchase_id.clone(),
            amount_sats: run.plan.top_up_sats,
            now: start,
        },
    )?;
    let hash = purchase.top_up.payment_hash.clone();
    receipt.invoice_payment_hash = Some(hash.clone());
    let invoice = openagents_wallet::IssuedInvoice {
        bolt11: purchase.top_up.invoice.clone(),
        payment_hash: hash.clone(),
        amount_msat: u64::try_from(purchase.top_up.amount_msat).unwrap_or(0),
        description_hash: String::new(),
        expiry_secs: topup::INVOICE_EXPIRY_SECS,
        pay_to: run.wallet.node_id(),
    };
    if let Some(proof) = run.payer.pay(&invoice).map_err(Error::Remote)?
        && (proof.payment_hash != hash || !preimage_matches(&proof.preimage, &hash))
    {
        return Err(Error::Invalid(
            "the payer's proof does not match the invoice",
        ));
    }
    let deadline = Instant::now() + run.payment_wait;
    loop {
        topup::reconcile(ledger, run.wallet, now())?;
        match ledger.top_up(&purchase_id)?.map(|p| p.state) {
            Some(PurchaseState::Paid) => break,
            Some(PurchaseState::Expired) => return Err(Error::Invalid("the top-up expired")),
            _ if Instant::now() >= deadline => {
                return Err(Error::Invalid("the top-up was not paid in time"));
            }
            _ => pause(run),
        }
    }
    // The receiver's own record must carry the preimage of this hash.
    let record = run
        .wallet
        .lookup(parse_hash32(&hash)?)?
        .ok_or(Error::Invalid("the wallet lost the paid invoice"))?;
    receipt.preimage_verified = record
        .preimage
        .as_deref()
        .is_some_and(|p| preimage_matches(p, &hash));
    if !receipt.preimage_verified {
        return Err(Error::Invalid("the paid invoice has no matching preimage"));
    }
    Ok(())
}

fn steps<W, B>(
    run: &Run<'_, W, B>,
    ledger: &mut Ledger,
    journal: &mut Journal,
    receipt: &mut QualificationReceipt,
) -> Result<()>
where
    W: LightningWallet,
    B: Provider + Sandbox + TaskOwner + StopOwner + Artifacts,
{
    let plan = run.plan;
    top_up(run, ledger, receipt)?;

    let book = contract::price_book();
    let capacity = Capacity {
        running: 0,
        plan_starts_left: Some(1),
    };
    let confirmed_at = now();
    let at = u64::try_from(confirmed_at).unwrap_or(0);
    let made = offer::make_offer(
        &book,
        &plan.account,
        "cf-qualification",
        &plan.request(),
        capacity,
        at,
    )
    .map_err(Error::Refused)?;
    let funded = offer::confirm(
        journal,
        &made,
        &made.offer.digest,
        ConfirmedVia::OfferControl,
        &book,
        capacity,
        at + 1,
    )?;
    receipt.execution = Some(funded.execution.clone());
    receipt.hold = Some(funded.request.clone());
    let current = rights(&funded);
    retail_cloud::reserve::reserve(ledger, &funded, &current, now())?;

    let resource = ready(run, ledger, journal, &funded)?;
    receipt.sandbox = Some(resource.clone());
    material::deliver(
        journal,
        run.boat,
        &funded,
        &current,
        &resource,
        &funded.admission.source,
        &Credential::ApiKey {
            provider: run.model_provider.clone(),
            secret: CustomerSecret::new(run.model_key.clone()),
        },
        now(),
    )?;
    meter::dispatch_metered(
        journal,
        ledger,
        run.boat,
        run.boat,
        &funded,
        &current,
        &book,
        now(),
    )?;
    let task = dispatch::task_id(&funded.execution);
    receipt.task = Some(task.clone());

    let (ending, timed_out, sequence) = wait(run, ledger, journal, &funded, &resource, &task)?;
    let _ = material::remove_credentials(journal, run.boat, &funded.execution, now());
    retain::request(journal, &funded, now())?;
    let cleanup_deadline = confirmed_at + plan.cleanup_deadline_seconds;
    let deleted = loop {
        let cleanup = retain::advance(journal, run.boat, run.boat, &funded.execution, now())?;
        if cleanup.deleted() {
            break true;
        }
        if now() > cleanup_deadline {
            break false;
        }
        pause(run);
    };
    receipt.teardown_acknowledged = deleted && now() <= cleanup_deadline;
    meter::poll(
        journal,
        run.boat,
        &funded.execution,
        "final",
        sequence,
        now(),
    )?;
    let settled = settle::settle(journal, ledger, &funded, ending, now())?;
    receipt.check = settled.checks.map(|v| match v {
        dispatch::Verdict::Verified => "verified".into(),
        dispatch::Verdict::CheckFailed => "check_failed".into(),
        dispatch::Verdict::Unchecked => "unchecked".into(),
    });
    receipt.settlement_source = settled.source.clone();
    receipt.charge_msat = settled.charge_msat;
    receipt.released_msat = Some(settled.released_msat);
    receipt.provider_seconds = journal
        .usage(&funded.execution)?
        .and_then(|u| u.events.last().and_then(|e| e.seconds));
    if settled
        .charge_msat
        .is_some_and(|c| c > i64::try_from(plan.ceiling_sats * 1000).unwrap_or(i64::MAX))
    {
        return Err(Error::Invalid("the charge passed the plan's ceiling"));
    }
    if timed_out {
        return Err(Error::Invalid(
            "the task passed its wall time and was cancelled",
        ));
    }
    Ok(())
}

/// Reserve and provision until the sandbox is ready.
fn ready<W: LightningWallet, B: Provider>(
    run: &Run<'_, W, B>,
    ledger: &Ledger,
    journal: &mut Journal,
    funded: &FundedRequest,
) -> Result<String> {
    let deadline = now() + provision::READY_DEADLINE_SECS * 2 + 60;
    loop {
        let record = provision::advance(
            journal,
            ledger,
            run.boat,
            funded,
            &rights(funded),
            &run.template,
            now(),
        )?;
        match record.state {
            ProvisionState::Ready { resource, .. } => return Ok(resource),
            ProvisionState::Refused { .. } => return Err(Error::Invalid("Boat refused the start")),
            ProvisionState::Unavailable => return Err(Error::Invalid("no sandbox became ready")),
            _ if now() > deadline => return Err(Error::Invalid("the sandbox never became ready")),
            _ => pause(run),
        }
    }
}

/// Observe the task until it ends, metering as it runs. Past the wall time
/// the task is cancelled through its exact stop receipt.
fn wait<W, B>(
    run: &Run<'_, W, B>,
    ledger: &mut Ledger,
    journal: &mut Journal,
    funded: &FundedRequest,
    resource: &str,
    task: &str,
) -> Result<(Ending, bool, u32)>
where
    W: LightningWallet,
    B: Provider + TaskOwner + StopOwner + Artifacts,
{
    let deadline = now() + i64::try_from(run.plan.max_seconds).unwrap_or(i64::MAX) + 120;
    let mut sequence = 1u32;
    loop {
        recover::step(journal, ledger, run.boat, run.boat, funded, now())?;
        // The meter keeps at most 4,096 readings; the last is the final one.
        if sequence < 4_000 {
            meter::poll(
                journal,
                run.boat,
                &funded.execution,
                &format!("poll-{sequence}"),
                sequence,
                now(),
            )?;
            sequence += 1;
        }
        meter::enforce_ceiling(journal, run.boat, &funded.execution, now())?;
        match run.boat.status(resource, task) {
            Ok(Some(TaskStatus::Ended { .. })) => {
                return Ok((Ending::ExecutorEnded, false, sequence));
            }
            Ok(Some(TaskStatus::Cancelled)) => return Ok((Ending::Cancelled, false, sequence)),
            _ if now() > deadline => break,
            _ => pause(run),
        }
    }
    cancel::request(journal, funded, &rights(funded), now())?;
    let stop_deadline = now() + 600;
    loop {
        let stopped = cancel::advance(
            journal,
            ledger,
            run.boat,
            run.boat,
            run.boat,
            &funded.execution,
            now(),
        )?;
        if stopped.provider_deleted || now() > stop_deadline {
            return Ok((Ending::Cancelled, true, sequence));
        }
        pause(run);
    }
}
