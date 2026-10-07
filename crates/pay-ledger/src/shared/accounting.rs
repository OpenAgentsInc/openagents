//! Original unit settlements and conserved, evidence-bound cleanup.
use super::*;

pub(crate) const TABLES: &str = "
CREATE TABLE IF NOT EXISTS shared_source_settlement(intent TEXT PRIMARY KEY REFERENCES shared_intent(id),bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_refund(id TEXT PRIMARY KEY,intent TEXT NOT NULL REFERENCES shared_intent(id),pool TEXT NOT NULL,units INTEGER NOT NULL,returned INTEGER NOT NULL,reduced INTEGER NOT NULL,loss INTEGER NOT NULL,evidence TEXT NOT NULL UNIQUE,bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_refund_plan(id TEXT PRIMARY KEY,intent TEXT NOT NULL REFERENCES shared_intent(id),pool TEXT NOT NULL,units INTEGER NOT NULL,evidence TEXT NOT NULL UNIQUE,digest TEXT NOT NULL UNIQUE,bytes TEXT NOT NULL,invoice TEXT);
CREATE TABLE IF NOT EXISTS shared_inbound_claim(payment_hash TEXT PRIMARY KEY,role TEXT NOT NULL,source TEXT NOT NULL);
CREATE UNIQUE INDEX IF NOT EXISTS shared_funding_reversal_evidence ON shared_funding_reversal(evidence);
CREATE TRIGGER IF NOT EXISTS shared_funding_reversal_no_update BEFORE UPDATE ON shared_funding_reversal BEGIN SELECT RAISE(ABORT,'Original funding reversal is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_funding_reversal_no_delete BEFORE DELETE ON shared_funding_reversal BEGIN SELECT RAISE(ABORT,'Original funding loss remains retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_source_settlement_no_update BEFORE UPDATE ON shared_source_settlement BEGIN SELECT RAISE(ABORT,'Original source settlement is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_source_settlement_no_delete BEFORE DELETE ON shared_source_settlement BEGIN SELECT RAISE(ABORT,'Original source settlement is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_refund_no_update BEFORE UPDATE ON shared_refund BEGIN SELECT RAISE(ABORT,'Original refund is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_refund_no_delete BEFORE DELETE ON shared_refund BEGIN SELECT RAISE(ABORT,'Original refund is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_refund_plan_no_update BEFORE UPDATE OF id,intent,pool,units,evidence,digest,bytes ON shared_refund_plan BEGIN SELECT RAISE(ABORT,'Original refund preparation is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_refund_plan_invoice_once BEFORE UPDATE OF invoice ON shared_refund_plan WHEN OLD.invoice IS NOT NULL BEGIN SELECT RAISE(ABORT,'Original refund invoice is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_refund_plan_no_delete BEFORE DELETE ON shared_refund_plan BEGIN SELECT RAISE(ABORT,'Unknown refund preparation stays reserved'); END;
CREATE TRIGGER IF NOT EXISTS shared_inbound_claim_no_update BEFORE UPDATE ON shared_inbound_claim BEGIN SELECT RAISE(ABORT,'Incoming evidence role is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_inbound_claim_no_delete BEFORE DELETE ON shared_inbound_claim BEGIN SELECT RAISE(ABORT,'Incoming evidence role is retained'); END;
";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceSettlement {
    pub intent: String,
    pub intent_digest: String,
    pub units: u64,
    pub converted_msat: u64,
    /// Numerator remainder of the original conversion, before upward rounding.
    pub remainder: u64,
    pub denominator: u64,
    pub fee_msat: u64,
    pub evidence: String,
    pub expense: Option<Value>,
    pub at: i64,
}
pub(crate) fn source_settlement_in(c: &Connection, id: &str) -> Result<Option<SourceSettlement>> {
    c.query_row(
        "SELECT bytes FROM shared_source_settlement WHERE intent=?",
        [id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| parse(&s))
    .transpose()
}
pub(crate) fn seal_settlement_in(
    c: &rusqlite::Transaction<'_>,
    intent: &Intent,
    units: u64,
    fee: u64,
    evidence: &str,
    expense: Option<&Value>,
    at: i64,
) -> Result<()> {
    let conversion = &intent.binding.conversion;
    let remainder = (u128::from(units) * u128::from(conversion.numerator)
        % u128::from(conversion.denominator)) as u64;
    let proposed = SourceSettlement {
        intent: intent.id.clone(),
        intent_digest: intent.digest(),
        units,
        converted_msat: intent.convert(units)?,
        remainder,
        denominator: conversion.denominator,
        fee_msat: fee,
        evidence: evidence.into(),
        expense: expense.cloned(),
        at,
    };
    if at < 0 {
        return Err(Error::Invalid("original source settlement time"));
    }
    if let Some(old) = source_settlement_in(c, &intent.id)? {
        let mut comparison = proposed;
        comparison.at = old.at;
        if old != comparison {
            return Err(Error::Conflict(
                "original source units, fees, or evidence changed",
            ));
        }
        return Ok(());
    }
    let settled: bool = c.query_row(
        "SELECT state='settled' FROM shared_outcome WHERE id=?",
        [&intent.id],
        |r| r.get(0),
    )?;
    if settled {
        return Err(Error::Denied("original source settlement is unavailable"));
    }
    c.execute(
        "INSERT INTO shared_source_settlement VALUES(?,?)",
        params![intent.id, json(&proposed)?],
    )?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RefundReview {
    pub id: String,
    pub intent: String,
    pub intent_digest: String,
    /// This evidence returns these original source units, once.
    pub units: u64,
    pub evidence: String,
    pub reviewed_at: u64,
    pub valid_until: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Refund {
    pub review: RefundReview,
    pub original: SourceSettlement,
    pub cumulative_units: u64,
    pub returned_msat: i64,
    pub reduced_msat: i64,
    pub loss_msat: i64,
    /// A verified original-custodian inbound hash for an external expense.
    pub incoming_hash: Option<String>,
    pub at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RefundPlan {
    pub review: RefundReview,
    pub original: SourceSettlement,
    pub binding: Binding,
    pub cumulative_units: u64,
    pub returned_msat: i64,
    pub created_at: u64,
    pub due: u64,
    pub request_hash: String,
}
impl RefundPlan {
    pub fn digest(&self) -> String {
        digest(self)
    }
}
fn plan_in(c: &Connection, id: &str) -> Result<Option<RefundPlan>> {
    c.query_row(
        "SELECT bytes FROM shared_refund_plan WHERE id=?",
        [id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| parse(&s))
    .transpose()
}
fn prepare_in(c: &rusqlite::Transaction<'_>, review: &RefundReview, at: u64) -> Result<RefundPlan> {
    if let Some(old) = plan_in(c, &review.id)? {
        if old.review != *review {
            return Err(Error::Conflict("original refund preparation changed"));
        }
        return Ok(old);
    }
    if at < review.reviewed_at || at >= review.valid_until {
        return Err(Error::Denied(
            "original refund review expired before preparation",
        ));
    }
    let intent =
        intent_in(c, &review.intent)?.ok_or(Error::Invalid("original refunded intent absent"))?;
    if intent.digest() != review.intent_digest {
        return Err(Error::Conflict("original refunded source changed"));
    }
    let original = source_settlement_in(c, &review.intent)?
        .ok_or(Error::Invalid("original source settlement absent"))?;
    let (prior_units,prior_returned):(u64,i64)=c.query_row("SELECT COALESCE(SUM(units),0),COALESCE(SUM(json_extract(bytes,'$.returned_msat')),0) FROM shared_refund_plan WHERE intent=?",[&review.intent],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let cumulative_units = prior_units
        .checked_add(review.units)
        .ok_or(Error::Invalid("original refund unit overflow"))?;
    let target =
        i64::try_from(refundable(&intent, &original, cumulative_units)?).map_err(invalid)?;
    let returned_msat = target
        .checked_sub(prior_returned)
        .ok_or(Error::Invalid("original refund projection changed"))?;
    let due = at
        .checked_add(3600)
        .ok_or(Error::Invalid("original refund time overflow"))?;
    let request_hash = digest(&(
        "shared-original-expense-return",
        review,
        &intent.binding,
        returned_msat,
        at,
        due,
    ));
    let plan = RefundPlan {
        review: review.clone(),
        original,
        binding: intent.binding.clone(),
        cumulative_units,
        returned_msat,
        created_at: at,
        due,
        request_hash,
    };
    c.execute("INSERT INTO shared_refund_plan(id,intent,pool,units,evidence,digest,bytes) VALUES(?,?,?,?,?,?,?)",params![review.id,intent.id,intent.binding.pool,i64::try_from(review.units).map_err(invalid)?,review.evidence,plan.digest(),json(&plan)?])?;
    Ok(plan)
}
fn refund_in(c: &Connection, id: &str) -> Result<Option<Refund>> {
    c.query_row("SELECT bytes FROM shared_refund WHERE id=?", [id], |r| {
        r.get::<_, String>(0)
    })
    .optional()?
    .map(|s| parse(&s))
    .transpose()
}
/// Refunds remove source units cumulatively at their original rate. The fee is
/// retained. Precision remains unavailable until the original conversion permits it.
fn refundable(intent: &Intent, original: &SourceSettlement, cumulative: u64) -> Result<u64> {
    if cumulative > original.units {
        return Err(Error::Invalid("refund exceeds original source units"));
    }
    let remaining = original.units - cumulative;
    let c = &intent.binding.conversion;
    let n = u128::from(remaining) * u128::from(c.numerator);
    let d = u128::from(c.denominator);
    let remaining = u64::try_from(n / d + u128::from(n % d != 0)).map_err(invalid)?;
    original
        .converted_msat
        .checked_sub(remaining)
        .ok_or(Error::Invalid("original refund rounding"))
}
pub(crate) fn claim_inbound_in(
    c: &rusqlite::Transaction<'_>,
    hash: &str,
    role: &str,
    source: &str,
) -> Result<()> {
    if !super::hash(hash, 64) || !matches!(role, "funding" | "refund") || !identifier(source) {
        return Err(Error::Invalid("original incoming evidence role"));
    }
    if let Some((old_role, old_source)) = c
        .query_row(
            "SELECT role,source FROM shared_inbound_claim WHERE payment_hash=?",
            [hash],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if old_role != role || old_source != source {
            return Err(Error::Conflict(
                "incoming confirmation already belongs to another source",
            ));
        }
        return Ok(());
    }
    c.execute(
        "INSERT INTO shared_inbound_claim VALUES(?,?,?)",
        params![hash, role, source],
    )?;
    Ok(())
}
pub(crate) fn protected_loss_in(c: &Connection, pool: &str) -> Result<i64> {
    c.query_row("SELECT COALESCE((SELECT SUM(loss) FROM shared_refund WHERE pool=?1),0)+COALESCE((SELECT SUM(loss) FROM shared_funding_reversal WHERE pool=?1),0)",[pool],|r|r.get(0)).map_err(Into::into)
}
impl Ledger {
    pub fn shared_seal_native_actor(&mut self, intent: &str, proof: &Value) -> Result<()> {
        self.require_shared_writer()?;
        let bytes = json(proof)?;
        let old: Option<String> = self
            .connection
            .query_row(
                "SELECT bytes FROM shared_native_actor WHERE intent=?",
                [intent],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            return if old == bytes {
                Ok(())
            } else {
                Err(Error::Conflict("original native actor changed"))
            };
        }
        self.connection.execute(
            "INSERT INTO shared_native_actor VALUES(?,?)",
            params![intent, bytes],
        )?;
        Ok(())
    }
    pub fn shared_native_actor(&self, intent: &str) -> Result<Option<Value>> {
        self.connection
            .query_row(
                "SELECT bytes FROM shared_native_actor WHERE intent=?",
                [intent],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| parse(&s))
            .transpose()
    }
    pub fn shared_source_settlement(&self, id: &str) -> Result<Option<SourceSettlement>> {
        source_settlement_in(&self.connection, id)
    }
    pub fn shared_refund(&self, id: &str) -> Result<Option<Refund>> {
        refund_in(&self.connection, id)
    }
    pub fn shared_prepare_return(&mut self, review: &RefundReview, at: u64) -> Result<RefundPlan> {
        self.require_shared_writer()?;
        if !identifier(&review.id)
            || !identifier(&review.evidence)
            || !hash(&review.intent_digest, 64)
            || review.units == 0
            || review.valid_until <= review.reviewed_at
            || at < review.reviewed_at
            || at >= review.valid_until
        {
            return Err(Error::Invalid("original refund review"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let plan = prepare_in(&tx, review, at)?;
        tx.commit()?;
        Ok(plan)
    }
    pub fn shared_return_plan(&self, id: &str) -> Result<Option<RefundPlan>> {
        plan_in(&self.connection, id)
    }
    pub fn shared_return_by_digest(&self, value: &str) -> Result<Option<RefundPlan>> {
        self.connection
            .query_row(
                "SELECT bytes FROM shared_refund_plan WHERE digest=?",
                [value],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| parse(&s))
            .transpose()
    }
    pub fn shared_return_invoice(&self, id: &str) -> Result<Option<Value>> {
        self.connection
            .query_row(
                "SELECT invoice FROM shared_refund_plan WHERE id=?",
                [id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .map(|s| parse(&s))
            .transpose()
    }
    pub fn shared_seal_return_invoice(&mut self, id: &str, invoice: &Value) -> Result<()> {
        self.require_shared_writer()?;
        if let Some(old) = self.shared_return_invoice(id)? {
            return if old == *invoice {
                Ok(())
            } else {
                Err(Error::Conflict("original refund invoice changed"))
            };
        }
        if self.connection.execute(
            "UPDATE shared_refund_plan SET invoice=? WHERE id=? AND invoice IS NULL",
            params![json(invoice)?, id],
        )? != 1
        {
            return Err(Error::Conflict("original refund preparation absent"));
        }
        Ok(())
    }
    /// The controller verifies protected original review and any inbound wallet
    /// confirmation before calling this. Native claim reduction and pool return
    /// share this transaction; no author or bonus claim changes.
    pub fn shared_return_expense(
        &mut self,
        review: &RefundReview,
        incoming: Option<&str>,
        at: i64,
    ) -> Result<Refund> {
        self.require_shared_writer()?;
        if !identifier(&review.id)
            || !identifier(&review.evidence)
            || !hash(&review.intent_digest, 64)
            || review.units == 0
            || review.valid_until <= review.reviewed_at
            || at < 0
        {
            return Err(Error::Invalid("original refund review"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = refund_in(&tx, &review.id)? {
            if old.review != *review || old.incoming_hash.as_deref() != incoming {
                return Err(Error::Conflict("original refund terms changed"));
            }
            return Ok(old);
        }
        let intent = intent_in(&tx, &review.intent)?
            .ok_or(Error::Invalid("original refunded intent absent"))?;
        if intent.digest() != review.intent_digest {
            return Err(Error::Conflict("original refunded source changed"));
        }
        let plan = prepare_in(&tx, review, at as u64)?;
        let original = plan.original;
        let cumulative_units = plan.cumulative_units;
        let returned_msat = plan.returned_msat;
        let (reduced_msat, loss_msat) = match &intent.liability {
            Liability::NativeService { .. } if incoming.is_none() => {
                if returned_msat == 0 {
                    (0, 0)
                } else {
                    let adjustment = crate::adjustment::reduce_in(
                        &tx,
                        &format!("shared-refund:{}", review.id),
                        &format!("debit:{}", intent.id),
                        crate::OPENAGENTS,
                        "openagents",
                        &review.evidence,
                        returned_msat,
                        at,
                    )?;
                    (adjustment.reduced_msat, adjustment.loss_msat)
                }
            }
            Liability::ExternalInvoice { .. }
                if incoming.is_some() && original.expense.is_some() =>
            {
                claim_inbound_in(&tx, incoming.unwrap(), "refund", &review.id)?;
                (returned_msat, 0)
            }
            _ => {
                return Err(Error::Denied(
                    "refund needs original native liability or confirmed incoming expense return",
                ));
            }
        };
        let row = Refund {
            review: review.clone(),
            original,
            cumulative_units,
            returned_msat,
            reduced_msat,
            loss_msat,
            incoming_hash: incoming.map(str::to_owned),
            at,
        };
        tx.execute(
            "INSERT INTO shared_refund VALUES(?,?,?,?,?,?,?,?,?)",
            params![
                review.id,
                intent.id,
                intent.binding.pool,
                i64::try_from(review.units).map_err(invalid)?,
                returned_msat,
                reduced_msat,
                loss_msat,
                review.evidence,
                json(&row)?
            ],
        )?;
        tx.commit()?;
        Ok(row)
    }
}

// A source reversal retains its recovered amount and protected loss separately.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FundingReversal {
    pub id: String,
    pub pool: String,
    pub funding: String,
    pub amount_msat: i64,
    pub recovered_msat: i64,
    pub loss_msat: i64,
    pub evidence: String,
}
fn reversal_in(c: &Connection, id: &str) -> Result<Option<FundingReversal>> {
    c.query_row("SELECT id,pool,funding,amount,recovered,loss,evidence FROM shared_funding_reversal WHERE id=?", [id], |r| Ok(FundingReversal {
        id:r.get(0)?,pool:r.get(1)?,funding:r.get(2)?,amount_msat:r.get(3)?,recovered_msat:r.get(4)?,loss_msat:r.get(5)?,evidence:r.get(6)?,
    })).optional().map_err(Into::into)
}
impl Ledger {
    /// Only the canonical writer posts protected, original funding loss evidence.
    /// Held and unknown liabilities remain intact; fresh funding cannot clear loss.
    pub fn shared_reverse_funding(
        &mut self,
        id: &str,
        funding: &str,
        amount: i64,
        evidence: &str,
    ) -> Result<FundingReversal> {
        self.require_shared_writer()?;
        if !identifier(id) || !identifier(funding) || !identifier(evidence) || amount <= 0 {
            return Err(Error::Invalid("original funding reversal bounds"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = reversal_in(&tx, id)? {
            if old.funding != funding || old.amount_msat != amount || old.evidence != evidence {
                return Err(Error::Conflict("original funding reversal changed"));
            }
            return Ok(old);
        }
        let (pool, hash, original, state): (String, String, i64, String) = tx.query_row(
            "SELECT account,payment_hash,amount_msat,state FROM compute_purchase WHERE id=?",
            [funding],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let native: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shared_funding_record WHERE id=? AND invoice IS NOT NULL)",
            [funding],
            |r| r.get(0),
        )?;
        let credited: Option<i64> = tx
            .query_row(
                "SELECT amount_msat FROM compute_credit WHERE account=? AND source=?",
                params![pool, format!("topup:{hash}")],
                |r| r.get(0),
            )
            .optional()?;
        if !native || state != "paid" || credited != Some(original) {
            return Err(Error::Denied("original confirmed shared funding required"));
        }
        let reversed: i64 = tx.query_row(
            "SELECT COALESCE(SUM(amount),0) FROM shared_funding_reversal WHERE funding=?",
            [funding],
            |r| r.get(0),
        )?;
        if reversed.checked_add(amount).is_none_or(|v| v > original) {
            return Err(Error::Invalid(
                "reversal exceeds original confirmed funding",
            ));
        }
        let balance = crate::compute::hold::balance_in(&tx, &pool)?;
        let free = balance
            .available_msat
            .checked_add(balance.restricted_msat)
            .ok_or(Error::Invalid("original funding reversal overflow"))?;
        let recovered_msat = amount.min(free);
        let row = FundingReversal {
            id: id.into(),
            pool,
            funding: funding.into(),
            amount_msat: amount,
            recovered_msat,
            loss_msat: amount - recovered_msat,
            evidence: evidence.into(),
        };
        tx.execute(
            "INSERT INTO shared_funding_reversal VALUES(?,?,?,?,?,?,?)",
            params![
                row.id,
                row.pool,
                row.funding,
                row.amount_msat,
                row.recovered_msat,
                row.loss_msat,
                row.evidence
            ],
        )?;
        tx.commit()?;
        Ok(row)
    }
}
