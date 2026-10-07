//! Append-only reductions of original unpaid claims. Reserved claims stay held.
use crate::{AVAILABLE, Error, Ledger, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

pub(crate) const TABLES: &str = "CREATE TABLE IF NOT EXISTS payable_adjustment (
 id TEXT PRIMARY KEY, settlement TEXT NOT NULL REFERENCES settlement(payment_hash),
 party TEXT NOT NULL, role TEXT NOT NULL, evidence TEXT NOT NULL,
 requested_msat INTEGER NOT NULL CHECK(requested_msat>0),
 reduced_msat INTEGER NOT NULL CHECK(reduced_msat>=0),
 loss_msat INTEGER NOT NULL CHECK(loss_msat>=0), at INTEGER NOT NULL,
 CHECK(requested_msat=reduced_msat+loss_msat)
);";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Adjustment {
    pub id: String,
    pub settlement: String,
    pub party: String,
    pub role: String,
    pub evidence: String,
    pub requested_msat: i64,
    pub reduced_msat: i64,
    /// Money already reserved, consumed, or absent cannot be clawed back.
    pub loss_msat: i64,
    pub at: i64,
}
fn read(c: &Connection, id: &str) -> Result<Option<Adjustment>> {
    Ok(c.query_row("SELECT id,settlement,party,role,evidence,requested_msat,reduced_msat,loss_msat,at FROM payable_adjustment WHERE id=?", [id], |r| Ok(Adjustment { id:r.get(0)?,settlement:r.get(1)?,party:r.get(2)?,role:r.get(3)?,evidence:r.get(4)?,requested_msat:r.get(5)?,reduced_msat:r.get(6)?,loss_msat:r.get(7)?,at:r.get(8)? })).optional()?)
}
/// The selected native adapter verifies the evidence before posting this journal.
/// Author and bonus obligations cannot be reduced through this path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce_in(
    c: &rusqlite::Transaction<'_>,
    id: &str,
    settlement: &str,
    party: &str,
    role: &str,
    evidence: &str,
    requested_msat: i64,
    at: i64,
) -> Result<Adjustment> {
    if id.is_empty()
        || id.len() > 256
        || settlement.is_empty()
        || evidence.is_empty()
        || evidence.len() > 256
        || requested_msat <= 0
        || at < 0
        || !matches!(role, "openagents" | "resource")
        || role == "openagents" && party != crate::OPENAGENTS
    {
        return Err(Error::Invalid("payable adjustment"));
    }
    if let Some(old) = read(c, id)? {
        if old.settlement != settlement
            || old.party != party
            || old.role != role
            || old.evidence != evidence
            || old.requested_msat != requested_msat
            || old.at != at
        {
            return Err(Error::Conflict("payable adjustment identity"));
        }
        return Ok(old);
    }
    let original: i64 = c
        .query_row(
            "SELECT amount_msat FROM share WHERE settlement=? AND party=? AND role=?",
            params![settlement, party, role],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(Error::Invalid("original payable claim absent"))?;
    let prior:i64=c.query_row("SELECT COALESCE(SUM(requested_msat),0) FROM payable_adjustment WHERE settlement=? AND party=? AND role=?",params![settlement,party,role],|r|r.get(0))?;
    if prior
        .checked_add(requested_msat)
        .is_none_or(|v| v > original)
    {
        return Err(Error::Invalid("adjustment exceeds original claim"));
    }
    let available:i64=c.query_row(&format!("SELECT s.amount_msat FROM payable_share s WHERE s.settlement=? AND s.party=? AND s.role=? AND {AVAILABLE}"),params![settlement,party,role],|r|r.get(0)).optional()?.unwrap_or(0);
    let reduced_msat = available.max(0).min(requested_msat);
    let loss_msat = requested_msat - reduced_msat;
    c.execute(
        "INSERT INTO payable_adjustment VALUES(?,?,?,?,?,?,?,?,?)",
        params![
            id,
            settlement,
            party,
            role,
            evidence,
            requested_msat,
            reduced_msat,
            loss_msat,
            at
        ],
    )?;
    Ok(Adjustment {
        id: id.into(),
        settlement: settlement.into(),
        party: party.into(),
        role: role.into(),
        evidence: evidence.into(),
        requested_msat,
        reduced_msat,
        loss_msat,
        at,
    })
}
impl Ledger {
    #[allow(clippy::too_many_arguments)]
    pub fn reduce_payable(
        &mut self,
        id: &str,
        settlement: &str,
        party: &str,
        role: &str,
        evidence: &str,
        amount_msat: i64,
        at: i64,
    ) -> Result<Adjustment> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = reduce_in(&tx, id, settlement, party, role, evidence, amount_msat, at)?;
        tx.commit()?;
        Ok(row)
    }
    pub fn payable_adjustments(&self, settlement: &str) -> Result<Vec<Adjustment>> {
        let mut q = self.connection.prepare(
            "SELECT id FROM payable_adjustment WHERE settlement=? ORDER BY id LIMIT 257",
        )?;
        let ids = q
            .query_map([settlement], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if ids.len() > 256 {
            return Err(Error::Invalid("adjustment report exceeds bound"));
        }
        ids.into_iter()
            .map(|id| read(&self.connection, &id)?.ok_or(Error::Invalid("adjustment missing")))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OPENAGENTS, Payee, PayoutState, Rail, SettlementInput, Split};
    #[test]
    fn immutable_claim_replay_reserved_loss() {
        let mut l = Ledger::in_memory().unwrap();
        l.record_settlement(SettlementInput {
            key: "source".into(),
            resource: "endpoint".into(),
            plugin_id: None,
            release_id: None,
            price_msat: 10_000,
            received_msat: 10_000,
            rail: Rail::Lightning,
            payer_alias: None,
            settled_at: 1_800_000_000,
            split: Split::OpenAgents,
        })
        .unwrap();
        let a = l
            .reduce_payable(
                "refund1",
                "source",
                OPENAGENTS,
                "openagents",
                "proof",
                2000,
                1,
            )
            .unwrap();
        assert_eq!(a.reduced_msat, 2000);
        assert_eq!(l.available_shares(OPENAGENTS).unwrap()[0].amount_msat, 8000);
        assert_eq!(
            l.reduce_payable(
                "refund1",
                "source",
                OPENAGENTS,
                "openagents",
                "proof",
                2000,
                1
            )
            .unwrap(),
            a
        );
        assert!(
            l.reduce_payable(
                "refund1",
                "source",
                OPENAGENTS,
                "openagents",
                "changed",
                2000,
                1
            )
            .is_err()
        );
        l.register_payee(Payee {
            party: OPENAGENTS.into(),
            destination_kind: "spark".into(),
            destination_value: "test".into(),
            source: "fixture".into(),
            verified_at: 1,
        })
        .unwrap();
        let shares = l.available_shares(OPENAGENTS).unwrap();
        l.reserve_payout("p", OPENAGENTS, &shares, 2).unwrap();
        let loss = l
            .reduce_payable(
                "refund2",
                "source",
                OPENAGENTS,
                "openagents",
                "proof2",
                3000,
                3,
            )
            .unwrap();
        assert_eq!(loss.loss_msat, 3000);
        l.finish_payout("p", PayoutState::Failed, None, None, 4)
            .unwrap();
        assert_eq!(l.available_shares(OPENAGENTS).unwrap()[0].amount_msat, 8000);
        assert!(l.commission_payouts_held().unwrap());
        assert_eq!(
            l.settlement("source")
                .unwrap()
                .unwrap()
                .shares
                .iter()
                .find(|s| s.role == "openagents")
                .unwrap()
                .amount_msat,
            10_000
        );
    }
}
