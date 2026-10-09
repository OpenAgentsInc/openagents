//! Commission custody over one original merchant settlement. This module never
//! credits gross receipts or changes author and bonus shares. Native adapters
//! authenticate admission, settlement, and reversal evidence before posting.
use crate::{Error, Ledger, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.merchant-commission.v1";
pub(crate) const TABLES:&str="
CREATE TABLE IF NOT EXISTS commission_cost_evidence(admission TEXT PRIMARY KEY REFERENCES commission_admission(id), policy TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_refund(id TEXT PRIMARY KEY, admission TEXT NOT NULL REFERENCES commission_admission(id), json TEXT NOT NULL, state TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_admission(id TEXT PRIMARY KEY, payment_hash TEXT NOT NULL UNIQUE, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_obligation(id TEXT PRIMARY KEY REFERENCES commission_admission(id), settlement TEXT NOT NULL UNIQUE REFERENCES settlement(payment_hash), party TEXT NOT NULL, evidence TEXT NOT NULL, observed_at INTEGER NOT NULL, base_msat INTEGER NOT NULL, held_msat INTEGER NOT NULL, earned_msat INTEGER NOT NULL, reversed_msat INTEGER NOT NULL, cost_msat INTEGER, state TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_native_reversal(id TEXT PRIMARY KEY REFERENCES commission_reversal(id), json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_reversal(id TEXT PRIMARY KEY, admission TEXT NOT NULL REFERENCES commission_admission(id), evidence TEXT NOT NULL UNIQUE, amount_msat INTEGER NOT NULL CHECK(amount_msat>0), commission_msat INTEGER NOT NULL CHECK(commission_msat>=0), reduced_msat INTEGER NOT NULL CHECK(reduced_msat>=0), loss_msat INTEGER NOT NULL CHECK(loss_msat>=0), at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS native_payout_claim(payout TEXT NOT NULL REFERENCES payout(id), settlement TEXT NOT NULL REFERENCES settlement(payment_hash), party TEXT NOT NULL, role TEXT NOT NULL, amount_msat INTEGER NOT NULL CHECK(amount_msat>0), PRIMARY KEY(payout,settlement,party,role));
CREATE TABLE IF NOT EXISTS commission_payout_item(payout TEXT NOT NULL REFERENCES payout(id), settlement TEXT NOT NULL REFERENCES settlement(payment_hash), party TEXT NOT NULL, role TEXT NOT NULL CHECK(role='commission'), PRIMARY KEY(payout,settlement,party,role));
";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub category: String,
    pub amount_msat: Option<u64>,
    /// `operator-declared` does not mean independently measured.
    pub provenance: String,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub schema: String,
    pub id: String,
    pub ledger_origin: String,
    pub payment_hash: String,
    pub request_hash: String,
    pub authorization: String,
    pub buyer_account: String,
    pub buyer_workspace: String,
    /// Missing pins are retained legacy history, unavailable to native effects.
    #[serde(default)]
    pub operator_account: String,
    #[serde(default)]
    pub operator_workspace: String,
    pub customer: String,
    pub referrer: String,
    pub party: String,
    pub agreement: String,
    pub terms: String,
    /// Exact retained bilateral agreement and publication, not current terms.
    pub contract: String,
    pub offer_digest: String,
    pub invoice: String,
    pub receiver: String,
    pub payer: String,
    pub plugin: String,
    pub release: String,
    pub author: String,
    pub author_fee_msat: u64,
    pub price_msat: u64,
    pub numerator: u64,
    pub denominator: u64,
    pub exact_rounding: bool,
    pub hold_secs: u64,
    pub minimum_msat: u64,
    pub destinations: Vec<String>,
    pub costs: Vec<Cost>,
    pub cost_policy: String,
    pub admitted_at: u64,
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn encode<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::Invalid("commission encoding"))
}
fn read(c: &Connection, id: &str) -> Result<Option<Admission>> {
    let json: Option<String> = c
        .query_row(
            "SELECT json FROM commission_admission WHERE id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    json.map(|v| {
        serde_json::from_str(&v).map_err(|_| Error::Invalid("retained commission admission"))
    })
    .transpose()
}
impl Admission {
    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA
            || !hex(&self.id)
            || !hex(&self.ledger_origin)
            || !hex(&self.payment_hash)
            || !hex(&self.request_hash)
            || !hex(&self.authorization)
            || self.contract.len() > 32_768
            || self.invoice.len() > 16_384
            || self.customer.is_empty()
            || self.operator_account.is_empty()
            || self.operator_workspace.is_empty()
            || self.buyer_account == self.operator_account
            || self.payer == self.receiver
            || self.referrer.is_empty()
            || self.party != format!("referrer:{}", self.referrer)
            || self.denominator == 0
            || self.numerator > self.denominator
            || self.price_msat == 0
            || self.price_msat > i64::MAX as u64
            || self.author_fee_msat > self.price_msat
            || self.minimum_msat == 0
            || !self.minimum_msat.is_multiple_of(1000)
            || self.minimum_msat > i64::MAX as u64
            || self.destinations.is_empty()
            || self
                .destinations
                .iter()
                .any(|v| !matches!(v.as_str(), "spark" | "lud16"))
            || self.costs.len() != 6
            || self.cost_policy.is_empty()
        {
            return Err(Error::Invalid("commission admission"));
        }
        for category in [
            "model", "compute", "payment", "delivery", "support", "other",
        ] {
            let matches: Vec<_> = self
                .costs
                .iter()
                .filter(|c| c.category == category)
                .collect();
            if matches.len() != 1
                || matches[0].evidence.is_empty()
                || !matches!(
                    matches[0].provenance.as_str(),
                    "operator-declared" | "native-measured"
                )
                || matches[0].amount_msat.is_some_and(|v| v > i64::MAX as u64)
            {
                return Err(Error::Invalid("commission cost provenance"));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub admission: Admission,
    pub state: String,
    /// New payout planning stays closed while native custody is unresolved.
    pub payouts_held: bool,
    pub abuse_review: Option<crate::commission_abuse::Record>,
    pub evidence: Option<String>,
    pub held_msat: i64,
    pub earned_msat: i64,
    pub cost_msat: Option<i64>,
    pub reversed_msat: i64,
    pub commission_reversed_msat: i64,
    pub available_msat: i64,
    pub reserved_msat: i64,
    /// Original claims consumed by sent attempts, including rounding.
    pub consumed_msat: i64,
    /// Attributed rail amount, allocated proportionally across original claims.
    pub sent_msat: i64,
    /// Consumed claims whose historical payout has no recorded rail amount.
    pub unverified_sent_msat: i64,
    pub retained_remainder_msat: i64,
    /// Physical merchant funding deficit; it excludes double-counting payee loss.
    pub loss_msat: i64,
    pub paid_or_reserved_reversal_loss_msat: i64,
    pub payout_states: Vec<String>,
    pub limitations: Vec<&'static str>,
}
impl Ledger {
    /// The caller must freeze a native agreement before payment begins.
    pub fn admit_commission(&mut self, a: &Admission) -> Result<Admission> {
        a.validate()?;
        if a.ledger_origin != self.origin()? {
            return Err(Error::Conflict("commission ledger origin"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = read(&tx, &a.id)? {
            if old != *a {
                return Err(Error::Conflict("commission admission identity"));
            }
            return Ok(old);
        }
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM settlement WHERE payment_hash=?)",
            [&a.payment_hash],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(Error::Invalid(
                "commission requires original prepayment admission",
            ));
        }
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM commission_admission", [], |r| {
            r.get(0)
        })?;
        if count >= 4096 {
            return Err(Error::Invalid("commission admission bound"));
        }
        tx.execute(
            "INSERT INTO commission_admission VALUES(?,?,?)",
            params![a.id, a.payment_hash, encode(a)?],
        )?;
        tx.commit()?;
        Ok(a.clone())
    }
    pub fn commission_admission(&self, id: &str) -> Result<Option<Admission>> {
        read(&self.connection, id)
    }
    /// A native adapter verified current outcome custody and receiver lookup.
    /// Unknown costs or invocation retain the original unpaid OpenAgents claim.
    pub fn observe_commission(
        &mut self,
        id: &str,
        evidence: &str,
        completed: Option<bool>,
        now: u64,
    ) -> Result<Report> {
        if !hex(evidence) || now > i64::MAX as u64 {
            return Err(Error::Invalid("commission observation"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let a = read(&tx, id)?.ok_or(Error::Invalid("commission admission absent"))?;
        let s = crate::read_record(&tx, &a.payment_hash)?
            .ok_or(Error::Invalid("original merchant settlement absent"))?;
        if s.plugin_id.as_deref() != Some(&a.plugin)
            || s.release_id.as_deref() != Some(&a.release)
            || s.price_msat != a.price_msat as i64
            || s.received_msat != s.price_msat
            || s.rail != crate::Rail::Lightning
            || s.short
            || s.shares
                .iter()
                .find(|v| v.role == "author" && v.party == a.author)
                .map(|v| v.amount_msat)
                != Some(a.author_fee_msat as i64)
        {
            return Err(Error::Conflict("original merchant settlement changed"));
        }
        let old: Option<(String, i64, String)> = tx
            .query_row(
                "SELECT state,earned_msat,evidence FROM commission_obligation WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((state, _, proof)) = &old {
            if state == "earned" || state == "ineligible" {
                if proof != evidence {
                    return Err(Error::Conflict("terminal commission evidence changed"));
                }
                tx.commit()?;
                return self.commission_report(id);
            }
        }
        let base: i64 = match old {
            Some(_) => tx.query_row("SELECT base_msat FROM commission_obligation WHERE id=?", [id], |r| r.get(0))?,
            None => tx.query_row("SELECT s.amount_msat FROM payable_share s WHERE s.settlement=? AND s.party='openagents' AND s.role='openagents' AND NOT EXISTS(SELECT 1 FROM payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed')",[&a.payment_hash],|r|r.get(0)).optional()?.unwrap_or(0),
        };
        let effective_costs = qualified_costs(&tx, &a)?;
        let cost = effective_costs
            .iter()
            .try_fold(0u64, |sum, c| sum.checked_add(c.amount_msat?));
        let matured = (s.settled_at as u64)
            .checked_add(a.hold_secs)
            .is_some_and(|at| now >= at);
        let (state, held, earned) = match completed {
            Some(false) => ("ineligible", 0, 0),
            Some(true) if cost.is_some() && matured => {
                let after = base
                    .checked_sub(
                        i64::try_from(cost.unwrap())
                            .map_err(|_| Error::Invalid("commission costs overflow"))?,
                    )
                    .ok_or(Error::Invalid("commission costs overflow"))?
                    .max(0);
                let n = (after as u128) * (a.numerator as u128);
                if a.exact_rounding && !n.is_multiple_of(a.denominator as u128) {
                    return Err(Error::Invalid("commission precision"));
                }
                let earned = i64::try_from(n / (a.denominator as u128))
                    .map_err(|_| Error::Invalid("commission overflow"))?;
                ("earned", earned, earned)
            }
            _ => ("held", base, 0),
        };
        let refunds: i64 = tx.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM commission_reversal WHERE admission=?",
            [id],
            |r| r.get(0),
        )?;
        let reversed = proportional(earned, refunds, a.price_msat)?;
        tx.execute("INSERT INTO commission_obligation VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET evidence=excluded.evidence,observed_at=excluded.observed_at,held_msat=excluded.held_msat,earned_msat=excluded.earned_msat,reversed_msat=excluded.reversed_msat,cost_msat=excluded.cost_msat,state=excluded.state",params![id,a.payment_hash,a.party,evidence,now as i64,base,held,earned,reversed,cost.and_then(|v|i64::try_from(v).ok()),state])?;
        tx.commit()?;
        self.commission_report(id)
    }
    /// Verified refunds are cumulative against the original paid obligation.
    /// The referrer's proportional reversal uses original pinned terms; paid
    /// or unresolved payouts become losses rather than reusable money.
    pub fn reverse_commission(
        &mut self,
        id: &str,
        reversal: &str,
        evidence: &str,
        amount: i64,
        at: i64,
    ) -> Result<Report> {
        self.post_commission_reversal(id, reversal, evidence, amount, at, None)
    }
    /// Retain the first verified native outbound record beside its stable
    /// signed refund identity. Optional later wallet metadata cannot remint it.
    pub fn reverse_commission_with_payment(
        &mut self,
        id: &str,
        reversal: &str,
        evidence: &str,
        amount: i64,
        at: i64,
        payment: &str,
    ) -> Result<Report> {
        if payment.is_empty()
            || payment.len() > 16_384
            || serde_json::from_str::<serde_json::Value>(payment).is_err()
        {
            return Err(Error::Invalid("native refund payment evidence"));
        }
        self.post_commission_reversal(id, reversal, evidence, amount, at, Some(payment))
    }
    fn post_commission_reversal(
        &mut self,
        id: &str,
        reversal: &str,
        evidence: &str,
        amount: i64,
        at: i64,
        payment: Option<&str>,
    ) -> Result<Report> {
        if !hex(reversal) || !hex(evidence) || amount <= 0 || at < 0 {
            return Err(Error::Invalid("commission reversal"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let a = read(&tx, id)?.ok_or(Error::Invalid("commission admission absent"))?;
        let old: Option<(String, String, i64, i64)> = tx
            .query_row(
                "SELECT admission,evidence,amount_msat,at FROM commission_reversal WHERE id=?",
                [reversal],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        if let Some(old) = old {
            if old != (id.into(), evidence.into(), amount, at) {
                return Err(Error::Conflict("commission reversal identity"));
            }
            tx.commit()?;
            return self.commission_report(id);
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM commission_reversal WHERE admission=?",
            [id],
            |r| r.get(0),
        )?;
        if count >= 256 {
            return Err(Error::Invalid("native reversal record bound"));
        }
        let (earned, previous): (i64, i64) = tx.query_row(
            "SELECT earned_msat,reversed_msat FROM commission_obligation WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let prior: i64 = tx.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM commission_reversal WHERE admission=?",
            [id],
            |r| r.get(0),
        )?;
        let cumulative = prior
            .checked_add(amount)
            .filter(|v| *v <= a.price_msat as i64)
            .ok_or(Error::Invalid("refund exceeds original collection"))?;
        let target = proportional(earned, cumulative, a.price_msat)?;
        // The immutable journal records verified cash reversal. Current payout
        // uncertainty and losses derive from the original reservations, so a
        // subsequently proven failed payout releases only its own uncertainty.
        tx.execute(
            "INSERT INTO commission_reversal VALUES(?,?,?,?,?,?,?,?)",
            params![reversal, id, evidence, amount, target - previous, 0, 0, at],
        )?;
        if let Some(payment) = payment {
            tx.execute(
                "INSERT INTO commission_native_reversal VALUES(?,?)",
                params![reversal, payment],
            )?;
        }
        tx.execute(
            "UPDATE commission_obligation SET reversed_msat=? WHERE id=?",
            params![target, id],
        )?;
        tx.commit()?;
        self.commission_report(id)
    }
    pub fn commission_report(&self, id: &str) -> Result<Report> {
        let a = read(&self.connection, id)?.ok_or(Error::Invalid("commission admission absent"))?;
        let row:Option<(String,String,i64,i64,Option<i64>)>=self.connection.query_row("SELECT state,evidence,held_msat,earned_msat,cost_msat FROM commission_obligation WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let (state, evidence, _held, earned, cost) = row
            .map(|(s, e, h, v, c)| (s, Some(e), h, v, c))
            .unwrap_or(("admitted-unsettled".into(), None, 0, 0, None));
        let (reversed,commission_reversed,net,protected,encumbered,retained,loss,payee_loss):(i64,i64,i64,i64,i64,i64,i64,i64)=self.connection.query_row("SELECT refunds,reversed_msat,net,protected,encumbered,remainder,funding_loss,payee_loss FROM commission_balance WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).optional()?.unwrap_or_default();
        let mut q=self.connection.prepare("SELECT p.state,n.amount_msat,p.amount_msat,p.sent_msat FROM payout p JOIN commission_payout_item i ON i.payout=p.id JOIN native_payout_claim n ON n.payout=i.payout AND n.settlement=i.settlement AND n.party=i.party AND n.role=i.role WHERE i.settlement=? ORDER BY p.rowid LIMIT 257")?;
        let payouts = q
            .query_map([&a.payment_hash], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if payouts.len() > 256 {
            return Err(Error::Invalid("commission payout report bound"));
        }
        let reserved = payouts
            .iter()
            .filter(|(s, _, _, _)| matches!(s.as_str(), "planned" | "sending" | "unknown"))
            .map(|(_, v, _, _)| *v)
            .sum();
        let mut consumed = 0i64;
        let mut sent = 0i64;
        let mut unverified = 0i64;
        for (state, claim, total, rail) in &payouts {
            if state != "sent" {
                continue;
            }
            consumed = consumed
                .checked_add(*claim)
                .ok_or(Error::Invalid("commission report overflow"))?;
            if let Some(rail) = rail {
                let allocated =
                    i64::try_from((*claim as u128) * (*rail as u128) / (*total as u128))
                        .map_err(|_| Error::Invalid("commission report allocation"))?;
                sent = sent
                    .checked_add(allocated)
                    .ok_or(Error::Invalid("commission report overflow"))?;
            } else {
                unverified = unverified
                    .checked_add(*claim)
                    .ok_or(Error::Invalid("commission report overflow"))?;
            }
        }
        let mut available = if protected > 0 { 0 } else { net - retained };
        let mut held = if state == "held" { encumbered } else { 0 };
        let abuse_review = self.commission_abuse(id)?;
        let abuse_held = crate::commission_abuse::held(&self.connection, id)?;
        if abuse_held {
            held += available;
            available = 0;
        }
        Ok(Report {
            schema: SCHEMA,
            admission: a,
            state,
            payouts_held: self.commission_payouts_held()? || abuse_held,
            abuse_review,
            evidence,
            held_msat: held,
            earned_msat: earned,
            cost_msat: cost,
            reversed_msat: reversed,
            commission_reversed_msat: commission_reversed,
            available_msat: available,
            reserved_msat: reserved,
            consumed_msat: consumed,
            sent_msat: sent,
            unverified_sent_msat: unverified,
            retained_remainder_msat: retained,
            loss_msat: loss,
            paid_or_reserved_reversal_loss_msat: payee_loss,
            payout_states: payouts.into_iter().map(|(s, _, _, _)| s).collect(),
            limitations: vec![
                "Provider execution evidence is attributable, not independent customer acceptance.",
                "Declared cost policies retain their provenance; missing costs never mean zero.",
                "Consumed or reserved reversal losses hold new payouts until qualified native remediation.",
                "Per-source sent amounts allocate the journaled rail amount proportionally; division retains less than one msat per source outside this display.",
            ],
        })
    }
    pub(crate) fn commission_payout_qualified(
        &self,
        party: &str,
        destination: &str,
        amount: i64,
    ) -> Result<bool> {
        let items = self.available_shares(party)?;
        qualify_payout_in(&self.connection, party, destination, amount, Some(&items))
    }
}
fn qualify_payout_in(
    c: &Connection,
    party: &str,
    destination: &str,
    amount: i64,
    items: Option<&[crate::Share]>,
) -> Result<bool> {
    let Some(referrer) = party.strip_prefix("referrer:") else {
        return Ok(true);
    };
    let mut q=c.prepare("SELECT a.json FROM commission_admission a JOIN commission_obligation o ON o.id=a.id WHERE o.party=? AND o.state='earned' ORDER BY a.id LIMIT 4097")?;
    let rows = q
        .query_map([party], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if rows.len() > 4096 {
        return Err(Error::Invalid("commission policy bound"));
    }
    for json in rows {
        let a: Admission =
            serde_json::from_str(&json).map_err(|_| Error::Invalid("commission policy"))?;
        if items.is_some_and(|items| !items.iter().any(|item| item.settlement == a.payment_hash)) {
            continue;
        }
        if a.referrer != referrer
            || amount < a.minimum_msat as i64
            || !a.destinations.iter().any(|v| v == destination)
        {
            return Ok(false);
        }
    }
    Ok(!referrer.is_empty() && amount > 0 && amount % 1000 == 0)
}
pub(crate) fn qualify_reservation(
    c: &Connection,
    party: &str,
    destination: &str,
    amount: i64,
    items: &[crate::Share],
) -> Result<bool> {
    qualify_payout_in(
        c,
        party,
        destination.split(':').next().unwrap_or(""),
        amount,
        Some(items),
    )
}
fn proportional(earned: i64, refunds: i64, price: u64) -> Result<i64> {
    i64::try_from((earned as u128) * (refunds as u128) / u128::from(price))
        .map_err(|_| Error::Invalid("commission reversal overflow"))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Refund {
    pub schema: String,
    pub id: String,
    pub admission: String,
    pub amount_msat: u64,
    pub description_hash: String,
    pub buyer: String,
    pub operator: String,
    pub beneficiary_node: String,
    pub merchant_node: String,
    pub created_at: u64,
    pub invoice: Option<String>,
    pub state: String,
}
impl Ledger {
    /// The native adapter posts preparation before asking the original buyer
    /// resident for an invoice. Unknown creation never remints automatically.
    pub fn begin_commission_refund(&mut self, r: &Refund) -> Result<Refund> {
        self.begin_commission_refund_once(r)
            .map(|(record, _)| record)
    }
    /// Only the writer that created a preparation may issue its first invoice.
    /// A concurrent or restarted writer retains the original unknown issuance.
    pub fn begin_commission_refund_once(&mut self, r: &Refund) -> Result<(Refund, bool)> {
        if r.schema != "openagents.commission-refund.v1"
            || !hex(&r.id)
            || !hex(&r.admission)
            || !hex(&r.description_hash)
            || r.amount_msat == 0
            || r.amount_msat > i64::MAX as u64
            || r.invoice.is_some()
            || r.state != "preparing"
        {
            return Err(Error::Invalid("native commission refund preparation"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = read_refund(&tx, &r.id)? {
            let mut expected = old.clone();
            expected.invoice = None;
            expected.state = "preparing".into();
            if expected != *r {
                return Err(Error::Conflict("refund preparation identity"));
            }
            return Ok((old, false));
        }
        let count: i64 =
            tx.query_row("SELECT COUNT(*) FROM commission_refund", [], |v| v.get(0))?;
        if count >= 4096 {
            return Err(Error::Invalid("native refund record bound"));
        }
        let a = read(&tx, &r.admission)?.ok_or(Error::Invalid("commission admission absent"))?;
        let prior:i64=tx.query_row("SELECT COALESCE(SUM(json_extract(json,'$.amount_msat')),0) FROM commission_refund WHERE admission=? AND state!='failed'",[&r.admission],|v|v.get(0))?;
        if r.buyer != a.buyer_account
            || r.operator != a.operator_account
            || r.beneficiary_node != a.payer
            || r.merchant_node != a.receiver
            || prior
                .checked_add(r.amount_msat as i64)
                .is_none_or(|v| v > a.price_msat as i64)
        {
            return Err(Error::Invalid(
                "refund exceeds original admitted collection",
            ));
        }
        tx.execute(
            "INSERT INTO commission_refund VALUES(?,?,?,?)",
            params![r.id, r.admission, encode(r)?, r.state],
        )?;
        tx.commit()?;
        Ok((r.clone(), true))
    }
    pub fn commission_refund(&self, id: &str) -> Result<Option<Refund>> {
        read_refund(&self.connection, id)
    }
    pub fn seal_commission_refund(&mut self, id: &str, invoice: &str) -> Result<Refund> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut r = read_refund(&tx, id)?.ok_or(Error::Invalid("refund preparation absent"))?;
        if r.invoice.is_some() {
            if r.invoice.as_deref() != Some(invoice) {
                return Err(Error::Conflict("refund invoice changed"));
            }
            return Ok(r);
        }
        if r.state != "preparing" || invoice.len() > 16384 {
            return Err(Error::Invalid("refund invoice state"));
        }
        r.invoice = Some(invoice.into());
        r.state = "issued-held".into();
        tx.execute(
            "UPDATE commission_refund SET json=?,state=? WHERE id=?",
            params![encode(&r)?, r.state, id],
        )?;
        tx.commit()?;
        Ok(r)
    }
    pub fn set_commission_refund_outcome(&mut self, id: &str, state: &str) -> Result<Refund> {
        if !matches!(state, "unknown" | "failed" | "reversed") {
            return Err(Error::Invalid("refund outcome"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut r = read_refund(&tx, id)?.ok_or(Error::Invalid("refund record absent"))?;
        if matches!(r.state.as_str(), "reversed" | "failed") && r.state != state {
            return Err(Error::Conflict("terminal native refund changed"));
        }
        r.state = state.into();
        tx.execute(
            "UPDATE commission_refund SET json=?,state=? WHERE id=?",
            params![encode(&r)?, state, id],
        )?;
        tx.commit()?;
        Ok(r)
    }
}
fn read_refund(c: &Connection, id: &str) -> Result<Option<Refund>> {
    let v: Option<String> = c
        .query_row("SELECT json FROM commission_refund WHERE id=?", [id], |r| {
            r.get(0)
        })
        .optional()?;
    v.map(|j| serde_json::from_str(&j).map_err(|_| Error::Invalid("native refund record")))
        .transpose()
}

/// A pylon forfeit that came too late (`role='provider'`, the share was
/// already swept) leaves nothing unbacked, so it never holds payouts.
pub(crate) fn payouts_held_in(c: &Connection) -> Result<bool> {
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM payable_adjustment WHERE loss_msat>0 AND role!='provider') OR EXISTS(SELECT 1 FROM commission_balance WHERE funding_loss>0 OR payee_loss>0) OR EXISTS(SELECT 1 FROM commission_refund WHERE state IN ('preparing','issued-held','unknown')) OR EXISTS(SELECT 1 FROM commission_admission a JOIN settlement s ON s.payment_hash=a.payment_hash WHERE NOT EXISTS(SELECT 1 FROM commission_obligation o WHERE o.id=a.id))",[],|r|r.get(0))?)
}
impl Ledger {
    /// Unobserved admitted collections, losses, and unresolved native refund
    /// dispatch hold new payout planning.
    /// Existing sending attempts are still resolved by their original reference.
    pub fn commission_payouts_held(&self) -> Result<bool> {
        payouts_held_in(&self.connection)
    }
    /// Liabilities outside whole-satoshi available claims remain backed. A
    /// hold or sub-sat remainder does not disappear from wallet reconciliation.
    pub fn commission_held_liability(&self) -> Result<i64> {
        Ok(self.connection.query_row("SELECT COALESCE(SUM(CASE WHEN b.state='held' THEN b.encumbered ELSE b.remainder + CASE WHEN h.state='held' AND b.protected=0 THEN b.net-b.remainder ELSE 0 END END),0) FROM commission_balance b LEFT JOIN commission_abuse h ON h.admission=b.id",[],|r|r.get(0))?)
    }
}

#[cfg(unix)]
pub(crate) struct Custody {
    path: std::path::PathBuf,
    directory: std::fs::File,
    file: std::fs::File,
}
#[cfg(unix)]
impl Custody {
    fn check(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let visible = std::fs::symlink_metadata(&self.path)
            .map_err(|_| Error::Denied("native ledger replaced"))?;
        let file = self
            .file
            .metadata()
            .map_err(|_| Error::Denied("native ledger unavailable"))?;
        let dir = self
            .directory
            .metadata()
            .map_err(|_| Error::Denied("native ledger directory unavailable"))?;
        let current_dir = std::fs::symlink_metadata(self.path.parent().unwrap())
            .map_err(|_| Error::Denied("native ledger directory replaced"))?;
        // SAFETY: geteuid takes no arguments and does not mutate process state.
        let uid = unsafe { libc::geteuid() };
        if !visible.is_file()
            || visible.uid() != uid
            || visible.mode() & 0o077 != 0
            || file.nlink() != 1
            || file.dev() != visible.dev()
            || file.ino() != visible.ino()
            || !current_dir.is_dir()
            || current_dir.uid() != uid
            || current_dir.mode() & 0o077 != 0
            || dir.dev() != current_dir.dev()
            || dir.ino() != current_dir.ino()
        {
            return Err(Error::Denied("native ledger custody changed or disclosed"));
        }
        Ok(())
    }
}
#[cfg(unix)]
impl Ledger {
    /// Open an existing private merchant ledger while retaining its directory
    /// and file identities through blocking wallet evidence reads.
    pub fn open_native(path: &std::path::Path) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() {
            return Err(Error::Invalid("explicit native ledger path"));
        }
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(
                path.parent()
                    .ok_or(Error::Invalid("native ledger parent"))?,
            )
            .map_err(|_| Error::Denied("native ledger directory unavailable"))?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| Error::Denied("native ledger file unavailable"))?;
        let custody = Custody {
            path: path.into(),
            directory,
            file,
        };
        custody.check()?;
        let mut ledger = Self::open(path)?;
        custody.check()?;
        ledger.custody = Some(custody);
        Ok(ledger)
    }
    pub fn require_native_custody(&self) -> Result<()> {
        self.custody
            .as_ref()
            .ok_or(Error::Denied("protected native ledger required"))?
            .check()
    }
}

#[cfg(test)]
#[path = "commission_tests.rs"]
mod tests;

fn qualified_costs(c: &Connection, a: &Admission) -> Result<Vec<Cost>> {
    let json: Option<String> = c
        .query_row(
            "SELECT json FROM commission_cost_evidence WHERE admission=?",
            [&a.id],
            |r| r.get(0),
        )
        .optional()?;
    match json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|_| Error::Invalid("native qualified cost evidence")),
        None => Ok(a.costs.clone()),
    }
}
impl Ledger {
    /// One immutable cost qualification can fill previously unknown classes.
    /// Original known costs and their provenance never change retroactively.
    pub fn qualify_commission_costs(
        &mut self,
        id: &str,
        policy: &str,
        costs: &[Cost],
    ) -> Result<()> {
        if !hex(policy) || costs.len() != 6 {
            return Err(Error::Invalid("qualified cost policy"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let a = read(&tx, id)?.ok_or(Error::Invalid("commission admission absent"))?;
        let encoded = encode(&costs)?;
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT policy,json FROM commission_cost_evidence WHERE admission=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some(old) = prior {
            if old != (policy.into(), encoded) {
                return Err(Error::Conflict("qualified costs already pinned"));
            }
            return Ok(());
        }
        for old in &a.costs {
            let matches: Vec<_> = costs
                .iter()
                .filter(|c| c.category == old.category)
                .collect();
            if matches.len() != 1
                || matches[0].amount_msat.is_none()
                || matches[0].amount_msat.is_some_and(|v| v > i64::MAX as u64)
                || matches[0].evidence.is_empty()
                || matches[0].provenance != "operator-declared"
                || old.amount_msat.is_some() && matches[0] != old
            {
                return Err(Error::Conflict(
                    "original known cost or exact provenance changed",
                ));
            }
        }
        tx.execute(
            "INSERT INTO commission_cost_evidence VALUES(?,?,?)",
            params![id, policy, encoded],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// A payee's bounded private figures omit buyer identities, invoices, purchase
/// authorizations, cost policy text, and other payees' shares.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PayeeFigures {
    pub original_earned_msat: i64,
    pub held_msat: i64,
    pub reversed_msat: i64,
    pub paid_or_reserved_reversal_loss_msat: i64,
    pub retained_remainder_msat: i64,
    pub unknown_cost_obligations: i64,
}
impl Ledger {
    pub fn commission_payee_figures(&self, party: &str) -> Result<PayeeFigures> {
        let mut q = self
            .connection
            .prepare("SELECT id FROM commission_obligation WHERE party=? ORDER BY id LIMIT 4097")?;
        let ids = q
            .query_map([party], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if ids.len() > 4096 {
            return Err(Error::Invalid("commission payee report bound"));
        }
        let mut figures = PayeeFigures::default();
        for id in ids {
            let report = self.commission_report(&id)?;
            figures.original_earned_msat = figures
                .original_earned_msat
                .checked_add(report.earned_msat)
                .ok_or(Error::Invalid("commission figures overflow"))?;
            figures.held_msat = figures
                .held_msat
                .checked_add(report.held_msat)
                .ok_or(Error::Invalid("commission figures overflow"))?;
            figures.reversed_msat = figures
                .reversed_msat
                .checked_add(report.commission_reversed_msat)
                .ok_or(Error::Invalid("commission figures overflow"))?;
            figures.paid_or_reserved_reversal_loss_msat = figures
                .paid_or_reserved_reversal_loss_msat
                .checked_add(report.paid_or_reserved_reversal_loss_msat)
                .ok_or(Error::Invalid("commission figures overflow"))?;
            figures.retained_remainder_msat = figures
                .retained_remainder_msat
                .checked_add(report.retained_remainder_msat)
                .ok_or(Error::Invalid("commission figures overflow"))?;
            figures.unknown_cost_obligations +=
                i64::from(report.cost_msat.is_none() && report.state == "held");
        }
        Ok(figures)
    }
    pub fn commission_destination_qualified(&self, party: &str, kind: &str) -> Result<bool> {
        let mut q = self.connection.prepare(
            "SELECT a.json FROM commission_admission a JOIN commission_balance b ON b.id=a.id WHERE json_extract(a.json,'$.party')=? AND ((b.state='held' AND b.encumbered>0) OR b.net>b.protected) LIMIT 4097",
        )?;
        let json = q
            .query_map([party], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if json.len() > 4096 {
            return Err(Error::Invalid("commission destination policy bound"));
        }
        for value in &json {
            let a: Admission = serde_json::from_str(value)
                .map_err(|_| Error::Invalid("native commission destination terms"))?;
            if !a.destinations.iter().any(|v| v == kind) {
                return Ok(false);
            }
        }
        Ok(!json.is_empty())
    }
}

impl Ledger {
    pub(crate) fn native_commission_refunds(&self) -> Result<Vec<Refund>> {
        let mut q = self
            .connection
            .prepare("SELECT json FROM commission_refund ORDER BY id LIMIT 4097")?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if rows.len() > 4096 {
            return Err(Error::Invalid("native refund reconciliation bound"));
        }
        rows.into_iter()
            .map(|j| {
                serde_json::from_str(&j)
                    .map_err(|_| Error::Invalid("native refund reconciliation record"))
            })
            .collect()
    }
    pub fn commission_reversal_payment(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT json FROM commission_native_reversal WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| {
            serde_json::from_str(&j).map_err(|_| Error::Invalid("native reversal evidence"))
        })
        .transpose()
    }
}
