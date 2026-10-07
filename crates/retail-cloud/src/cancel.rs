//! Cancellation is a durable request, not proof that work or costs stopped.
//! Executor acknowledgment, provider teardown, usage, and ledger settlement
//! remain separate observations of the original funded execution.

use crate::authority::{self, Current, Step};
use crate::dispatch::{OwnerError, TaskStatus};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::provision::Provider;
use crate::retain::{self, Artifacts};
use crate::{Error, Result};
use pay_ledger::Ledger;
use route_contract::price_book::{Ending, Settlement};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS cancellation (
 execution TEXT PRIMARY KEY, binding TEXT NOT NULL, reason TEXT NOT NULL,
 requested_at INTEGER NOT NULL, sent_at INTEGER, evidence TEXT
);
";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Cancelled,
    Revoked,
}

/// The task owner's durable stop receipt. Effects are exact retained
/// artifact digests, not an inference from transport or provider loss.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopEvidence {
    pub at: i64,
    pub started: bool,
    pub status: TaskStatus,
    pub effects: Vec<String>,
}

/// The stop method and its read-only reconciliation use one stable request
/// identity. `stop` must deduplicate that identity durably on the task owner.
pub trait StopOwner {
    fn stop(
        &self,
        resource: &str,
        task: &str,
        request: &str,
    ) -> std::result::Result<StopEvidence, OwnerError>;
    fn stopped(
        &self,
        resource: &str,
        task: &str,
        request: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub execution: String,
    pub reason: Reason,
    pub requested_at: i64,
    pub stop_sent: bool,
    pub executor: Option<StopEvidence>,
    pub stop_latency_seconds: Option<u64>,
    pub provider_deleted: bool,
    pub final_usage: Option<crate::meter::Usage>,
    pub charge: Settlement,
    /// A charge projection is separate from an actual ledger settlement.
    pub settled_msat: Option<i64>,
    pub remaining_hold_msat: i64,
}

/// A human control action needs observe and execute rights, never spend.
pub fn request(
    journal: &mut Journal,
    funded: &FundedRequest,
    current: &Current,
    now: i64,
) -> Result<()> {
    authority::check(Step::Control, &funded.admission, current).map_err(Error::Denied)?;
    record(journal, funded, Reason::Cancelled, now)
}

/// A revoked execution or withdrawn disclosure triggers the service's stop
/// duty even when the former client can no longer control or observe it.
/// A missing right alone is not evidence of revocation.
pub fn revoke(
    journal: &mut Journal,
    funded: &FundedRequest,
    current: &Current,
    now: i64,
) -> Result<()> {
    let revoked = current.execute.as_ref().is_some_and(|grant| {
        grant.revoked
            && grant.execution == funded.execution
            && grant.generation == funded.admission.grant_generation
    }) || current
        .disclose
        .as_ref()
        .is_some_and(|consent| consent.withdrawn && consent.admission == funded.admission.digest());
    if !revoked {
        return Err(Error::Invalid(
            "no matching execution or disclosure revocation",
        ));
    }
    record(journal, funded, Reason::Revoked, now)
}

fn record(journal: &mut Journal, funded: &FundedRequest, reason: Reason, now: i64) -> Result<()> {
    if now < funded.confirmed_at as i64
        || journal.funded(&funded.execution)?.as_ref() != Some(funded)
    {
        return Err(Error::Conflict(
            "cancellation requires the original funded identity",
        ));
    }
    let tx = journal.immediate()?;
    tx.execute("INSERT INTO cancellation(execution,binding,reason,requested_at) VALUES(?,?,?,?) ON CONFLICT(execution) DO NOTHING",params![funded.execution,serde_json::to_string(funded)?,serde_json::to_string(&reason)?,now])?;
    tx.commit()?;
    // Retention and provider cleanup are obligations of the service, not
    // commands that need the revoked client's permission again.
    retain::request(journal, funded, now)
}

/// Advance stop acknowledgment and resource cleanup. Lost stop replies are
/// observed through the exact receipt identity, never resent automatically.
/// The provider's idempotent cleanup remains active independently of a
/// missing task-owner acknowledgment.
pub fn advance(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &impl Provider,
    owner: &impl StopOwner,
    artifacts: &impl Artifacts,
    execution: &str,
    now: i64,
) -> Result<Receipt> {
    advance_mode(
        journal, ledger, provider, owner, artifacts, execution, now, false,
    )
}

/// Advance resident cleanup with bounded cumulative metering. Identical or
/// exhausted nonfinal observations cannot block later stop/deletion duties.
pub fn advance_bounded(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &impl Provider,
    owner: &impl StopOwner,
    artifacts: &impl Artifacts,
    execution: &str,
    now: i64,
) -> Result<Receipt> {
    advance_mode(
        journal, ledger, provider, owner, artifacts, execution, now, true,
    )
}
fn advance_mode(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &impl Provider,
    owner: &impl StopOwner,
    artifacts: &impl Artifacts,
    execution: &str,
    now: i64,
    bounded: bool,
) -> Result<Receipt> {
    let (binding, requested, sent, evidence) = journal.connection.query_row(
        "SELECT binding,requested_at,sent_at,evidence FROM cancellation WHERE execution=?",
        [execution],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    let funded: FundedRequest = serde_json::from_str(&binding)?;
    if evidence.is_none() {
        if let Some(dispatch) = journal.dispatch(execution)? {
            let request = format!("stop:{execution}");
            match owner.stopped(&dispatch.resource, &dispatch.task, &request) {
                Ok(Some(evidence)) => save_evidence(journal, execution, requested, &evidence)?,
                Ok(None) if sent.is_none() => {
                    journal.check_custody()?;
                    let claimed = journal.connection.execute(
                        "UPDATE cancellation SET sent_at=? WHERE execution=? AND sent_at IS NULL",
                        params![now, execution],
                    )?;
                    if claimed == 1
                        && let Ok(evidence) =
                            owner.stop(&dispatch.resource, &dispatch.task, &request)
                    {
                        save_evidence(journal, execution, requested, &evidence)?;
                    }
                }
                _ => {}
            }
        } else {
            save_evidence(
                journal,
                execution,
                requested,
                &StopEvidence {
                    at: now,
                    started: false,
                    status: TaskStatus::Cancelled,
                    effects: vec![],
                },
            )?;
        }
    }
    let _ = retain::advance(journal, provider, artifacts, execution, now)?;
    // Preserve final provider usage before rendering a known charge. A
    // missing provider counter remains unknown after deletion as well.
    if let Some(usage) = journal.usage(execution)?
        && !usage.final_usage
    {
        let sequence = usage
            .events
            .last()
            .map_or(1, |e| e.sequence.saturating_add(1));
        let event = format!("cancel:{execution}:usage:{sequence}");
        if bounded {
            let _ = crate::meter::poll_bounded(
                journal,
                provider,
                execution,
                &format!("cancel:{execution}:bounded"),
                now,
            )?;
        } else {
            let _ = crate::meter::poll(journal, provider, execution, &event, sequence, now)?;
        }
    }
    receipt(journal, ledger, &funded, now)
}

fn save_evidence(
    journal: &Journal,
    execution: &str,
    _requested: i64,
    evidence: &StopEvidence,
) -> Result<()> {
    if evidence.at < 0
        || !matches!(
            evidence.status,
            TaskStatus::Cancelled | TaskStatus::Ended { .. }
        )
        || evidence.effects.len() > 16
        || evidence
            .effects
            .iter()
            .any(|digest| digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()))
        || (!evidence.started
            && (!evidence.effects.is_empty()
                || matches!(evidence.status, TaskStatus::Ended { .. })))
    {
        return Err(Error::Invalid("invalid stop acknowledgment"));
    }
    journal.check_custody()?;
    journal.connection.execute(
        "UPDATE cancellation SET evidence=? WHERE execution=? AND evidence IS NULL",
        params![serde_json::to_string(evidence)?, execution],
    )?;
    Ok(())
}

fn receipt(
    journal: &Journal,
    ledger: &Ledger,
    funded: &FundedRequest,
    now: i64,
) -> Result<Receipt> {
    let (reason, requested, sent, evidence) = journal.connection.query_row(
        "SELECT reason,requested_at,sent_at,evidence FROM cancellation WHERE execution=?",
        [&funded.execution],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    let executor: Option<StopEvidence> = evidence
        .map(|bytes| serde_json::from_str(&bytes))
        .transpose()?;
    let cleanup = journal.retention_receipt(&funded.execution, now)?;
    let final_usage = journal.usage(&funded.execution)?;
    let ending = match &executor {
        Some(evidence) if !evidence.started => Ending::NotStarted,
        Some(evidence) if matches!(evidence.status, TaskStatus::Ended { .. }) => {
            Ending::ExecutorEnded
        }
        Some(_) => Ending::Cancelled,
        None => Ending::Unknown,
    };
    let mut charge = match &final_usage {
        Some(usage) => usage.settlement(funded, ending),
        None => funded.quote.settle(ending, None),
    };
    let hold = ledger.hold(&funded.request)?;
    if hold.is_none() {
        charge = Settlement {
            charge_sats: Some(0),
            released_sats: 0,
            held_sats: 0,
        };
    }
    let stop_latency_seconds = executor
        .as_ref()
        .and_then(|e| u64::try_from(e.at.saturating_sub(requested).max(0)).ok());
    Ok(Receipt {
        execution: funded.execution.clone(),
        reason: serde_json::from_str(&reason)?,
        requested_at: requested,
        stop_sent: sent.is_some(),
        executor,
        stop_latency_seconds,
        provider_deleted: cleanup.is_some_and(|r| r.deleted()),
        final_usage,
        charge,
        settled_msat: hold.as_ref().and_then(|h| h.charge_msat),
        remaining_hold_msat: hold.as_ref().map_or(0, |h| {
            if h.state == pay_ledger::compute::HoldState::Settled {
                0
            } else {
                h.request.amount_msat
            }
        }),
    })
}

/// Observe the existing cancellation after reconnect without sending a
/// control command. A read grant permits this even without spend or execute.
pub fn observe(
    journal: &Journal,
    ledger: &Ledger,
    funded: &FundedRequest,
    current: &Current,
    now: i64,
) -> Result<Option<Receipt>> {
    authority::check(Step::Read, &funded.admission, current).map_err(Error::Denied)?;
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict(
            "cancellation reader has another funded identity",
        ));
    }
    if !journal.cancellation_requested(&funded.execution)? {
        return Ok(None);
    }
    Ok(Some(receipt(journal, ledger, funded, now)?))
}
impl Journal {
    /// Whether the service has a durable stop and cleanup duty.
    /// Reading this flag grants no control or observation authority.
    pub fn cancellation_requested(&self, execution: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM cancellation WHERE execution=?",
                [execution],
                |r| r.get::<_, i32>(0),
            )
            .optional()?
            .is_some())
    }
}
