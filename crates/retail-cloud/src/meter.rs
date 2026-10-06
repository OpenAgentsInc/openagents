//! Durable cumulative usage for one exclusive funded sandbox. Readings are
//! observations, never ledger debits. Missing counters remain unknown.

use pay_ledger::Ledger;
use route_contract::price_book::{Ending, PriceBook, Settlement};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::{self, Current, Step};
use crate::dispatch::{self, Dispatch, TaskOwner};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::provision::{Provider, ProvisionState, ResourceState};
use crate::{Error, Result};

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meter (
 execution TEXT PRIMARY KEY, resource TEXT NOT NULL UNIQUE,
 binding TEXT NOT NULL, baseline TEXT NOT NULL, started_at INTEGER NOT NULL,
 stop_intent INTEGER, stop_ack INTEGER
);
CREATE TABLE IF NOT EXISTS meter_reading (
 execution TEXT NOT NULL, sequence INTEGER NOT NULL,
 event TEXT NOT NULL, bytes TEXT NOT NULL,
 PRIMARY KEY(execution, sequence), UNIQUE(execution,event)
);
";

/// A cumulative provider observation, with a stable source reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub event: String,
    pub sequence: u32,
    pub resource: String,
    pub at: i64,
    /// Cumulative sandbox seconds, including operator setup. The baseline
    /// subtracts setup; missing usage is not zero.
    pub seconds: Option<u64>,
    /// Whether the provider acknowledged stopped or deleted state.
    pub stopped: bool,
    /// Provider/model evidence is retained separately from the customer's
    /// compute charge. The customer's key pays model usage directly.
    pub model: Option<ModelUsage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelUsage {
    pub provider: String,
    pub reference: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Reproducible usage and charge under the original price book.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub execution: String,
    pub resource: String,
    pub book: String,
    pub quote: String,
    pub seconds: Option<u64>,
    pub final_usage: bool,
    pub ceiling_reached: bool,
    pub deadline_at: i64,
    pub stop_requested: bool,
    pub stop_acknowledged: bool,
    pub events: Vec<Reading>,
}

impl Usage {
    /// Only acknowledged final usage can settle a running execution.
    #[must_use]
    pub fn settlement(&self, funded: &FundedRequest, ending: Ending) -> Settlement {
        if self.execution != funded.execution
            || self.quote != route_contract::digest::digest_of(&funded.quote).as_str()
        {
            return funded.quote.settle(Ending::Unknown, None);
        }
        funded
            .quote
            .settle(ending, self.seconds.filter(|_| self.final_usage))
    }
}

/// Pin the exact quote and baseline before submitting the task. A retry
/// uses the original baseline rather than discarding already incurred time.
/// The provider's counter must be known before a new dispatch is admitted.
pub fn dispatch_metered(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &impl Provider,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    current: &Current,
    book: &PriceBook,
    now: i64,
) -> Result<Dispatch> {
    authority::check(Step::Dispatch, &funded.admission, current).map_err(Error::Denied)?;
    funded
        .quote
        .check(book)
        .map_err(|_| Error::Invalid("the metering book differs from the quote"))?;
    if journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict("the funded metering identity differs"));
    }
    let Some(ProvisionState::Ready { resource, .. }) =
        journal.provisioning(&funded.execution)?.map(|p| p.state)
    else {
        return Err(Error::Invalid("the metered sandbox is not ready"));
    };
    let hold = ledger
        .hold(&funded.request)?
        .ok_or(Error::Invalid("no metering hold"))?;
    if hold.state != pay_ledger::compute::HoldState::Held
        || hold.request.execution != funded.execution
        || hold.request.account != funded.account
        || hold.request.quote != route_contract::digest::digest_of(&funded.quote).as_str()
    {
        return Err(Error::Invalid("metering requires the original live hold"));
    }
    match journal.delivered(&funded.execution)? {
        Some((delivered, false)) if delivered.resource == resource => {}
        _ => return Err(Error::Invalid("metering requires delivered material")),
    }
    let binding = serde_json::to_string(funded)?;
    if let Some((stored, stored_resource)) = journal
        .connection
        .query_row(
            "SELECT binding,resource FROM meter WHERE execution=?",
            [&funded.execution],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if stored != binding || stored_resource != resource {
            return Err(Error::Conflict(
                "the meter is bound to another request or sandbox",
            ));
        }
    } else {
        if journal.dispatch(&funded.execution)?.is_some() {
            return Err(Error::Invalid(
                "dispatch already started without a metering baseline",
            ));
        }
        let baseline = provider
            .usage_seconds(&resource)
            .ok()
            .flatten()
            .ok_or(Error::Invalid("the initial provider usage is unknown"))?;
        let tx = journal.immediate()?;
        tx.execute(
            "INSERT INTO meter(execution,resource,binding,baseline,started_at) VALUES(?,?,?,?,?)",
            params![
                funded.execution,
                resource,
                binding,
                baseline.to_string(),
                now
            ],
        )?;
        tx.commit()?;
    }
    if let Some(usage) = journal.usage(&funded.execution)?
        && (usage.ceiling_reached
            || usage.stop_requested
            || usage.final_usage
            || now >= usage.deadline_at)
    {
        return Err(Error::Invalid(
            "the metered execution cannot dispatch more work",
        ));
    }
    dispatch::dispatch(journal, ledger, owner, funded, current, now)
}

impl Journal {
    /// Append an immutable observation. Exact redelivery is inert; changed
    /// bytes under the same event or sequence conflict. Arrival order does
    /// not determine the counter order.
    pub fn record_usage(&mut self, execution: &str, reading: &Reading) -> Result<()> {
        if reading.sequence == 0
            || reading.sequence > 4096
            || reading.event.is_empty()
            || reading.event.len() > 256
            || reading.at < 0
            || reading.model.as_ref().is_some_and(|m| {
                m.provider.is_empty() || m.reference.is_empty() || m.reference.len() > 256
            })
        {
            return Err(Error::Invalid("invalid usage observation"));
        }
        let bytes = serde_json::to_string(reading)?;
        let tx = self.immediate()?;
        let (resource, started, binding): (String, i64, String) = tx.query_row(
            "SELECT resource,started_at,binding FROM meter WHERE execution=?",
            [execution],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if reading.resource != resource || reading.at < started {
            return Err(Error::Conflict(
                "usage belongs to another sandbox or interval",
            ));
        }
        let funded: FundedRequest = serde_json::from_str(&binding)?;
        if let Some(model) = &reading.model {
            let matching=funded.quote.lines.iter().any(|line| matches!(&line.payer, route_contract::price_book::QuotePayer::CallerKey { provider } if provider==&model.provider));
            if !matching {
                return Err(Error::Conflict("model usage belongs to another payer"));
            }
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT bytes FROM meter_reading WHERE execution=? AND (sequence=? OR event=?)",
                params![execution, reading.sequence, reading.event],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != bytes {
                return Err(Error::Conflict("usage event changed"));
            }
            return Ok(());
        }
        tx.execute(
            "INSERT INTO meter_reading(execution,sequence,event,bytes) VALUES(?,?,?,?)",
            params![execution, reading.sequence, reading.event, bytes],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Read retained observations under their original quote. Counter
    /// regressions, gaps, unknown readings, and contradictory final events
    /// leave the charge unknown until a complete known sequence is retained.
    pub fn usage(&self, execution: &str) -> Result<Option<Usage>> {
        let row = self.connection.query_row(
            "SELECT resource,binding,baseline,started_at,stop_intent,stop_ack FROM meter WHERE execution=?",[execution],
            |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,Option<i64>>(5)?)))
            .optional()?;
        let Some((resource, binding, baseline, started, stop_intent, stop_ack)) = row else {
            return Ok(None);
        };
        let funded: FundedRequest = serde_json::from_str(&binding)?;
        let baseline: u64 = baseline
            .parse()
            .map_err(|_| Error::Invalid("invalid metering baseline"))?;
        let mut statement = self
            .connection
            .prepare("SELECT bytes FROM meter_reading WHERE execution=? ORDER BY sequence")?;
        let rows = statement.query_map([execution], |r| r.get::<_, String>(0))?;
        let events = rows
            .map(|row| Ok(serde_json::from_str::<Reading>(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        let mut last = baseline;
        let mut valid = !events.is_empty();
        let mut stopped = false;
        let mut final_counter = None;
        for (index, event) in events.iter().enumerate() {
            valid &= event.sequence as usize == index + 1 && (!stopped || event.stopped);
            if let (Some(final_counter), Some(counter)) = (final_counter, event.seconds) {
                valid &= final_counter == counter;
            }
            match event.seconds {
                Some(seconds) if seconds >= last => last = seconds,
                Some(_) => valid = false,
                None => {}
            }
            if index > 0 {
                valid &= event.at >= events[index - 1].at;
            }
            stopped = event.stopped;
            if stopped && final_counter.is_none() {
                final_counter = event.seconds;
            }
        }
        // A later known cumulative sample reconciles a missing intermediate
        // counter, but never a gap, regression, or changed immutable event.
        let seconds = (valid && events.last().is_some_and(|e| e.seconds.is_some()))
            .then_some(last - baseline);
        Ok(Some(Usage {
            execution: execution.into(),
            resource,
            book: funded.quote.book.as_str().into(),
            quote: route_contract::digest::digest_of(&funded.quote)
                .as_str()
                .into(),
            seconds,
            final_usage: seconds.is_some() && valid && stopped,
            ceiling_reached: seconds.is_some_and(|s| s >= funded.quote.max_seconds),
            deadline_at: started
                .saturating_add(i64::try_from(funded.quote.max_seconds).unwrap_or(i64::MAX)),
            stop_requested: stop_intent.is_some(),
            stop_acknowledged: stop_ack.is_some(),
            events,
        }))
    }
}

/// Poll the exact sandbox's cumulative provider counter. The event identity
/// is supplied by the service so retrying an observation cannot add usage.
pub fn poll(
    journal: &mut Journal,
    provider: &impl Provider,
    execution: &str,
    event: &str,
    sequence: u32,
    now: i64,
) -> Result<Usage> {
    let usage = journal
        .usage(execution)?
        .ok_or(Error::Invalid("no metering baseline"))?;
    if let Some(existing) = usage.events.iter().find(|reading| reading.event == event) {
        if existing.sequence != sequence {
            return Err(Error::Conflict("the poll event changed sequence"));
        }
        return Ok(usage);
    }
    let stopped = matches!(
        provider.state(&usage.resource),
        Ok(ResourceState::Stopped | ResourceState::Deleted)
    );
    journal.record_usage(
        execution,
        &Reading {
            event: event.into(),
            sequence,
            resource: usage.resource.clone(),
            at: now,
            seconds: provider.usage_seconds(&usage.resource).ok().flatten(),
            stopped,
            model: None,
        },
    )?;
    journal
        .usage(execution)?
        .ok_or(Error::Invalid("missing meter"))
}

/// Stop at the quoted ceiling. The admitted service owns this cleanup duty
/// even if the client disconnects or its grant is revoked. Intent precedes
/// the call; an unknown acknowledgment is observed before any retry.
pub fn enforce_ceiling(
    journal: &mut Journal,
    provider: &impl Provider,
    execution: &str,
    now: i64,
) -> Result<Usage> {
    let usage = journal
        .usage(execution)?
        .ok_or(Error::Invalid("no metering baseline"))?;
    if now < usage.deadline_at && !usage.ceiling_reached && !usage.stop_requested {
        return Ok(usage);
    }
    if matches!(
        provider.state(&usage.resource),
        Ok(ResourceState::Stopped | ResourceState::Deleted)
    ) {
        journal.connection.execute(
            "UPDATE meter SET stop_ack=COALESCE(stop_ack,?) WHERE execution=?",
            params![now, execution],
        )?;
    } else if !usage.stop_requested {
        // Claim the first stop atomically across service workers.
        let changed = journal.connection.execute(
            "UPDATE meter SET stop_intent=? WHERE execution=? AND stop_intent IS NULL",
            params![now, execution],
        )?;
        if changed == 1 && provider.delete(&usage.resource).is_ok() {
            journal.connection.execute(
                "UPDATE meter SET stop_ack=COALESCE(stop_ack,?) WHERE execution=?",
                params![now, execution],
            )?;
        }
    }
    journal
        .usage(execution)?
        .ok_or(Error::Invalid("missing meter"))
}
