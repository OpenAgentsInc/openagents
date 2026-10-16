//! The accepted labor terms this host admits, the role relationships, and
//! the rights policy ([NIP-LAB](../../../nips/openagents/NIP-LAB.md#labor-terms)).
use crate::{Result, digest_of};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub pubkey: String,
    pub operator: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deliverable {
    pub id: String,
    pub max_bytes: u64,
}

/// The closed rights policy, `openagents.labor-rights.v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rights {
    pub v: String,
    /// Digest of the exact license terms.
    pub license: String,
    pub input_use: String,
    pub output_use: String,
    pub publication: String,
    pub training: String,
    pub evaluation_reuse: String,
    pub redistribution: String,
    pub recipients: Vec<String>,
    pub retention: String,
}

/// A use of order material that rights govern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Read the inputs to perform and review the order.
    PerformAndReview,
    /// Use delivered outputs beyond review, under the license.
    UseOutput,
    Publish,
    Train,
    EvaluationReuse,
    Redistribute,
}

/// A later grant for a `separate-grant` use, signed off by buyer and provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub order: String,
    pub purpose: String,
    pub recipient: String,
    pub granted_by: Vec<String>,
}

pub const RIGHTS_V: &str = "openagents.labor-rights.v1";

impl Rights {
    /// Whether `reader` may make `use_` of the order's material, given any
    /// later grants. `separate-grant` authorizes nothing without a grant
    /// from both buyer and provider; payment conveys no extra right.
    #[must_use]
    pub fn permits(&self, terms: &Terms, use_: Use, reader: &str, grants: &[Grant]) -> bool {
        if !self.recipients.iter().any(|r| r == reader) {
            return false;
        }
        let (value, purpose) = match use_ {
            Use::PerformAndReview => return true,
            Use::UseOutput => return self.output_use == "use-under-license",
            Use::Publish => (&self.publication, "publication"),
            Use::Train => (&self.training, "training"),
            Use::EvaluationReuse => (&self.evaluation_reuse, "evaluation_reuse"),
            Use::Redistribute => (&self.redistribution, "redistribution"),
        };
        value == "separate-grant"
            && grants.iter().any(|g| {
                g.order == terms.order
                    && g.purpose == purpose
                    && g.recipient == reader
                    && g.granted_by.contains(&terms.buyer)
                    && g.granted_by.contains(&terms.provider)
            })
    }

    fn validate(&self, roles: &[&String]) -> Result<()> {
        let grant = |v: &str| matches!(v, "deny" | "separate-grant");
        if self.v != RIGHTS_V
            || self.input_use != "perform-and-review-order"
            || !matches!(
                self.output_use.as_str(),
                "review-only" | "use-under-license"
            )
            || !grant(&self.publication)
            || !grant(&self.training)
            || !grant(&self.evaluation_reuse)
            || !grant(&self.redistribution)
            || self.retention != "through-market-retain-until"
        {
            return Err("the rights policy is not the closed labor-rights-v1 policy".into());
        }
        crate::hex64(&self.license, "license")?;
        if roles.iter().any(|r| !self.recipients.contains(r)) {
            return Err("the rights recipients omit a participant who needs the records".into());
        }
        Ok(())
    }
}

/// The accepted labor terms, as far as delivery roles need them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub v: String,
    pub market: String,
    pub order: String,
    pub buyer: String,
    pub provider: String,
    pub worker: String,
    pub reviewer: String,
    pub resolver: String,
    pub resolver_policy: String,
    pub cancellation: String,
    pub partial_delivery: String,
    pub buyer_unavailable: String,
    pub role_relationships: Vec<Relationship>,
    pub deliverables: Vec<Deliverable>,
    /// The acceptance policy's criteria; rule `all-pass-v1`.
    pub criteria: Vec<String>,
    pub max_reworks: u8,
    pub delivery_due_at: u64,
    pub rework_due_at: Option<u64>,
    pub review_due_at: u64,
    pub dispute_due_at: u64,
    pub resolution_due_at: u64,
    pub payment_due_at: u64,
    pub retain_until: u64,
    /// The fixed MKT price in BTC millisatoshis; zero for free work.
    pub price_msat: u64,
    pub rights: Rights,
}

pub const TERMS_V: &str = "openagents.labor-terms.v1";

impl Terms {
    /// The terms' identity: the digest of their canonical bytes.
    ///
    /// # Errors
    ///
    /// When the terms do not encode.
    pub fn digest(&self) -> Result<String> {
        digest_of(self)
    }

    fn operator(&self, key: &str) -> Option<&str> {
        self.role_relationships
            .iter()
            .find(|r| r.pubkey == key)
            .map(|r| r.operator.as_str())
    }

    /// Admit the terms: distinct roles, declared independent operators,
    /// deadline order, bounded rework, closed literals and rights.
    ///
    /// # Errors
    ///
    /// Names the first rule the terms break.
    pub fn admit(&self, now: u64) -> Result<()> {
        if self.v != TERMS_V
            || self.resolver_policy != "labor-evidence-v1"
            || self.cancellation != "evaluate-delivered-work-v1"
            || self.partial_delivery != "no-partial-payment-v1"
            || self.buyer_unavailable != "resolver-required-v1"
        {
            return Err("unsupported labor terms version or policy".into());
        }
        let roles = [
            &self.buyer,
            &self.provider,
            &self.worker,
            &self.reviewer,
            &self.resolver,
        ];
        for key in roles {
            crate::hex64(key, "role key")?;
        }
        crate::hex64(&self.order, "order")?;
        crate::hex64(&self.market, "market")?;
        if self.reviewer == self.provider || self.reviewer == self.worker {
            return Err("the reviewer must differ from the provider and worker".into());
        }
        if [&self.buyer, &self.provider, &self.worker, &self.reviewer].contains(&&self.resolver) {
            return Err("the resolver must differ from every other role".into());
        }
        if self.buyer == self.provider || self.buyer == self.worker {
            return Err("the buyer must differ from the provider and worker".into());
        }
        let keys: BTreeSet<&String> = self.role_relationships.iter().map(|r| &r.pubkey).collect();
        if keys.len() != self.role_relationships.len()
            || roles.iter().any(|k| !keys.contains(k))
            || keys.len() != roles.iter().collect::<BTreeSet<_>>().len()
        {
            return Err("role relationships must name every role key exactly once".into());
        }
        if self
            .role_relationships
            .iter()
            .any(|r| r.operator.is_empty() || r.operator.len() > 128)
        {
            return Err("an operator ID is empty or too long".into());
        }
        let op = |k: &str| self.operator(k).unwrap_or_default();
        // Independence: the buyer's side, the provider's side, and the
        // resolver are separately operated; a reviewer that is not the
        // buyer is operated apart from the provider and worker.
        let provider_side: BTreeSet<&str> = [op(&self.provider), op(&self.worker)].into();
        if provider_side.contains(op(&self.buyer)) {
            return Err("the buyer and the provider or worker share an operator".into());
        }
        if self.reviewer != self.buyer && provider_side.contains(op(&self.reviewer)) {
            return Err("the reviewer shares an operator with the provider or worker".into());
        }
        if [&self.buyer, &self.provider, &self.worker, &self.reviewer]
            .iter()
            .any(|k| op(k) == op(&self.resolver))
        {
            return Err("the resolver shares an operator with another role".into());
        }
        if self.max_reworks > 3 || (self.max_reworks == 0) != self.rework_due_at.is_none() {
            return Err("rework is bounded to three, with a deadline exactly when enabled".into());
        }
        let ordered = now < self.delivery_due_at
            && self.delivery_due_at <= self.review_due_at
            && self.review_due_at <= self.dispute_due_at
            && self.dispute_due_at < self.resolution_due_at
            && self.resolution_due_at < self.payment_due_at
            && self.payment_due_at < self.retain_until
            && self
                .rework_due_at
                .is_none_or(|r| self.delivery_due_at <= r && r < self.review_due_at);
        if !ordered {
            return Err("the labor deadlines are out of order".into());
        }
        let ids: BTreeSet<&String> = self.deliverables.iter().map(|d| &d.id).collect();
        if self.deliverables.is_empty()
            || ids.len() != self.deliverables.len()
            || self.deliverables.iter().any(|d| d.max_bytes == 0)
        {
            return Err("deliverables must be nonempty, unique, and bounded".into());
        }
        let criteria: BTreeSet<&String> = self.criteria.iter().collect();
        if self.criteria.is_empty() || criteria.len() != self.criteria.len() {
            return Err("acceptance criteria must be nonempty and unique".into());
        }
        self.rights.validate(&roles)?;
        Ok(())
    }

    /// The deadline for a submission number: the original delivery
    /// deadline, or the rework deadline for a replacement.
    #[must_use]
    pub fn due(&self, number: u32) -> u64 {
        if number == 0 {
            self.delivery_due_at
        } else {
            self.rework_due_at.unwrap_or(self.delivery_due_at)
        }
    }

    /// The pay-ledger worker terms for the funded obligation.
    ///
    /// # Errors
    ///
    /// When a pin is not a 64-hex digest.
    pub fn worker_terms(
        &self,
        pins: &BTreeMap<&str, String>,
    ) -> Result<pay_ledger::markets::worker::WorkerTerms> {
        let pin = |k: &str| {
            pins.get(k)
                .cloned()
                .ok_or_else(|| format!("missing pin {k}"))
        };
        let unprefixed = |d: String| d.trim_start_matches("sha256:").to_string();
        let as_i64 =
            |v: u64| i64::try_from(v).map_err(|_| "a value exceeds the ledger range".to_string());
        Ok(pay_ledger::markets::worker::WorkerTerms {
            profile: pay_ledger::markets::worker::PROFILE.into(),
            buyer: self.buyer.clone(),
            provider: self.provider.clone(),
            buyer_operator: self.operator(&self.buyer).unwrap_or_default().into(),
            provider_operator: self.operator(&self.provider).unwrap_or_default().into(),
            market: self.market.clone(),
            order: self.order.clone(),
            labor_terms: unprefixed(self.digest()?),
            source: pin("source")?,
            checker: pin("checker")?,
            disclosure: pin("disclosure")?,
            execution_requirements: pin("execution_requirements")?,
            cancellation_policy: unprefixed(digest_of(&self.cancellation)?),
            delivery_rights: unprefixed(digest_of(&self.rights)?),
            price_msat: as_i64(self.price_msat)?,
            fee_limit_msat: 0,
            capacity_units: 1,
            max_rework: u16::from(self.max_reworks),
            deadlines: pay_ledger::markets::Deadlines {
                delivery: as_i64(self.delivery_due_at)?,
                review: as_i64(self.review_due_at)?,
                dispute: as_i64(self.dispute_due_at)?,
                resolution: as_i64(self.resolution_due_at)?,
                payment: as_i64(self.payment_due_at)?,
                retain_until: as_i64(self.retain_until)?,
            },
        })
    }
}
