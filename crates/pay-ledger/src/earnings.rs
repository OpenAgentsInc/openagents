//! Private payee statements and account destination settings.
//!
//! The transport authorizes the payee before calling these methods. Queries
//! select one party in SQL and omit payer aliases, invoices, and other parties'
//! shares. Account settings are a fallback to signed sources. Reservations
//! continue to pin the destination they already recorded.

use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Serialize;

use crate::{AVAILABLE, Error, Ledger, PayoutState, Result, payee};

pub const MAX_PAGE: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct DestinationSetting {
    pub version: u64,
    pub value: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Obligation {
    pub role: String,
    pub amount_msat: i64,
    pub state: String,
    pub payout: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Earning {
    pub sequence: i64,
    pub resource: String,
    pub plugin_id: Option<String>,
    pub release_id: Option<String>,
    pub settled_at: i64,
    pub rule_version: i64,
    pub obligations: Vec<Obligation>,
}

/// The wallet reference identifies the same attempt for reconciliation. Raw
/// wallet errors and invoices stay with the operator; a reason digest permits
/// comparison without exporting those values.
#[derive(Debug, Clone, Serialize)]
pub struct Payment {
    pub cursor: i64,
    pub id: String,
    pub amount_msat: i64,
    pub destination: String,
    pub rail: String,
    pub state: PayoutState,
    pub wallet_reference: Option<String>,
    pub attempts: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub sent_msat: Option<i64>,
    pub fee_msat: Option<i64>,
    pub reason: Option<&'static str>,
    pub reason_digest: Option<String>,
    pub lookup_required: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Figures {
    pub earned_msat: i64,
    pub accrued_msat: i64,
    pub reserved_msat: i64,
    /// Claims consumed by sent payouts, including sub-sat rounding.
    pub consumed_msat: i64,
    /// The exact rail amount in successful, journaled attempts.
    pub sent_msat: i64,
    pub rounding_msat: i64,
    pub unverified_sent_msat: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Statement {
    pub figures: Figures,
    pub earnings: Vec<Earning>,
    pub payouts: Vec<Payment>,
    pub next_earning: Option<i64>,
    pub next_payout: Option<i64>,
}

pub(crate) const TABLES: &str = "CREATE TABLE IF NOT EXISTS account_payout (
    party TEXT PRIMARY KEY,
    version INTEGER NOT NULL CHECK(version > 0),
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);";

impl Ledger {
    pub fn account_payout(&self, party: &str) -> Result<Option<DestinationSetting>> {
        Ok(self
            .connection
            .query_row(
                "SELECT version,value,updated_at FROM account_payout WHERE party=?",
                [party],
                |r| {
                    Ok(DestinationSetting {
                        version: r.get(0)?,
                        value: r.get(1)?,
                        updated_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Compare and replace the account fallback. Zero creates the first
    /// setting; an exact version changes it. Signed sources retain priority.
    /// This transaction never updates a payout or its reserved shares.
    pub fn change_account_payout(
        &mut self,
        party: &str,
        expected_version: u64,
        value: &str,
        now: i64,
    ) -> Result<DestinationSetting> {
        if party.is_empty() || party == crate::OPENAGENTS || value.len() > 256 {
            return Err(Error::Invalid("account payout setting"));
        }
        let found = payee::resolve(&payee::Sources {
            account_payout: Some(value.to_owned()),
            ..Default::default()
        })
        .ok_or(Error::Invalid("unsupported mainnet payout address"))?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(u64, String)> = tx
            .query_row(
                "SELECT version,value FROM account_payout WHERE party=?",
                [party],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if previous.as_ref().map_or(0, |v| v.0) != expected_version {
            return Err(Error::Conflict("destination version changed"));
        }
        let version = expected_version
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Invalid("destination version overflow"))?;
        if previous.as_ref().is_some_and(|v| v.1 != found.value) && party.starts_with("referrer:") {
            crate::commission_abuse::hold_destination_in(
                &tx,
                party,
                version,
                u64::try_from(now).map_err(|_| Error::Invalid("destination clock"))?,
            )?;
        }
        tx.execute("INSERT INTO account_payout VALUES(?,?,?,?) ON CONFLICT(party) DO UPDATE SET version=excluded.version,value=excluded.value,updated_at=excluded.updated_at", params![party, version, found.value, now])?;
        tx.execute("INSERT INTO payee VALUES(?,?,?,'account',?) ON CONFLICT(party) DO UPDATE SET destination_kind=excluded.destination_kind,destination_value=excluded.destination_value,verified_at=excluded.verified_at WHERE payee.source='account'", params![party, found.kind.as_str(), found.value, now])?;
        tx.commit()?;
        Ok(DestinationSetting {
            version,
            value: found.value,
            updated_at: now,
        })
    }

    /// Return a bounded statement over one SQLite read snapshot. Cursors are
    /// exclusive settlement sequences and payout row ids, scoped to the same
    /// party on every page. Totals cover the whole party ledger.
    pub fn earnings_statement(
        &mut self,
        party: &str,
        after_earning: i64,
        after_payout: i64,
        limit: usize,
    ) -> Result<Statement> {
        if limit == 0 || limit > MAX_PAGE || after_earning < 0 || after_payout < 0 {
            return Err(Error::Invalid("statement page bounds"));
        }
        let tx = self.connection.transaction()?;
        let earned_msat = tx.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM payable_share WHERE party=?",
            [party],
            |r| r.get(0),
        )?;
        let accrued_msat = tx.query_row(&format!("SELECT COALESCE(SUM(s.amount_msat),0) FROM payable_share s WHERE s.party=? AND {AVAILABLE}"), [party], |r| r.get(0))?;
        let (reserved_msat, consumed_msat, sent_msat, unverified_sent_msat) = tx.query_row("SELECT COALESCE(SUM(CASE WHEN state IN ('planned','sending','unknown') THEN amount_msat ELSE 0 END),0),COALESCE(SUM(CASE WHEN state='sent' THEN amount_msat ELSE 0 END),0),COALESCE(SUM(CASE WHEN state='sent' THEN sent_msat ELSE 0 END),0),COALESCE(SUM(CASE WHEN state='sent' AND sent_msat IS NULL THEN amount_msat ELSE 0 END),0) FROM payout WHERE party=?", [party], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?;
        let figures = Figures {
            earned_msat,
            accrued_msat,
            reserved_msat,
            consumed_msat,
            sent_msat,
            rounding_msat: consumed_msat - sent_msat - unverified_sent_msat,
            unverified_sent_msat,
        };
        let mut stmt = tx.prepare("SELECT DISTINCT t.seq,t.resource,t.plugin_id,t.release_id,t.settled_at,t.rule_version FROM payable_share s JOIN settlement t ON t.payment_hash=s.settlement WHERE s.party=? AND t.seq>? ORDER BY t.seq LIMIT ?")?;
        let mut earnings = stmt
            .query_map(params![party, after_earning, limit + 1], |r| {
                Ok(Earning {
                    sequence: r.get(0)?,
                    resource: r
                        .get::<_, String>(1)?
                        .split(['?', '#'])
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    plugin_id: r.get(2)?,
                    release_id: r.get(3)?,
                    settled_at: r.get(4)?,
                    rule_version: r.get(5)?,
                    obligations: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let next_earning = (earnings.len() > limit).then(|| earnings[limit - 1].sequence);
        earnings.truncate(limit);
        drop(stmt);
        for earning in &mut earnings {
            let mut stmt = tx.prepare(
                "WITH active AS (
                SELECT i.settlement,i.party,i.role,p.state,p.id
                FROM (SELECT payout,settlement,party,role FROM payout_item
                      UNION ALL SELECT payout,settlement,party,role FROM bonus_payout_item
                      UNION ALL SELECT payout,settlement,party,role FROM commission_payout_item) i
                JOIN payout p ON p.id=i.payout AND p.state!='failed'
            )
            SELECT s.role,s.amount_msat,a.state,a.id
            FROM payable_share s JOIN settlement t ON t.payment_hash=s.settlement
            LEFT JOIN active a ON a.settlement=s.settlement AND a.party=s.party AND a.role=s.role
            WHERE s.party=? AND t.seq=? ORDER BY s.role",
            )?;
            earning.obligations = stmt
                .query_map(params![party, earning.sequence], |r| {
                    Ok(Obligation {
                        role: r.get(0)?,
                        amount_msat: r.get(1)?,
                        state: r
                            .get::<_, Option<String>>(2)?
                            .unwrap_or_else(|| "accrued".into()),
                        payout: r.get(3)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
        }
        let mut stmt = tx.prepare(
            "SELECT rowid,id FROM payout WHERE party=? AND rowid>? ORDER BY rowid LIMIT ?",
        )?;
        let ids = stmt
            .query_map(params![party, after_payout, limit + 1], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let next_payout = (ids.len() > limit).then(|| ids[limit - 1].0);
        drop(stmt);
        let payouts = ids
            .into_iter()
            .take(limit)
            .map(|(cursor, id)| {
                let p =
                    crate::payout::read_one(&tx, &id)?.ok_or(Error::Invalid("missing payout"))?;
                Ok(payment(cursor, p))
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(Statement {
            figures,
            earnings,
            payouts,
            next_earning,
            next_payout,
        })
    }

    pub fn earnings_payout(&self, party: &str, id: &str) -> Result<Option<Payment>> {
        let cursor = self
            .connection
            .query_row(
                "SELECT rowid FROM payout WHERE party=? AND id=?",
                params![party, id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        cursor
            .map(|cursor| {
                self.payout(id)?
                    .map(|p| payment(cursor, p))
                    .ok_or(Error::Invalid("missing payout"))
            })
            .transpose()
    }
}

fn payment(cursor: i64, p: crate::payout::Payout) -> Payment {
    let reason = match p.state {
        PayoutState::Failed => Some("payout_failed"),
        PayoutState::Unknown => Some("outcome_unresolved"),
        _ => None,
    };
    Payment {
        cursor,
        id: p.id,
        amount_msat: p.amount_msat,
        destination: p.destination,
        rail: p.rail,
        state: p.state,
        wallet_reference: p.wallet_reference,
        attempts: p.attempts,
        created_at: p.created_at,
        updated_at: p.updated_at,
        sent_msat: p.sent_msat,
        fee_msat: p.fee_msat,
        reason,
        reason_digest: p.error.as_deref().map(crate::digest),
        lookup_required: matches!(p.state, PayoutState::Sending | PayoutState::Unknown),
    }
}
