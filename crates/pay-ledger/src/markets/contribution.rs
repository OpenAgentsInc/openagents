//! One later useful-contribution class: precommitted, independently checked optimization.
use super::{exact, worker::FundingReceipt};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    VerifiedOptimization,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub class: Class,
    pub obligation: String,
    pub source: String,
    pub source_group: String,
    pub evaluation_group: String,
    pub license: String,
    pub attribution: String,
    pub beneficiary: String,
    pub protected_evaluator: String,
    pub evaluation_policy: String,
    pub acceptance_authority: String,
    pub funding_authority: String,
    pub artifact_contract: String,
    pub reward_msat: i64,
    pub committed_at: i64,
    pub expires_at: i64,
}
impl Terms {
    pub fn fingerprint(&self) -> Result<String> {
        let bytes = nostr::contracts::jcs(
            &serde_json::to_value(self).map_err(|_| Error::Invalid("contribution encoding"))?,
        )
        .map_err(|_| Error::Invalid("canonical contribution"))?;
        Ok(nostr::contracts::digest_bytes(&bytes)
            .trim_start_matches("sha256:")
            .to_owned())
    }
    pub fn validate(&self, now: i64) -> Result<()> {
        for pin in [
            &self.obligation,
            &self.source,
            &self.source_group,
            &self.evaluation_group,
            &self.license,
            &self.attribution,
            &self.beneficiary,
            &self.protected_evaluator,
            &self.evaluation_policy,
            &self.acceptance_authority,
            &self.funding_authority,
            &self.artifact_contract,
        ] {
            exact(pin)?;
        }
        if self.source_group == self.evaluation_group
            || self.reward_msat <= 0
            || self.expires_at <= now
            || self.committed_at >= now
            || self.beneficiary == self.protected_evaluator
            || self.beneficiary == self.acceptance_authority
        {
            return Err(Error::Denied(
                "contribution independent acceptance or bounds",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub terms_fingerprint: String,
    pub artifact: String,
    pub evaluation_receipt: String,
    pub acceptance_receipt: String,
    pub improvement: i64,
    pub evaluated_at: i64,
    pub accepted_at: i64,
}
/// Exact decoded protected evaluator and acceptance records, verified by the caller.
#[derive(Clone, Debug)]
pub struct Trust {
    pub frozen_terms_fingerprint: String,
    pub rights_verified: bool,
    pub source_groups_verified: bool,
    pub independently_controlled_evaluator: bool,
    pub beneficiary_destination_verified: bool,
    pub verified_acceptance: Acceptance,
    pub current_funding_authority: String,
}
impl crate::Ledger {
    /// Accrue a fully funded accepted contribution in the central author-fee payout path.
    /// The host must independently verify the evaluator, acceptance, and funding adapter.
    pub fn record_contribution_earned(
        &mut self,
        terms: &Terms,
        accepted: &Acceptance,
        trust: &Trust,
        receipt: &FundingReceipt,
    ) -> Result<crate::Recorded> {
        terms.validate(receipt.received_at)?;
        exact(&receipt.payment_hash)?;
        for pin in [
            &accepted.terms_fingerprint,
            &accepted.artifact,
            &accepted.evaluation_receipt,
            &accepted.acceptance_receipt,
        ] {
            exact(pin)?;
        }
        let fingerprint = terms.fingerprint()?;
        if !trust.source_groups_verified
            || !trust.rights_verified
            || !trust.independently_controlled_evaluator
            || !trust.beneficiary_destination_verified
            || trust.current_funding_authority != terms.funding_authority
            || trust.frozen_terms_fingerprint != fingerprint
            || accepted.terms_fingerprint != fingerprint
            || accepted != &trust.verified_acceptance
            || accepted.improvement <= 0
            || terms.committed_at >= accepted.evaluated_at
            || accepted.evaluated_at > accepted.accepted_at
            || accepted.accepted_at > receipt.received_at
        {
            return Err(Error::Denied("precommitted verified contribution"));
        }
        if receipt.platform_fee_msat < 0
            || terms.reward_msat.checked_add(receipt.platform_fee_msat)
                != Some(receipt.received_msat)
        {
            return Err(Error::Invalid("fully funded contribution reward"));
        }
        let bytes=nostr::contracts::jcs(&serde_json::json!({"terms":terms,"acceptance":accepted,"platform_fee_msat":receipt.platform_fee_msat}))
            .map_err(|_|Error::Invalid("contribution receipt encoding"))?;
        let resource = format!(
            "contribution:{}:{}",
            terms.obligation,
            nostr::contracts::digest_bytes(&bytes)
        );
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        use rusqlite::OptionalExtension;
        let prior: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT payment_hash,resource,received_msat FROM settlement WHERE resource LIKE ?",
                [format!("contribution:{}:%", terms.obligation)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((key, prior_resource, amount)) = prior
            && (key != receipt.payment_hash
                || prior_resource != resource
                || amount != receipt.received_msat)
        {
            return Err(Error::Conflict(
                "contribution obligation already funded with other terms",
            ));
        }
        if let Some(existing) = crate::read_record(&tx, &receipt.payment_hash)?
            && (existing.resource != resource || existing.received_msat != receipt.received_msat)
        {
            return Err(Error::Conflict(
                "contribution payment hash already allocated",
            ));
        }
        let recorded = crate::record_settlement_in(
            &tx,
            crate::SettlementInput {
                key: receipt.payment_hash.clone(),
                resource,
                plugin_id: None,
                release_id: None,
                price_msat: receipt.received_msat,
                received_msat: receipt.received_msat,
                rail: crate::Rail::Lightning,
                payer_alias: Some(terms.funding_authority.clone()),
                settled_at: receipt.received_at,
                split: crate::Split::Earned {
                    beneficiary: terms.beneficiary.clone(),
                    amount_msat: terms.reward_msat,
                    kind: crate::EarnedKind::Contribution,
                },
            },
        )?;
        tx.commit()?;
        Ok(recorded)
    }
}
