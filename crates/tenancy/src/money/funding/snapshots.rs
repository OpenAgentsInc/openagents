//! Atomic provider reconciliation under the original accepted funding terms.

use super::*;

/// An original payment's verified current state. Only a trusted funding adapter
/// may produce this value after checking native processor evidence. Browser
/// success, webhook labels, and current customer labels are not that evidence.
///
/// Reversed units are disjoint portions of the original convertible source
/// backing. Refund reduction requires new, once-only native recovery proofs;
/// disputed backing may return after verified recovery. Neither portion includes
/// fees or authorizes another payment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub quote: String,
    pub funding: Funding,
    pub paid_at: u64,
    pub finality: Finality,
    pub evidence: String,
    /// Starts at one, then advances exactly once per durable reconciliation.
    pub revision: u64,
    pub refunded_source_units: u64,
    /// Original source units returned after a refund failed, keyed by unique
    /// native return evidence. Existing proofs are immutable and cannot restore
    /// another purchase. Empty preserves earlier snapshot journal digests.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refund_recovery_proofs: BTreeMap<String, u64>,
    pub disputed_source_units: u64,
    /// Restrict uncommitted credit while native provider lookup is incomplete.
    /// This records uncertainty, not a refund, a loss, or released liability.
    pub reconciliation_pending: bool,
}

impl Book {
    /// Returns the newly issued original credit, not restored backing. The
    /// caller applies this and its reversed position in one native journal row.
    pub fn reconcile(&mut self, snapshot: &Snapshot, at: u64) -> Result<u64, String> {
        identity(&snapshot.evidence)?;
        if snapshot.refund_recovery_proofs.len() > 128 {
            return Err("refund recovery evidence exceeds its retained bound".into());
        }
        for (proof, amount) in &snapshot.refund_recovery_proofs {
            identity(proof)?;
            if *amount == 0 || *amount > snapshot.funding.gross_units {
                return Err("refund recovery exceeds its original source units".into());
            }
        }
        let id = &snapshot.funding.id;
        if let Some(prior) = self.snapshots.get(id) {
            if snapshot.quote != prior.quote
                || snapshot.funding != prior.funding
                || snapshot.paid_at != prior.paid_at
                || snapshot.revision != prior.revision.checked_add(1).ok_or("snapshot overflow")?
                || snapshot.finality < prior.finality
                || !prior.refund_recovery_proofs.iter().all(|(proof, amount)| {
                    snapshot.refund_recovery_proofs.get(proof) == Some(amount)
                })
            {
                return Err(
                    "provider reconciliation differs from its original payment or revision".into(),
                );
            }
            let restored = snapshot
                .refund_recovery_proofs
                .iter()
                .filter(|(proof, _)| !prior.refund_recovery_proofs.contains_key(*proof))
                .try_fold(0_u64, |sum, (_, amount)| sum.checked_add(*amount))
                .ok_or("refund recovery overflow")?;
            if prior
                .refunded_source_units
                .saturating_sub(snapshot.refunded_source_units)
                > restored
            {
                return Err("refund reduction requires new original native return evidence".into());
            }
        } else {
            if snapshot.revision != 1 || self.funding.contains_key(id) {
                return Err("provider reconciliation requires an unused original quote".into());
            }
            self.begin_quoted(&snapshot.quote, &snapshot.funding, snapshot.paid_at, at)?;
        }
        let record = self.funding.get(id).ok_or("funding is missing")?;
        let terms = &self.policies[&record.funding.policy].purchases;
        let removed = snapshot
            .refunded_source_units
            .checked_add(snapshot.disputed_source_units)
            .ok_or("provider reversal overflow")?;
        if removed > record.quote.convertible_units
            || ((snapshot.refunded_source_units > 0 || !snapshot.refund_recovery_proofs.is_empty())
                && !terms.refunds_allowed)
            || (snapshot.disputed_source_units > 0 && !terms.disputes_allowed)
        {
            return Err("provider reversal exceeds its original backing or purchase terms".into());
        }
        let issued = self.confirm_admitted(id, snapshot.finality, &snapshot.evidence)?;
        self.set_reversed_source(id, removed)?;
        self.snapshots.insert(id.clone(), snapshot.clone());
        Ok(issued)
    }
}
