//! Permanent relationship decisions stay separate from commission eligibility.
use super::*;

pub const SCHEMA: &str = "openagents.referral.attribution.v1";
pub const POLICY_SCHEMA: &str = "openagents.referral.policy.v1";
pub const RULE: &str = "consented-permanent-review-v1";
const HISTORY_LIMIT: usize = 32;

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "sha256:{}",
        hash(canonicalize(&serde_json::to_value(value).expect("a referral serializes")).as_bytes())
    )
}
fn digest_ok(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The operator publishes exact terms before a customer accepts this digest.
/// The fixed rule retains accepted relationships and sends exceptions to review.
/// It does not define a percentage, a payee, or a right to commission.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub version: String,
    pub rule: String,
    pub terms: String,
    pub digest: String,
}
impl Policy {
    pub fn new(version: String, terms: String) -> Result<Self, Error> {
        bounded(&version, 64)?;
        bounded(&terms, 4096)?;
        let mut policy = Self {
            schema: POLICY_SCHEMA.into(),
            version,
            rule: RULE.into(),
            terms,
            digest: String::new(),
        };
        policy.digest = policy.compute_digest();
        Ok(policy)
    }
    fn compute_digest(&self) -> String {
        digest(&(&self.schema, &self.version, &self.rule, &self.terms))
    }
    fn check(&self) -> Result<(), Error> {
        if self.schema != POLICY_SCHEMA || self.rule != RULE || self.digest != self.compute_digest()
        {
            return Err(Error::Invalid);
        }
        bounded(&self.version, 64)?;
        bounded(&self.terms, 4096)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// An opaque private reference, never the agreement text or a credential.
    pub reference: String,
    pub digest: String,
}
impl Evidence {
    fn check(&self) -> Result<(), Error> {
        bounded(&self.reference, 128)?;
        if !digest_ok(&self.digest) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Introduction {
    CapturedSource,
    EarlyAgreement,
    PreexistingCustomer,
    MissingEvidence,
    Correction,
}
/// Customer-authenticated consent to a specific policy and introduction.
/// Early agreements are attributed claims until both parties confirm them.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub request: String,
    pub policy_digest: String,
    pub introduction: Introduction,
    pub referrer: Option<String>,
    pub evidence: Vec<Evidence>,
    pub reason: String,
    pub consent: bool,
    /// Required for a correction, and pins the decision the customer reviewed.
    pub expected_decision: Option<String>,
}
impl Proposal {
    fn check(&self) -> Result<(), Error> {
        bounded(&self.request, 128)?;
        bounded(&self.reason, 512)?;
        if !self.consent
            || !digest_ok(&self.policy_digest)
            || self.evidence.len() > 8
            || self
                .expected_decision
                .as_ref()
                .is_some_and(|v| !digest_ok(v))
        {
            return Err(Error::Invalid);
        }
        if let Some(id) = &self.referrer {
            bounded(id, 128)?;
        }
        for evidence in &self.evidence {
            evidence.check()?;
        }
        let unique: std::collections::BTreeSet<_> =
            self.evidence.iter().map(|e| &e.reference).collect();
        if unique.len() != self.evidence.len() {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Accepted,
    Review,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    MissingEvidence,
    PreexistingCustomer,
    CompetingIntroduction,
    AwaitingConfirmation,
    SelfReferral,
    SourceOnly,
    UnknownSignup,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub customer: String,
    pub referrer: Identity,
    pub policy_digest: String,
    pub accepted_decision: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub schema: String,
    pub customer: String,
    pub sequence: u64,
    pub prior: Option<String>,
    pub request: String,
    pub policy_digest: String,
    pub introduction: Introduction,
    pub status: Status,
    pub review: Option<Review>,
    pub referrer: Option<Identity>,
    pub referrer_owner: Option<String>,
    pub source: Option<Source>,
    pub evidence: Vec<Evidence>,
    pub reason: String,
    pub actor: String,
    pub confirmed: Option<String>,
    pub at: u64,
    pub digest: String,
}
impl Decision {
    fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a decision serializes");
        value
            .as_object_mut()
            .expect("a decision is an object")
            .remove("digest");
        digest(&value)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub schema: String,
    pub customer: String,
    pub status: Status,
    /// A previous accepted binding stays visible while a conflict is reviewed.
    pub binding: Option<Binding>,
    pub decisions: Vec<Decision>,
    pub commission_eligibility: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceView {
    pub schema: String,
    pub workspace: String,
    pub status: Status,
    pub binding: Binding,
    pub commission_eligibility: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Successor {
    pub referrer: String,
    pub from: String,
    pub to: String,
    pub version: u64,
    pub accepted_at: u64,
    pub management_only: bool,
    pub digest: String,
}
impl Successor {
    fn compute_digest(&self) -> String {
        digest(&(
            &self.referrer,
            &self.from,
            &self.to,
            self.version,
            self.accepted_at,
            self.management_only,
        ))
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    policies: BTreeMap<String, Policy>,
    current_policy: Option<String>,
    customers: BTreeMap<String, View>,
    requests: BTreeMap<String, (String, String)>,
    workspaces: BTreeMap<String, String>,
    successors: Vec<Successor>,
}
impl State {
    pub(super) fn retained_binding(&self, customer: &str) -> Option<Binding> {
        self.customers.get(customer)?.binding.clone()
    }
    fn accepted(&self, binding: &Binding) -> Option<&Decision> {
        self.customers
            .get(&binding.customer)?
            .decisions
            .iter()
            .find(|d| {
                d.status == Status::Accepted
                    && d.digest == binding.accepted_decision
                    && binding_for(d) == *binding
            })
    }
    pub(super) fn retains_accepted(&self, binding: &Binding) -> bool {
        self.accepted(binding).is_some()
    }
    /// Pin the exact native management authority that admitted this consent.
    pub(super) fn commission_manager(
        &self,
        book: &Book,
        binding: &Binding,
        actor: &str,
    ) -> Result<(u64, Option<String>), Error> {
        let decision = self.accepted(binding).ok_or(Error::Unavailable)?;
        let referrer = book
            .referrers
            .get(&binding.referrer.id)
            .ok_or(Error::Unavailable)?;
        if referrer.owner != actor {
            return Err(Error::Unauthorized);
        }
        if let Some(successor) = self
            .successors
            .iter()
            .rev()
            .find(|s| s.referrer == referrer.id)
        {
            if successor.to == actor && successor.version <= referrer.version {
                return Ok((referrer.version, Some(successor.digest.clone())));
            }
            return Err(Error::Unauthorized);
        }
        if decision.referrer_owner.as_deref() == Some(actor) {
            Ok((referrer.version, None))
        } else {
            Err(Error::Unauthorized)
        }
    }
    pub(super) fn retains_commission_manager(
        &self,
        binding: &Binding,
        actor: &str,
        version: u64,
        successor: Option<&str>,
        at: u64,
    ) -> bool {
        if version < binding.referrer.version {
            return false;
        }
        match successor {
            Some(id) => self.successors.iter().any(|s| {
                s.digest == id
                    && s.referrer == binding.referrer.id
                    && s.to == actor
                    && s.version <= version
                    && s.accepted_at <= at
            }),
            None => self
                .accepted(binding)
                .is_some_and(|d| d.referrer_owner.as_deref() == Some(actor)),
        }
    }
    pub(super) fn current_commission_binding(
        &self,
        book: &Book,
        customer: &str,
    ) -> Result<Binding, Error> {
        let view = self.customers.get(customer).ok_or(Error::Unavailable)?;
        if view.status != Status::Accepted {
            return Err(Error::Conflict);
        }
        let binding = view.binding.clone().ok_or(Error::Unavailable)?;
        let referrer = book
            .referrers
            .get(&binding.referrer.id)
            .ok_or(Error::Unavailable)?;
        if binding.referrer.source_only || referrer.source_only || referrer.owner == customer {
            return Err(Error::Conflict);
        }
        self.commission_manager(book, &binding, &referrer.owner)?;
        Ok(binding)
    }
    pub(super) fn confirms_manager(&self, referrer: &Referrer, actor: &str) -> bool {
        self.successors
            .iter()
            .rev()
            .find(|successor| successor.referrer == referrer.id)
            .is_some_and(|successor| successor.to == actor && successor.version <= referrer.version)
    }
    pub(super) fn is_empty(&self) -> bool {
        self.policies.is_empty()
            && self.customers.is_empty()
            && self.workspaces.is_empty()
            && self.successors.is_empty()
    }
    pub(super) fn record_successor(
        &mut self,
        referrer: &Referrer,
        from: &str,
    ) -> Result<(), Error> {
        if self.successors.len() >= LIMIT {
            return Err(Error::Bound);
        }
        let mut successor = Successor {
            referrer: referrer.id.clone(),
            from: from.into(),
            to: referrer.owner.clone(),
            version: referrer.version,
            accepted_at: unix_now(),
            management_only: true,
            digest: String::new(),
        };
        successor.digest = successor.compute_digest();
        self.successors.push(successor);
        Ok(())
    }
    pub(super) fn validate(
        &self,
        book: &Book,
        accounts: &BTreeMap<String, Account>,
        workspaces: &BTreeMap<String, Workspace>,
    ) -> Result<(), String> {
        let invalid = || "invalid permanent referral attribution".to_string();
        if self.policies.len() > HISTORY_LIMIT
            || self.customers.len() > LIMIT
            || self.workspaces.len() > LIMIT
            || self.requests.len() > LIMIT
            || self.successors.len() > LIMIT
            || self
                .current_policy
                .as_ref()
                .is_some_and(|d| !self.policies.contains_key(d))
        {
            return Err(invalid());
        }
        let mut versions = std::collections::BTreeSet::new();
        for (key, policy) in &self.policies {
            if key != &policy.digest || policy.check().is_err() || !versions.insert(&policy.version)
            {
                return Err(invalid());
            }
        }
        let mut decisions = BTreeMap::new();
        for (customer, view) in &self.customers {
            if customer != &view.customer
                || view.schema != SCHEMA
                || !accounts.contains_key(customer)
                || view.decisions.is_empty()
                || view.decisions.len() > HISTORY_LIMIT
                || view.commission_eligibility
            {
                return Err(invalid());
            }
            let mut prior = None;
            let mut binding = None;
            for (index, d) in view.decisions.iter().enumerate() {
                if d.schema != SCHEMA
                    || &d.customer != customer
                    || d.sequence != index as u64 + 1
                    || d.prior != prior
                    || d.digest != d.compute_digest()
                    || !self.policies.contains_key(&d.policy_digest)
                    || d.actor != *customer
                    || (d.status == Status::Review) != d.review.is_some()
                    || bounded(&d.reason, 512).is_err()
                    || bounded(&d.request, 128).is_err()
                    || d.evidence.len() > 8
                    || d.evidence.iter().any(|e| e.check().is_err())
                {
                    return Err(invalid());
                }
                if let Some(r) = &d.referrer {
                    if !book.referrers.get(&r.id).is_some_and(|record| {
                        r.version > 0
                            && r.version <= record.version
                            && r.kind == record.kind
                            && r.source_only == record.source_only
                    }) || !d
                        .referrer_owner
                        .as_ref()
                        .is_some_and(|owner| accounts.contains_key(owner))
                    {
                        return Err(invalid());
                    }
                } else if d.referrer_owner.is_some() {
                    return Err(invalid());
                }
                if d.source.as_ref().is_some_and(|source| {
                    source.account != *customer
                        || book.sources.get(customer).map(|r| &r.source) != Some(source)
                }) {
                    return Err(invalid());
                }
                if d.status == Status::Accepted {
                    let r = d.referrer.as_ref().ok_or_else(invalid)?;
                    if r.source_only
                        || d.referrer_owner.as_deref() == Some(customer)
                        || d.actor != *customer
                        || (!matches!(d.introduction, Introduction::CapturedSource)
                            && d.confirmed.as_ref() != d.referrer_owner.as_ref())
                    {
                        return Err(invalid());
                    }
                    if d.introduction == Introduction::CapturedSource
                        && d.confirmed.is_none()
                        && (!book
                            .sources
                            .get(customer)
                            .is_some_and(|r| r.at_signup == Some(true))
                            || d.source.as_ref().and_then(|s| s.referrer.as_ref()) != Some(r))
                    {
                        return Err(invalid());
                    }
                    binding = Some(binding_for(d));
                }
                prior = Some(d.digest.clone());
                if decisions
                    .insert(d.digest.clone(), request_key(customer, &d.request))
                    .is_some()
                {
                    return Err(invalid());
                }
            }
            if view.status != view.decisions.last().ok_or_else(invalid)?.status
                || view.binding != binding
            {
                return Err(invalid());
            }
        }
        for (request, (input, decision)) in &self.requests {
            if !digest_ok(input) || decisions.get(decision) != Some(request) {
                return Err(invalid());
            }
        }
        for (workspace, customer) in &self.workspaces {
            if !workspaces.contains_key(workspace)
                || !self
                    .customers
                    .get(customer)
                    .is_some_and(|v| v.binding.is_some())
            {
                return Err(invalid());
            }
        }
        for s in &self.successors {
            if !s.management_only
                || s.from == s.to
                || !accounts.contains_key(&s.from)
                || !accounts.contains_key(&s.to)
                || s.digest != s.compute_digest()
                || !book
                    .referrers
                    .get(&s.referrer)
                    .is_some_and(|r| s.version > 1 && s.version <= r.version)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    fn append(&mut self, mut decision: Decision) -> Result<Decision, Error> {
        if !self.customers.contains_key(&decision.customer) && self.customers.len() >= LIMIT {
            return Err(Error::Bound);
        }
        let view = self
            .customers
            .entry(decision.customer.clone())
            .or_insert_with(|| View {
                schema: SCHEMA.into(),
                customer: decision.customer.clone(),
                status: Status::Review,
                binding: None,
                decisions: vec![],
                commission_eligibility: false,
            });
        if view.decisions.len() >= HISTORY_LIMIT {
            return Err(Error::Bound);
        }
        decision.sequence = view.decisions.len() as u64 + 1;
        decision.prior = view.decisions.last().map(|d| d.digest.clone());
        decision.digest = decision.compute_digest();
        view.status = decision.status;
        if decision.status == Status::Accepted {
            view.binding = Some(binding_for(&decision));
        }
        view.decisions.push(decision.clone());
        Ok(decision)
    }
}
fn request_key(customer: &str, request: &str) -> String {
    digest(&(customer, request))
}
fn binding_for(d: &Decision) -> Binding {
    Binding {
        id: format!("attr_{}", hash(d.customer.as_bytes())),
        customer: d.customer.clone(),
        referrer: d.referrer.clone().expect("accepted referrer"),
        policy_digest: d.policy_digest.clone(),
        accepted_decision: d.digest.clone(),
    }
}

impl Book {
    pub(in crate::accounts) fn inherit_workspace(
        &mut self,
        workspace: &Workspace,
    ) -> Result<(), Error> {
        let owner = workspace
            .members
            .values()
            .find(|m| m.role == Role::Owner && m.status == MemberStatus::Active)
            .ok_or(Error::Invalid)?;
        if self
            .attribution
            .customers
            .get(&owner.account)
            .is_some_and(|v| v.binding.is_some())
        {
            if self.attribution.workspaces.len() >= LIMIT
                && !self.attribution.workspaces.contains_key(&workspace.id)
            {
                return Err(Error::Bound);
            }
            self.attribution
                .workspaces
                .entry(workspace.id.clone())
                .or_insert(owner.account.clone());
        }
        Ok(())
    }
}
impl Accounts {
    /// Operator-only publication. No customer route can install or replace terms.
    pub fn publish_attribution_policy(&self, policy: &Policy) -> Result<Policy, Error> {
        policy.check()?;
        self.referral_write(|store, _| {
            let state = &mut store.referrals.attribution;
            if let Some(prior) = state
                .policies
                .values()
                .find(|p| p.version == policy.version)
            {
                return if prior == policy {
                    Ok((prior.clone(), false))
                } else {
                    Err(Error::Conflict)
                };
            }
            if state.policies.len() >= HISTORY_LIMIT {
                return Err(Error::Bound);
            }
            state.policies.insert(policy.digest.clone(), policy.clone());
            state.current_policy = Some(policy.digest.clone());
            Ok((policy.clone(), true))
        })
    }
    pub fn attribution_policy(&self, actor: &str) -> Result<Option<Policy>, Error> {
        self.attribution_policy_version(actor, None)
    }
    pub fn attribution_policy_version(
        &self,
        actor: &str,
        version: Option<&str>,
    ) -> Result<Option<Policy>, Error> {
        if version.is_some_and(|v| !digest_ok(v)) {
            return Err(Error::Invalid);
        }
        let store = load(&self.dir).map_err(storage)?;
        if !store.accounts.contains_key(actor) {
            return Err(Error::Unauthorized);
        }
        let state = &store.referrals.attribution;
        Ok(version
            .or(state.current_policy.as_deref())
            .and_then(|d| state.policies.get(d))
            .cloned())
    }
    pub fn attribution(&self, actor: &str) -> Result<Option<View>, Error> {
        let store = load(&self.dir).map_err(storage)?;
        if !store.accounts.contains_key(actor) {
            return Err(Error::Unauthorized);
        }
        Ok(store.referrals.attribution.customers.get(actor).cloned())
    }
    pub fn propose_attribution(&self, actor: &str, input: &Proposal) -> Result<Decision, Error> {
        self.propose_attribution_guarded(actor, input, || true)
    }
    /// The service reauthenticates the original credential under account custody.
    pub fn propose_attribution_guarded(
        &self,
        actor: &str,
        input: &Proposal,
        current: impl FnOnce() -> bool,
    ) -> Result<Decision, Error> {
        input.check()?;
        self.referral_write(|store, now| {
            if !current() || !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            let state = &store.referrals.attribution;
            let key = request_key(actor, &input.request);
            let input_digest = digest(&(
                &input.policy_digest,
                input.introduction,
                &input.referrer,
                &input.evidence,
                &input.reason,
                input.consent,
                &input.expected_decision,
            ));
            if let Some((prior, decision)) = state.requests.get(&key) {
                if prior != &input_digest {
                    return Err(Error::Conflict);
                }
                let d = state
                    .customers
                    .get(actor)
                    .and_then(|v| v.decisions.iter().find(|d| &d.digest == decision))
                    .ok_or(Error::Unavailable)?;
                return Ok((d.clone(), false));
            }
            if let Some(d) = state
                .requests
                .values()
                .filter(|(input, _)| input == &input_digest)
                .find_map(|(_, decision)| {
                    state
                        .customers
                        .get(actor)
                        .and_then(|v| v.decisions.iter().find(|d| &d.digest == decision))
                })
            {
                return Ok((d.clone(), false));
            }
            if state.current_policy.as_deref() != Some(&input.policy_digest) {
                return Err(Error::Conflict);
            }
            let prior = state.customers.get(actor).and_then(|v| v.decisions.last());
            if input.introduction == Introduction::Correction
                && (prior.is_none()
                    || input.expected_decision.is_none()
                    || input.expected_decision.as_deref() != prior.map(|d| d.digest.as_str()))
            {
                return Err(Error::Conflict);
            }
            if input.introduction != Introduction::Correction && input.expected_decision.is_some() {
                return Err(Error::Invalid);
            }
            let source = store.referrals.sources.get(actor).map(|r| r.source.clone());
            let referrer_id = if input.introduction == Introduction::CapturedSource {
                let id = source
                    .as_ref()
                    .and_then(|s| s.referrer.as_ref())
                    .map(|r| r.id.as_str());
                if input.referrer.as_deref().is_some_and(|r| Some(r) != id) {
                    return Err(Error::Conflict);
                }
                id
            } else {
                input.referrer.as_deref()
            };
            let referrer = referrer_id
                .map(|id| store.referrals.referrers.get(id).ok_or(Error::Invalid))
                .transpose()?;
            let identity = if input.introduction == Introduction::CapturedSource {
                source.as_ref().and_then(|s| s.referrer.clone())
            } else {
                referrer.map(|r| Identity {
                    id: r.id.clone(),
                    version: r.version,
                    kind: r.kind,
                    source_only: r.source_only,
                })
            };
            let review = if input.introduction == Introduction::MissingEvidence
                || identity.is_none()
                || (input.introduction != Introduction::CapturedSource && input.evidence.is_empty())
            {
                Some(Review::MissingEvidence)
            } else if referrer.is_some_and(|r| r.source_only) {
                Some(Review::SourceOnly)
            } else if referrer.is_some_and(|r| r.owner == actor) {
                Some(Review::SelfReferral)
            } else if prior.is_some() && input.introduction != Introduction::Correction {
                Some(Review::CompetingIntroduction)
            } else if input.introduction == Introduction::CapturedSource
                && store
                    .referrals
                    .sources
                    .get(actor)
                    .and_then(|r| r.at_signup)
                    .is_none()
            {
                Some(Review::UnknownSignup)
            } else if input.introduction == Introduction::CapturedSource
                && !store
                    .referrals
                    .sources
                    .get(actor)
                    .is_some_and(|r| r.at_signup == Some(true))
            {
                Some(Review::PreexistingCustomer)
            } else if input.introduction == Introduction::CapturedSource {
                None
            } else if input.introduction == Introduction::PreexistingCustomer {
                Some(Review::PreexistingCustomer)
            } else {
                Some(Review::AwaitingConfirmation)
            };
            let d = Decision {
                schema: SCHEMA.into(),
                customer: actor.into(),
                sequence: 0,
                prior: None,
                request: input.request.clone(),
                policy_digest: input.policy_digest.clone(),
                introduction: input.introduction,
                status: if review.is_none() {
                    Status::Accepted
                } else {
                    Status::Review
                },
                review,
                referrer: identity,
                referrer_owner: referrer.map(|r| r.owner.clone()),
                source,
                evidence: input.evidence.clone(),
                reason: input.reason.clone(),
                actor: actor.into(),
                confirmed: None,
                at: now,
                digest: String::new(),
            };
            let d = store.referrals.attribution.append(d)?;
            if store.referrals.attribution.requests.len() >= LIMIT {
                return Err(Error::Bound);
            }
            store
                .referrals
                .attribution
                .requests
                .insert(key, (input_digest, d.digest.clone()));
            inherit_personal(store, actor)?;
            Ok((d, true))
        })
    }
    pub fn confirm_attribution(
        &self,
        actor: &str,
        customer: &str,
        reviewed: &str,
    ) -> Result<Decision, Error> {
        self.confirm_attribution_guarded(actor, customer, reviewed, || true)
    }
    pub fn confirm_attribution_guarded(
        &self,
        actor: &str,
        customer: &str,
        reviewed: &str,
        current: impl FnOnce() -> bool,
    ) -> Result<Decision, Error> {
        bounded(customer, 128)?;
        if !digest_ok(reviewed) {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, now| {
            if !current() || !store.accounts.contains_key(actor) {
                return Err(Error::Unauthorized);
            }
            let view = store
                .referrals
                .attribution
                .customers
                .get(customer)
                .ok_or(Error::Unauthorized)?;
            let prior = view.decisions.last().ok_or(Error::Unavailable)?;
            let referrer = prior
                .referrer
                .as_ref()
                .and_then(|id| store.referrals.referrers.get(&id.id))
                .ok_or(Error::Unauthorized)?;
            if referrer.owner != actor
                || actor == customer
                || prior.referrer_owner.as_deref() != Some(actor)
            {
                return Err(Error::Unauthorized);
            }
            if prior.status == Status::Accepted
                && prior.prior.as_deref() == Some(reviewed)
                && prior.confirmed.as_deref() == Some(actor)
            {
                return Ok((prior.clone(), false));
            }
            if prior.digest != reviewed || prior.status != Status::Review {
                return Err(Error::Conflict);
            }
            if prior.evidence.is_empty()
                || referrer.source_only
                || !matches!(
                    prior.review,
                    Some(Review::AwaitingConfirmation | Review::PreexistingCustomer)
                )
                || store.referrals.attribution.current_policy.as_deref()
                    != Some(&prior.policy_digest)
            {
                return Err(Error::Conflict);
            }
            let mut decision = prior.clone();
            decision.request = format!("confirm_{}", hash(reviewed.as_bytes()));
            decision.status = Status::Accepted;
            decision.review = None;
            decision.confirmed = Some(actor.into());
            decision.at = now;
            let d = store.referrals.attribution.append(decision)?;
            inherit_personal(store, customer)?;
            Ok((d, true))
        })
    }
    /// Current owners and admins receive only this workspace's relationship.
    pub fn workspace_attribution(
        &self,
        actor: &str,
        workspace: &str,
    ) -> Result<Option<WorkspaceView>, Error> {
        let store = load(&self.dir).map_err(storage)?;
        let ws = store.workspaces.get(workspace).ok_or(Error::Unauthorized)?;
        let member = active_member(ws, actor).map_err(|_| Error::Unauthorized)?;
        if member.role < Role::Admin {
            return Err(Error::Unauthorized);
        }
        let Some(customer) = store.referrals.attribution.workspaces.get(workspace) else {
            return Ok(None);
        };
        let view = store
            .referrals
            .attribution
            .customers
            .get(customer)
            .ok_or(Error::Unavailable)?;
        Ok(Some(WorkspaceView {
            schema: SCHEMA.into(),
            workspace: workspace.into(),
            status: view.status,
            binding: view.binding.clone().ok_or(Error::Unavailable)?,
            commission_eligibility: false,
        }))
    }
    pub fn referrer_successors(&self, actor: &str, id: &str) -> Result<Vec<Successor>, Error> {
        let store = load(&self.dir).map_err(storage)?;
        if !store
            .referrals
            .referrers
            .get(id)
            .is_some_and(|r| r.owner == actor)
        {
            return Err(Error::Unauthorized);
        }
        Ok(store
            .referrals
            .attribution
            .successors
            .iter()
            .filter(|s| s.referrer == id)
            .cloned()
            .collect())
    }
    pub fn adopt_workspace_attribution_guarded(
        &self,
        actor: &str,
        workspace: &str,
        expected: &str,
        current: impl FnOnce() -> bool,
    ) -> Result<WorkspaceView, Error> {
        bounded(workspace, 128)?;
        if !digest_ok(expected) {
            return Err(Error::Invalid);
        }
        self.referral_write(|store, _| {
            if !current() {
                return Err(Error::Unauthorized);
            }
            let ws = store.workspaces.get(workspace).ok_or(Error::Unauthorized)?;
            if active_member(ws, actor)
                .map_err(|_| Error::Unauthorized)?
                .role
                != Role::Owner
            {
                return Err(Error::Unauthorized);
            }
            let view = store
                .referrals
                .attribution
                .customers
                .get(actor)
                .ok_or(Error::Unavailable)?;
            if view.status != Status::Accepted
                || view.decisions.last().map(|d| d.digest.as_str()) != Some(expected)
            {
                return Err(Error::Conflict);
            }
            let binding = view.binding.clone().ok_or(Error::Unavailable)?;
            let changed = match store.referrals.attribution.workspaces.get(workspace) {
                Some(customer) if customer != actor => return Err(Error::Conflict),
                Some(_) => false,
                None => {
                    if store.referrals.attribution.workspaces.len() >= LIMIT {
                        return Err(Error::Bound);
                    }
                    store
                        .referrals
                        .attribution
                        .workspaces
                        .insert(workspace.into(), actor.into());
                    true
                }
            };
            Ok((
                WorkspaceView {
                    schema: SCHEMA.into(),
                    workspace: workspace.into(),
                    status: Status::Accepted,
                    binding,
                    commission_eligibility: false,
                },
                changed,
            ))
        })
    }
}
fn inherit_personal(store: &mut Store, customer: &str) -> Result<(), Error> {
    for workspace in store.workspaces.values().filter(|ws| {
        ws.kind == WorkspaceKind::Personal
            && ws
                .members
                .get(customer)
                .is_some_and(|m| m.role == Role::Owner && m.status == MemberStatus::Active)
    }) {
        store.referrals.inherit_workspace(workspace)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
