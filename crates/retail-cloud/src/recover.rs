//! Read-side reconciliation of the original funded execution. A timeout
//! never becomes proof that a provider resource or task did not exist.
//! Recovery observes existing identities; it creates, dispatches, and pays
//! nothing. Replacing a v1 sandbox after dispatch requires a new offer.

use crate::authority::{self, Current, Step};
use crate::dispatch::{TaskOwner, TaskStatus};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::provision::{Provider, ProvisionState, ResourceState};
use crate::{Error, Result};
use pay_ledger::{Ledger, compute::HoldState};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub(crate) const SCHEMA:&str="
CREATE TABLE IF NOT EXISTS recovery (
 execution TEXT PRIMARY KEY, revision INTEGER NOT NULL, bytes TEXT NOT NULL, observed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS recovery_history (
 execution TEXT NOT NULL, revision INTEGER NOT NULL, bytes TEXT NOT NULL,
 observed_at INTEGER NOT NULL, PRIMARY KEY(execution,revision)
);
";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    AwaitingReservation,
    AwaitingProvision,
    ProvisionObserved {
        resource: String,
    },
    AwaitingMaterial {
        resource: String,
    },
    AwaitingDispatch {
        resource: String,
    },
    TaskObserved {
        resource: String,
        task: String,
        status: TaskStatus,
    },
    Unknown {
        provider: bool,
        task: bool,
    },
    /// The original grant binds one sandbox. A replacement after dispatch
    /// cannot inherit its execution authority or its spending reservation.
    NewOfferRequired {
        resource: String,
        checkpoint: Option<String>,
        reason: Loss,
    },
    Cleanup {
        complete: bool,
        artifacts_complete: bool,
    },
    Refused {
        reason: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Loss {
    Stopped,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementRefusal {
    pub resource: String,
    pub checkpoint: Option<String>,
    pub reason: Loss,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub execution: String,
    pub request: String,
    pub account: String,
    pub quote: String,
    pub state: State,
    pub hold_state: Option<String>,
    pub held_msat: i64,
    /// All already observed usage remains bound to its original resource.
    pub usage: Option<Usage>,
    pub replacement: Option<ReplacementRefusal>,
    pub checks: Option<crate::dispatch::Verdict>,
    pub retention: Option<crate::retain::Receipt>,
}

/// A compact usage projection. Original observations remain in the meter
/// journal; recovery history names their digest instead of copying every
/// counter reading into each revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub resource: String,
    pub book: String,
    pub quote: String,
    pub seconds: Option<u64>,
    pub final_usage: bool,
    pub observations: usize,
    pub observations_digest: String,
}
impl From<crate::meter::Usage> for Usage {
    fn from(usage: crate::meter::Usage) -> Self {
        Self {
            resource: usage.resource,
            book: usage.book,
            quote: usage.quote,
            seconds: usage.seconds,
            final_usage: usage.final_usage,
            observations: usage.events.len(),
            observations_digest: route_contract::digest::digest_of(&usage.events).to_string(),
        }
    }
}

/// Reconcile the original journals with provider and task-owner reads. A
/// discovered task acknowledges the original dispatch, even when its send
/// reply was lost. Unknown resources/tasks preserve the entire unsettled
/// hold. No transport or listing failure can authorize another attempt.
pub fn step(
    journal: &mut Journal,
    ledger: &mut Ledger,
    provider: &impl Provider,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    now: i64,
) -> Result<Snapshot> {
    step_mode(journal, ledger, provider, owner, funded, now, false)
}

/// Reconcile for a resident service using bounded cumulative final readings.
/// Identity, no-replacement, and unknown-hold rules are identical to [`step`].
pub fn step_bounded(
    journal: &mut Journal,
    ledger: &mut Ledger,
    provider: &impl Provider,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    now: i64,
) -> Result<Snapshot> {
    step_mode(journal, ledger, provider, owner, funded, now, true)
}
fn step_mode(
    journal: &mut Journal,
    ledger: &mut Ledger,
    provider: &impl Provider,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    now: i64,
    bounded: bool,
) -> Result<Snapshot> {
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict(
            "recovery requires the original funded identity",
        ));
    }
    let mut hold = ledger.hold(&funded.request)?;
    if let Some(existing) = &hold {
        let mut expected = crate::reserve::hold_request(funded, existing.request.at);
        expected.at = existing.request.at;
        if existing.request != expected {
            return Err(Error::Conflict(
                "the recovered hold differs from the funded request",
            ));
        }
    }
    let mut state = State::AwaitingReservation;
    let mut unknown = false;
    let mut replacement = None;
    let dispatch = journal.dispatch(&funded.execution)?;
    if hold.is_some() {
        state = State::AwaitingProvision;
        if let Some(provision) = journal.provisioning(&funded.execution)? {
            let known_resource = match &provision.state {
                ProvisionState::Starting { resource } | ProvisionState::Ready { resource, .. } => {
                    Some(resource.clone())
                }
                _ => None,
            };
            let label = format!("{}#{}", funded.execution, provision.attempt);
            let resource = match provider.find(&label) {
                Ok(Some(resource))
                    if resource.account == funded.account && resource.provisioning == label =>
                {
                    Some(resource.id)
                }
                Ok(Some(_)) => {
                    state = State::Refused {
                        reason: "provider identity changed".into(),
                    };
                    None
                }
                Ok(None) if known_resource.is_some() => known_resource.clone(),
                Ok(None)
                    if matches!(
                        provision.state,
                        ProvisionState::Intent
                            | ProvisionState::Refused { .. }
                            | ProvisionState::Unavailable
                    ) =>
                {
                    None
                }
                Ok(None) | Err(_) => {
                    state = State::Unknown {
                        provider: true,
                        task: dispatch.is_some(),
                    };
                    unknown = true;
                    None
                }
            };
            if let Some(resource) = resource {
                if known_resource.is_none() {
                    journal.check_custody()?;
                    journal.connection.execute("UPDATE provision SET resource=?,state='starting' WHERE execution=? AND attempt=? AND state='creating'",params![resource,funded.execution,provision.attempt])?;
                }
                if let Some(expected) = known_resource
                    && expected != resource
                {
                    state = State::Refused {
                        reason: "provider resource changed under its original label".into(),
                    };
                } else {
                    match provider.state(&resource) {
                        Ok(loss_state @ (ResourceState::Stopped | ResourceState::Deleted))
                            if dispatch.is_some() =>
                        {
                            let loss = match loss_state {
                                ResourceState::Deleted => Loss::Deleted,
                                _ => Loss::Stopped,
                            };
                            let checkpoint = checkpoint(journal, &funded.execution, now)?;
                            replacement = Some(ReplacementRefusal {
                                resource: resource.clone(),
                                checkpoint: checkpoint.clone(),
                                reason: loss,
                            });
                            state = State::NewOfferRequired {
                                resource: resource.clone(),
                                checkpoint,
                                reason: loss,
                            };
                            // Final provider readings may arrive after loss;
                            // never replace them with an estimated zero.
                            if let Some(usage) = journal.usage(&funded.execution)?
                                && !usage.final_usage
                            {
                                let sequence = usage
                                    .events
                                    .last()
                                    .map_or(1, |r| r.sequence.saturating_add(1));
                                if bounded {
                                    let _ = crate::meter::poll_bounded(
                                        journal,
                                        provider,
                                        &funded.execution,
                                        "recovery:final:bounded",
                                        now,
                                    )?;
                                } else {
                                    let _ = crate::meter::poll(
                                        journal,
                                        provider,
                                        &funded.execution,
                                        &format!("recovery:final:{sequence}"),
                                        sequence,
                                        now,
                                    )?;
                                }
                            }
                        }
                        Ok(ResourceState::Ready { address }) => {
                            journal.check_custody()?;
                            journal.connection.execute("UPDATE provision SET state='ready',address=?,ready_at=COALESCE(ready_at,?) WHERE execution=? AND attempt=? AND resource=? AND state IN ('creating','starting')",params![address,now,funded.execution,provision.attempt,resource])?;
                            match journal.delivered(&funded.execution)? {
                                Some((material, false))
                                    if material.resource == resource
                                        && material.source == funded.admission.source =>
                                {
                                    if let Some(dispatch) = &dispatch {
                                        if dispatch.resource != resource {
                                            state = State::Refused {
                                                reason:
                                                    "task is bound to another provider resource"
                                                        .into(),
                                            };
                                        } else {
                                            match owner.status(&resource, &dispatch.task) {
                                                Ok(Some(status)) => {
                                                    journal.check_custody()?;
                                                    journal.connection.execute("UPDATE dispatch SET state='acknowledged',acknowledged_at=COALESCE(acknowledged_at,?) WHERE execution=?",params![now,funded.execution])?;
                                                    state = State::TaskObserved {
                                                        resource: resource.clone(),
                                                        task: dispatch.task.clone(),
                                                        status,
                                                    };
                                                }
                                                Ok(None) | Err(_) => {
                                                    state = State::Unknown {
                                                        provider: false,
                                                        task: true,
                                                    };
                                                    unknown = true;
                                                }
                                            }
                                        }
                                    } else {
                                        state = State::AwaitingDispatch {
                                            resource: resource.clone(),
                                        };
                                    }
                                }
                                Some((material, _))
                                    if material.source != funded.admission.source
                                        || material.resource != resource =>
                                {
                                    state = State::Refused {
                                        reason: "retained material binding changed".into(),
                                    };
                                }
                                _ => {
                                    state = State::AwaitingMaterial {
                                        resource: resource.clone(),
                                    };
                                }
                            }
                        }
                        Ok(_) => {
                            state = State::ProvisionObserved {
                                resource: resource.clone(),
                            };
                        }
                        Err(_) => {
                            state = State::Unknown {
                                provider: true,
                                task: dispatch.is_some(),
                            };
                            unknown = true;
                        }
                    }
                }
            }
        }
    }
    // Cleanup is a separate lifecycle projection. Preserve an exact observed
    // candidate/check verdict even when a stop request changes that state.
    let checks = match &state {
        State::TaskObserved {
            status: TaskStatus::Ended { patch, checks, .. },
            ..
        } => Some(crate::dispatch::verdict(
            patch.as_deref(),
            &funded.task.checks,
            checks,
        )),
        _ => None,
    };
    if let Some(retention) = journal.retention_receipt(&funded.execution, now)?
        && (!matches!(state, State::NewOfferRequired { .. })
            || journal.cancellation_requested(&funded.execution)?
            || hold.as_ref().is_some_and(|h| h.state == HoldState::Settled))
    {
        state = State::Cleanup {
            complete: retention.deleted(),
            artifacts_complete: retention.complete,
        };
    }
    if ((unknown && dispatch.is_some())
        || matches!(
            state,
            State::NewOfferRequired { .. } | State::Refused { .. }
        ))
        && hold.as_ref().is_some_and(|h| h.state == HoldState::Held)
    {
        journal.check_custody()?;
        hold = Some(ledger.mark_hold_unknown(&funded.request)?);
    }
    let held_msat = hold.as_ref().map_or(0, |h| {
        if h.state == HoldState::Settled {
            0
        } else {
            h.request.amount_msat
        }
    });
    let usage = journal.usage(&funded.execution)?.map(Usage::from);
    let snapshot = Snapshot {
        execution: funded.execution.clone(),
        request: funded.request.clone(),
        account: funded.account.clone(),
        quote: route_contract::digest::digest_of(&funded.quote).to_string(),
        state,
        hold_state: hold.map(|h| h.state.as_str().into()),
        held_msat,
        usage,
        replacement,
        checks,
        retention: journal.retention_receipt(&funded.execution, now)?,
    };
    let bytes = serde_json::to_string(&snapshot)?;
    let tx = journal.immediate()?;
    let old: Option<(u32, String)> = tx
        .query_row(
            "SELECT revision,bytes FROM recovery WHERE execution=?",
            [&funded.execution],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if old.as_ref().is_none_or(|(_, old)| old != &bytes) {
        if old.as_ref().is_some_and(|(revision, _)| *revision >= 4096) {
            return Err(Error::Invalid("recovery history limit reached"));
        }
        let revision = old.map_or(Ok(1), |(rev, _)| {
            rev.checked_add(1)
                .ok_or(Error::Invalid("recovery revision limit reached"))
        })?;
        tx.execute("INSERT INTO recovery(execution,revision,bytes,observed_at) VALUES(?,?,?,?) ON CONFLICT(execution) DO UPDATE SET revision=excluded.revision,bytes=excluded.bytes,observed_at=excluded.observed_at",params![funded.execution,revision,bytes,now])?;
        tx.execute(
            "INSERT INTO recovery_history(execution,revision,bytes,observed_at) VALUES(?,?,?,?)",
            params![funded.execution, revision, bytes, now],
        )?;
    }
    tx.commit()?;
    Ok(snapshot)
}

/// A read grant may inspect recovery without resuming or replacing work.
pub fn observe(
    journal: &Journal,
    funded: &FundedRequest,
    current: &Current,
) -> Result<Vec<Snapshot>> {
    authority::check(Step::Read, &funded.admission, current).map_err(Error::Denied)?;
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict(
            "recovery reader has another funded identity",
        ));
    }
    let mut statement = journal
        .connection
        .prepare("SELECT bytes FROM recovery_history WHERE execution=? ORDER BY revision")?;
    statement
        .query_map([&funded.execution], |r| r.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect()
}

fn checkpoint(journal: &Journal, execution: &str, now: i64) -> Result<Option<String>> {
    let Some(receipt) = journal.retention_receipt(execution, now)? else {
        return Ok(None);
    };
    if receipt.expired || !receipt.complete {
        return Ok(None);
    }
    if let Some(manifest) = receipt.manifest {
        for artifact in manifest
            .artifacts
            .into_iter()
            .filter(|a| a.kind == crate::retain::Kind::Patch)
        {
            let digest:Option<String>=journal.connection.query_row("SELECT digest FROM retained_artifact WHERE execution=? AND name=? AND bytes IS NOT NULL",params![execution,artifact.name],|r|r.get(0)).optional()?;
            if digest.as_ref() == Some(&artifact.digest) {
                return Ok(digest);
            }
        }
    }
    Ok(None)
}
