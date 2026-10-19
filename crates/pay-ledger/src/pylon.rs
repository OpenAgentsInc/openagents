//! Brokered pylon compute jobs (`docs/compute/verse-compute.md`, P3).
//!
//! A buyer pays OpenAgents for one job on a pylon, by an x402 payment or a
//! debit from the purchased compute balance. The settlement splits under
//! the effective rule's `[pylon_job]` ([`crate::V2`]): the pylon's provider
//! gets `provider_bps` of the net receipts and OpenAgents the rest. Each
//! such settlement names the NIP-PYLON receipt (`3201` event ID) it pays,
//! one settlement per receipt, so a reader matches ledger rows to public
//! receipts.
//!
//! Providers are paid by balance sweeps through the ordinary payout worker
//! ([`crate::payout::tick`] under [`crate::payout::Policy::pylon_sweeps`]),
//! never per job. A trusted checker's `check-fail` on a job's receipt
//! forfeits the provider's share while it is unpaid
//! ([`Ledger::forfeit_pylon_job`]); a share a sweep already reserved or
//! paid cannot come back, and the adjustment records it as a loss.
//! Forfeited money stays with OpenAgents.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::adjustment::Adjustment;
use crate::{Error, Ledger, Result};

/// The resource a pylon job settlement records.
pub const RESOURCE: &str = "openagents.pylon.job.v1";

pub(crate) const TABLES: &str = "CREATE TABLE IF NOT EXISTS pylon_job (
 settlement TEXT PRIMARY KEY REFERENCES settlement(payment_hash),
 receipt TEXT NOT NULL UNIQUE,
 provider TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pylon_decision (
 settlement TEXT PRIMARY KEY REFERENCES pylon_job(settlement),
 pylon TEXT NOT NULL, owner_linked INTEGER NOT NULL, house_expense INTEGER NOT NULL,
 closes_at INTEGER NOT NULL, released INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS pylon_launch_party (
 party TEXT PRIMARY KEY, settlement TEXT NOT NULL UNIQUE
);";

/// One brokered pylon job as the ledger holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The settlement key: a payment hash or `debit:<hold>`.
    pub settlement: String,
    /// The `3201` receipt event ID the settlement pays.
    pub receipt: String,
    /// The provider's party.
    pub provider: String,
    pub received_msat: i64,
    /// The provider's share as recorded.
    pub provider_msat: i64,
    /// How much of it a failed check forfeited.
    pub forfeited_msat: i64,
    /// How much of it went out in a sent payout.
    pub paid_msat: i64,
}

/// A NIP-01 event ID: 64 lowercase hex digits.
pub(crate) fn is_event_id(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn settlement_for(c: &Connection, receipt: &str) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT settlement FROM pylon_job WHERE receipt=?",
        [receipt],
        |r| r.get(0),
    )
    .optional()?)
}

pub(crate) fn job_in(c: &Connection, settlement: &str) -> Result<Option<Job>> {
    Ok(c.query_row(
        "SELECT j.settlement,j.receipt,j.provider,t.received_msat,
         COALESCE((SELECT amount_msat FROM share WHERE settlement=j.settlement AND party=j.provider AND role='provider'),0),
         COALESCE((SELECT SUM(reduced_msat) FROM payable_adjustment WHERE settlement=j.settlement AND party=j.provider AND role='provider'),0),
         COALESCE((SELECT SUM(s.amount_msat) FROM payout_item i JOIN payout p ON p.id=i.payout JOIN payable_share s ON s.settlement=i.settlement AND s.party=i.party AND s.role=i.role WHERE i.settlement=j.settlement AND i.party=j.provider AND i.role='provider' AND p.state='sent'),0)
         FROM pylon_job j JOIN settlement t ON t.payment_hash=j.settlement WHERE j.settlement=?",
        [settlement],
        |r| {
            Ok(Job {
                settlement: r.get(0)?,
                receipt: r.get(1)?,
                provider: r.get(2)?,
                received_msat: r.get(3)?,
                provider_msat: r.get(4)?,
                forfeited_msat: r.get(5)?,
                paid_msat: r.get(6)?,
            })
        },
    )
    .optional()?)
}

impl Ledger {
    /// Install [`crate::V2`], the rule with the pylon job split. Installing
    /// it again changes nothing.
    ///
    /// # Errors
    ///
    /// When another rule already holds version 2 or its effective time.
    pub fn install_pylon_rule(&mut self) -> Result<()> {
        self.load_rule(crate::V2, &crate::digest(crate::V2))
    }

    /// The pylon job that paid `receipt`, if any.
    ///
    /// # Errors
    ///
    /// A storage failure.
    pub fn pylon_job(&self, receipt: &str) -> Result<Option<Job>> {
        match settlement_for(&self.connection, receipt)? {
            Some(settlement) => job_in(&self.connection, &settlement),
            None => Ok(None),
        }
    }

    /// Every pylon job, in settlement order.
    ///
    /// # Errors
    ///
    /// A storage failure.
    pub fn pylon_jobs(&self) -> Result<Vec<Job>> {
        let mut statement = self.connection.prepare(
            "SELECT j.settlement FROM pylon_job j JOIN settlement t ON t.payment_hash=j.settlement ORDER BY t.seq",
        )?;
        let keys = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        keys.iter()
            .map(|key| job_in(&self.connection, key)?.ok_or(Error::Invalid("missing pylon job")))
            .collect()
    }

    /// Forfeit the provider's unpaid share of the job that paid `receipt`,
    /// on the evidence of `check`, a trusted checker's `check-fail` label
    /// (its event ID) on that receipt. The caller verifies the label and
    /// its checker before calling. Replaying the same check returns the
    /// same adjustment.
    ///
    /// # Errors
    ///
    /// An unknown receipt, a malformed check ID, or a storage failure.
    pub fn forfeit_pylon_job(&mut self, receipt: &str, check: &str, at: i64) -> Result<Adjustment> {
        if !is_event_id(check) {
            return Err(Error::Invalid("check label ID"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let settlement = settlement_for(&tx, receipt)?
            .ok_or(Error::Invalid("no pylon job pays this receipt"))?;
        let job = job_in(&tx, &settlement)?.ok_or(Error::Invalid("missing pylon job"))?;
        let id = format!("pylon-check:{check}");
        let prior: i64 = tx.query_row(
            "SELECT COALESCE(SUM(requested_msat),0) FROM payable_adjustment WHERE settlement=? AND party=? AND role='provider' AND id!=?",
            params![settlement, job.provider, id],
            |r| r.get(0),
        )?;
        let requested = job.provider_msat - prior;
        if requested <= 0 {
            // Another check already forfeited the whole share.
            let existing = tx
                .query_row(
                    "SELECT id FROM payable_adjustment WHERE settlement=? AND party=? AND role='provider' ORDER BY at,id LIMIT 1",
                    params![settlement, job.provider],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .ok_or(Error::Invalid("nothing to forfeit"))?;
            let read: Vec<Adjustment> = self_adjustments(&tx, &settlement)?;
            return read
                .into_iter()
                .find(|a| a.id == existing)
                .ok_or(Error::Invalid("adjustment missing"));
        }
        let adjustment = crate::adjustment::reduce_in(
            &tx,
            &id,
            &settlement,
            &job.provider,
            "provider",
            check,
            requested,
            at,
        )?;
        tx.commit()?;
        Ok(adjustment)
    }
}

fn self_adjustments(c: &Connection, settlement: &str) -> Result<Vec<Adjustment>> {
    let mut q = c.prepare(
        "SELECT id,settlement,party,role,evidence,requested_msat,reduced_msat,loss_msat,at FROM payable_adjustment WHERE settlement=? ORDER BY id",
    )?;
    let rows = q.query_map([settlement], |r| {
        Ok(Adjustment {
            id: r.get(0)?,
            settlement: r.get(1)?,
            party: r.get(2)?,
            role: r.get(3)?,
            evidence: r.get(4)?,
            requested_msat: r.get(5)?,
            reduced_msat: r.get(6)?,
            loss_msat: r.get(7)?,
            at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Decisions remain unpaid for ten minutes unless the gateway configures another window.
pub const SAMPLING_WINDOW_SECS: i64 = 600;
pub const DECISION_RESOURCE: &str = "openagents.pylon.decision.v1";

/// Verified receipt and owner attribution supplied by the gateway, never by the pylon.
#[derive(Debug, Clone)]
pub struct Decision {
    pub pylon: String,
    pub party: String,
    pub receipt: String,
    pub owner_linked: bool,
    pub house_expense: bool,
    /// Mainnet house expenses are checked atomically against per-payment and daily ceilings.
    pub expense_ceilings_msat: Option<(u64, u64)>,
    pub sampling_window_secs: i64,
}

pub(crate) fn check_decision(c: &Connection, key: &str, terms: &Decision, at: i64) -> Result<()> {
    let stored: Option<(String, bool, bool, i64)> = c
        .query_row(
            "SELECT pylon,owner_linked,house_expense,closes_at FROM pylon_decision WHERE settlement=?",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let job = job_in(c, key)?.ok_or(Error::Conflict("decision settlement"))?;
    if stored
        != Some((
            terms.pylon.clone(),
            terms.owner_linked,
            terms.house_expense,
            at.checked_add(terms.sampling_window_secs)
                .ok_or(Error::Invalid("sampling time"))?,
        ))
        || job.provider != terms.party
        || job.receipt != terms.receipt
    {
        return Err(Error::Conflict("decision terms"));
    }
    Ok(())
}

pub(crate) fn record_decision(c: &Connection, key: &str, terms: &Decision, at: i64) -> Result<()> {
    if terms.sampling_window_secs < 0 || !is_event_id(&terms.pylon) || at < 0 {
        return Err(Error::Invalid("decision sampling window or pylon"));
    }
    let closes = at
        .checked_add(terms.sampling_window_secs)
        .ok_or(Error::Invalid("sampling time"))?;
    if terms.house_expense
        && let Some((per_payment, daily)) = terms.expense_ceilings_msat
    {
        let expense: i64 = c.query_row(
            "SELECT received_msat FROM settlement WHERE payment_hash=?",
            [key],
            |r| r.get(0),
        )?;
        let spent: i64 = c.query_row("SELECT COALESCE(SUM(s.received_msat),0) FROM pylon_decision d JOIN settlement s ON s.payment_hash=d.settlement WHERE d.house_expense=1 AND s.settled_at>=?",[at.saturating_sub(86400)],|r|r.get(0))?;
        if u64::try_from(expense).map_or(true, |n| n > per_payment)
            || u64::try_from(spent)
                .unwrap_or(u64::MAX)
                .saturating_add(u64::try_from(expense).unwrap_or(u64::MAX))
                > daily
        {
            return Err(Error::Denied(
                "the decision exceeds the owner's spending ceiling",
            ));
        }
    }
    c.execute("INSERT INTO pylon_decision(settlement,pylon,owner_linked,house_expense,closes_at) VALUES(?,?,?,?,?)", params![key,terms.pylon,terms.owner_linked,terms.house_expense,closes])?;
    Ok(())
}

impl Ledger {
    /// Close sampling windows before a sweep. Only checked, unforfeited decisions
    /// qualify for a launch award. Awards transfer existing OpenAgents claims.
    pub fn release_decisions(&mut self, now: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut q = tx.prepare("SELECT d.settlement,j.provider,d.owner_linked FROM pylon_decision d JOIN pylon_job j ON j.settlement=d.settlement JOIN settlement s ON s.payment_hash=d.settlement WHERE d.released=0 AND d.closes_at<=? ORDER BY d.closes_at,s.seq")?;
        let jobs = q
            .query_map([now], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(q);
        for (key, party, linked) in jobs {
            tx.execute(
                "UPDATE pylon_decision SET released=1 WHERE settlement=?",
                [&key],
            )?;
            let job = job_in(&tx, &key)?.ok_or(Error::Invalid("missing decision"))?;
            if !linked || job.forfeited_msat > 0 || job.provider_msat == 0 {
                continue;
            }
            let count: i64 =
                tx.query_row("SELECT COUNT(*) FROM pylon_launch_party", [], |r| r.get(0))?;
            let seen: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM pylon_launch_party WHERE party=?)",
                [&party],
                |r| r.get(0),
            )?;
            if count >= 100 || seen {
                continue;
            }
            tx.execute(
                "INSERT INTO pylon_launch_party VALUES(?,?)",
                params![party, key],
            )?;
        }
        let mut q = tx.prepare("SELECT l.settlement,l.party FROM pylon_launch_party l LEFT JOIN bonus b ON b.settlement=l.settlement AND b.party=l.party AND b.kind='first_paid_call' WHERE b.outcome IS NULL OR b.outcome='bonus_unfunded' ORDER BY l.rowid")?;
        let pending = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(q);
        for (key, party) in pending {
            tx.execute("DELETE FROM bonus WHERE settlement=? AND party=? AND kind='first_paid_call' AND outcome='bonus_unfunded'", params![key,party])?;
            let job = job_in(&tx, &key)?.ok_or(Error::Invalid("missing decision"))?;
            // Reuse funded bonus obligations and their payout reservations.
            let input = crate::SettlementInput {
                key: key.clone(),
                resource: DECISION_RESOURCE.into(),
                plugin_id: Some(format!("pylon-decision:{party}")),
                release_id: None,
                price_msat: job.received_msat,
                received_msat: job.received_msat,
                rail: crate::Rail::Balance,
                payer_alias: None,
                settled_at: now,
                split: crate::Split::OpenAgents,
            };
            crate::first_call_bonus(&tx, &input, &party, 1_000_000)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The gateway calls this on a verified strike. Only decisions in the
    /// indicated window are affected, including held shares. Replays are harmless.
    pub fn forfeit_unpaid(&mut self, pylon: &str, since: i64) -> Result<Vec<Adjustment>> {
        let mut q = self.connection.prepare("SELECT j.receipt,d.closes_at FROM pylon_decision d JOIN pylon_job j ON j.settlement=d.settlement JOIN settlement s ON s.payment_hash=d.settlement WHERE d.pylon=? AND s.settled_at>=? AND d.released=0 ORDER BY s.seq")?;
        let receipts = q
            .query_map(params![pylon, since], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(q);
        receipts
            .into_iter()
            .map(|(receipt, at)| {
                let evidence = crate::digest(&format!("decision-strike:{pylon}:{since}:{receipt}"));
                self.forfeit_pylon_job(&receipt, &evidence, at)
            })
            .collect()
    }
}
