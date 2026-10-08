//! Earned sales (REV-71): which service sales count as revenue, the bell
//! that rings once for each, and the delayed aggregate the owner reviews
//! before anything shared shows it. A sale earns only on a verified `Paid`
//! settlement and reconciled delivery evidence; top-ups, grants, meetings,
//! unpaid invoices, and agent-reported success produce nothing here.
use super::{Access, Result, Store, digest};
use receipts::service_sale::{Disposition, Sale};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LEDGER_SCHEMA: &str = "openagents.sales-earned-ledger.v1";
pub const AGGREGATE_SCHEMA: &str = "openagents.sales-shared-aggregate.v1";
/// Shared aggregates lag the books by at least this long, s.
pub const SHARED_DELAY: u64 = 7 * 86_400;
/// Shared amounts are floored to this many USD millionths (USD 100).
pub const SHARED_GRAIN: u64 = 100_000_000;
pub const MAX_RUNG: usize = 4096;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    /// Sale key → when its bell rang. A key rings once for all time.
    pub rung: BTreeMap<String, u64>,
    /// Aggregate digest → the owner's review of that exact projection.
    pub approvals: BTreeMap<String, Approval>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub approved_by: String,
    pub approved_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Settlement {
    Paid,
    Reversed,
    Disputed,
    Unknown,
    Pending,
    Invalid,
}

/// One service sale as the money books see it. Owner-private.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub key: String,
    pub lead: String,
    pub sale: String,
    pub settlement: Settlement,
    pub gross_usd_millionths: u64,
    pub refunded_usd_millionths: u64,
    pub net_usd_millionths: u64,
    pub delivery_reconciled: bool,
    pub eligible: bool,
    pub ineligible_because: Vec<String>,
    pub rung_at: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub gross_usd_millionths: u64,
    pub refunded_usd_millionths: u64,
    pub net_usd_millionths: u64,
    pub earned_sales: u64,
    pub unknown_settlement: u64,
    pub reversals: u64,
    pub rung: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    pub schema: String,
    pub generated_at: u64,
    pub owner: String,
    pub rows: Vec<Row>,
    pub totals: Totals,
}

/// A bell that just rang, by sale key only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ring {
    pub key: String,
    pub at: u64,
}

/// What a shared surface may show: counts and a floored amount through a
/// day boundary at least [`SHARED_DELAY`] old. No lead, sale, or timing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aggregate {
    pub schema: String,
    pub through: u64,
    pub earned_sales: u64,
    pub net_usd_millionths_floor: u64,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub aggregate: Aggregate,
    /// Paul's weekly update, for the owner to publish or discard.
    pub weekly_update: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Shared {
    Available {
        aggregate: Aggregate,
        approved_at: u64,
        expires_at: u64,
    },
    Unavailable {
        reason: String,
    },
}

fn day(t: u64) -> u64 {
    t - t % 86_400
}

fn usd(sale: &Sale, minor: u64) -> Result<u64> {
    let invoice = &sale.admission.invoice;
    receipts::service_sale::usd_millionths(&invoice.currency, invoice.currency_scale, minor)
}

fn row(lead: &str, id: &str, sale: &Sale, rung: &BTreeMap<String, u64>) -> Row {
    let key = digest(format!("{lead}\n{id}").as_bytes());
    let mut why = Vec::new();
    let (settlement, gross, refunded) = match sale.summary() {
        Ok(summary) => {
            let latest = sale.payments.last().map(|v| v.input.disposition);
            let settlement = match latest {
                Some(Disposition::Paid) => Settlement::Paid,
                Some(Disposition::Reversed) => Settlement::Reversed,
                Some(Disposition::Disputed) => Settlement::Disputed,
                Some(Disposition::Unknown) => Settlement::Unknown,
                Some(Disposition::Pending) | None => Settlement::Pending,
            };
            let gross = usd(sale, summary.paid_minor).unwrap_or(0);
            let refunded = usd(
                sale,
                summary
                    .refunded_minor
                    .saturating_sub(summary.refund_reversals_minor),
            )
            .unwrap_or(0);
            (settlement, gross, refunded.min(gross))
        }
        Err(e) => {
            why.push(format!("settlement record invalid: {e}"));
            (Settlement::Invalid, 0, 0)
        }
    };
    let delivery = match sale.effective_fulfillment() {
        Ok(Some(f)) => f.bill.is_some(),
        Ok(None) => false,
        Err(e) => {
            why.push(format!("delivery record invalid: {e}"));
            false
        }
    };
    if !matches!(settlement, Settlement::Paid | Settlement::Reversed) {
        why.push(format!("settlement is {settlement:?}").to_lowercase());
    }
    if gross == 0 {
        why.push("no verified collection".into());
    }
    if !delivery {
        why.push("delivery not reconciled".into());
    }
    Row {
        eligible: why.is_empty(),
        rung_at: rung.get(&key).copied(),
        key,
        lead: lead.into(),
        sale: id.into(),
        settlement,
        gross_usd_millionths: gross,
        refunded_usd_millionths: refunded,
        net_usd_millionths: gross - refunded,
        delivery_reconciled: delivery,
        ineligible_because: why,
    }
}

impl Store {
    fn earned_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .state
            .leads
            .iter()
            .flat_map(|(lead, l)| {
                l.service_sales
                    .iter()
                    .map(move |(id, sale)| row(lead, id, sale, &self.state.earned.rung))
            })
            .collect();
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        rows
    }

    fn totals(rows: &[Row]) -> Totals {
        let mut t = Totals::default();
        for r in rows {
            match r.settlement {
                Settlement::Unknown | Settlement::Pending | Settlement::Disputed => {
                    t.unknown_settlement += 1;
                }
                Settlement::Reversed => t.reversals += 1,
                Settlement::Paid | Settlement::Invalid => {}
            }
            if r.eligible {
                t.earned_sales += 1;
                t.gross_usd_millionths = t
                    .gross_usd_millionths
                    .saturating_add(r.gross_usd_millionths);
                t.refunded_usd_millionths = t
                    .refunded_usd_millionths
                    .saturating_add(r.refunded_usd_millionths);
                t.net_usd_millionths = t.net_usd_millionths.saturating_add(r.net_usd_millionths);
            }
            if r.rung_at.is_some() {
                t.rung += 1;
            }
        }
        t
    }

    /// The owner's earned-sale ledger, recomputed from the service sales.
    pub fn earned_ledger(&mut self, owner: &Access) -> Result<Ledger> {
        self.refresh()?;
        self.admin(owner)?;
        let rows = self.earned_rows();
        Ok(Ledger {
            schema: LEDGER_SCHEMA.into(),
            generated_at: (self.clock)(),
            owner: owner.principal.clone(),
            totals: Self::totals(&rows),
            rows,
        })
    }

    /// Rings the bell for every eligible sale that has not rung, durably,
    /// and returns only those. A replay, restart, or later refund returns
    /// nothing.
    pub fn ring_earned(&mut self, owner: &Access) -> Result<Vec<Ring>> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let fresh: Vec<Ring> = self
            .earned_rows()
            .into_iter()
            .filter(|r| r.eligible && r.rung_at.is_none())
            .map(|r| Ring {
                key: r.key,
                at: now,
            })
            .collect();
        if fresh.is_empty() {
            return Ok(fresh);
        }
        let mut next = self.state.clone();
        for ring in &fresh {
            next.earned.rung.insert(ring.key.clone(), ring.at);
        }
        if next.earned.rung.len() > MAX_RUNG {
            return Err("earned bell history exceeds bound".into());
        }
        self.persist(next)?;
        Ok(fresh)
    }

    fn aggregate_through(&self, through: u64) -> Result<Aggregate> {
        let now = (self.clock)();
        let through = day(through);
        if through == 0 || through.saturating_add(SHARED_DELAY) > now {
            return Err("shared aggregates lag the books by seven days".into());
        }
        let rows = self.earned_rows();
        let mut count = 0u64;
        let mut net = 0u64;
        for r in &rows {
            let settled_at = self
                .state
                .leads
                .get(&r.lead)
                .and_then(|l| l.service_sales.get(&r.sale))
                .and_then(|s| {
                    s.payments
                        .iter()
                        .find(|v| v.input.disposition == Disposition::Paid)
                        .map(|v| v.verified_at)
                });
            if r.eligible && settled_at.is_some_and(|t| t < through) {
                count += 1;
                net = net.saturating_add(r.net_usd_millionths);
            }
        }
        let floor = net - net % SHARED_GRAIN;
        let digest = digest(format!("{AGGREGATE_SCHEMA}\n{through}\n{count}\n{floor}").as_bytes());
        Ok(Aggregate {
            schema: AGGREGATE_SCHEMA.into(),
            through,
            earned_sales: count,
            net_usd_millionths_floor: floor,
            digest,
        })
    }

    /// Paul's draft of the shared aggregate through a day boundary, with
    /// the weekly update text. Publishes nothing.
    pub fn shared_aggregate_draft(&mut self, owner: &Access, through: u64) -> Result<Draft> {
        self.refresh()?;
        self.admin(owner)?;
        let aggregate = self.aggregate_through(through)?;
        let weekly_update = format!(
            "Through day {}: {} earned sale{} on settled, delivered service work; net at least USD {}. Counts come from verified settlements and reconciled delivery, not pipeline stages.",
            aggregate.through / 86_400,
            aggregate.earned_sales,
            if aggregate.earned_sales == 1 { "" } else { "s" },
            aggregate.net_usd_millionths_floor / 1_000_000
        );
        Ok(Draft {
            aggregate,
            weekly_update,
        })
    }

    /// The owner approves one exact aggregate digest until `expires_at`.
    pub fn approve_shared_aggregate(
        &mut self,
        owner: &Access,
        through: u64,
        digest: &str,
        expires_at: u64,
    ) -> Result<Aggregate> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let aggregate = self.aggregate_through(through)?;
        if aggregate.digest != digest {
            return Err("shared aggregate changed since the draft; review the current one".into());
        }
        if expires_at <= now || expires_at > now + 90 * 86_400 {
            return Err("shared aggregate approval expires within 90 days".into());
        }
        let mut next = self.state.clone();
        next.earned.approvals.insert(
            aggregate.digest.clone(),
            Approval {
                approved_by: owner.principal.clone(),
                approved_at: now,
                expires_at,
            },
        );
        if next.earned.approvals.len() > 64 {
            return Err("shared aggregate approvals exceed bound".into());
        }
        self.persist(next)?;
        Ok(aggregate)
    }

    /// Withdraws an approval; shared surfaces go unavailable.
    pub fn revoke_shared_aggregate(&mut self, owner: &Access, digest: &str) -> Result<bool> {
        self.refresh()?;
        self.admin(owner)?;
        let mut next = self.state.clone();
        let removed = next.earned.approvals.remove(digest).is_some();
        if removed {
            self.persist(next)?;
        }
        Ok(removed)
    }

    /// What a shared surface may show now: the newest unexpired approval
    /// whose aggregate still recomputes to the approved digest. Needs no
    /// credential because it carries nothing private.
    pub fn shared_aggregate(&mut self) -> Result<Shared> {
        self.refresh()?;
        let now = (self.clock)();
        let mut best: Option<Shared> = None;
        let mut reason = "no reviewed aggregate".to_string();
        let approvals: Vec<_> = self
            .state
            .earned
            .approvals
            .iter()
            .map(|(d, a)| (d.clone(), a.clone()))
            .collect();
        for (digest, approval) in approvals {
            if approval.expires_at <= now {
                reason = "reviewed aggregate expired".into();
                continue;
            }
            let through = self
                .state
                .leads
                .values()
                .flat_map(|l| l.service_sales.values())
                .flat_map(|s| s.payments.iter().map(|v| day(v.verified_at) + 86_400))
                .filter(|t| t.saturating_add(SHARED_DELAY) <= now)
                .chain(std::iter::once(day(now.saturating_sub(SHARED_DELAY))))
                .collect::<Vec<_>>();
            let matches = through
                .iter()
                .filter_map(|t| self.aggregate_through(*t).ok())
                .find(|a| a.digest == digest);
            match matches {
                Some(aggregate) => {
                    let newer = best.as_ref().is_none_or(|b| match b {
                        Shared::Available { approved_at, .. } => {
                            approval.approved_at > *approved_at
                        }
                        Shared::Unavailable { .. } => true,
                    });
                    if newer {
                        best = Some(Shared::Available {
                            aggregate,
                            approved_at: approval.approved_at,
                            expires_at: approval.expires_at,
                        });
                    }
                }
                None => reason = "reviewed aggregate no longer matches the books".into(),
            }
        }
        Ok(best.unwrap_or(Shared::Unavailable { reason }))
    }
}

#[cfg(test)]
#[path = "earned/tests.rs"]
mod tests;
