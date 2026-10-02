//! An offer: an immutable proposal with an expiry and a digest of its
//! effects, recipients, price terms, and source (plan section 4).
//! Confirming it cannot approve a changed proposal: any material change is
//! a new offer.

use serde::{Deserialize, Serialize};

use crate::digest::{Digest, digest_of};
use crate::snapshot::{Effects, Fee, Recipient, SourcePin};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    /// [`crate::OFFER_SCHEMA`].
    pub schema: String,
    /// The public confirmation id (`cf_...`).
    pub id: String,
    pub action: Action,
    /// What the control says; not part of the terms.
    pub label: String,
    /// Unix seconds.
    pub created_at: u64,
    pub expires_at: u64,
    pub terms: Terms,
    /// [`Offer::compute_digest`] of the fields above except `id` and
    /// `label`.
    pub digest: Digest,
}

/// What the person agrees to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    /// The route result's digest.
    pub route: Digest,
    /// The proposed admission snapshot's digest.
    pub snapshot: Digest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    pub effects: Effects,
    pub recipients: Vec<Recipient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourcePin>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub max_sats: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fees: Vec<Fee>,
}

/// The API's offer `action` words (section 4.2 of the API design).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "run.start")]
    RunStart,
    #[serde(rename = "command.run")]
    CommandRun,
    #[serde(rename = "plugin.run")]
    PluginRun,
    #[serde(rename = "plugin.create")]
    PluginCreate,
    #[serde(rename = "plugin.install")]
    PluginInstall,
    #[serde(rename = "rule.enable")]
    RuleEnable,
    #[serde(rename = "wallet.pay")]
    WalletPay,
}

/// Why a confirmation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Expired,
    /// The confirmation named another digest.
    Mismatch,
    /// The terms as they stand now differ from the offer's.
    Changed,
}

impl Offer {
    /// A new offer, its digest computed.
    #[must_use]
    pub fn new(
        id: String,
        action: Action,
        label: String,
        created_at: u64,
        expires_at: u64,
        terms: Terms,
    ) -> Self {
        let digest = Self::compute_digest(action, created_at, expires_at, &terms);
        Self {
            schema: crate::OFFER_SCHEMA.into(),
            id,
            action,
            label,
            created_at,
            expires_at,
            terms,
            digest,
        }
    }

    /// The digest over the schema, action, times, and terms.
    #[must_use]
    pub fn compute_digest(
        action: Action,
        created_at: u64,
        expires_at: u64,
        terms: &Terms,
    ) -> Digest {
        digest_of(&(crate::OFFER_SCHEMA, action, created_at, expires_at, terms))
    }

    /// Whether the stored digest matches the stored fields.
    #[must_use]
    pub fn intact(&self) -> bool {
        self.schema == crate::OFFER_SCHEMA
            && self.digest
                == Self::compute_digest(self.action, self.created_at, self.expires_at, &self.terms)
    }

    /// Admits a confirmation of `confirmed` at `now` against the terms as
    /// they stand now.
    ///
    /// # Errors
    ///
    /// Expired at or after `expires_at`, a confirmation of another digest,
    /// or terms that changed (a tampered offer counts as changed).
    pub fn confirm(&self, confirmed: &Digest, now: u64, current: &Terms) -> Result<(), Refusal> {
        if now >= self.expires_at {
            return Err(Refusal::Expired);
        }
        if confirmed != &self.digest {
            return Err(Refusal::Mismatch);
        }
        if !self.intact() || current != &self.terms {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
}
