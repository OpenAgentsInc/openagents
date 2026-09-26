//! Closed NIP-LAB terms, checker, and rights contracts.
//!
//! The MKT adapter checks exact bytes and these structural/role/deadline rules.
//! The host must implement the required closure-admission interface: source
//! provenance, complete task frames, execution and checker locks, supported
//! schemas, and disclosure authority. There is no permissive default and no
//! delivery, acceptance, execution, or payment implementation in this module.

use super::*;
use crate::contracts::{
    BoundAssignment, SchemaRef, check_artifact_bytes, parse_bound, parse_schema_ref,
};

pub const PROFILE: &str = "openagents.labor.v1";
pub const LABOR_TERMS_SCHEMA: &str = "openagents.labor-terms.v1";
pub const ACCEPTANCE_POLICY_SCHEMA: &str = "openagents.labor-acceptance-policy.v1";
pub const RIGHTS_SCHEMA: &str = "openagents.labor-rights.v1";

/// Commercial roles fixed by the host for this profile instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parties {
    pub buyer: String,
    pub provider: String,
    pub worker: String,
}
impl Parties {
    fn check(&self) -> Result<(), ContractError> {
        for k in [&self.buyer, &self.provider, &self.worker] {
            key(&Value::String(k.clone()), "labor party")?;
        }
        if self.buyer == self.provider {
            return Err(malformed("distinct commercial parties"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deliverable {
    pub id: String,
    pub schema: SchemaRef,
    pub max_bytes: u64,
}

/// Frozen execution inputs; none of these references confers a host grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    pub target: DefinitionRef,
    pub lock: ArtifactRef,
    pub input: ArtifactRef,
    pub context: ArtifactRef,
    pub requirements: ArtifactRef,
    pub bounds: Vec<BoundAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaborTerms {
    pub task_frame: ArtifactRef,
    pub execution: Execution,
    pub deliverables: Vec<Deliverable>,
    pub reviewer: String,
    pub acceptance_policy: ArtifactRef,
    pub resolver: String,
    pub max_reworks: u64,
    pub rework_due_at: Option<u64>,
    pub dispute_due_at: u64,
    pub resolution_due_at: u64,
    pub rights: ArtifactRef,
    pub role_relationships: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptancePolicy {
    pub checker: DefinitionRef,
    pub lock: ArtifactRef,
    pub criteria: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reuse {
    Deny,
    SeparateGrant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputUse {
    ReviewOnly,
    UseUnderLicense,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rights {
    pub license: ArtifactRef,
    pub output_use: OutputUse,
    pub publication: Reuse,
    pub training: Reuse,
    pub evaluation_reuse: Reuse,
    pub redistribution: Reuse,
    pub recipients: BTreeSet<String>,
}

/// Check a LAB terms artifact and its role invariants. When quoted market terms
/// are present, check all cross-contract deadlines and parties as well.
pub fn parse_labor_terms(
    bytes: &[u8],
    parties: &Parties,
    market: Option<&Terms>,
) -> Result<LaborTerms, ContractError> {
    parties.check()?;
    let value = canonical(bytes)?;
    let m = versioned(
        &value,
        LABOR_TERMS_SCHEMA,
        &[
            "task_frame",
            "execution",
            "deliverables",
            "reviewer",
            "acceptance_policy",
            "resolver",
            "resolver_policy",
            "max_reworks",
            "rework_due_at",
            "dispute_due_at",
            "resolution_due_at",
            "cancellation",
            "partial_delivery",
            "buyer_unavailable",
            "rights",
            "role_relationships",
        ],
    )?;
    literal(field(m, "resolver_policy")?, "labor-evidence-v1")?;
    literal(field(m, "cancellation")?, "evaluate-delivered-work-v1")?;
    literal(field(m, "partial_delivery")?, "no-partial-payment-v1")?;
    literal(field(m, "buyer_unavailable")?, "resolver-required-v1")?;
    let reviewer = key(field(m, "reviewer")?, "reviewer")?;
    let resolver = key(field(m, "resolver")?, "resolver")?;
    if reviewer == parties.provider
        || reviewer == parties.worker
        || [
            &parties.buyer,
            &parties.provider,
            &parties.worker,
            &reviewer,
        ]
        .contains(&&resolver)
    {
        return Err(not_admitted("labor reviewer/resolver roles"));
    }
    let max_reworks = uint(field(m, "max_reworks")?, "max_reworks")?;
    let rework_due_at = optional_uint(field(m, "rework_due_at")?, "rework_due_at")?;
    if max_reworks > 3 || (max_reworks == 0) != rework_due_at.is_none() {
        return Err(malformed("labor rework bounds"));
    }
    let dispute_due_at = uint(field(m, "dispute_due_at")?, "dispute_due_at")?;
    let resolution_due_at = uint(field(m, "resolution_due_at")?, "resolution_due_at")?;
    if dispute_due_at >= resolution_due_at {
        return Err(malformed("labor dispute deadlines"));
    }
    let e = object(field(m, "execution")?, "execution")?;
    closed(
        e,
        &[
            "target",
            "lock",
            "input",
            "context",
            "requirements",
            "bounds",
        ],
    )?;
    let mut bounds = Vec::new();
    for item in list(field(e, "bounds")?, false, "bounds")? {
        let bound = parse_bound(item)?;
        if bounds
            .iter()
            .any(|b: &BoundAssignment| b.bound == bound.bound)
        {
            return Err(malformed("duplicate execution bound"));
        }
        bounds.push(bound);
    }
    let input = parse_artifact(field(e, "input")?)?;
    if input.schema.is_none() {
        return Err(malformed("labor input schema"));
    }
    let execution = Execution {
        target: parse_definition(field(e, "target")?)?,
        lock: structured(field(e, "lock")?, Some("openagents.lock.v1"))?,
        input,
        context: structured(field(e, "context")?, Some("openagents.context.v1"))?,
        requirements: structured(field(e, "requirements")?, None)?,
        bounds,
    };
    let mut deliverables = Vec::new();
    let mut ids = BTreeSet::new();
    for item in list(field(m, "deliverables")?, true, "deliverables")? {
        let d = object(item, "deliverable")?;
        closed(d, &["id", "schema", "max_bytes"])?;
        let id = slug(field(d, "id")?, "deliverable id")?;
        let max_bytes = uint(field(d, "max_bytes")?, "deliverable max_bytes")?;
        if !ids.insert(id.clone()) || max_bytes == 0 {
            return Err(malformed("deliverable identity or bound"));
        }
        deliverables.push(Deliverable {
            id,
            schema: parse_schema_ref(field(d, "schema")?)?,
            max_bytes,
        });
    }
    let mut role_relationships = BTreeMap::new();
    for item in list(field(m, "role_relationships")?, true, "role_relationships")? {
        let r = object(item, "role relationship")?;
        closed(r, &["pubkey", "operator"])?;
        let person = key(field(r, "pubkey")?, "role pubkey")?;
        let operator = display(field(r, "operator")?, "operator")?;
        if operator.is_empty() || role_relationships.insert(person, operator).is_some() {
            return Err(malformed("role relationship identity"));
        }
    }
    if [
        &parties.buyer,
        &parties.provider,
        &parties.worker,
        &reviewer,
        &resolver,
    ]
    .iter()
    .any(|k| !role_relationships.contains_key(*k))
    {
        return Err(malformed("missing role relationship"));
    }
    let parsed = LaborTerms {
        task_frame: structured(field(m, "task_frame")?, Some("openagents.task-frame.v1"))?,
        execution,
        deliverables,
        reviewer,
        acceptance_policy: structured(
            field(m, "acceptance_policy")?,
            Some(ACCEPTANCE_POLICY_SCHEMA),
        )?,
        resolver,
        max_reworks,
        rework_due_at,
        dispute_due_at,
        resolution_due_at,
        rights: structured(field(m, "rights")?, Some(RIGHTS_SCHEMA))?,
        role_relationships,
    };
    if let Some(market) = market {
        if market.profile != PROFILE
            || market.buyer != parties.buyer
            || market.provider != parties.provider
            || market.worker != parties.worker
        {
            return Err(identity("labor market parties/profile"));
        }
        if market.review_due_at > dispute_due_at
            || resolution_due_at >= market.payment_due_at
            || rework_due_at.is_some_and(|deadline| {
                deadline < market.delivery_due_at || deadline >= market.review_due_at
            })
        {
            return Err(malformed("labor market deadline ordering"));
        }
    }
    Ok(parsed)
}

/// A checker policy fixes a complete all-pass criterion set before execution.
pub fn parse_acceptance_policy(bytes: &[u8]) -> Result<AcceptancePolicy, ContractError> {
    let value = canonical(bytes)?;
    let m = versioned(
        &value,
        ACCEPTANCE_POLICY_SCHEMA,
        &["checker", "lock", "criteria", "rule"],
    )?;
    literal(field(m, "rule")?, "all-pass-v1")?;
    let mut criteria = Vec::new();
    let mut seen = BTreeSet::new();
    for c in list(field(m, "criteria")?, true, "criteria")? {
        let id = slug(c, "criterion")?;
        if !seen.insert(id.clone()) {
            return Err(malformed("duplicate criterion"));
        }
        criteria.push(id);
    }
    Ok(AcceptancePolicy {
        checker: parse_definition(field(m, "checker")?)?,
        lock: structured(field(m, "lock")?, Some("openagents.lock.v1"))?,
        criteria,
    })
}

/// Rights restrict reuse. A separate-grant value is not itself permission.
pub fn parse_rights(bytes: &[u8]) -> Result<Rights, ContractError> {
    let value = canonical(bytes)?;
    let m = versioned(
        &value,
        RIGHTS_SCHEMA,
        &[
            "license",
            "input_use",
            "output_use",
            "publication",
            "training",
            "evaluation_reuse",
            "redistribution",
            "recipients",
            "retention",
        ],
    )?;
    literal(field(m, "input_use")?, "perform-and-review-order")?;
    literal(field(m, "retention")?, "through-market-retain-until")?;
    let output_use = match text(field(m, "output_use")?, "output_use")? {
        "review-only" => OutputUse::ReviewOnly,
        "use-under-license" => OutputUse::UseUnderLicense,
        _ => return Err(unsupported("labor output use")),
    };
    let mut recipients = BTreeSet::new();
    for value in list(field(m, "recipients")?, true, "recipients")? {
        if !recipients.insert(key(value, "recipient")?) {
            return Err(malformed("duplicate rights recipient"));
        }
    }
    Ok(Rights {
        license: parse_artifact(field(m, "license")?)?,
        output_use,
        publication: reuse(field(m, "publication")?)?,
        training: reuse(field(m, "training")?)?,
        evaluation_reuse: reuse(field(m, "evaluation_reuse")?)?,
        redistribution: reuse(field(m, "redistribution")?)?,
        recipients,
    })
}

/// The host supplies bounded, exact content resolution and actual supported
/// execution/checker/frame semantics. It must authenticate required original
/// declarations and enforce disclosure without executing work. An unavailable
/// dependency or unsupported schema must refuse, never become a default value.
pub trait ClosureAdmission {
    fn resolve(&self, reference: &ArtifactRef) -> Result<Vec<u8>, ContractError>;
    fn check(
        &self,
        parties: &Parties,
        terms: &LaborTerms,
        checker: &AcceptancePolicy,
        rights: &Rights,
        capability: Option<&DefinitionRef>,
    ) -> Result<(), ContractError>;
}

/// Strict LAB adapter for MKT, with required host support for its full closure.
pub struct LaborProfile<'a, H> {
    parties: Parties,
    host: &'a H,
}
impl<'a, H: ClosureAdmission> LaborProfile<'a, H> {
    pub fn new(parties: Parties, host: &'a H) -> Result<Self, ContractError> {
        parties.check()?;
        Ok(Self { parties, host })
    }
    fn validate(
        &self,
        reference: &ArtifactRef,
        market: Option<&Terms>,
        capability: Option<&DefinitionRef>,
    ) -> Result<(), ContractError> {
        let bytes = self.read(reference, LABOR_TERMS_SCHEMA)?;
        let terms = parse_labor_terms(&bytes, &self.parties, market)?;
        let checker = parse_acceptance_policy(
            &self.read(&terms.acceptance_policy, ACCEPTANCE_POLICY_SCHEMA)?,
        )?;
        let rights = parse_rights(&self.read(&terms.rights, RIGHTS_SCHEMA)?)?;
        for participant in [
            &self.parties.buyer,
            &self.parties.provider,
            &self.parties.worker,
            &terms.reviewer,
            &terms.resolver,
        ] {
            if !rights.recipients.contains(participant) {
                return Err(not_admitted("required labor recipient missing"));
            }
        }
        self.host
            .check(&self.parties, &terms, &checker, &rights, capability)
    }
    fn read(&self, reference: &ArtifactRef, schema: &str) -> Result<Vec<u8>, ContractError> {
        if reference.schema.as_deref() != Some(schema) || reference.media_type != "application/json"
        {
            return Err(identity("labor artifact schema"));
        }
        if reference.size > crate::contracts::MAX_BODY_BYTES as u64 {
            return Err(ContractError::new(
                RefusalCode::LimitExceeded,
                "labor artifact bytes",
            ));
        }
        let bytes = self.host.resolve(reference)?;
        check_artifact_bytes(reference, &bytes)?;
        Ok(bytes)
    }
}
impl<H: ClosureAdmission> DomainProfile for LaborProfile<'_, H> {
    fn id(&self) -> &str {
        PROFILE
    }
    fn validate_request(&self, request: &ArtifactRef) -> Result<(), ContractError> {
        self.validate(request, None, None)
    }
    fn validate_terms(
        &self,
        terms: &Terms,
        capability: &DefinitionRef,
    ) -> Result<(), ContractError> {
        self.validate(&terms.profile_terms, Some(terms), Some(capability))
    }
}

fn list<'a>(v: &'a Value, nonempty: bool, detail: &str) -> Result<&'a [Value], ContractError> {
    let a = v.as_array().ok_or_else(|| malformed(detail))?;
    if a.len() > 64 || nonempty && a.is_empty() {
        return Err(ContractError::new(RefusalCode::LimitExceeded, detail));
    }
    Ok(a)
}
fn literal(v: &Value, wanted: &str) -> Result<(), ContractError> {
    if text(v, wanted)? == wanted {
        Ok(())
    } else {
        Err(unsupported(wanted))
    }
}
fn reuse(v: &Value) -> Result<Reuse, ContractError> {
    match text(v, "reuse")? {
        "deny" => Ok(Reuse::Deny),
        "separate-grant" => Ok(Reuse::SeparateGrant),
        _ => Err(unsupported("labor reuse policy")),
    }
}

#[cfg(test)]
mod tests;
