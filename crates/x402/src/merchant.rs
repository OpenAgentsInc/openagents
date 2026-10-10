//! Merchant-hosted receivers (#11149; deployment profile
//! `docs/payments/proposal/2026-10-09-x402-merchant-receiver-profile.md`).
//!
//! A third-party merchant runs its own receiver: its own Lightning node is
//! `payTo`, its own replay store consumes each payment hash once, and its
//! sales settle at its node with nobody holding the sats in between. That
//! is the existing [`crate::front::Front`] pointed at the merchant's node;
//! this module adds the few pieces the profile names:
//!
//! - [`MerchantReceiver`] pins the merchant's node key as `payTo` and
//!   refuses any invoice its wallet issues under another key (NIP-X402:
//!   "Hosts MUST pin the receiver-key association").
//! - [`MerchantSink`] records the merchant's own sale. There is no split
//!   ledger and no payout worker on this path: the merchant was paid
//!   directly at settlement.
//! - [`AttestationVerifier`], the [`BuyerAttestation`] check a merchant can
//!   require before it issues an invoice: the caller is an agent admitted
//!   by an authority the merchant trusts (NIP-SOV), holding an operational
//!   `spend` grant from that authority (NIP-CAP) whose per-payment ceiling
//!   covers the price. Its checks run in the fixed order of
//!   `docs/payments/proposal/fixtures/buyer-attestation-v1.json` and the
//!   first failure wins.
//!
//! The platform fee as a second payment requirement is not here. It waits
//! for the NIP-X402 review, and its failure order is written down for that
//! review in the deployment profile, section 7.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::front::{NdjsonSettlements, Settlement, SettlementSink, Usage};
use crate::server::Receiver;

/// The NIP-SOV profile schema an attestation presents.
pub const PROFILE_SCHEMA: &str = "openagents.sovereign-profile.v1";
/// The NIP-CAP grant schema an attestation presents.
pub const GRANT_SCHEMA: &str = "openagents.grant.v1";

/// Millisatoshis as NIP-X402 writes them: a canonical nonnegative decimal
/// string (`"1000"` is one sat; no sign, no leading zero, no fraction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Msat(pub u64);

impl Msat {
    /// Parse a canonical decimal string.
    ///
    /// # Errors
    ///
    /// Anything but a canonical nonnegative decimal that fits in `u64`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let canonical = !text.is_empty()
            && text.bytes().all(|b| b.is_ascii_digit())
            && (text == "0" || !text.starts_with('0'));
        if !canonical {
            return Err(format!("{text:?} is not a canonical msat amount"));
        }
        text.parse::<u64>()
            .map(Self)
            .map_err(|_| format!("{text:?} is too large an msat amount"))
    }
}

impl Serialize for Msat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Msat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// A content-addressed reference to a POL/CAP/SOV artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub schema: String,
    pub digest: String,
}

/// NIP-SOV `sovereign-profile.v1`: the agent key and the authority that
/// admitted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SovereignProfile {
    pub v: String,
    /// The agent's Nostr x-only key, 64 lowercase hex.
    pub agent: String,
    /// The admitting authority (the owner), 64 lowercase hex.
    pub authority: String,
    pub policy: ArtifactRef,
    pub custody: ArtifactRef,
    pub treasury: ArtifactRef,
}

/// One NIP-CAP effect. Each is granted separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Reads,
    Writes,
    Network,
    Process,
    Delegates,
    Spend,
}

/// What a grant is for: an evaluation (arena) grant never buys real goods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Evaluation,
    Operational,
}

/// A grant's spending bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantBounds {
    /// The most one payment may be: the ceiling the price is checked
    /// against.
    pub max_per_tx_msat: Msat,
    /// The aggregate allowance. Checked against the treasury's
    /// reconciliation authority, not per call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_msat: Option<Msat>,
}

/// NIP-CAP `grant.v1` with (it must be) the `spend` effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendGrant {
    pub v: String,
    /// Who granted it: the profile's authority.
    pub issuer: String,
    /// Who it was granted to: the profile's agent.
    pub principal: String,
    pub binding: ArtifactRef,
    pub effects: Vec<Effect>,
    pub bounds: GrantBounds,
    pub scope: String,
    pub purpose: Purpose,
    /// Unix seconds; the grant is good while `now < expires_at`.
    pub expires_at: u64,
}

fn is_key(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl SovereignProfile {
    /// Whether the profile is the right schema with well-formed keys.
    ///
    /// # Errors
    ///
    /// A sentence naming what is wrong.
    pub fn check(&self) -> Result<(), String> {
        if self.v != PROFILE_SCHEMA {
            return Err(format!("the profile is {:?}, not {PROFILE_SCHEMA}", self.v));
        }
        if !is_key(&self.agent) || !is_key(&self.authority) {
            return Err("the profile's agent and authority must be 64 lowercase hex".into());
        }
        Ok(())
    }
}

impl SpendGrant {
    /// Whether the grant is the right schema with well-formed keys.
    ///
    /// # Errors
    ///
    /// A sentence naming what is wrong.
    pub fn check(&self) -> Result<(), String> {
        if self.v != GRANT_SCHEMA {
            return Err(format!("the grant is {:?}, not {GRANT_SCHEMA}", self.v));
        }
        if !is_key(&self.issuer) || !is_key(&self.principal) {
            return Err("the grant's issuer and principal must be 64 lowercase hex".into());
        }
        Ok(())
    }
}

/// What a caller presented, once its transport has parsed it.
///
/// `signature_valid` is the transport's check of the attestation's
/// signature over the bound request (a NIP-98-style event over the method,
/// URL, and body). A profile or grant that does not parse, or fails its
/// [`check`](SovereignProfile::check), is not presented at all: the
/// transport passes `None`, which refuses as [`AttestationError::Missing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presented {
    pub profile: SovereignProfile,
    pub grant: SpendGrant,
    pub signature_valid: bool,
}

impl Presented {
    /// The parsed attestation, or `None` when either part does not parse
    /// or check.
    #[must_use]
    pub fn parse(profile: &str, grant: &str, signature_valid: bool) -> Option<Self> {
        let profile: SovereignProfile = serde_json::from_str(profile).ok()?;
        let grant: SpendGrant = serde_json::from_str(grant).ok()?;
        profile.check().ok()?;
        grant.check().ok()?;
        Some(Self {
            profile,
            grant,
            signature_valid,
        })
    }
}

/// A verified buyer: who it is, who stands behind it, and its ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// The agent's key (NIP-SOV `agent`).
    pub buyer: String,
    /// The admitting authority (NIP-SOV `authority`).
    pub owner: String,
    /// The grant's per-payment ceiling.
    pub max_per_tx_msat: u64,
    /// The buyer's reputation when a source gave one.
    pub reputation: Option<u32>,
}

/// Why an attestation was refused: the nine codes of
/// `buyer-attestation-v1.json`, in its evaluation order.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttestationError {
    #[error("no attestation was presented")]
    Missing,
    #[error("the attestation's signature does not verify over this request")]
    Signature,
    #[error("the agent was not admitted by an authority this merchant trusts")]
    Authority,
    #[error("the spend grant was issued to a different agent")]
    PrincipalMismatch,
    #[error("the grant is for evaluation and can't buy real goods")]
    PurposeNotOperational,
    #[error("the grant does not include spending")]
    EffectAbsent,
    #[error("the grant has expired")]
    Expired,
    #[error("the grant's ceiling is {ceiling_msat} msat, under the {price_msat} msat price")]
    OverCeiling { price_msat: u64, ceiling_msat: u64 },
    #[error("the buyer's reputation is {got}, under the required {required}")]
    ReputationTooLow { required: u32, got: u32 },
}

impl AttestationError {
    /// The fixture's code for this refusal.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Signature => "signature",
            Self::Authority => "authority",
            Self::PrincipalMismatch => "principal_mismatch",
            Self::PurposeNotOperational => "purpose_not_operational",
            Self::EffectAbsent => "effect_absent",
            Self::Expired => "expired",
            Self::OverCeiling { .. } => "over_ceiling",
            Self::ReputationTooLow { .. } => "reputation_too_low",
        }
    }
}

/// The merchant-side check, run before an invoice is issued so a refused
/// buyer never gets a challenge.
pub trait BuyerAttestation: Send + Sync {
    /// Verify `presented` for a purchase of `price_msat` at `now`.
    ///
    /// # Errors
    ///
    /// The first failing check, in the fixture's order.
    fn verify(
        &self,
        presented: Option<&Presented>,
        price_msat: u64,
        now: u64,
    ) -> Result<Attestation, AttestationError>;
}

/// Where a buyer's reputation comes from (a NIP-EVAL feed in production).
pub trait Reputation: Send + Sync {
    /// The agent's reputation, `None` when the source has none.
    fn of(&self, agent: &str) -> Option<u32>;
}

impl<F: Fn(&str) -> Option<u32> + Send + Sync> Reputation for F {
    fn of(&self, agent: &str) -> Option<u32> {
        self(agent)
    }
}

/// No reputation source: every agent has none.
pub struct NoReputation;

impl Reputation for NoReputation {
    fn of(&self, _agent: &str) -> Option<u32> {
        None
    }
}

/// The merchant's policy: which authorities it trusts to admit agents and
/// the least reputation it sells to.
///
/// Self-publication cannot establish an authority (NIP-SOV), so the
/// merchant names the authorities it trusts. A production verifier backs
/// this set with real host-admission evidence.
pub struct AttestationVerifier {
    pub trusted_authorities: BTreeSet<String>,
    /// `0` sells to any reputation, including none.
    pub min_reputation: u32,
    pub reputation: Arc<dyn Reputation>,
}

impl AttestationVerifier {
    /// A verifier trusting `authorities`, with no reputation gate.
    #[must_use]
    pub fn new(authorities: impl IntoIterator<Item = String>) -> Self {
        Self {
            trusted_authorities: authorities.into_iter().collect(),
            min_reputation: 0,
            reputation: Arc::new(NoReputation),
        }
    }

    /// The same verifier requiring at least `min` from `source`.
    #[must_use]
    pub fn with_reputation(mut self, min: u32, source: Arc<dyn Reputation>) -> Self {
        self.min_reputation = min;
        self.reputation = source;
        self
    }
}

impl BuyerAttestation for AttestationVerifier {
    fn verify(
        &self,
        presented: Option<&Presented>,
        price_msat: u64,
        now: u64,
    ) -> Result<Attestation, AttestationError> {
        // 1. Presented: a profile, a grant, and a signature.
        let presented = presented.ok_or(AttestationError::Missing)?;
        let (profile, grant) = (&presented.profile, &presented.grant);
        // 2. The signature verifies over the bound request.
        if !presented.signature_valid {
            return Err(AttestationError::Signature);
        }
        // 3. A trusted authority admitted the agent, and that same
        //    authority issued the grant (a trusted owner's agent can't
        //    spend under someone else's grant).
        if !self.trusted_authorities.contains(&profile.authority)
            || grant.issuer != profile.authority
        {
            return Err(AttestationError::Authority);
        }
        // 4. The grant is this agent's. A rotated key is a new identity
        //    whose old grants do not transfer (NIP-SOV).
        if grant.principal != profile.agent {
            return Err(AttestationError::PrincipalMismatch);
        }
        // 5. Operational, not evaluation.
        if grant.purpose != Purpose::Operational {
            return Err(AttestationError::PurposeNotOperational);
        }
        // 6. Spending is granted.
        if !grant.effects.contains(&Effect::Spend) {
            return Err(AttestationError::EffectAbsent);
        }
        // 7. Not expired.
        if now >= grant.expires_at {
            return Err(AttestationError::Expired);
        }
        // 8. The ceiling covers the price.
        let ceiling_msat = grant.bounds.max_per_tx_msat.0;
        if price_msat > ceiling_msat {
            return Err(AttestationError::OverCeiling {
                price_msat,
                ceiling_msat,
            });
        }
        // 9. Reputation, when the merchant asks for any. An agent the
        //    source knows nothing about has none.
        let reputation = self.reputation.of(&profile.agent);
        if self.min_reputation > 0 {
            let got = reputation.unwrap_or(0);
            if got < self.min_reputation {
                return Err(AttestationError::ReputationTooLow {
                    required: self.min_reputation,
                    got,
                });
            }
        }
        Ok(Attestation {
            buyer: profile.agent.clone(),
            owner: profile.authority.clone(),
            max_per_tx_msat: ceiling_msat,
            reputation,
        })
    }
}

/// Whether a route requires an attestation. Open routes (the default)
/// sell to anyone, like any x402 endpoint.
#[derive(Clone, Default)]
pub struct AttestationRequirement {
    pub required: bool,
}

impl AttestationRequirement {
    /// Run the gate: `Ok(None)` for an open route, the verified buyer for a
    /// required one.
    ///
    /// # Errors
    ///
    /// The verifier's refusal, before any invoice is issued.
    pub fn gate(
        &self,
        verifier: &dyn BuyerAttestation,
        presented: Option<&Presented>,
        price_msat: u64,
        now: u64,
    ) -> Result<Option<Attestation>, AttestationError> {
        if !self.required {
            return Ok(None);
        }
        verifier.verify(presented, price_msat, now).map(Some)
    }
}

/// The merchant's own node as `payTo`, pinned: every invoice its wallet
/// issues must be payable to exactly that key, for exactly the amount.
pub struct MerchantReceiver {
    inner: Arc<dyn Receiver>,
    pay_to: String,
}

/// Whether `text` is a 33-byte compressed public key in lowercase hex.
#[must_use]
pub fn is_node_key(text: &str) -> bool {
    text.len() == 66
        && (text.starts_with("02") || text.starts_with("03"))
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl MerchantReceiver {
    /// `inner` pinned to `pay_to`.
    ///
    /// # Errors
    ///
    /// When `pay_to` is not a node key or `inner` is a different node.
    pub fn new(inner: Arc<dyn Receiver>, pay_to: &str) -> Result<Self, String> {
        if !is_node_key(pay_to) {
            return Err("the merchant's payTo must be a 33-byte node key in hex".into());
        }
        if inner.pay_to() != pay_to {
            return Err("the merchant's wallet is a different node than its pinned payTo".into());
        }
        Ok(Self {
            inner,
            pay_to: pay_to.to_owned(),
        })
    }
}

impl Receiver for MerchantReceiver {
    fn pay_to(&self) -> String {
        self.pay_to.clone()
    }

    fn invoice(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<String, String> {
        let bolt11 = self.inner.invoice(amount_msat, request_hash, expiry_secs)?;
        let decoded = nostr::x402::decode_invoice(&bolt11)
            .map_err(|_| "the merchant's wallet issued an invoice that does not decode")?;
        if hex::encode(decoded.payee()) != self.pay_to {
            return Err("the merchant's wallet issued an invoice under another key".into());
        }
        if decoded.amount_msat() != amount_msat {
            return Err("the merchant's wallet issued an invoice for another amount".into());
        }
        Ok(bolt11)
    }

    fn received_msat(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
        self.inner.received_msat(payment_hash)
    }
}

/// A best-effort side effect of a sale: a webhook, a private purchase
/// record. It never refuses a settlement the merchant already received.
pub trait SaleObserver: Send + Sync {
    fn observe(&self, settlement: &Settlement);
}

/// The merchant's record of its own sales: one durable line per payment
/// hash, idempotent, synced before the purchase runs. No shares and no
/// payout: the merchant was paid at its own node.
pub struct MerchantSink {
    log: NdjsonSettlements,
    observer: Option<Arc<dyn SaleObserver>>,
}

impl MerchantSink {
    /// Sales recorded at `path`.
    ///
    /// # Errors
    ///
    /// When the record's folder can't be made.
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            log: NdjsonSettlements::open(path)?,
            observer: None,
        })
    }

    /// The same sink, telling `observer` about each recorded sale.
    #[must_use]
    pub fn observed(mut self, observer: Arc<dyn SaleObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub fn path(&self) -> &Path {
        self.log.path()
    }
}

impl SettlementSink for MerchantSink {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String> {
        self.log.on_settled(settlement)?;
        if let Some(observer) = &self.observer {
            observer.observe(settlement);
        }
        Ok(())
    }

    fn on_call(&self, usage: &Usage) {
        self.log.on_call(usage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msat_is_canonical_decimal() {
        assert_eq!(Msat::parse("0"), Ok(Msat(0)));
        assert_eq!(Msat::parse("1000"), Ok(Msat(1000)));
        for bad in ["", "01", "-1", "1.5", " 1", "1e3", "18446744073709551616"] {
            assert!(Msat::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn node_keys_are_compressed_hex() {
        assert!(is_node_key(&format!("02{}", "ab".repeat(32))));
        assert!(!is_node_key(&format!("04{}", "ab".repeat(32))));
        assert!(!is_node_key(&format!("02{}", "AB".repeat(32))));
        assert!(!is_node_key(&"ab".repeat(32)));
    }

    #[test]
    fn an_open_route_needs_no_attestation() {
        let verifier = AttestationVerifier::new([]);
        let open = AttestationRequirement::default();
        assert_eq!(open.gate(&verifier, None, 1, 0), Ok(None));
        let required = AttestationRequirement { required: true };
        assert_eq!(
            required.gate(&verifier, None, 1, 0),
            Err(AttestationError::Missing)
        );
    }
}
