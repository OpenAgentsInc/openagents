//! Later training-work qualification, separate from inference and computer rental.
use super::exact;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rights {
    pub provenance: String,
    pub license: String,
    pub train: bool,
    pub evaluate: bool,
    pub disclose: bool,
    pub redistribute: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub order: String,
    pub corpus: String,
    pub dataset: String,
    pub baseline_checkpoint: String,
    pub recipe: String,
    pub seed: u64,
    pub worker_class: String,
    pub deliverable_contract: String,
    pub checker: String,
    pub evaluation_partition: String,
    pub training_groups: Vec<String>,
    pub rights: Rights,
    pub requires_disclosure: bool,
    pub requires_redistribution: bool,
    pub max_all_in_msat: i64,
    pub compute_obligation: String,
    pub improvement_obligation: String,
    pub data_license_obligation: String,
}
/// Caller-owned evidence about rights and protected evaluation, never worker assertions.
#[derive(Clone, Debug)]
pub struct VerifiedEvaluation {
    pub receipt: String,
    pub artifact: String,
    pub terms_fingerprint: String,
    pub evaluation_partition: String,
    pub accepted: bool,
    pub improvement: i64,
}
#[derive(Clone, Debug)]
pub struct Trust {
    pub rights_verified: bool,
    pub checker: String,
    pub evaluation_partition: String,
    pub protected_groups: Vec<String>,
    pub worker_cannot_edit_checker: bool,
    pub verified_evaluation: Option<VerifiedEvaluation>,
    pub verified_costs_msat: Option<[i64; 4]>,
}
impl Terms {
    pub fn validate(&self, trust: &Trust) -> Result<()> {
        for pin in [
            &self.order,
            &self.corpus,
            &self.dataset,
            &self.baseline_checkpoint,
            &self.recipe,
            &self.worker_class,
            &self.deliverable_contract,
            &self.checker,
            &self.evaluation_partition,
            &self.rights.provenance,
            &self.rights.license,
            &self.compute_obligation,
            &self.improvement_obligation,
            &self.data_license_obligation,
        ] {
            exact(pin)?;
        }
        if !self.rights.train
            || !self.rights.evaluate
            || (self.requires_disclosure && !self.rights.disclose)
            || (self.requires_redistribution && !self.rights.redistribute)
            || self.max_all_in_msat <= 0
            || self.training_groups.is_empty()
            || self.training_groups.len() > 256
            || self.compute_obligation == self.improvement_obligation
            || self.compute_obligation == self.data_license_obligation
            || self.improvement_obligation == self.data_license_obligation
        {
            return Err(Error::Invalid("training rights, obligations, or bounds"));
        }
        let mut groups = std::collections::BTreeSet::new();
        for group in &self.training_groups {
            exact(group)?;
            if !groups.insert(group) {
                return Err(Error::Invalid("duplicate training group"));
            }
        }
        for group in &trust.protected_groups {
            exact(group)?;
        }
        if !trust.rights_verified
            || !trust.worker_cannot_edit_checker
            || trust.checker != self.checker
            || trust.evaluation_partition != self.evaluation_partition
            || trust.protected_groups.is_empty()
            || self
                .training_groups
                .iter()
                .any(|group| trust.protected_groups.contains(group))
        {
            return Err(Error::Denied("training provenance or protected evaluation"));
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<String> {
        let bytes = nostr::contracts::jcs(
            &serde_json::to_value(self).map_err(|_| Error::Invalid("training encoding"))?,
        )
        .map_err(|_| Error::Invalid("canonical training terms"))?;
        Ok(nostr::contracts::digest_bytes(&bytes)
            .trim_start_matches("sha256:")
            .to_owned())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub terms_fingerprint: String,
    pub artifact: String,
    pub protected_checker_receipt: String,
    pub evaluation_partition: String,
    pub accepted: bool,
    /// A score improvement measured by the pinned checker, not a worker-selected metric.
    pub improvement: i64,
    pub training_cost_msat: Option<i64>,
    pub check_cost_msat: Option<i64>,
    pub search_cost_msat: Option<i64>,
    pub failed_attempt_cost_msat: Option<i64>,
}
impl Evidence {
    pub fn eligible_improvement(&self, terms: &Terms, trust: &Trust) -> Result<i64> {
        terms.validate(trust)?;
        for pin in [
            &self.terms_fingerprint,
            &self.artifact,
            &self.protected_checker_receipt,
            &self.evaluation_partition,
        ] {
            exact(pin)?;
        }
        if self.terms_fingerprint != terms.fingerprint()?
            || self.evaluation_partition != terms.evaluation_partition
            || !self.accepted
            || self.improvement <= 0
        {
            return Err(Error::Denied("accepted pinned training improvement"));
        }
        let verified = trust
            .verified_evaluation
            .as_ref()
            .ok_or(Error::Denied("training checker receipt unavailable"))?;
        if verified.receipt != self.protected_checker_receipt
            || verified.artifact != self.artifact
            || verified.terms_fingerprint != self.terms_fingerprint
            || verified.evaluation_partition != self.evaluation_partition
            || verified.accepted != self.accepted
            || verified.improvement != self.improvement
        {
            return Err(Error::Denied("training checker receipt binding"));
        }
        let costs = [
            self.training_cost_msat,
            self.check_cost_msat,
            self.search_cost_msat,
            self.failed_attempt_cost_msat,
        ];
        if trust.verified_costs_msat.map(|a| a.map(Some)) != Some(costs) {
            return Err(Error::Denied("training independently verified costs"));
        }
        let mut total = 0i64;
        for cost in [
            self.training_cost_msat,
            self.check_cost_msat,
            self.search_cost_msat,
            self.failed_attempt_cost_msat,
        ] {
            let cost = cost
                .filter(|c| *c >= 0)
                .ok_or(Error::Invalid("training all-in cost unknown"))?;
            total = total
                .checked_add(cost)
                .ok_or(Error::Invalid("training cost overflow"))?;
        }
        if total > terms.max_all_in_msat {
            return Err(Error::Denied("training cost budget"));
        }
        Ok(total)
    }
}
