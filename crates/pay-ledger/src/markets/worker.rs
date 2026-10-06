//! Paid worker profile proposal over exact MKT/LAB terms, separate from computer rental.
use super::{Deadlines, exact};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub const PROFILE: &str = "openagents.independent-worker-qualification.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerTerms {
    pub profile: String,
    pub buyer: String,
    pub provider: String,
    pub buyer_operator: String,
    pub provider_operator: String,
    pub market: String,
    pub order: String,
    pub labor_terms: String,
    pub source: String,
    pub checker: String,
    pub disclosure: String,
    pub execution_requirements: String,
    pub cancellation_policy: String,
    pub delivery_rights: String,
    pub price_msat: i64,
    pub fee_limit_msat: i64,
    pub capacity_units: u32,
    pub max_rework: u16,
    pub deadlines: Deadlines,
}
/// Current trusted host admission, supplied separately from provider-controlled terms.
#[derive(Clone, Debug)]
pub struct Admission {
    pub buyer: String,
    pub provider: String,
    pub buyer_operator: String,
    pub provider_operator: String,
    pub independent_operators_verified: bool,
    pub execution_requirements: String,
    pub disclosure: String,
    pub available_capacity: u32,
    pub expires_at: i64,
    pub payout_destination_verified: bool,
}
impl WorkerTerms {
    pub fn validate(&self, now: i64, admission: &Admission) -> Result<()> {
        if self.profile != PROFILE {
            return Err(Error::Invalid("worker profile"));
        }
        for pin in [
            &self.buyer,
            &self.provider,
            &self.market,
            &self.order,
            &self.labor_terms,
            &self.source,
            &self.checker,
            &self.disclosure,
            &self.execution_requirements,
            &self.cancellation_policy,
            &self.delivery_rights,
        ] {
            exact(pin)?;
        }
        self.deadlines.validate(now)?;
        if self.price_msat < 0
            || self.fee_limit_msat < 0
            || self.capacity_units == 0
            || self.max_rework > 32
            || self.buyer == self.provider
            || self.buyer_operator.is_empty()
            || self.provider_operator.is_empty()
            || self.buyer_operator == self.provider_operator
        {
            return Err(Error::Invalid("worker parties, price, capacity, or rework"));
        }
        if admission.buyer != self.buyer
            || admission.provider != self.provider
            || admission.buyer_operator != self.buyer_operator
            || admission.provider_operator != self.provider_operator
            || !admission.independent_operators_verified
            || admission.expires_at <= now
            || admission.execution_requirements != self.execution_requirements
            || admission.disclosure != self.disclosure
            || admission.available_capacity < self.capacity_units
            || (self.price_msat > 0 && !admission.payout_destination_verified)
        {
            return Err(Error::Denied("current independent worker admission"));
        }
        if self.price_msat == 0 && self.fee_limit_msat != 0 {
            return Err(Error::Invalid("free worker fees"));
        }
        Ok(())
    }
    /// One obligation identity across invoice replacement and transport recovery.
    pub fn obligation(&self) -> String {
        crate::digest(&format!(
            "worker-earned-v1:{}:{}:{}:{}",
            self.buyer, self.market, self.order, self.provider
        ))
    }
}
/// Exact retained record linkage. Delivery, checking, acceptance, and settlement stay separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Earned {
    pub obligation: String,
    pub order: String,
    pub labor_terms: String,
    pub delivery: String,
    pub verification: String,
    pub acceptance: String,
    pub buyer: String,
    pub provider: String,
    pub accepted_msat: i64,
}
impl Earned {
    pub fn validate(&self, terms: &WorkerTerms) -> Result<()> {
        for pin in [&self.delivery, &self.verification, &self.acceptance] {
            exact(pin)?;
        }
        if self.obligation != terms.obligation()
            || self.order != terms.order
            || self.labor_terms != terms.labor_terms
            || self.buyer != terms.buyer
            || self.provider != terms.provider
            || self.accepted_msat != terms.price_msat
        {
            return Err(Error::Invalid("earned worker linkage"));
        }
        Ok(())
    }
}

/// A receipt supplied by the admitted central receive adapter, never by worker output.
#[derive(Clone, Debug)]
pub struct FundingReceipt {
    pub payment_hash: String,
    pub received_msat: i64,
    pub platform_fee_msat: i64,
    pub received_at: i64,
}
impl crate::Ledger {
    /// Accrue an accepted worker fee in the existing central ledger, with exact retries.
    /// The caller must verify the signed LAB closure and central funding receipt first.
    pub fn record_worker_earned(
        &mut self,
        terms: &WorkerTerms,
        earned: &Earned,
        receipt: &FundingReceipt,
    ) -> Result<crate::Recorded> {
        earned.validate(terms)?;
        exact(&receipt.payment_hash)?;
        if terms.price_msat <= 0
            || receipt.platform_fee_msat < 0
            || receipt.received_msat
                != terms
                    .price_msat
                    .checked_add(receipt.platform_fee_msat)
                    .ok_or(Error::Invalid("worker funding overflow"))?
            || receipt.received_at > terms.deadlines.payment
        {
            return Err(Error::Invalid("worker funding amount or deadline"));
        }
        let bytes = nostr::contracts::jcs(&serde_json::json!({"terms":terms,"earned":earned,
            "platform_fee_msat":receipt.platform_fee_msat}))
        .map_err(|_| Error::Invalid("worker canonical terms"))?;
        let resource = format!(
            "worker:{}:{}",
            terms.obligation(),
            nostr::contracts::digest_bytes(&bytes)
        );
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        use rusqlite::OptionalExtension;
        let prior: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT payment_hash,resource,received_msat FROM settlement WHERE resource LIKE ?",
                [format!("worker:{}:%", terms.obligation())],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((key, prior_resource, amount)) = prior {
            if key != receipt.payment_hash
                || prior_resource != resource
                || amount != receipt.received_msat
            {
                return Err(Error::Conflict(
                    "worker obligation already funded with other terms",
                ));
            }
        }
        if let Some(existing) = crate::read_record(&tx, &receipt.payment_hash)?
            && (existing.resource != resource || existing.received_msat != receipt.received_msat)
        {
            return Err(Error::Conflict("worker payment hash already allocated"));
        }
        let result = crate::record_settlement_in(
            &tx,
            crate::SettlementInput {
                key: receipt.payment_hash.clone(),
                resource,
                plugin_id: None,
                release_id: None,
                price_msat: receipt.received_msat,
                received_msat: receipt.received_msat,
                rail: crate::Rail::Lightning,
                payer_alias: Some(terms.buyer.clone()),
                settled_at: receipt.received_at,
                split: crate::Split::Earned {
                    beneficiary: terms.provider.clone(),
                    amount_msat: terms.price_msat,
                    kind: crate::EarnedKind::Worker,
                },
            },
        )?;
        tx.commit()?;
        Ok(result)
    }
}
