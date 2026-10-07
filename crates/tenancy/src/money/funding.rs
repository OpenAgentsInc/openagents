//! Pinned funding terms and credit provenance within the workspace ledger.
//!
//! Conversion sources are operator-configured documents, never a live rate
//! oracle. A payment adapter verifies finality before recording it here.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Price, identity};

mod quotes;
pub use quotes::{AdmittedQuote, Quote};
mod snapshots;
pub use snapshots::Snapshot;

pub const POLICY_SCHEMA: &str = "openagents.money.funding-policy.v1";

pub use receipts::funding_units::{Conversion, Converted, FeePayer, Rounding, Unit};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Finality {
    Pending,
    Confirmed,
    Final,
}

/// Unrecoverable spent credit remains the operator's loss. This contract does
/// not create customer debt or authorize a second charge or balance redemption.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpentCreditLoss {
    Operator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseTerms {
    pub required_finality: Finality,
    pub refunds_allowed: bool,
    pub disputes_allowed: bool,
    pub spent_credit_loss: SpentCreditLoss,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionTerms {
    /// Lifetime workspace issuance cap, including expired and reversed grants.
    pub total_cap: u64,
    pub grant_cap: u64,
    pub max_lifetime_seconds: u64,
    /// Admissions consuming a grant, including subsequently released attempts.
    pub max_admissions: u64,
    pub price_policies: BTreeSet<String>,
    pub reversible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub version: String,
    pub unit: Unit,
    pub conversions: Vec<Conversion>,
    pub purchases: PurchaseTerms,
    pub promotions: PromotionTerms,
}

impl Policy {
    pub fn validate(&self) -> Result<(), String> {
        identity(&self.version)?;
        self.unit.validate()?;
        if self.schema != POLICY_SCHEMA
            || self.purchases.required_finality == Finality::Pending
            || self.conversions.len() > 64
            || self.promotions.price_policies.len() > 64
        {
            return Err("funding policy schema or required finality is invalid".into());
        }
        let mut versions = BTreeSet::new();
        for conversion in &self.conversions {
            conversion.validate()?;
            if conversion.target != self.unit || !versions.insert(&conversion.version) {
                return Err("conversion target or version conflicts with funding policy".into());
            }
        }
        let promo = &self.promotions;
        if promo.total_cap > 0
            && (promo.grant_cap == 0
                || promo.grant_cap > promo.total_cap
                || promo.max_lifetime_seconds == 0
                || promo.max_admissions == 0
                || promo.price_policies.is_empty())
        {
            return Err("promotional credit needs bounded issuance, expiry, and uses".into());
        }
        for value in &promo.price_policies {
            identity(value)?;
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
            .map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Funding {
    pub id: String,
    pub origin: String,
    /// Stable provider/payment identity; it cannot fund another purchase.
    pub payment: String,
    pub policy: String,
    pub conversion: String,
    pub gross_units: u64,
    pub fee_units: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Promotion {
    pub id: String,
    pub origin: String,
    pub policy: String,
    pub amount: u64,
    pub expires_at: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reversal {
    Refund,
    Dispute,
}

#[derive(Clone, Debug, Serialize)]
pub struct FundingRecord {
    pub funding: Funding,
    pub policy_digest: String,
    pub conversion: Conversion,
    pub quote: Converted,
    pub quoted_at: u64,
    pub finality: Finality,
    pub finality_evidence: Option<String>,
    pub credited: bool,
    pub reversed_source_units: u64,
    pub reversed_credit: u64,
    /// Fraction in the nominal conversion of cumulative reversed source.
    /// This is not the fraction of credit removed from the original quote.
    pub reversal_remainder: u64,
    /// Uncredited fraction in the remaining quoted source backing.
    pub remaining_remainder: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Purchased,
    Promotional,
}

#[derive(Clone, Debug, Serialize)]
pub struct Lot {
    pub id: String,
    pub kind: Kind,
    pub origin: String,
    pub policy: String,
    pub policy_digest: String,
    pub amount: u64,
    pub reversed: u64,
    pub expires_at: Option<u64>,
    pub admissions: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrantPosition {
    pub lot: Lot,
    pub available: u64,
    pub expired: bool,
    /// None denotes purchased credit without a trial-admission restriction.
    pub admissions_remaining: Option<u64>,
    pub price_policies: BTreeSet<String>,
}

/// These allocations are part of the same durable hold as its usage quote.
#[derive(Clone, Debug, Serialize)]
pub struct Allocation {
    pub lot: String,
    pub kind: Kind,
    pub policy_digest: String,
    pub reserved: u64,
    pub charged: u64,
    pub refunded: u64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Used {
    pub held: u64,
    pub spent: u64,
}

#[derive(Default)]
pub(super) struct Summary {
    pub purchased: u64,
    pub promotional: u64,
    pub reversed: u64,
    pub expired: u64,
    pub restricted: u64,
    pub available: u64,
    pub operator_loss: u64,
    pub uncovered_holds: u64,
    pub processor_expense_units: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Book {
    pub active: String,
    pub policies: BTreeMap<String, Policy>,
    pub funding: BTreeMap<String, FundingRecord>,
    pub lots: BTreeMap<String, Lot>,
    pub quotes: BTreeMap<String, AdmittedQuote>,
    pub snapshots: BTreeMap<String, Snapshot>,
}

fn add(total: &mut u64, amount: u64) -> Result<(), String> {
    *total = total
        .checked_add(amount)
        .ok_or("credit provenance overflow")?;
    Ok(())
}

impl Book {
    pub fn install(&mut self, policy: &Policy, code: &str) -> Result<(), String> {
        policy.validate()?;
        if policy.unit
            != (Unit::CurrencyMillionths {
                currency: code.into(),
            })
        {
            return Err("funding policy unit differs from workspace currency and scale".into());
        }
        if self
            .policies
            .get(&policy.version)
            .is_some_and(|prior| prior != policy)
        {
            return Err("funding policy version was reused for changed terms".into());
        }
        self.policies.insert(policy.version.clone(), policy.clone());
        self.active.clone_from(&policy.version);
        Ok(())
    }

    pub fn begin(&mut self, funding: &Funding, at: u64) -> Result<(), String> {
        if self.quotes.contains_key(&funding.id) {
            return Err("accepted funding quote requires its original native admission".into());
        }
        if funding.policy != self.active {
            return Err("new funding must pin the active policy".into());
        }
        self.begin_admitted(funding, at)
    }

    fn begin_admitted(&mut self, funding: &Funding, quoted_at: u64) -> Result<(), String> {
        for value in [&funding.id, &funding.origin, &funding.payment] {
            identity(value)?;
        }
        if self.funding.contains_key(&funding.id)
            || self.lots.contains_key(&funding.id)
            || self
                .funding
                .values()
                .any(|r| r.funding.payment == funding.payment)
        {
            return Err(
                "funding identity or payment already exists; retry its original source".into(),
            );
        }
        let policy = self
            .policies
            .get(&funding.policy)
            .ok_or("funding policy is missing")?;
        let conversion = policy
            .conversions
            .iter()
            .find(|c| c.version == funding.conversion)
            .ok_or("conversion is unsupported or unknown; no credit is available")?;
        let quote = conversion.quote(funding.gross_units, funding.fee_units, quoted_at)?;
        self.funding.insert(
            funding.id.clone(),
            FundingRecord {
                funding: funding.clone(),
                policy_digest: policy.digest()?,
                conversion: conversion.clone(),
                remaining_remainder: quote.remainder,
                quote,
                quoted_at,
                finality: Finality::Pending,
                finality_evidence: None,
                credited: false,
                reversed_source_units: 0,
                reversed_credit: 0,
                reversal_remainder: 0,
            },
        );
        Ok(())
    }

    pub fn confirm(&mut self, id: &str, finality: Finality, evidence: &str) -> Result<u64, String> {
        if self.snapshots.contains_key(id) {
            return Err("provider funding must use its original snapshot reconciliation".into());
        }
        self.confirm_admitted(id, finality, evidence)
    }

    fn confirm_admitted(
        &mut self,
        id: &str,
        finality: Finality,
        evidence: &str,
    ) -> Result<u64, String> {
        identity(evidence)?;
        let record = self.funding.get_mut(id).ok_or("funding is missing")?;
        if finality < record.finality {
            return Err("payment finality cannot move backward; record a reversal".into());
        }
        record.finality = finality;
        record.finality_evidence = Some(evidence.into());
        let policy = &self.policies[&record.funding.policy];
        if record.credited || finality < policy.purchases.required_finality {
            return Ok(0);
        }
        record.credited = true;
        let amount = record.quote.credited_units;
        self.lots.insert(
            id.into(),
            Lot {
                id: id.into(),
                kind: Kind::Purchased,
                origin: record.funding.origin.clone(),
                policy: policy.version.clone(),
                policy_digest: record.policy_digest.clone(),
                amount,
                reversed: 0,
                expires_at: None,
                admissions: 0,
            },
        );
        Ok(amount)
    }

    pub fn promote(&mut self, grant: &Promotion, at: u64) -> Result<u64, String> {
        identity(&grant.id)?;
        identity(&grant.origin)?;
        if grant.policy != self.active
            || self.lots.contains_key(&grant.id)
            || self.funding.contains_key(&grant.id)
            || self.quotes.contains_key(&grant.id)
        {
            return Err("promotion policy or identity conflicts; retry its original source".into());
        }
        let policy = &self.policies[&self.active];
        let terms = &policy.promotions;
        let issued = self
            .lots
            .values()
            .filter(|l| l.kind == Kind::Promotional)
            .try_fold(0_u64, |n, l| {
                n.checked_add(l.amount).ok_or("promotion issuance overflow")
            })?;
        if grant.amount == 0
            || grant.amount > terms.grant_cap
            || issued
                .checked_add(grant.amount)
                .ok_or("promotion issuance overflow")?
                > terms.total_cap
            || grant.expires_at <= at
            || grant.expires_at
                > at.checked_add(terms.max_lifetime_seconds)
                    .ok_or("promotion expiry overflow")?
        {
            return Err("promotional credit exceeds its issuance cap or expiry terms".into());
        }
        self.lots.insert(
            grant.id.clone(),
            Lot {
                id: grant.id.clone(),
                kind: Kind::Promotional,
                origin: grant.origin.clone(),
                policy: policy.version.clone(),
                policy_digest: policy.digest()?,
                amount: grant.amount,
                reversed: 0,
                expires_at: Some(grant.expires_at),
                admissions: 0,
            },
        );
        Ok(grant.amount)
    }

    pub fn reverse_funding(
        &mut self,
        id: &str,
        units: u64,
        reason: Reversal,
    ) -> Result<(), String> {
        if self.snapshots.contains_key(id) {
            return Err("provider funding must use its original snapshot reconciliation".into());
        }
        let record = self.funding.get_mut(id).ok_or("funding is missing")?;
        let terms = &self.policies[&record.funding.policy].purchases;
        let allowed = match reason {
            Reversal::Refund => terms.refunds_allowed,
            Reversal::Dispute => terms.disputes_allowed,
        };
        if !record.credited || units == 0 || !allowed {
            return Err("funding reversal is not permitted by the pinned purchase terms".into());
        }
        let cumulative = record
            .reversed_source_units
            .checked_add(units)
            .ok_or("funding reversal overflow")?;
        if cumulative > record.quote.convertible_units {
            return Err(
                "funding reversal exceeds its convertible source amount; fees are not refundable"
                    .into(),
            );
        }
        self.set_reversed_source(id, cumulative)
    }

    fn set_reversed_source(&mut self, id: &str, cumulative: u64) -> Result<(), String> {
        let record = self.funding.get_mut(id).ok_or("funding is missing")?;
        // Credit must fit the remaining backing after every partial reversal.
        // Flooring the refunded value alone could leave one unsupported unit.
        // Compute from the original quote so splitting cannot change rounding.
        let remaining_source = record.quote.convertible_units - cumulative;
        let (remaining_credit, remaining_remainder) = record.conversion.amount(remaining_source)?;
        let credit = record
            .quote
            .credited_units
            .checked_sub(remaining_credit)
            .ok_or("funding reversal exceeds its original credit")?;
        let (_, reversal_remainder) = record.conversion.amount(cumulative)?;
        if record.credited {
            self.lots
                .get_mut(id)
                .ok_or("funding credit is missing")?
                .reversed = credit;
        }
        record.reversed_source_units = cumulative;
        record.reversed_credit = credit;
        record.reversal_remainder = reversal_remainder;
        record.remaining_remainder = remaining_remainder;
        Ok(())
    }

    pub fn reverse_promotion(&mut self, id: &str, amount: u64) -> Result<(), String> {
        let lot = self
            .lots
            .get_mut(id)
            .ok_or("promotional credit is missing")?;
        if lot.kind != Kind::Promotional
            || !self.policies[&lot.policy].promotions.reversible
            || amount == 0
        {
            return Err("promotional credit reversal is not permitted".into());
        }
        let reversed = lot
            .reversed
            .checked_add(amount)
            .ok_or("promotion reversal overflow")?;
        if reversed > lot.amount {
            return Err("promotion reversal exceeds its original grant".into());
        }
        lot.reversed = reversed;
        Ok(())
    }

    fn eligible(&self, lot: &Lot, at: u64, price: Option<&Price>) -> bool {
        if self
            .snapshots
            .get(&lot.id)
            .is_some_and(|snapshot| snapshot.reconciliation_pending)
        {
            return false;
        }
        if lot.expires_at.is_some_and(|expiry| at >= expiry) {
            return false;
        }
        if lot.kind == Kind::Purchased {
            return true;
        }
        let terms = &self.policies[&lot.policy].promotions;
        lot.admissions < terms.max_admissions
            && price.is_none_or(|p| terms.price_policies.contains(&p.policy))
    }

    /// Existing obligations survive a reversal. With no approved operator risk
    /// allowance, an uncovered obligation restricts every lot in the workspace.
    fn exposure_unapproved(&self, used: &BTreeMap<String, Used>) -> Result<bool, String> {
        if self
            .snapshots
            .values()
            .any(|snapshot| snapshot.processor_expense_units > 0)
        {
            return Ok(true);
        }
        for lot in self.lots.values() {
            let used = used.get(&lot.id).copied().unwrap_or_default();
            let committed = used
                .held
                .checked_add(used.spent)
                .ok_or("credit allocation overflow")?;
            let nominal = lot
                .amount
                .checked_sub(lot.reversed)
                .ok_or("invalid reversed credit")?;
            if committed > nominal {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn allocate(
        &mut self,
        amount: u64,
        price: &Price,
        used: &BTreeMap<String, Used>,
        at: u64,
    ) -> Result<Vec<Allocation>, String> {
        if self.exposure_unapproved(used)? {
            return Err("workspace funding has unapproved uncovered obligations".into());
        }
        let mut order: Vec<_> = self.lots.keys().cloned().collect();
        order.sort_by_key(|id| {
            let lot = &self.lots[id];
            (
                lot.kind == Kind::Purchased,
                lot.expires_at.unwrap_or(u64::MAX),
                id.clone(),
            )
        });
        let mut remaining = amount;
        let mut allocations = Vec::new();
        for id in order {
            let lot = &self.lots[&id];
            if remaining == 0 {
                break;
            }
            if !self.eligible(lot, at, Some(price)) {
                continue;
            }
            let used = used.get(&id).copied().unwrap_or_default();
            let committed = used
                .held
                .checked_add(used.spent)
                .ok_or("credit allocation overflow")?;
            let free = (lot.amount - lot.reversed).saturating_sub(committed);
            let reserved = remaining.min(free);
            if reserved == 0 {
                continue;
            }
            allocations.push(Allocation {
                lot: id.clone(),
                kind: lot.kind,
                policy_digest: lot.policy_digest.clone(),
                reserved,
                charged: 0,
                refunded: 0,
            });
            remaining -= reserved;
            let lot = self.lots.get_mut(&id).unwrap();
            lot.admissions = lot
                .admissions
                .checked_add(1)
                .ok_or("credit admission overflow")?;
        }
        if remaining != 0 {
            return Err(
                "insufficient eligible credit for this price, expiry, or trial use policy".into(),
            );
        }
        Ok(allocations)
    }

    pub fn summary(
        &self,
        used: &BTreeMap<String, Used>,
        at: u64,
        price: Option<&Price>,
    ) -> Result<Summary, String> {
        let mut result = Summary::default();
        let exposure_unapproved = self.exposure_unapproved(used)?;
        for snapshot in self.snapshots.values() {
            result.processor_expense_units = result
                .processor_expense_units
                .checked_add(snapshot.processor_expense_units)
                .ok_or("processor expense overflow")?;
        }
        for lot in self.lots.values() {
            match lot.kind {
                Kind::Purchased => add(&mut result.purchased, lot.amount)?,
                Kind::Promotional => add(&mut result.promotional, lot.amount)?,
            }
            add(&mut result.reversed, lot.reversed)?;
            let used = used.get(&lot.id).copied().unwrap_or_default();
            let nominal = lot
                .amount
                .checked_sub(lot.reversed)
                .ok_or("invalid reversed credit")?;
            let committed = used
                .held
                .checked_add(used.spent)
                .ok_or("credit allocation overflow")?;
            let free = nominal.saturating_sub(committed);
            add(
                &mut result.operator_loss,
                used.spent.saturating_sub(nominal),
            )?;
            add(
                &mut result.uncovered_holds,
                used.held.saturating_sub(nominal.saturating_sub(used.spent)),
            )?;
            if lot.expires_at.is_some_and(|expiry| at >= expiry) {
                add(&mut result.expired, free)?;
            } else if exposure_unapproved || !self.eligible(lot, at, price) {
                add(&mut result.restricted, free)?;
            } else {
                add(&mut result.available, free)?;
            }
        }
        Ok(result)
    }

    pub fn positions(
        &self,
        used: &BTreeMap<String, Used>,
        at: u64,
    ) -> Result<Vec<GrantPosition>, String> {
        self.lots
            .values()
            .map(|lot| {
                let used = used.get(&lot.id).copied().unwrap_or_default();
                let committed = used
                    .held
                    .checked_add(used.spent)
                    .ok_or("credit allocation overflow")?;
                let (admissions_remaining, price_policies) = if lot.kind == Kind::Promotional {
                    let terms = &self.policies[&lot.policy].promotions;
                    (
                        Some(terms.max_admissions.saturating_sub(lot.admissions)),
                        terms.price_policies.clone(),
                    )
                } else {
                    (None, BTreeSet::new())
                };
                Ok(GrantPosition {
                    lot: lot.clone(),
                    available: if self.eligible(lot, at, None) {
                        (lot.amount - lot.reversed).saturating_sub(committed)
                    } else {
                        0
                    },
                    expired: lot.expires_at.is_some_and(|expiry| at >= expiry),
                    admissions_remaining,
                    price_policies,
                })
            })
            .collect()
    }
}

pub(super) fn settle(allocations: &mut [Allocation], charge: u64) -> Result<(), String> {
    let mut remaining = charge;
    for allocation in allocations {
        allocation.charged = remaining.min(allocation.reserved);
        remaining -= allocation.charged;
    }
    if remaining != 0 {
        return Err("charge exceeds credit allocations".into());
    }
    Ok(())
}

pub(super) fn refund(
    allocations: &mut [Allocation],
    amount: u64,
    reverse: bool,
) -> Result<(), String> {
    let mut remaining = amount;
    // Return purchased funding first when a hold also used promotional credit.
    for allocation in allocations.iter_mut().rev() {
        let available = if reverse {
            allocation.refunded
        } else {
            allocation.charged - allocation.refunded
        };
        let delta = remaining.min(available);
        if reverse {
            allocation.refunded -= delta;
        } else {
            allocation.refunded += delta;
        }
        remaining -= delta;
    }
    if remaining != 0 {
        return Err("refund exceeds its credit allocations".into());
    }
    Ok(())
}

pub(super) fn commissionable(allocations: &[Allocation]) -> Result<u64, String> {
    allocations
        .iter()
        .filter(|a| a.kind == Kind::Purchased)
        .try_fold(0_u64, |total, a| {
            total
                .checked_add(a.charged - a.refunded)
                .ok_or_else(|| "commissionable usage overflow".into())
        })
}
