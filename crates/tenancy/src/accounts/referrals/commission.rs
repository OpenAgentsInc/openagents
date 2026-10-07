//! Reviewed commission terms and consent, without accrual or payout authority.
use super::*;
use crate::money::funding::{Rounding, Unit};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "openagents.referral.commission-terms.v1";
pub const AGREEMENT_SCHEMA: &str = "openagents.referral.commission-agreement.v1";
const VERSIONS: usize = 32;

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "sha256:{}",
        hash(canonicalize(&serde_json::to_value(value).expect("a contract serializes")).as_bytes())
    )
}
fn digest_ok(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Product {
    PluginCall,
    HostedResource,
    GatewayUsage,
    AcceptedService,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fraction {
    pub numerator: u64,
    pub denominator: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Base {
    OpenagentsAvailableEarnedShare,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Conversion {
    SameUnitOnly,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PayoutPrecision {
    WholeSatoshiRetainRemainder,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Destination {
    QualifiedSpark,
    QualifiedLightningAddress,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Reversal {
    VerifiedRefundDisputeAdjustsReferrerLiabilityPreservesAuthorShares,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Hold {
    VerifiedEarnedCostsAfterHold,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Conflict {
    SuspendNewEligibilityRetainHistory,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Permanence {
    RetainAcceptedVersionUntilBothReaccept,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Exclusion {
    UnusedFunding,
    PromotionalFreeCredit,
    SelfReferral,
    RecycledFunding,
    UnknownCosts,
    UnresolvedAttribution,
}

/// Every economic value is supplied by the operator. There are no commercial defaults.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub schema: String,
    pub version: String,
    pub products: BTreeSet<Product>,
    pub base: Base,
    pub share: Fraction,
    pub unit: Unit,
    pub conversion: Conversion,
    pub rounding: Rounding,
    pub payout_precision: PayoutPrecision,
    pub hold_secs: u64,
    pub hold: Hold,
    pub minimum: u64,
    pub destinations: BTreeSet<Destination>,
    pub reversal: Reversal,
    pub permanence: Permanence,
    pub attribution_conflict: Conflict,
    pub exclusions: BTreeSet<Exclusion>,
    pub effective_from: u64,
    pub terms: String,
    pub digest: String,
}
impl Terms {
    /// Exact denomination scaling for the selected native BTC rails. This
    /// performs no currency conversion, settlement, or wallet operation.
    pub fn amount_msat(&self, amount: u64) -> Result<u64, Error> {
        let multiplier = match &self.unit {
            Unit::Satoshis => 1000,
            Unit::Millisatoshis => 1,
            Unit::CurrencyMillionths { currency } if currency == "BTC" => 100_000,
            Unit::CurrencyMillionths { .. } => return Err(Error::Invalid),
        };
        amount
            .checked_mul(multiplier)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(Error::Invalid)
    }
    fn shape(&self) -> Result<(), Error> {
        bounded(&self.version, 64)?;
        bounded(&self.terms, 4096)?;
        self.unit.validate().map_err(|_| Error::Invalid)?;
        // The existing Spark and Lightning payout worker sends whole sats and
        // retains the unpaid msat remainder. A minimum must be payable exactly.
        let minimum = self.amount_msat(self.minimum)?;
        if self.schema != SCHEMA
            || self.products.is_empty()
            || self.destinations.is_empty()
            || self.share.denominator == 0
            || self.share.numerator > self.share.denominator
            || minimum == 0
            || !minimum.is_multiple_of(1000)
            || self.exclusions
                != [
                    Exclusion::UnusedFunding,
                    Exclusion::PromotionalFreeCredit,
                    Exclusion::SelfReferral,
                    Exclusion::RecycledFunding,
                    Exclusion::UnknownCosts,
                    Exclusion::UnresolvedAttribution,
                ]
                .into()
            || self.effective_from.checked_add(self.hold_secs).is_none()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn computed(&self) -> String {
        let mut value = serde_json::to_value(self).expect("terms serialize");
        value.as_object_mut().unwrap().remove("digest");
        digest(&value)
    }
    pub fn seal(mut self) -> Result<Self, Error> {
        self.shape()?;
        self.digest = self.computed();
        Ok(self)
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.shape()?;
        if self.digest != self.computed() {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    /// A calculation preview over supplied facts, not verified earnings or a liability.
    /// A settlement adapter must independently verify every fact before REV-30 accrues.
    pub fn preview(&self, facts: &EconomicFacts) -> Result<Option<Preview>, Error> {
        self.validate()?;
        if facts.unit != self.unit || !self.products.contains(&facts.product) {
            return Err(Error::Invalid);
        }
        if !facts.earned
            || !facts.admitted_attribution_verified
            || facts.promotional_or_free
            || facts.self_or_recycled
        {
            return Ok(None);
        }
        self.amount_msat(facts.settled_distributable)?;
        if facts
            .author_resource_shares
            .checked_add(facts.openagents_share)
            != Some(facts.settled_distributable)
        {
            return Err(Error::Invalid);
        }
        let Some(costs) = facts
            .costs
            .iter()
            .try_fold(0u64, |n, v| n.checked_add((*v)?))
        else {
            return Ok(None);
        };
        let Some(base) = facts
            .openagents_share
            .checked_sub(costs)
            .and_then(|n| n.checked_sub(facts.promotions))
        else {
            return Ok(None);
        };
        let n = (base as u128) * (self.share.numerator as u128);
        let denominator = self.share.denominator as u128;
        let remainder = (n % denominator) as u64;
        if self.rounding == Rounding::Exact && remainder != 0 {
            return Err(Error::Invalid);
        }
        let commission = (n / denominator) as u64;
        Ok(Some(Preview {
            unit: self.unit.clone(),
            author_resource_shares: facts.author_resource_shares,
            costs,
            promotions: facts.promotions,
            base,
            commission,
            openagents_remaining: base - commission,
            remainder,
            denominator: self.share.denominator,
            accrual_enabled: false,
        }))
    }
}
#[derive(Clone, Debug)]
pub struct EconomicFacts {
    pub product: Product,
    pub unit: Unit,
    pub earned: bool,
    /// Verify the originally admitted decision and agreement, not a mutable
    /// current view that could erase an earlier transaction's accepted terms.
    pub admitted_attribution_verified: bool,
    pub promotional_or_free: bool,
    pub self_or_recycled: bool,
    pub settled_distributable: u64,
    pub author_resource_shares: u64,
    pub openagents_share: u64,
    /// Model, compute, payment, delivery, support, and other expense, in that order.
    /// Each must be known; zero is an explicit supplied fact, never a default.
    pub costs: [Option<u64>; 6],
    pub promotions: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    pub unit: Unit,
    pub author_resource_shares: u64,
    pub costs: u64,
    pub promotions: u64,
    pub base: u64,
    pub commission: u64,
    pub openagents_remaining: u64,
    pub remainder: u64,
    pub denominator: u64,
    pub accrual_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub terms: Terms,
    pub published_at: u64,
    pub account_revision: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub request: String,
    pub customer: String,
    pub terms_digest: String,
    pub attribution_decision: String,
    pub consent: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Party {
    Customer,
    Referrer,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub party: Party,
    pub actor: String,
    pub request: String,
    pub account_revision: String,
    pub accepted_at: u64,
    pub manager_version: Option<u64>,
    pub manager_successor: Option<String>,
    pub digest: String,
}
impl Acceptance {
    fn computed(&self, agreement: &str) -> String {
        digest(&(
            agreement,
            self.party,
            &self.actor,
            &self.request,
            &self.account_revision,
            self.accepted_at,
            self.manager_version,
            &self.manager_successor,
        ))
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Agreement {
    pub schema: String,
    pub id: String,
    pub customer: String,
    pub binding: attribution::Binding,
    pub attribution_decision: String,
    pub terms_digest: String,
    pub acceptances: BTreeMap<Party, Acceptance>,
}
impl Agreement {
    fn id(binding: &attribution::Binding, terms: &str) -> String {
        digest(&(AGREEMENT_SCHEMA, binding, terms))
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub agreement: Agreement,
    pub terms: Publication,
    pub state: String,
    /// Current qualification never changes the retained agreement or its
    /// original transaction pins. Later settlement verifies those exact pins.
    pub terms_qualified: bool,
    pub active_for_new_transactions: bool,
    pub accrual_enabled: bool,
    /// Only a later canonical liability and current destination qualification
    /// can establish payout readiness; a terms agreement cannot do so.
    pub payout_qualified: bool,
    pub payout_enabled: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    publications: BTreeMap<String, Publication>,
    current: Option<String>,
    agreements: BTreeMap<String, Agreement>,
    active: BTreeMap<String, String>,
    requests: BTreeMap<String, (String, String)>,
}
impl State {
    pub(super) fn is_empty(&self) -> bool {
        self.publications.is_empty()
            && self.current.is_none()
            && self.agreements.is_empty()
            && self.active.is_empty()
            && self.requests.is_empty()
    }
    pub(super) fn validate(
        &self,
        book: &Book,
        accounts: &BTreeMap<String, Account>,
    ) -> Result<(), String> {
        let invalid = || "Invalid retained commission contract.".to_string();
        if self.publications.len() > VERSIONS
            || self.agreements.len() > LIMIT
            || self.requests.len() > LIMIT * 2
            || self.active.len() > LIMIT
        {
            return Err(invalid());
        }
        let mut versions = BTreeSet::new();
        for (key, p) in &self.publications {
            p.terms.validate().map_err(|_| invalid())?;
            if key != &p.terms.digest
                || !versions.insert(&p.terms.version)
                || !digest_ok(&p.account_revision)
            {
                return Err(invalid());
            }
        }
        if self
            .current
            .as_ref()
            .is_some_and(|key| !self.publications.contains_key(key))
        {
            return Err(invalid());
        }
        for (key, a) in &self.agreements {
            // Corrections may retain another current binding; historical consent
            // must still name its exact accepted native decision.
            if key != &a.id
                || a.schema != AGREEMENT_SCHEMA
                || !accounts.contains_key(&a.customer)
                || !self.publications.contains_key(&a.terms_digest)
                || a.attribution_decision != a.binding.accepted_decision
                || a.customer != a.binding.customer
                || a.id != Agreement::id(&a.binding, &a.terms_digest)
                || a.acceptances.len() > 2
                || !book.attribution.retains_accepted(&a.binding)
            {
                return Err(invalid());
            }
            for (party, accepted) in &a.acceptances {
                if party != &accepted.party
                    || !accounts.contains_key(&accepted.actor)
                    || !digest_ok(&accepted.account_revision)
                    || accepted.digest != accepted.computed(&a.id)
                    || bounded(&accepted.request, 128).is_err()
                    || accepted.accepted_at
                        < self.publications[&a.terms_digest].terms.effective_from
                    || (*party == Party::Customer
                        && (accepted.actor != a.customer
                            || accepted.manager_version.is_some()
                            || accepted.manager_successor.is_some()))
                    || (*party == Party::Referrer
                        && (accepted.actor == a.customer
                            || !accepted.manager_version.is_some_and(|version| {
                                book.referrers
                                    .get(&a.binding.referrer.id)
                                    .is_some_and(|r| version <= r.version)
                                    && book.attribution.retains_commission_manager(
                                        &a.binding,
                                        &accepted.actor,
                                        version,
                                        accepted.manager_successor.as_deref(),
                                        accepted.accepted_at,
                                    )
                            })))
                {
                    return Err(invalid());
                }
            }
        }
        for (customer, id) in &self.active {
            let a = self.agreements.get(id).ok_or_else(invalid)?;
            if &a.customer != customer || a.acceptances.len() != 2 {
                return Err(invalid());
            }
        }
        for (input, agreement) in self.requests.values() {
            if !digest_ok(input) || !self.agreements.contains_key(agreement) {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn authorized(book: &Book, actor: &str, binding: &attribution::Binding) -> Result<(), Error> {
    if actor != binding.customer {
        book.attribution.commission_manager(book, binding, actor)?;
    }
    Ok(())
}
fn view(book: &Book, agreement: &Agreement) -> Result<View, Error> {
    let publication = book
        .commissions
        .publications
        .get(&agreement.terms_digest)
        .ok_or(Error::Unavailable)?
        .clone();
    let accepted = agreement.acceptances.len() == 2;
    let current = book
        .attribution
        .current_commission_binding(book, &agreement.customer)
        .is_ok_and(|binding| binding == agreement.binding);
    Ok(View {
        agreement: agreement.clone(),
        terms: publication,
        state: if !current {
            "suspended-attribution-review"
        } else if accepted {
            "accepted-terms"
        } else {
            "awaiting-other-party"
        }
        .into(),
        terms_qualified: current && accepted,
        active_for_new_transactions: current
            && accepted
            && book.commissions.active.get(&agreement.customer) == Some(&agreement.id),
        accrual_enabled: false,
        payout_qualified: false,
        payout_enabled: false,
    })
}
impl Accounts {
    /// Local operator custody only. No authenticated customer route publishes terms.
    pub fn publish_commission_terms(
        &self,
        terms: &Terms,
        approved: &str,
        expected: Option<&str>,
    ) -> Result<Publication, Error> {
        terms.validate()?;
        if approved != terms.digest {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, now| {
            let state = &mut store.referrals.commissions;
            if let Some(prior) = state
                .publications
                .values()
                .find(|p| p.terms.version == terms.version)
            {
                return if &prior.terms == terms {
                    Ok((prior.clone(), false))
                } else {
                    Err(Error::Conflict)
                };
            }
            if state.current.as_deref() != expected {
                return Err(Error::Conflict);
            }
            if state.publications.len() >= VERSIONS {
                return Err(Error::Bound);
            }
            let p = Publication {
                terms: terms.clone(),
                published_at: now,
                account_revision: store.digest.clone(),
            };
            state.publications.insert(terms.digest.clone(), p.clone());
            state.current = Some(terms.digest.clone());
            Ok((p, true))
        })
    }
    /// Inspectable terms contain no private customer or acceptance record.
    pub fn commission_publication(
        &self,
        version: Option<&str>,
    ) -> Result<Option<Publication>, Error> {
        if version.is_some_and(|v| !digest_ok(v)) {
            return Err(Error::Invalid);
        }
        let store = load(&self.dir).map_err(storage)?;
        let state = &store.referrals.commissions;
        Ok(version
            .or(state.current.as_deref())
            .and_then(|id| state.publications.get(id))
            .cloned())
    }
    pub fn commission_publication_guarded(
        &self,
        actor: &str,
        version: Option<&str>,
        current: impl FnOnce() -> bool,
    ) -> Result<Option<Publication>, Error> {
        if version.is_some_and(|v| !digest_ok(v)) {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, _| {
            if !current() || !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            let state = &store.referrals.commissions;
            Ok((
                version
                    .or(state.current.as_deref())
                    .and_then(|id| state.publications.get(id))
                    .cloned(),
                false,
            ))
        })
    }
    pub fn commission_agreement_guarded(
        &self,
        actor: &str,
        customer: &str,
        selected: Option<&str>,
        current: impl FnOnce() -> bool,
    ) -> Result<Option<View>, Error> {
        bounded(customer, 128)?;
        if selected.is_some_and(|v| !digest_ok(v)) {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, _| {
            if !current() || !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            let state = &store.referrals.commissions;
            let agreement = selected
                .and_then(|id| state.agreements.get(id))
                .or_else(|| {
                    if selected.is_none() {
                        state
                            .active
                            .get(customer)
                            .and_then(|id| state.agreements.get(id))
                    } else {
                        None
                    }
                });
            if let Some(a) = agreement {
                if a.customer != customer {
                    return Err(Error::Unauthorized);
                }
                authorized(&store.referrals, actor, &a.binding)?;
            } else {
                let binding = store
                    .referrals
                    .attribution
                    .retained_binding(customer)
                    .ok_or(Error::Unavailable)?;
                authorized(&store.referrals, actor, &binding)?;
            }
            Ok((
                agreement.map(|a| view(&store.referrals, a)).transpose()?,
                false,
            ))
        })
    }
    pub fn accept_commission_terms_guarded(
        &self,
        actor: &str,
        input: &Input,
        current: impl FnOnce() -> bool,
    ) -> Result<View, Error> {
        bounded(&input.request, 128)?;
        bounded(&input.customer, 128)?;
        if !input.consent
            || !digest_ok(&input.terms_digest)
            || !digest_ok(&input.attribution_decision)
        {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, now| {
            if !current() || !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            let binding = store
                .referrals
                .attribution
                .current_commission_binding(&store.referrals, &input.customer)?;
            authorized(&store.referrals, actor, &binding)?;
            if binding.accepted_decision != input.attribution_decision {
                return Err(Error::Conflict);
            }
            let request_key = format!("{actor}:{}", input.request);
            let fingerprint = digest(input);
            if let Some((prior, id)) = store.referrals.commissions.requests.get(&request_key) {
                if prior != &fingerprint {
                    return Err(Error::Conflict);
                }
                return Ok((
                    view(
                        &store.referrals,
                        &store.referrals.commissions.agreements[id],
                    )?,
                    false,
                ));
            }
            let manager = if actor != input.customer {
                Some(store.referrals.attribution.commission_manager(
                    &store.referrals,
                    &binding,
                    actor,
                )?)
            } else {
                None
            };
            let state = &mut store.referrals.commissions;
            if state.current.as_deref() != Some(&input.terms_digest) {
                return Err(Error::Conflict);
            }
            let published = state
                .publications
                .get(&input.terms_digest)
                .ok_or(Error::Unavailable)?;
            if published.terms.effective_from > now {
                return Err(Error::Conflict);
            }
            let id = Agreement::id(&binding, &input.terms_digest);
            if state.agreements.len() >= LIMIT && !state.agreements.contains_key(&id)
                || state.requests.len() >= LIMIT * 2
            {
                return Err(Error::Bound);
            }
            let party = if actor == input.customer {
                Party::Customer
            } else {
                Party::Referrer
            };
            let agreement = state
                .agreements
                .entry(id.clone())
                .or_insert_with(|| Agreement {
                    schema: AGREEMENT_SCHEMA.into(),
                    id: id.clone(),
                    customer: input.customer.clone(),
                    attribution_decision: binding.accepted_decision.clone(),
                    binding,
                    terms_digest: input.terms_digest.clone(),
                    acceptances: BTreeMap::new(),
                });
            if !agreement.acceptances.contains_key(&party) {
                let mut acceptance = Acceptance {
                    party,
                    actor: actor.into(),
                    request: input.request.clone(),
                    account_revision: store.digest.clone(),
                    accepted_at: now,
                    manager_version: manager.as_ref().map(|m| m.0),
                    manager_successor: manager.and_then(|m| m.1),
                    digest: String::new(),
                };
                acceptance.digest = acceptance.computed(&id);
                agreement.acceptances.insert(party, acceptance);
            }
            if agreement.acceptances.len() == 2 {
                state.active.insert(input.customer.clone(), id.clone());
            }
            state
                .requests
                .insert(request_key, (fingerprint, id.clone()));
            Ok((
                view(
                    &store.referrals,
                    &store.referrals.commissions.agreements[&id],
                )?,
                true,
            ))
        })
    }
}

#[cfg(test)]
mod tests;
