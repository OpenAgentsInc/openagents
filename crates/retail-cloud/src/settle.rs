//! Final measured charges use the original quote and the central hold ledger.
//! Unknown usage stays reserved. Releasing a hold never represents a refund.
use crate::{Error, Result, journal::Journal, offer::FundedRequest};
use pay_ledger::{Ledger, compute::HoldState};
use route_contract::price_book::{Ending, Settlement};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS retail_settlement (execution TEXT PRIMARY KEY, binding TEXT NOT NULL, ending TEXT NOT NULL, bytes TEXT NOT NULL);";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub execution: String,
    pub request: String,
    pub quote: String,
    pub book: String,
    pub ending: Ending,
    pub charge: Settlement,
    pub charge_msat: Option<i64>,
    pub released_msat: i64,
    pub held_msat: i64,
    pub settled_at: Option<i64>,
    pub source: Option<String>,
    pub resource: String,
    pub usage_digest: Option<String>,
    pub checks: Option<crate::dispatch::Verdict>,
}
/// Settle a known disposition once, with no new spending or payout authority.
/// The disposition must agree with retained owner or recovery observations.
pub fn settle(
    journal: &mut Journal,
    ledger: &mut Ledger,
    funded: &FundedRequest,
    ending: Ending,
    now: i64,
) -> Result<Receipt> {
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict("settlement has another funded identity"));
    }
    let binding = serde_json::to_string(funded)?;
    let previous: Option<(String, String, String)> = journal
        .connection
        .query_row(
            "SELECT binding,ending,bytes FROM retail_settlement WHERE execution=?",
            [&funded.execution],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((original, disposition, bytes)) = previous {
        if original != binding || disposition != serde_json::to_string(&ending)? {
            return Err(Error::Conflict("settlement terms changed"));
        }
        return Ok(serde_json::from_str(&bytes)?);
    }
    if funded
        .quote
        .lines
        .iter()
        .any(|line| line.max_sats > 0 && line.recipient.as_deref() != Some("openagents"))
    {
        return Err(Error::Invalid("unsupported retail settlement recipient"));
    }
    let hold = ledger
        .hold(&funded.request)?
        .ok_or(Error::Invalid("settlement needs the original hold"))?;
    if hold.request != crate::reserve::hold_request(funded, hold.request.at) {
        return Err(Error::Conflict("settlement hold differs"));
    }
    let snapshot: Option<String> = journal
        .connection
        .query_row(
            "SELECT bytes FROM recovery WHERE execution=?",
            [&funded.execution],
            |r| r.get(0),
        )
        .optional()?;
    let snapshot: Option<crate::recover::Snapshot> = snapshot
        .map(|bytes| serde_json::from_str(&bytes))
        .transpose()?;
    let mut history_query = journal.connection.prepare(
        "SELECT bytes FROM recovery_history WHERE execution=? ORDER BY revision DESC LIMIT 4096",
    )?;
    let history = history_query
        .query_map([&funded.execution], |r| r.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str::<crate::recover::Snapshot>(&row?)?))
        .collect::<Result<Vec<_>>>()?;
    drop(history_query);
    let evidence: Option<Option<String>> = journal
        .connection
        .query_row(
            "SELECT evidence FROM cancellation WHERE execution=?",
            [&funded.execution],
            |r| r.get(0),
        )
        .optional()?;
    let evidence: Option<crate::cancel::StopEvidence> = evidence
        .flatten()
        .map(|bytes| serde_json::from_str(&bytes))
        .transpose()?;
    let cleanup = journal.retention_receipt(&funded.execution, now)?;
    let dispatch = journal.dispatch(&funded.execution)?;
    let never_dispatched = dispatch
        .as_ref()
        .is_none_or(|d| d.state == crate::dispatch::DispatchState::Intent);
    let no_resource = journal.provisioning(&funded.execution)?.is_none_or(|p| {
        p.abandoned.is_none()
            && matches!(
                p.state,
                crate::provision::ProvisionState::Intent
                    | crate::provision::ProvisionState::Refused { .. }
            )
    });
    let owner_ended = evidence
        .as_ref()
        .is_some_and(|e| matches!(e.status, crate::dispatch::TaskStatus::Ended { .. }))
        || history.iter().any(|s| {
            matches!(
                s.state,
                crate::recover::State::TaskObserved {
                    status: crate::dispatch::TaskStatus::Ended { .. },
                    ..
                }
            )
        });
    let admitted = match ending {
        Ending::Unknown => true,
        Ending::NotStarted | Ending::ProviderUnavailable | Ending::ProviderLostBeforeExecutor => {
            (never_dispatched || evidence.as_ref().is_some_and(|e| !e.started))
                && (no_resource || cleanup.as_ref().is_some_and(|r| r.deleted()))
        }
        Ending::ExecutorEnded => owner_ended,
        Ending::Cancelled => evidence.as_ref().is_some_and(|e| {
            e.started && matches!(e.status, crate::dispatch::TaskStatus::Cancelled)
        }),
        Ending::ProviderLostAfterExecutor => {
            snapshot.as_ref().is_some_and(|s| s.replacement.is_some()) && !never_dispatched
        }
    };
    if !admitted {
        return Err(Error::Invalid(
            "settlement disposition lacks retained evidence",
        ));
    }
    let usage = journal.usage(&funded.execution)?;
    let charge = usage.as_ref().map_or_else(
        || funded.quote.settle(ending, None),
        |usage| usage.settlement(funded, ending),
    );
    let charge_msat = charge.charge_sats.map(msat).transpose()?;
    let mut receipt = Receipt {
        execution: funded.execution.clone(),
        request: funded.request.clone(),
        quote: route_contract::digest_of(&funded.quote).to_string(),
        book: funded.quote.version.clone(),
        ending,
        charge,
        charge_msat,
        released_msat: 0,
        held_msat: hold.request.amount_msat,
        settled_at: None,
        source: None,
        resource: pay_ledger::compute::hold::RETAIL_RESOURCE.into(),
        usage_digest: usage
            .as_ref()
            .map(|u| route_contract::digest_of(u).to_string()),
        checks: history.iter().find_map(|s| s.checks.clone()),
    };
    if let Some(amount) = charge_msat {
        let (settled, _) = ledger.settle_hold(&funded.request, amount, now)?;
        receipt.released_msat = settled.request.amount_msat - amount;
        receipt.held_msat = 0;
        receipt.settled_at = settled.settled_at;
        receipt.source = (amount > 0).then(|| format!("debit:{}", funded.request));
        // The ledger commits debit, obligations, and hold release atomically.
        // A crash here reuses that transaction before sealing this receipt.
        journal.connection.execute(
            "INSERT INTO retail_settlement(execution,binding,ending,bytes) VALUES(?,?,?,?)",
            params![
                funded.execution,
                binding,
                serde_json::to_string(&ending)?,
                serde_json::to_string(&receipt)?
            ],
        )?;
    } else if hold.state != HoldState::Settled {
        ledger.mark_hold_unknown(&funded.request)?;
    } else {
        return Err(Error::Conflict("settled hold cannot become unknown"));
    }
    Ok(receipt)
}
fn msat(sats: u64) -> Result<i64> {
    sats.checked_mul(1000)
        .and_then(|amount| i64::try_from(amount).ok())
        .ok_or(Error::Invalid("settlement amount overflow"))
}
/// Read the sealed receipt with a fresh execution-scoped observe grant.
pub fn observe(
    journal: &Journal,
    funded: &FundedRequest,
    current: &crate::authority::Current,
) -> Result<Option<Receipt>> {
    crate::authority::check(crate::authority::Step::Read, &funded.admission, current)
        .map_err(Error::Denied)?;
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict("settlement reader identity differs"));
    }
    let bytes: Option<String> = journal
        .connection
        .query_row(
            "SELECT bytes FROM retail_settlement WHERE execution=?",
            [&funded.execution],
            |r| r.get(0),
        )
        .optional()?;
    bytes
        .map(|bytes| Ok(serde_json::from_str(&bytes)?))
        .transpose()
}
