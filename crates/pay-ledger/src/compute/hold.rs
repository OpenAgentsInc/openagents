//! Holds on the purchased compute balance (#10710) and their settlement
//! (#10718).
//!
//! A hold reserves an exact quote's maximum for one funded request before
//! anything is provisioned. It is bound to the account, the quote's digest,
//! the funded execution identity, and the digest of the request's terms, so
//! a retry of the same request reuses the hold and a request whose bytes or
//! terms changed conflicts. Reservation runs in one immediate transaction,
//! so parallel requests cannot hold more than the account has available.
//!
//! A hold is `held`, `unknown` (a crash or a provider loss left its outcome
//! open), or `settled`. Nothing frees an `unknown` hold except a settlement
//! with a known charge; a restart changes nothing. Settlement posts the
//! charge to the central settlement table as the balance debit
//! `debit:<hold>` in the same transaction, and releases the rest of the
//! hold to the balance. A release is not a refund: nothing was paid for it.
//!
//! Conservation: for every account, credited = available + held + settled.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Error, Ledger, Rail, Recorded, Result, SettlementInput, Split, record_settlement_in};

/// The resource a retail compute settlement records.
pub const RETAIL_RESOURCE: &str = "openagents.cloud.retail.v1";

/// A request to hold funds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldRequest {
    /// The funded request's identity.
    pub id: String,
    pub account: String,
    /// The quote's digest.
    pub quote: String,
    /// The funded execution identity.
    pub execution: String,
    /// The digest of the request's exact terms.
    pub terms: String,
    pub amount_msat: i64,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldState {
    Held,
    /// The outcome is open; the whole hold stays reserved.
    Unknown,
    Settled,
}

impl HoldState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Unknown => "unknown",
            Self::Settled => "settled",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "held" => Self::Held,
            "unknown" => Self::Unknown,
            "settled" => Self::Settled,
            _ => return None,
        })
    }
}

/// A hold as the ledger keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    pub request: HoldRequest,
    pub state: HoldState,
    /// The settled charge, once settled.
    pub charge_msat: Option<i64>,
    pub settled_at: Option<i64>,
}

impl Hold {
    /// What settlement released back to the balance.
    #[must_use]
    pub fn released_msat(&self) -> Option<i64> {
        self.charge_msat.map(|c| self.request.amount_msat - c)
    }
}

/// An account's balance, split so that
/// `credited == available + held + settled`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComputeBalance {
    /// Every credited top-up.
    pub credited_msat: i64,
    /// What can be held now.
    pub available_msat: i64,
    /// Held by `held` and `unknown` holds.
    pub held_msat: i64,
    /// Charged by settled holds.
    pub settled_msat: i64,
    /// Returned to the balance by settled holds. Already counted in
    /// `available_msat`; shown separately, never as a refund.
    pub released_msat: i64,
}

fn balance_in(connection: &Connection, account: &str) -> Result<ComputeBalance> {
    let credited: i64 = connection.query_row(
        "SELECT COALESCE(SUM(amount_msat),0) FROM compute_credit WHERE account=?",
        [account],
        |r| r.get(0),
    )?;
    let (held, settled, released): (i64, i64, i64) = connection.query_row(
        "SELECT COALESCE(SUM(CASE WHEN state IN ('held','unknown') THEN amount_msat ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN state='settled' THEN charge_msat ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN state='settled' THEN amount_msat-charge_msat ELSE 0 END),0)
         FROM compute_hold WHERE account=?",
        [account],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(ComputeBalance {
        credited_msat: credited,
        available_msat: credited - held - settled,
        held_msat: held,
        settled_msat: settled,
        released_msat: released,
    })
}

impl Ledger {
    /// The account's balance.
    pub fn compute_balance(&self, account: &str) -> Result<ComputeBalance> {
        balance_in(&self.connection, account)
    }

    /// Hold `request.amount_msat` for one funded request. The same request
    /// with the same quote, execution, terms, and amount returns its hold
    /// unchanged, whatever state it is in; the same identity with anything
    /// else conflicts.
    ///
    /// # Errors
    ///
    /// [`Error::Insufficient`] when the account cannot cover the hold,
    /// [`Error::Conflict`] for a reused identity, and [`Error::Invalid`] for
    /// a malformed request.
    pub fn reserve(&mut self, request: &HoldRequest) -> Result<Hold> {
        if request.id.is_empty()
            || request.quote.is_empty()
            || request.execution.is_empty()
            || request.terms.is_empty()
            || request.amount_msat <= 0
        {
            return Err(Error::Invalid("hold identity or amount"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = read_hold(&tx, "id", &request.id)? {
            let mut same = existing.request.clone();
            same.at = request.at;
            if &same == request {
                return Ok(existing);
            }
            return Err(Error::Conflict("the hold exists with other terms"));
        }
        if read_hold(&tx, "execution", &request.execution)?.is_some() {
            return Err(Error::Conflict("the execution already has a hold"));
        }
        let balance = balance_in(&tx, &request.account)?;
        if balance.available_msat < request.amount_msat {
            return Err(Error::Insufficient {
                available_msat: balance.available_msat,
            });
        }
        tx.execute(
            "INSERT INTO compute_hold(id,account,quote,execution,terms,amount_msat,state,created_at) VALUES(?,?,?,?,?,?,'held',?)",
            params![
                request.id,
                request.account,
                request.quote,
                request.execution,
                request.terms,
                request.amount_msat,
                request.at
            ],
        )?;
        let hold = read_hold(&tx, "id", &request.id)?.ok_or(Error::Invalid("missing hold"))?;
        tx.commit()?;
        Ok(hold)
    }

    /// Mark a hold's outcome unknown. The whole hold stays reserved until a
    /// settlement with a known charge.
    pub fn mark_hold_unknown(&mut self, id: &str) -> Result<Hold> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let hold = read_hold(&tx, "id", id)?.ok_or(Error::Invalid("no such hold"))?;
        if hold.state == HoldState::Held {
            tx.execute("UPDATE compute_hold SET state='unknown' WHERE id=?", [id])?;
        }
        let hold = read_hold(&tx, "id", id)?.ok_or(Error::Invalid("missing hold"))?;
        tx.commit()?;
        Ok(hold)
    }

    /// One hold by its funded request identity.
    pub fn hold(&self, id: &str) -> Result<Option<Hold>> {
        read_hold(&self.connection, "id", id)
    }

    /// One hold by its execution identity.
    pub fn hold_for_execution(&self, execution: &str) -> Result<Option<Hold>> {
        read_hold(&self.connection, "execution", execution)
    }

    /// An account's holds, oldest first.
    pub fn holds(&self, account: &str) -> Result<Vec<Hold>> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM compute_hold WHERE account=? ORDER BY created_at, id")?;
        let ids = statement
            .query_map([account], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| read_hold(&self.connection, "id", id)?.ok_or(Error::Invalid("missing")))
            .collect()
    }

    /// Settle a hold at `charge_msat`: post the charge as the balance debit
    /// `debit:<hold>` and release the rest, in one transaction. Settling a
    /// settled hold at the same charge returns it and its settlement
    /// unchanged; at another charge it conflicts.
    ///
    /// # Errors
    ///
    /// A charge above the hold, a conflicting replay, or an unknown hold.
    pub fn settle_hold(
        &mut self,
        id: &str,
        charge_msat: i64,
        at: i64,
    ) -> Result<(Hold, Option<Recorded>)> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let hold = read_hold(&tx, "id", id)?.ok_or(Error::Invalid("no such hold"))?;
        if charge_msat < 0 || charge_msat > hold.request.amount_msat {
            return Err(Error::Invalid("a charge is within its hold"));
        }
        if hold.state == HoldState::Settled {
            if hold.charge_msat != Some(charge_msat) {
                return Err(Error::Conflict("the hold settled at another charge"));
            }
            let recorded = crate::read_record(&tx, &format!("debit:{id}"))?;
            return Ok((hold, recorded));
        }
        tx.execute(
            "UPDATE compute_hold SET state='settled', charge_msat=?, settled_at=? WHERE id=?",
            params![charge_msat, at, id],
        )?;
        let recorded = if charge_msat > 0 {
            Some(record_settlement_in(
                &tx,
                SettlementInput {
                    key: format!("debit:{id}"),
                    resource: RETAIL_RESOURCE.into(),
                    plugin_id: None,
                    release_id: None,
                    price_msat: charge_msat,
                    received_msat: charge_msat,
                    rail: Rail::Balance,
                    payer_alias: None,
                    settled_at: at,
                    split: Split::OpenAgents,
                },
            )?)
        } else {
            None
        };
        let hold = read_hold(&tx, "id", id)?.ok_or(Error::Invalid("missing hold"))?;
        tx.commit()?;
        Ok((hold, recorded))
    }
}

fn read_hold(connection: &Connection, column: &str, value: &str) -> Result<Option<Hold>> {
    let sql = if column == "id" {
        "SELECT id,account,quote,execution,terms,amount_msat,created_at,state,charge_msat,settled_at FROM compute_hold WHERE id=?"
    } else {
        "SELECT id,account,quote,execution,terms,amount_msat,created_at,state,charge_msat,settled_at FROM compute_hold WHERE execution=?"
    };
    let row = connection
        .query_row(sql, [value], |r| {
            Ok((
                HoldRequest {
                    id: r.get(0)?,
                    account: r.get(1)?,
                    quote: r.get(2)?,
                    execution: r.get(3)?,
                    terms: r.get(4)?,
                    amount_msat: r.get(5)?,
                    at: r.get(6)?,
                },
                r.get::<_, String>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<i64>>(9)?,
            ))
        })
        .optional()?;
    let Some((request, state, charge_msat, settled_at)) = row else {
        return Ok(None);
    };
    Ok(Some(Hold {
        request,
        state: HoldState::parse(&state).ok_or(Error::Invalid("hold state"))?,
        charge_msat,
        settled_at,
    }))
}
