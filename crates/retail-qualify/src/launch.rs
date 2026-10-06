//! The fail-closed launch gate and operator monitoring (#10724).
//!
//! [`advertise`] decides what the service may offer. Paid capacity is
//! advertised only when the owner has confirmed the retail contract, a
//! funded qualification receipt for the exact supported configuration
//! exists and qualified, and capacity is free. A fake receipt never counts.
//! Hosted inference fallback and operator Boat and GCE placements are
//! always labeled separately and are never for sale.
//!
//! [`health`] reads the journal and the ledger and reports what an operator
//! must look at: stuck reservations, uncertain dispatch, slow readiness,
//! cleanup failures, meter gaps, ledger drift, and payout age. Reconciling a
//! stuck request is [`retail_cloud::recover::step`], which never creates
//! another execution.

use pay_ledger::Ledger;
use pay_ledger::compute::HoldState;
use retail_cloud::contract;
use retail_cloud::dispatch::DispatchState;
use retail_cloud::journal::Journal;
use retail_cloud::offer::Capacity;
use retail_cloud::provision::{ProvisionState, READY_DEADLINE_SECS};
use serde::{Deserialize, Serialize};

use crate::qualify::{Mode, QualificationReceipt};

/// The service version a deployment declares.
pub const SERVICE: &str = "openagents.cloud.retail-service.v1";

/// What the gate reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
    /// The owner confirmed the retail contract and price book.
    pub contract_confirmed: bool,
    /// The retained funded qualification receipt, when one exists.
    pub qualification: Option<QualificationReceipt>,
    /// The plan digest the receipt must be for: the supported configuration.
    pub supported_plan: String,
    pub capacity: Capacity,
}

/// Why paid capacity is not advertised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Closed {
    ContractUnconfirmed,
    NoFundedQualification,
    /// The receipt is fake, failed, or for another configuration.
    QualificationNotValid,
    NoCapacity,
}

/// What the service publishes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advertisement {
    pub service: String,
    pub price_book: String,
    /// The paid computer class, only when every gate passed.
    pub paid_capacity: Option<String>,
    pub closed: Option<Closed>,
    /// Always separate and never for sale.
    pub hosted_inference: String,
    /// Always separate and never for sale.
    pub operator_placements: String,
}

/// Decide what to advertise. Fails closed.
#[must_use]
pub fn advertise(gate: &Gate) -> Advertisement {
    let closed = if !gate.contract_confirmed {
        Some(Closed::ContractUnconfirmed)
    } else {
        match &gate.qualification {
            None => Some(Closed::NoFundedQualification),
            Some(receipt)
                if receipt.mode != Mode::Funded
                    || !receipt.qualified
                    || receipt.plan != gate.supported_plan =>
            {
                Some(Closed::QualificationNotValid)
            }
            Some(_)
                if gate.capacity.running >= contract::SANDBOXES_MAX
                    || gate.capacity.plan_starts_left == Some(0) =>
            {
                Some(Closed::NoCapacity)
            }
            Some(_) => None,
        }
    };
    Advertisement {
        service: SERVICE.into(),
        price_book: contract::price_book().version,
        paid_capacity: closed.is_none().then(|| contract::COMPUTER_CLASS.to_owned()),
        closed,
        hosted_inference: "Sponsored hosted inference fallback for local Coder turns; not a paid computer and not for sale.".into(),
        operator_placements: "Operator Boat and GCE placements (chat work --on boat|gce); OpenAgents' own runs, not for sale.".into(),
    }
}

/// One thing an operator must look at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "alert", rename_all = "snake_case")]
pub enum Alert {
    /// A hold with no provisioning progress.
    StuckReservation { execution: String, age_seconds: i64 },
    /// A dispatch whose acknowledgment never arrived.
    UncertainDispatch { execution: String, age_seconds: i64 },
    /// A sandbox past its readiness deadline.
    SlowReadiness { execution: String, age_seconds: i64 },
    /// Teardown requested and not acknowledged.
    CleanupPending { execution: String },
    /// Dispatched with no usage reading.
    MeterGap { execution: String },
    /// An account whose balance does not conserve, or a settled hold with
    /// no debit.
    LedgerDrift { account: String },
    /// Unpaid OpenAgents shares older than the payout window.
    PayoutAge { oldest_seconds: i64 },
}

/// How long a hold may wait for provisioning progress.
pub const STUCK_SECONDS: i64 = 15 * 60;
/// How long a dispatch may stay unacknowledged.
pub const DISPATCH_SECONDS: i64 = 5 * 60;
/// How old unpaid shares may get.
pub const PAYOUT_SECONDS: i64 = 7 * 86_400;

/// Read the journal and the ledger and report every alert.
///
/// # Errors
///
/// A journal or ledger failure.
pub fn health(journal: &Journal, ledger: &Ledger, now: i64) -> retail_cloud::Result<Vec<Alert>> {
    let mut alerts = Vec::new();
    let mut accounts = std::collections::BTreeSet::new();
    let debits: std::collections::BTreeSet<String> =
        ledger.since(0)?.into_iter().map(|r| r.key).collect();
    for funded in journal.all_funded()? {
        accounts.insert(funded.account.clone());
        let age = now - i64::try_from(funded.confirmed_at).unwrap_or(now);
        let hold = ledger.hold_for_execution(&funded.execution)?;
        let provisioning = journal.provisioning(&funded.execution)?;
        let live = hold.as_ref().is_some_and(|h| h.state != HoldState::Settled);
        if live && provisioning.is_none() && age > STUCK_SECONDS {
            alerts.push(Alert::StuckReservation {
                execution: funded.execution.clone(),
                age_seconds: age,
            });
        }
        if let Some(p) = &provisioning
            && matches!(
                p.state,
                ProvisionState::Starting { .. } | ProvisionState::Creating
            )
            && now - p.intent_at > READY_DEADLINE_SECS * 2
        {
            alerts.push(Alert::SlowReadiness {
                execution: funded.execution.clone(),
                age_seconds: now - p.intent_at,
            });
        }
        if let Some(d) = journal.dispatch(&funded.execution)? {
            if d.state != DispatchState::Acknowledged && now - d.intent_at > DISPATCH_SECONDS {
                alerts.push(Alert::UncertainDispatch {
                    execution: funded.execution.clone(),
                    age_seconds: now - d.intent_at,
                });
            }
            let readings = journal
                .usage(&funded.execution)?
                .map_or(0, |u| u.events.len());
            if live && readings == 0 && now - d.intent_at > DISPATCH_SECONDS {
                alerts.push(Alert::MeterGap {
                    execution: funded.execution.clone(),
                });
            }
        }
        if let Some(r) = journal.retention_receipt(&funded.execution, now)?
            && !r.deleted()
        {
            alerts.push(Alert::CleanupPending {
                execution: funded.execution.clone(),
            });
        }
        if let Some(h) = &hold
            && h.state == HoldState::Settled
            && h.charge_msat.unwrap_or(0) > 0
            && !debits.contains(&format!("debit:{}", h.request.id))
        {
            alerts.push(Alert::LedgerDrift {
                account: funded.account.clone(),
            });
        }
    }
    for account in accounts {
        let b = ledger.compute_balance(&account)?;
        if b.credited_msat != b.available_msat + b.held_msat + b.settled_msat
            || b.available_msat < 0
        {
            alerts.push(Alert::LedgerDrift { account });
        }
    }
    let unpaid = ledger.available_shares(pay_ledger::OPENAGENTS)?;
    if !unpaid.is_empty() {
        let settled: std::collections::BTreeMap<String, i64> = ledger
            .since(0)?
            .into_iter()
            .map(|r| (r.key, r.settled_at))
            .collect();
        if let Some(oldest) = unpaid
            .iter()
            .filter(|s| s.amount_msat > 0)
            .filter_map(|s| settled.get(&s.settlement))
            .min()
            && now - oldest > PAYOUT_SECONDS
        {
            alerts.push(Alert::PayoutAge {
                oldest_seconds: now - oldest,
            });
        }
    }
    alerts.dedup();
    Ok(alerts)
}
