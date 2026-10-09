//! The payout worker: drains accrued shares to each payee's registered
//! destination, batched per payee, on two rails, restart-safe.
//!
//! Design: `docs/payments/2026-10-02-central-receive-and-splits.md`,
//! section 5, "How payouts go out". One [`tick`] does, in order:
//!
//! 1. **Recover.** A `planned` payout never reached the wallet (no
//!    reference was written), so it fails and its shares return. A
//!    `sending` or `unknown` payout is resolved only by looking up its
//!    recorded wallet reference: succeeded is `sent`, failed is `failed`,
//!    anything else stays `unknown` with its shares reserved. It is never
//!    sent again.
//! 2. **Plan.** For each party owed money (never OpenAgents itself): skip it
//!    while one of its payouts is open or while it is backing off after
//!    failures; resolve its destination (a party with none stays owed);
//!    send when the accrued amount reaches the rail's threshold, or when the
//!    oldest owed share is a day old and the amount is over 1 sat.
//! 3. **Send.** Reserve the shares (`planned`, `payout_item` rows drain
//!    them), get the wallet reference (a Lightning address's invoice and its
//!    payment hash; for Spark the payout id is the transfer's idempotency
//!    key), write it (`sending`), and only then pay. The outcome is `sent`,
//!    `failed` (shares return, the payee backs off), or `unknown`.
//!
//! Amounts go out in whole sats: a payout drains whole shares in msat and
//! sends the amount rounded down; the sub-sat remainder stays with
//! OpenAgents (`sent_msat` records what went out).
//!
//! The rails are a trait so the state machine is tested with a fake wallet;
//! the real ones (`crates/wallet` and the payout Spark wallet) are in
//! `openagents pay payouts`.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{AVAILABLE, Error, Ledger, OPENAGENTS, Payee, PayoutState, Result};

const COLUMNS: &str = "(
    id TEXT PRIMARY KEY,
    party TEXT NOT NULL REFERENCES payee(party),
    amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
    destination TEXT NOT NULL,
    rail TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('planned','sending','unknown','sent','failed')),
    wallet_reference TEXT,
    attempts INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sent_msat INTEGER CHECK(sent_msat IS NULL OR (sent_msat > 0 AND sent_msat <= amount_msat)),
    fee_msat INTEGER CHECK(fee_msat IS NULL OR fee_msat >= 0),
    invoice TEXT,
    error TEXT
)";

/// Create the `payout` table, or move a ledger written before the payout
/// worker (states `pending`/`succeeded`) to the current states: a pending
/// row without a reference is `planned`, one with a reference is `unknown`
/// (it may have been sent), and `succeeded` is `sent`.
pub(crate) fn create_table(connection: &mut Connection) -> Result<()> {
    connection.execute_batch(&format!("CREATE TABLE IF NOT EXISTS payout {COLUMNS};"))?;
    let old = |c: &Connection| -> Result<bool> {
        let sql: String = c.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='payout'",
            [],
            |r| r.get(0),
        )?;
        Ok(sql.contains("'pending'"))
    };
    if !old(connection)? {
        return Ok(());
    }
    // A table rebuild needs foreign keys off, which only applies outside a
    // transaction.
    connection.execute_batch("PRAGMA foreign_keys=OFF;")?;
    let migrated = (|| -> Result<()> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if old(&tx)? {
            tx.execute_batch(&format!(
                "CREATE TABLE payout_v2 {COLUMNS};
                 INSERT INTO payout_v2(id,party,amount_msat,destination,rail,state,wallet_reference,attempts,created_at,updated_at)
                   SELECT id,party,amount_msat,destination,rail,
                     CASE state WHEN 'pending' THEN CASE WHEN wallet_reference IS NULL THEN 'planned' ELSE 'unknown' END
                                WHEN 'succeeded' THEN 'sent' ELSE state END,
                     wallet_reference,attempts,created_at,updated_at FROM payout ORDER BY rowid;
                 DROP TABLE payout;
                 ALTER TABLE payout_v2 RENAME TO payout;"
            ))?;
        }
        tx.commit()?;
        Ok(())
    })();
    connection.execute_batch("PRAGMA foreign_keys=ON;")?;
    migrated
}

/// Consecutive failed payouts for `party` since its last non-failed one,
/// and when the newest of them failed.
pub(crate) fn failure_streak(connection: &Connection, party: &str) -> Result<(i64, Option<i64>)> {
    let mut stmt = connection
        .prepare("SELECT state,updated_at FROM payout WHERE party=? ORDER BY rowid DESC")?;
    let mut rows = stmt.query([party])?;
    let (mut count, mut last) = (0, None);
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(0)? != "failed" {
            break;
        }
        count += 1;
        last = last.or(Some(row.get(1)?));
    }
    Ok((count, last))
}

/// One payout row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payout {
    pub id: String,
    pub party: String,
    /// The shares it drained, in msat.
    pub amount_msat: i64,
    /// `kind:value`, as the payee was resolved when it was planned.
    pub destination: String,
    /// `lightning` or `spark`.
    pub rail: String,
    pub state: PayoutState,
    /// The payment hash (Lightning) or transfer id (Spark).
    pub wallet_reference: Option<String>,
    /// This payee's attempt number since its last payout that did not fail.
    pub attempts: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// What went out: the amount rounded down to whole sats.
    pub sent_msat: Option<i64>,
    pub fee_msat: Option<i64>,
    pub invoice: Option<String>,
    pub error: Option<String>,
}

const SELECT: &str = "SELECT id,party,amount_msat,destination,rail,state,wallet_reference,attempts,created_at,updated_at,sent_msat,fee_msat,invoice,error FROM payout";

fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Payout> {
    let state: String = r.get(5)?;
    Ok(Payout {
        id: r.get(0)?,
        party: r.get(1)?,
        amount_msat: r.get(2)?,
        destination: r.get(3)?,
        rail: r.get(4)?,
        state: PayoutState::parse(&state).unwrap_or(PayoutState::Unknown),
        wallet_reference: r.get(6)?,
        attempts: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        sent_msat: r.get(10)?,
        fee_msat: r.get(11)?,
        invoice: r.get(12)?,
        error: r.get(13)?,
    })
}

impl Ledger {
    pub fn payout(&self, id: &str) -> Result<Option<Payout>> {
        read_one(&self.connection, id)
    }

    /// Read bounded payout attempts that include an exact settlement share.
    /// Failed and unknown attempts remain visible. This performs no payment.
    pub fn settlement_payouts(&self, key: &str) -> Result<Vec<Payout>> {
        if key.is_empty() || key.len() > 512 || key.chars().any(char::is_control) {
            return Err(Error::Invalid("settlement payout identity"));
        }
        let mut query = self.connection.prepare("SELECT DISTINCT payout FROM (SELECT payout FROM payout_item WHERE settlement=?1 UNION SELECT payout FROM bonus_payout_item WHERE settlement=?1 UNION SELECT payout FROM commission_payout_item WHERE settlement=?1) ORDER BY payout LIMIT 257")?;
        let ids = query
            .query_map([key], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if ids.len() > 256 {
            return Err(Error::Invalid("settlement payout attempts exceed bound"));
        }
        ids.into_iter()
            .map(|id| {
                self.payout(&id)?
                    .ok_or(Error::Invalid("settlement payout is absent"))
            })
            .collect()
    }

    /// Payouts by state, oldest first; `None` lists every payout.
    pub fn payouts(&self, states: Option<&[PayoutState]>) -> Result<Vec<Payout>> {
        let mut stmt = self
            .connection
            .prepare(&format!("{SELECT} ORDER BY rowid"))?;
        let rows = stmt.query_map([], read)?;
        let all: Vec<Payout> = rows.collect::<std::result::Result<_, _>>()?;
        Ok(all
            .into_iter()
            .filter(|p| states.is_none_or(|s| s.contains(&p.state)))
            .collect())
    }

    /// Parties with unreserved, unpaid claims, except OpenAgents itself.
    pub fn owed_parties(&self) -> Result<Vec<String>> {
        let mut stmt = self.connection.prepare(&format!(
            "SELECT DISTINCT s.party FROM payable_share s WHERE s.amount_msat>0 AND s.party!=? AND {AVAILABLE} ORDER BY s.party"
        ))?;
        let rows = stmt.query_map([OPENAGENTS], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// When the oldest unreserved claim of `party` was settled.
    pub fn oldest_owed(&self, party: &str) -> Result<Option<i64>> {
        Ok(self.connection.query_row(
            &format!("SELECT MIN(t.settled_at) FROM payable_share s JOIN settlement t ON t.payment_hash=s.settlement WHERE s.party=? AND s.amount_msat>0 AND {AVAILABLE}"),
            [party],
            |r| r.get(0),
        )?)
    }

    /// The newest plugin release `party` earned under, whose signed `payout`
    /// is its first-choice destination.
    pub fn latest_release(&self, party: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT t.release_id FROM share s JOIN settlement t ON t.payment_hash=s.settlement WHERE s.party=? AND t.release_id IS NOT NULL ORDER BY t.seq DESC LIMIT 1",
                [party],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Consecutive failed payouts for `party` and the newest one's time.
    pub fn failure_streak(&self, party: &str) -> Result<(i64, Option<i64>)> {
        failure_streak(&self.connection, party)
    }

    /// Journal the wallet reference before anything is sent: `planned` to
    /// `sending`, with the invoice (Lightning) and the amount that goes out.
    pub fn begin_send(
        &mut self,
        id: &str,
        reference: &str,
        invoice: Option<&str>,
        sent_msat: i64,
        at: i64,
    ) -> Result<()> {
        if reference.is_empty() {
            return Err(Error::Invalid("a sending payout needs a wallet reference"));
        }
        let changed = self.connection.execute(
            "UPDATE payout SET state='sending',wallet_reference=?,invoice=?,sent_msat=?,updated_at=? WHERE id=? AND state='planned'",
            params![reference, invoice, sent_msat, at, id],
        )?;
        if changed != 1 {
            return Err(Error::Invalid("only a planned payout starts sending"));
        }
        Ok(())
    }

    /// Record an outcome on an open payout: its state, the routing fee, and
    /// why it failed or is unknown.
    pub fn finish_payout(
        &mut self,
        id: &str,
        state: PayoutState,
        fee_msat: Option<i64>,
        error: Option<&str>,
        at: i64,
    ) -> Result<()> {
        self.set_payout_state(id, state, None, at)?;
        self.connection.execute(
            "UPDATE payout SET fee_msat=COALESCE(?,fee_msat),error=? WHERE id=?",
            params![fee_msat, error, id],
        )?;
        Ok(())
    }
}

pub(crate) fn read_one(connection: &Connection, id: &str) -> Result<Option<Payout>> {
    Ok(connection
        .query_row(&format!("{SELECT} WHERE id=?"), [id], read)
        .optional()?)
}

/// When payouts go out and what they may cost.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Send to a Spark address once this much is owed.
    pub spark_threshold_msat: i64,
    /// Send to a Lightning address once this much is owed (routing fees
    /// bite on small amounts).
    pub lightning_threshold_msat: i64,
    /// Below the threshold, send once the oldest owed share is this old...
    pub daily_after_secs: i64,
    /// ...and more than this much is owed.
    pub daily_min_msat: i64,
    /// The Lightning routing-fee cap: this share of the amount...
    pub fee_cap_bps: i64,
    /// ...but never less than this.
    pub fee_cap_min_msat: i64,
    /// After a failure, wait `base * 2^(failures - 1)`, at most `max`.
    pub backoff_base_secs: i64,
    pub backoff_max_secs: i64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            spark_threshold_msat: 100_000,
            lightning_threshold_msat: 1_000_000,
            daily_after_secs: 86_400,
            daily_min_msat: 1_000,
            fee_cap_bps: 100,
            fee_cap_min_msat: 5_000,
            backoff_base_secs: 300,
            backoff_max_secs: 6 * 3600,
        }
    }
}

impl Policy {
    /// Pylon provider sweeps (`docs/compute/verse-compute.md`, Payments):
    /// a provider is paid once 1,000 sats are owed on either rail, or once
    /// its oldest owed share is ten minutes old. Never per job.
    #[must_use]
    pub fn pylon_sweeps() -> Self {
        Self {
            spark_threshold_msat: 1_000_000,
            lightning_threshold_msat: 1_000_000,
            daily_after_secs: 600,
            ..Self::default()
        }
    }

    /// Seconds to wait after the `failures`-th consecutive failure.
    #[must_use]
    pub fn backoff_secs(&self, failures: i64) -> i64 {
        if failures <= 0 {
            return 0;
        }
        let shift = u32::try_from(failures - 1).unwrap_or(u32::MAX).min(30);
        self.backoff_base_secs
            .saturating_mul(1i64 << shift)
            .min(self.backoff_max_secs)
    }

    /// The routing-fee cap for sending `amount_msat` over Lightning.
    #[must_use]
    pub fn fee_cap_msat(&self, amount_msat: i64) -> i64 {
        (amount_msat.saturating_mul(self.fee_cap_bps) / 10_000).max(self.fee_cap_min_msat)
    }
}

/// An invoice a Lightning address returned for a payout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub bolt11: String,
    /// 64 lowercase hex digits.
    pub payment_hash: String,
    pub amount_msat: i64,
}

/// What a send did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Sent {
        fee_msat: i64,
    },
    /// Nothing went out.
    Failed(String),
    /// It may have gone out; only a lookup can tell.
    Unknown(String),
}

/// What a wallet lookup of a reference says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Sent {
        fee_msat: i64,
    },
    Failed(String),
    Pending,
    /// The wallet has no record. After a crash this does not prove nothing
    /// went out, so the payout stays `unknown`.
    Absent,
}

/// The two payout rails. Every call blocks.
pub trait Rails {
    /// Resolve a Lightning address (LNURL-pay) to an invoice for exactly
    /// `amount_msat`.
    fn lightning_invoice(
        &self,
        address: &str,
        amount_msat: i64,
    ) -> std::result::Result<Invoice, String>;
    /// Pay `invoice` from the receiver wallet within `max_fee_msat`.
    fn pay_lightning(&self, invoice: &Invoice, max_fee_msat: i64) -> Outcome;
    fn lookup_lightning(&self, payment_hash: &str) -> std::result::Result<Lookup, String>;
    /// Make sure the payout Spark wallet holds `amount_sats` (topping it up
    /// from the receiver wallet if it does not). Nothing goes to the payee.
    fn fund_spark(&self, amount_sats: u64) -> std::result::Result<(), String>;
    /// Send `amount_sats` to a Spark address; `key` (a UUID) is the
    /// transfer's idempotency key and id.
    fn pay_spark(&self, address: &str, amount_sats: u64, key: &str) -> Outcome;
    fn lookup_spark(&self, key: &str) -> std::result::Result<Lookup, String>;
}

/// One thing a tick did or decided, for the log. It names no destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A payout changed state.
    Payout {
        id: String,
        party: String,
        rail: String,
        amount_msat: i64,
        state: PayoutState,
        error: Option<String>,
    },
    /// The party is owed but has no payout destination.
    NoDestination { party: String, owed_msat: i64 },
    /// The party's destination is a kind no rail pays (a node key).
    Unpayable { party: String, kind: String },
    /// The party is backing off after failures until `until`.
    BackingOff { party: String, until: i64 },
}

fn step(p: &Payout, state: PayoutState, error: Option<String>) -> Step {
    Step::Payout {
        id: p.id.clone(),
        party: p.party.clone(),
        rail: p.rail.clone(),
        amount_msat: p.amount_msat,
        state,
        error,
    }
}

/// The destination's value: `destination` is `kind:value`.
fn destination_value(destination: &str) -> &str {
    destination.split_once(':').map_or(destination, |(_, v)| v)
}

fn recover(ledger: &mut Ledger, rails: &dyn Rails, now: i64, steps: &mut Vec<Step>) -> Result<()> {
    let open = ledger.payouts(Some(&[
        PayoutState::Planned,
        PayoutState::Sending,
        PayoutState::Unknown,
    ]))?;
    for p in open {
        let Some(reference) = p
            .wallet_reference
            .clone()
            .filter(|_| p.state != PayoutState::Planned)
        else {
            let why = "stopped before sending";
            ledger.finish_payout(&p.id, PayoutState::Failed, None, Some(why), now)?;
            steps.push(step(&p, PayoutState::Failed, Some(why.into())));
            continue;
        };
        let found = if p.rail == "spark" {
            rails.lookup_spark(&reference)
        } else {
            rails.lookup_lightning(&reference)
        };
        let (state, fee, error) = match found {
            Ok(Lookup::Sent { fee_msat }) => (PayoutState::Sent, Some(fee_msat), None),
            Ok(Lookup::Failed(why)) => (PayoutState::Failed, None, Some(why)),
            Ok(Lookup::Pending) => (PayoutState::Unknown, None, Some("still pending".into())),
            Ok(Lookup::Absent) => (
                PayoutState::Unknown,
                None,
                Some("the wallet has no record of it".into()),
            ),
            Err(why) => (
                PayoutState::Unknown,
                None,
                Some(format!("lookup failed: {why}")),
            ),
        };
        let changed = state != p.state || error != p.error;
        ledger.finish_payout(&p.id, state, fee, error.as_deref(), now)?;
        if changed {
            steps.push(step(&p, state, error));
        }
    }
    Ok(())
}

/// Finds a party's payout destination at a time.
pub type Resolve<'a> = dyn FnMut(&mut Ledger, &str, i64) -> Result<Option<Payee>> + 'a;

/// One pass of the worker at `now` (Unix seconds). `resolve` returns the
/// party's destination (normally [`Ledger::resolve_payee`] over relay reads);
/// `new_id` returns a fresh UUID for a payout.
pub fn tick(
    ledger: &mut Ledger,
    rails: &dyn Rails,
    policy: &Policy,
    now: i64,
    resolve: &mut Resolve<'_>,
    new_id: &mut dyn FnMut() -> String,
) -> Result<Vec<Step>> {
    let mut steps = vec![];
    recover(ledger, rails, now, &mut steps)?;
    if ledger.commission_payouts_held()? {
        return Ok(steps);
    }
    let open: Vec<String> = ledger
        .payouts(Some(&[
            PayoutState::Planned,
            PayoutState::Sending,
            PayoutState::Unknown,
        ]))?
        .into_iter()
        .map(|p| p.party)
        .collect();
    for party in ledger.owed_parties()? {
        if open.contains(&party) {
            continue;
        }
        let (failures, last_failed) = ledger.failure_streak(&party)?;
        if let Some(at) = last_failed {
            let until = at + policy.backoff_secs(failures);
            if now < until {
                steps.push(Step::BackingOff { party, until });
                continue;
            }
        }
        let shares = ledger.available_shares(&party)?;
        let owed: i64 = shares.iter().map(|s| s.amount_msat).sum();
        let Some(payee) = resolve(ledger, &party, now)? else {
            steps.push(Step::NoDestination {
                party,
                owed_msat: owed,
            });
            continue;
        };
        if !ledger.commission_payout_qualified(&party, &payee.destination_kind, owed)? {
            steps.push(Step::Unpayable {
                party,
                kind: "commission-policy-or-minimum".into(),
            });
            continue;
        }
        let threshold = match payee.destination_kind.as_str() {
            "spark" => policy.spark_threshold_msat,
            "lud16" => policy.lightning_threshold_msat,
            other => {
                steps.push(Step::Unpayable {
                    party,
                    kind: other.into(),
                });
                continue;
            }
        };
        let daily = ledger
            .oldest_owed(&party)?
            .is_some_and(|at| now - at >= policy.daily_after_secs)
            && owed > policy.daily_min_msat;
        let sats = owed / 1000;
        if (owed < threshold && !daily) || sats == 0 {
            continue;
        }
        let id = new_id();
        match ledger.reserve_payout_at_destination(&id, &party, &shares, now, &payee) {
            Ok(_) => {}
            Err(Error::Conflict(_)) => continue,
            Err(error) => return Err(error),
        }
        let p = ledger
            .payout(&id)?
            .ok_or(Error::Invalid("reserved payout vanished"))?;
        steps.push(step(&p, PayoutState::Planned, None));
        send(ledger, rails, policy, &p, sats, now, &mut steps)?;
    }
    Ok(steps)
}

#[allow(clippy::too_many_arguments)]
fn send(
    ledger: &mut Ledger,
    rails: &dyn Rails,
    policy: &Policy,
    p: &Payout,
    sats: i64,
    now: i64,
    steps: &mut Vec<Step>,
) -> Result<()> {
    let spark = p.rail == "spark";
    let address = destination_value(&p.destination).to_owned();
    let sent_msat = sats * 1000;
    let fail = |ledger: &mut Ledger, steps: &mut Vec<Step>, why: String| -> Result<()> {
        ledger.finish_payout(&p.id, PayoutState::Failed, None, Some(&why), now)?;
        steps.push(step(p, PayoutState::Failed, Some(why)));
        Ok(())
    };
    let amount_sats = u64::try_from(sats).map_err(|_| Error::Invalid("payout amount"))?;
    // Get the wallet reference, then journal it, then pay.
    let outcome = if spark {
        if let Err(why) = rails.fund_spark(amount_sats) {
            return fail(ledger, steps, format!("funding the Spark wallet: {why}"));
        }
        ledger.begin_send(&p.id, &p.id, None, sent_msat, now)?;
        steps.push(step(p, PayoutState::Sending, None));
        rails.pay_spark(&address, amount_sats, &p.id)
    } else {
        let invoice = match rails.lightning_invoice(&address, sent_msat) {
            Ok(invoice) => invoice,
            Err(why) => return fail(ledger, steps, format!("resolving the address: {why}")),
        };
        if invoice.amount_msat != sent_msat {
            return fail(
                ledger,
                steps,
                format!(
                    "the address returned an invoice for {} msat, not {sent_msat}",
                    invoice.amount_msat
                ),
            );
        }
        ledger.begin_send(
            &p.id,
            &invoice.payment_hash,
            Some(&invoice.bolt11),
            sent_msat,
            now,
        )?;
        steps.push(step(p, PayoutState::Sending, None));
        rails.pay_lightning(&invoice, policy.fee_cap_msat(sent_msat))
    };
    let (state, fee, error) = match outcome {
        Outcome::Sent { fee_msat } => (PayoutState::Sent, Some(fee_msat), None),
        Outcome::Failed(why) => (PayoutState::Failed, None, Some(why)),
        Outcome::Unknown(why) => (PayoutState::Unknown, None, Some(why)),
    };
    ledger.finish_payout(&p.id, state, fee, error.as_deref(), now)?;
    steps.push(step(p, state, error));
    Ok(())
}
