//! Narrow owner-issued lead capabilities over the canonical private pipeline.
//! Drafts and manual certification references grant no outbound authority.
use super::*;
use coder_host::access::crew::JobRole;
use std::collections::BTreeSet;
pub(super) mod native;
#[cfg(test)]
mod tests;

pub const POLICY_SCHEMA: &str = "openagents.sales-policy.v1";
pub const OWNER_COMMAND_SCHEMA: &str = "openagents.sales-agent-owner-command.v1";
pub const AGENT_COMMAND_SCHEMA: &str = "openagents.sales-agent-command.v1";
pub const DRAFT_SCHEMA: &str = "openagents.sales-draft.v1";
pub const CERT_SCHEMA: &str = "openagents.sales-cert.v1";
pub const MEMORY_SCHEMA: &str = "openagents.sales-memory-projection.v1";
const MAX_POLICIES: usize = 128;
const MAX_ASSIGNMENTS: usize = 32;
const MAX_DRAFTS: usize = 128;
const MAX_CERTS: usize = 128;
const MAX_DAYS: usize = 366;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub name: String,
    pub pubkey: String,
    pub owner: String,
    pub role: JobRole,
    pub charter_revision: u64,
    pub crew_epoch: u64,
    pub charter_sha256: String,
    pub attestation_sha256: String,
    pub expires_at: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReadField {
    Stage,
    NextAction,
    Contact,
    Source,
    Workflow,
    Permission,
    CustomerDecision,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum WriteField {
    Stage,
    NextAction,
    Draft,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    IndividualReview,
    BatchReview,
    StandingReview,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub reference: String,
    pub sha256: String,
}
impl Artifact {
    fn check(&self) -> Result<()> {
        id(&self.reference)?;
        token(&self.sha256)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub channels: Vec<String>,
    pub jurisdictions: Vec<String>,
    pub allowed_agents: Vec<String>,
    pub data_recipients: Vec<String>,
    pub timezone: String,
    pub daily_floor_cap: u32,
    pub daily_agent_cap: u32,
    /// Declared for later execution admission. This local record route runs no model.
    pub execution_budget_usd_millionths: u64,
    pub trust: Trust,
    pub read_fields: BTreeSet<ReadField>,
    pub write_fields: BTreeSet<WriteField>,
    pub playbook: Artifact,
    pub permission_evidence_required: bool,
    pub expires_at: u64,
}
impl Policy {
    fn check(&self) -> Result<()> {
        id(&self.id)?;
        self.playbook.check()?;
        if self.schema != POLICY_SCHEMA
            || self.version == 0
            || self.expires_at == 0
            || self.channels.is_empty()
            || self.channels.len() > 3
            || self
                .channels
                .iter()
                .any(|c| !matches!(c.as_str(), "email" | "nostr" | "community"))
            || self.jurisdictions != ["US"]
            || self.timezone != "America/Chicago"
            || self.allowed_agents.is_empty()
            || self.allowed_agents.len() > 16
            || self.data_recipients.is_empty()
            || self.data_recipients.len() > 16
            || !(1..=20).contains(&self.daily_floor_cap)
            || self.daily_agent_cap == 0
            || self.daily_agent_cap > self.daily_floor_cap
            || !self.permission_evidence_required
            || self.write_fields.is_empty()
            || !self.read_fields.contains(&ReadField::Stage)
            || !self.read_fields.contains(&ReadField::NextAction)
        {
            return Err("sales policy requires explicit supported jurisdiction, consent, scope, timezone, and caps".into());
        }
        for key in &self.allowed_agents {
            token(key)?;
        }
        for recipient in &self.data_recipients {
            text(recipient, 256)?;
        }
        if self.allowed_agents.iter().collect::<BTreeSet<_>>().len() != self.allowed_agents.len()
            || self.channels.iter().collect::<BTreeSet<_>>().len() != self.channels.len()
            || self.data_recipients.iter().collect::<BTreeSet<_>>().len()
                != self.data_recipients.len()
        {
            return Err("sales policy scope contains duplicates".into());
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        self.check()?;
        Ok(digest(
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRecord {
    pub policy: Policy,
    pub sha256: String,
    pub recorded_by: String,
    pub recorded_at: u64,
    pub revoked_at: Option<u64>,
    pub revocation: Option<Artifact>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub reference: String,
    pub lead: String,
    pub anchor: Anchor,
    pub policy_sha256: String,
    pub scope_sha256: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub active: bool,
    pub recorded_by: String,
    pub revoked_at: Option<u64>,
    pub revocation: Option<Artifact>,
    /// Credential digests never leave an assigned agent projection.
    pub(super) credential_sha256: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftState {
    Proposed,
    OwnerReviewed,
    Rejected,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub schema: String,
    pub reference: String,
    pub assignment: String,
    pub author: Anchor,
    pub lead_revision: u64,
    pub policy_sha256: String,
    pub playbook: Artifact,
    pub template: Artifact,
    pub check_refs: Vec<Artifact>,
    pub recommendation: Option<Artifact>,
    pub body: String,
    pub proposed_at: u64,
    pub state: DraftState,
    pub review: Option<Artifact>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<u64>,
    pub outbound_authority: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeadRecords {
    /// Random native references carry no contact, message, or permission digest.
    pub memory_reference: Option<String>,
    pub assignments: BTreeMap<String, Assignment>,
    pub drafts: BTreeMap<String, Draft>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CertState {
    InTraining,
    OwnerMarked,
    Suspended,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certification {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub agent: Anchor,
    pub playbook: Artifact,
    pub state: CertState,
    pub suite_refs: Vec<Artifact>,
    pub roleplay_refs: Vec<Artifact>,
    pub draft_review_refs: Vec<Artifact>,
    pub owner_mark: Artifact,
    pub expires_at: u64,
}
impl Certification {
    fn check(&self) -> Result<()> {
        id(&self.id)?;
        self.playbook.check()?;
        self.owner_mark.check()?;
        if self.schema != CERT_SCHEMA
            || self.version == 0
            || self.expires_at == 0
            || self.suite_refs.len() > 8
            || self.roleplay_refs.len() > 32
            || self.draft_review_refs.len() > 32
        {
            return Err("unsupported or oversized manual sales certification".into());
        }
        for r in self
            .suite_refs
            .iter()
            .chain(&self.roleplay_refs)
            .chain(&self.draft_review_refs)
        {
            r.check()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertRecord {
    pub certification: Certification,
    pub recorded_by: String,
    pub recorded_at: u64,
    pub basis: String,
    pub measured_qualified: bool,
    pub outbound_authority: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    pub revision: u64,
    pub policies: BTreeMap<String, PolicyRecord>,
    pub current: BTreeMap<String, String>,
    pub certificates: BTreeMap<String, CertRecord>,
    /// Counts survive policy changes and reassignment. They contain no lead data.
    pub draft_days: BTreeMap<u64, BTreeMap<String, u32>>,
}
impl Book {
    pub fn cleanup_count(&self, leads: &BTreeMap<String, Lead>) -> usize {
        self.current
            .values()
            .filter(|sha| {
                self.policies
                    .get(*sha)
                    .is_some_and(|p| p.revoked_at.is_none())
            })
            .count()
            .saturating_add(
                leads
                    .values()
                    .map(|l| {
                        l.agent_records
                            .assignments
                            .values()
                            .filter(|a| a.active)
                            .count()
                    })
                    .sum::<usize>(),
            )
    }
    pub fn check(&self, leads: &BTreeMap<String, Lead>) -> Result<()> {
        if self.policies.len() > MAX_POLICIES
            || self.certificates.len() > MAX_CERTS
            || self.draft_days.len() > MAX_DAYS
        {
            return Err("sales agent record bound reached".into());
        }
        for counts in self.draft_days.values() {
            if counts.len() > 64 || counts.values().any(|n| *n > 20) {
                return Err("sales draft count bounds disagree".into());
            }
            for key in counts.keys() {
                token(key)?;
            }
        }
        for (sha, record) in &self.policies {
            if sha != &record.sha256
                || record.policy.sha256()? != *sha
                || record.revoked_at.is_some() != record.revocation.is_some()
            {
                return Err("sales policy identity disagrees".into());
            }
        }
        for (policy, sha) in &self.current {
            if !self
                .policies
                .get(sha)
                .is_some_and(|p| p.policy.id == *policy)
            {
                return Err("sales policy head is missing".into());
            }
        }
        for (key, cert) in &self.certificates {
            cert.certification.check()?;
            if key != &format!("{}:{}", cert.certification.id, cert.certification.version)
                || cert.basis != "owner_recorded"
                || cert.measured_qualified
                || cert.outbound_authority
            {
                return Err("sales certification identity or authority disagrees".into());
            }
        }
        for lead in leads.values() {
            let records = &lead.agent_records;
            if records.assignments.len() > MAX_ASSIGNMENTS || records.drafts.len() > MAX_DRAFTS {
                return Err("assigned sales record bound reached".into());
            }
            if let Some(reference) = &records.memory_reference {
                token(reference)?;
            }
            for (reference, assignment) in &records.assignments {
                token(reference)?;
                token(&assignment.credential_sha256)?;
                token(&assignment.scope_sha256)?;
                if reference != &assignment.reference
                    || assignment.lead != lead.id
                    || !self.policies.contains_key(&assignment.policy_sha256)
                {
                    return Err("sales assignment ownership disagrees".into());
                }
            }
            for (reference, draft) in &records.drafts {
                token(reference)?;
                if draft.schema != DRAFT_SCHEMA
                    || reference != &draft.reference
                    || draft.outbound_authority
                    || !records.assignments.contains_key(&draft.assignment)
                {
                    return Err("sales draft custody disagrees".into());
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerCommand {
    pub schema: String,
    pub id: String,
    pub expected_revision: u64,
    pub operation: OwnerOperation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OwnerOperation {
    PublishPolicy {
        policy: Policy,
    },
    RevokePolicy {
        policy_sha256: String,
        reference: Artifact,
    },
    Assign {
        lead: String,
        expected_lead_revision: u64,
        agent: Anchor,
        policy_sha256: String,
        expires_at: u64,
    },
    RevokeAssignment {
        lead: String,
        assignment: String,
        reference: Artifact,
    },
    ReviewDraft {
        lead: String,
        expected_lead_revision: u64,
        draft: String,
        state: DraftState,
        reference: Artifact,
    },
    RecordCertification {
        certification: Certification,
    },
}
/// A scoped credential capability. It cannot be constructed from a lead or memory.
#[derive(Clone)]
pub struct AgentAccess {
    assignment: String,
    lead: String,
    credential_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommand {
    pub schema: String,
    pub id: String,
    pub expected_lead_revision: u64,
    pub operation: AgentOperation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentOperation {
    UpdateStage {
        stage: Stage,
    },
    UpdateNextAction {
        next: Option<NextAction>,
    },
    ProposeDraft {
        body: String,
        template: Artifact,
        check_refs: Vec<Artifact>,
        recommendation: Option<Artifact>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentLead {
    pub lead: String,
    pub revision: u64,
    pub assignment: String,
    pub agent: Anchor,
    pub stage: Stage,
    pub next: Option<NextAction>,
    pub contact: Option<String>,
    pub source: Option<String>,
    pub workflow: Option<String>,
    pub permission: Option<Permission>,
    pub customer_decision: Option<CustomerDecision>,
    pub drafts: Vec<Draft>,
    pub outbound_authority: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryProjection {
    pub schema: String,
    pub lead_reference: String,
    pub assignment_reference: String,
    pub stage: Stage,
    pub has_next_action: bool,
    pub proposed_drafts: Vec<String>,
    pub lesson: MemoryLesson,
    pub authority: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLesson {
    OwnerReviewRequired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnerView {
    pub revision: u64,
    pub current: BTreeMap<String, String>,
    pub policies: Vec<PolicyRecord>,
    pub certificates: Vec<CertRecord>,
}
pub(super) fn scope(lead: &Lead) -> Result<String> {
    // This private admission digest never enters a memory or public projection.
    Ok(digest(
        &serde_json::to_vec(&(
            &lead.contact,
            &lead.details.account,
            &lead.details.jurisdiction,
            &lead.details.permission,
            &lead.details.data,
            &lead.responsible_human,
        ))
        .map_err(|e| e.to_string())?,
    ))
}
pub(super) fn parse<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_COMMAND {
        return Err("sales agent command exceeds 32 KiB".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "malformed sales agent command".into())
}
impl Store {
    pub fn sales_agent_anchor(&mut self, access: &Access, name: &str) -> Result<Anchor> {
        self.refresh()?;
        self.admin(access)?;
        Ok(native::Native::read(
            self.dir.parent().ok_or("host root is unavailable")?,
            name,
            (self.clock)(),
            self.native_keys.clone(),
        )?
        .anchor)
    }
    pub fn sales_agent_owner_view(&mut self, access: &Access) -> Result<OwnerView> {
        self.refresh()?;
        self.admin(access)?;
        Ok(OwnerView {
            revision: self.state.agents.revision,
            current: self.state.agents.current.clone(),
            policies: self.state.agents.policies.values().cloned().collect(),
            certificates: self.state.agents.certificates.values().cloned().collect(),
        })
    }
    fn agent_history(
        &self,
        key: &str,
        actor: &str,
        input: &str,
        cleanup: bool,
    ) -> Result<Option<Receipt>> {
        if let Some(record) = self.state.receipts.get(key) {
            if record.actor == actor && record.input_digest == input {
                return Ok(Some(record.receipt.clone()));
            }
            return Err("sales agent command idempotency conflict".into());
        }
        let limit = self
            .ordinary_history_limit(0)
            .saturating_add(usize::from(cleanup));
        if self.state.receipts.len() >= limit || self.state.audit.len() >= limit {
            return Err(
                "sales ordinary history bound reached; privacy cleanup remains available".into(),
            );
        }
        Ok(None)
    }
    fn agent_commit(
        &mut self,
        mut next: State,
        key: String,
        actor: String,
        input: String,
        lead: String,
        revision: u64,
        outcome: &str,
    ) -> Result<Receipt> {
        next.sequence = next
            .sequence
            .checked_add(1)
            .ok_or("sales sequence overflow")?;
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.into(),
            command_digest: key.clone(),
            lead: lead.clone(),
            revision,
            sequence: next.sequence,
            at: (self.clock)(),
            outcome: outcome.into(),
        };
        next.audit.push(Audit {
            sequence: next.sequence,
            at: receipt.at,
            actor: actor.clone(),
            lead,
            operation: outcome.into(),
            reference_digest: input.clone(),
        });
        next.receipts.insert(
            key,
            Recorded {
                actor,
                input_digest: input,
                receipt: receipt.clone(),
            },
        );
        let limit = (MAX_RECEIPTS - MAX_LEADS)
            .saturating_sub(Self::funnel_count(&next))
            .saturating_sub(next.agents.cleanup_count(&next.leads));
        if next.receipts.len() > limit || next.audit.len() > limit {
            return Err(
                "sales agent history cannot consume reserved privacy and revocation slots".into(),
            );
        }
        next.agents.check(&next.leads)?;
        self.persist(next)?;
        Ok(receipt)
    }
    /// Owner-only publication, grants, reviews, and manual certification custody.
    /// Assign writes a private credential outside the complete host root.
    pub fn apply_sales_agent_owner(
        &mut self,
        access: &Access,
        bytes: &[u8],
        credential: Option<&Path>,
    ) -> Result<Receipt> {
        self.refresh()?;
        self.admin(access)?;
        let command: OwnerCommand = parse(bytes)?;
        if command.schema != OWNER_COMMAND_SCHEMA {
            return Err("unsupported sales agent owner command".into());
        }
        let mut helper_native = None;
        if let OwnerOperation::ReviewDraft {
            lead,
            draft,
            state: DraftState::OwnerReviewed,
            ..
        } = &command.operation
        {
            let lead_record = self.state.leads.get(lead).ok_or("lead is unavailable")?;
            let value = lead_record
                .agent_records
                .drafts
                .get(draft)
                .ok_or("sales draft is unavailable")?;
            if value
                .check_refs
                .iter()
                .chain(value.recommendation.as_ref())
                .any(|r| r.reference.starts_with("sales-helper-"))
            {
                let grant = lead_record
                    .agent_records
                    .assignments
                    .get(&value.assignment)
                    .ok_or("sales helper assignment is unavailable")?;
                let native = native::Native::read(
                    self.dir.parent().ok_or("sales host root is unavailable")?,
                    &grant.anchor.name,
                    (self.clock)(),
                    self.native_keys.clone(),
                )?;
                self.validate_sales_helper_artifacts_with_native(
                    lead,
                    &value.assignment,
                    &value.check_refs,
                    value.recommendation.as_ref(),
                    &value.body,
                    &native,
                )?;
                helper_native = Some(native);
            }
        }
        id(&command.id)?;
        let key = digest(format!("sales-agent-owner:{}", command.id).as_bytes());
        let input = digest(bytes);
        let cleanup = matches!(
            command.operation,
            OwnerOperation::RevokeAssignment { .. } | OwnerOperation::RevokePolicy { .. }
        );
        if let Some(prior) = self.agent_history(&key, access.principal(), &input, cleanup)? {
            return Ok(prior);
        }
        if command.expected_revision != self.state.agents.revision {
            return Err("sales agent policy revision conflict".into());
        }
        if !matches!(command.operation, OwnerOperation::Assign { .. }) && credential.is_some() {
            return Err("only assignment accepts a new credential".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        let mut lead_ref = String::new();
        let mut revision = 0;
        let outcome;
        let mut held_native = helper_native;
        let mut admitted_until = None;
        match command.operation {
            OwnerOperation::PublishPolicy { policy } => {
                admitted_until = Some(policy.expires_at);
                let sha = policy.sha256()?;
                if policy.expires_at <= now
                    || policy.expires_at > now.saturating_add(RETENTION_MAX)
                    || next.agents.policies.len() >= MAX_POLICIES
                {
                    return Err("sales policy expiry or history bound is invalid".into());
                }
                let old_version = next
                    .agents
                    .current
                    .get(&policy.id)
                    .and_then(|s| next.agents.policies.get(s))
                    .map_or(0, |r| r.policy.version);
                if policy.version
                    != old_version
                        .checked_add(1)
                        .ok_or("sales policy version overflow")?
                {
                    return Err("sales policy version conflict".into());
                }
                next.agents.current.insert(policy.id.clone(), sha.clone());
                next.agents.policies.insert(
                    sha.clone(),
                    PolicyRecord {
                        policy,
                        sha256: sha,
                        recorded_by: access.principal().into(),
                        recorded_at: now,
                        revoked_at: None,
                        revocation: None,
                    },
                );
                outcome = "sales_policy_recorded";
            }
            OwnerOperation::RevokePolicy {
                policy_sha256,
                reference,
            } => {
                reference.check()?;
                let record = next
                    .agents
                    .policies
                    .get_mut(&policy_sha256)
                    .ok_or("sales policy is unavailable")?;
                if record.revoked_at.is_some() {
                    return Err("sales policy is already revoked".into());
                }
                record.revoked_at = Some(now);
                record.revocation = Some(reference);
                outcome = "sales_policy_revoked";
            }
            OwnerOperation::Assign {
                lead,
                expected_lead_revision,
                agent,
                policy_sha256,
                expires_at,
            } => {
                admitted_until = Some(expires_at);
                let found = self.state.leads.get(&lead).ok_or("lead is unavailable")?;
                self.readable(access, found)?;
                if found.revision != expected_lead_revision {
                    return Err("sales revision conflict".into());
                }
                let policy = self.current_sales_policy(&policy_sha256, now)?;
                let source = native::Native::read(
                    self.dir.parent().ok_or("host root is unavailable")?,
                    &agent.name,
                    now,
                    self.native_keys.clone(),
                )?;
                if source.anchor != agent {
                    return Err("native agent charter or identity changed".into());
                }
                self.agent_scope(found, &agent, policy, now)?;
                if expires_at <= now
                    || expires_at
                        > agent
                            .expires_at
                            .min(policy.expires_at)
                            .min(found.details.permission.expires_at)
                            .min(found.details.data.retain_until)
                {
                    return Err(
                        "sales assignment exceeds current consent or native grant expiry".into(),
                    );
                }
                if found.agent_records.assignments.len() >= MAX_ASSIGNMENTS {
                    return Err("sales assignment bound reached".into());
                }
                let path = credential.ok_or("assignment requires a private new credential file")?;
                if self.state.receipts.len().saturating_add(2) > self.ordinary_history_limit(0)
                    || self.state.audit.len().saturating_add(2) > self.ordinary_history_limit(0)
                {
                    return Err("assignment cannot consume its reserved revocation slot".into());
                }
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let resolved = parent
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .join(path.file_name().ok_or("credential file name is missing")?);
                if resolved.starts_with(self.dir.parent().ok_or("host root is unavailable")?) {
                    return Err("agent credential must be outside the complete host root".into());
                }
                if crate::task::regular_or_absent(path)
                    .map_err(|_| "agent credential path is unsafe")?
                {
                    return Err("assignment requires a new exclusive credential file".into());
                }
                let secret = self.credential(path)?;
                let credential_sha256 = digest(secret.as_bytes());
                if self
                    .state
                    .principals
                    .values()
                    .any(|p| p.token_digest == credential_sha256)
                    || self.state.leads.values().any(|l| {
                        l.agent_records
                            .assignments
                            .values()
                            .any(|a| a.credential_sha256 == credential_sha256)
                    })
                {
                    return Err("sales assignment credential is already in use".into());
                }
                let reference = random_token();
                let scope_sha256 = scope(found)?;
                let record = next.leads.get_mut(&lead).unwrap();
                record
                    .agent_records
                    .memory_reference
                    .get_or_insert_with(random_token);
                record.agent_records.assignments.insert(
                    reference.clone(),
                    Assignment {
                        reference: reference.clone(),
                        lead: lead.clone(),
                        anchor: agent,
                        policy_sha256,
                        scope_sha256,
                        issued_at: now,
                        expires_at,
                        active: true,
                        recorded_by: access.principal().into(),
                        revoked_at: None,
                        revocation: None,
                        credential_sha256,
                    },
                );
                record.revision = record
                    .revision
                    .checked_add(1)
                    .ok_or("lead revision overflow")?;
                record.updated_at = now;
                lead_ref = lead;
                revision = record.revision;
                // The receipt carries the random assignment reference, not a credential.
                outcome = "sales_agent_assigned";
                held_native = Some(source);
            }
            OwnerOperation::RevokeAssignment {
                lead,
                assignment,
                reference,
            } => {
                reference.check()?;
                let record = next.leads.get_mut(&lead).ok_or("lead is unavailable")?;
                self.readable(access, record)?;
                let grant = record
                    .agent_records
                    .assignments
                    .get_mut(&assignment)
                    .ok_or("sales assignment is unavailable")?;
                if !grant.active {
                    return Err("sales assignment is already revoked".into());
                }
                grant.active = false;
                grant.revoked_at = Some(now);
                grant.revocation = Some(reference);
                record.revision = record
                    .revision
                    .checked_add(1)
                    .ok_or("lead revision overflow")?;
                record.updated_at = now;
                lead_ref = lead;
                revision = record.revision;
                outcome = "sales_assignment_revoked";
            }
            OwnerOperation::ReviewDraft {
                lead,
                expected_lead_revision,
                draft,
                state,
                reference,
            } => {
                reference.check()?;
                if state == DraftState::Proposed {
                    return Err("owner review must accept a review or reject the draft".into());
                }
                let record = next.leads.get_mut(&lead).ok_or("lead is unavailable")?;
                self.readable(access, record)?;
                if record.revision != expected_lead_revision {
                    return Err("sales revision conflict".into());
                }
                let draft = record
                    .agent_records
                    .drafts
                    .get_mut(&draft)
                    .ok_or("sales draft is unavailable")?;
                if draft.state != DraftState::Proposed {
                    return Err("sales draft already has an owner decision".into());
                }
                draft.state = state;
                draft.review = Some(reference);
                draft.reviewed_by = Some(access.principal().into());
                draft.reviewed_at = Some(now);
                record.revision = record
                    .revision
                    .checked_add(1)
                    .ok_or("lead revision overflow")?;
                record.updated_at = now;
                lead_ref = lead;
                revision = record.revision;
                outcome = "sales_draft_reviewed";
            }
            OwnerOperation::RecordCertification { certification } => {
                admitted_until = Some(certification.expires_at);
                certification.check()?;
                let source = native::Native::read(
                    self.dir.parent().ok_or("host root is unavailable")?,
                    &certification.agent.name,
                    now,
                    self.native_keys.clone(),
                )?;
                if source.anchor != certification.agent
                    || certification.expires_at <= now
                    || certification.expires_at > certification.agent.expires_at
                {
                    return Err(
                        "manual certification requires exact current native identity and expiry"
                            .into(),
                    );
                }
                let latest = next
                    .agents
                    .certificates
                    .values()
                    .filter(|r| r.certification.id == certification.id)
                    .map(|r| r.certification.version)
                    .max()
                    .unwrap_or(0);
                if certification.version
                    != latest
                        .checked_add(1)
                        .ok_or("certification version overflow")?
                    || next.agents.certificates.len() >= MAX_CERTS
                {
                    return Err("manual certification revision or history bound conflict".into());
                }
                next.agents.certificates.insert(
                    format!("{}:{}", certification.id, certification.version),
                    CertRecord {
                        certification,
                        recorded_by: access.principal().into(),
                        recorded_at: now,
                        basis: "owner_recorded".into(),
                        measured_qualified: false,
                        outbound_authority: false,
                    },
                );
                held_native = Some(source);
                outcome = "sales_manual_certification_recorded";
            }
        }
        next.agents.revision = next
            .agents
            .revision
            .checked_add(1)
            .ok_or("sales agent revision overflow")?;
        if revision == 0 {
            revision = next.agents.revision;
        }
        if let Some(source) = &held_native {
            source.recheck()?;
        }
        if admitted_until.is_some_and(|until| until <= (self.clock)()) {
            return Err("sales policy or assignment expired before recording".into());
        }
        self.agent_commit(
            next,
            key,
            access.principal().into(),
            input,
            lead_ref,
            revision,
            outcome,
        )
    }
    fn current_sales_policy(&self, sha: &str, now: u64) -> Result<&Policy> {
        let record = self
            .state
            .agents
            .policies
            .get(sha)
            .ok_or("sales policy is unavailable")?;
        if record.revoked_at.is_some()
            || record.policy.expires_at <= now
            || self.state.agents.current.get(&record.policy.id) != Some(&record.sha256)
        {
            return Err("sales policy is expired, revoked, or superseded".into());
        }
        Ok(&record.policy)
    }
    fn agent_scope(&self, lead: &Lead, agent: &Anchor, policy: &Policy, now: u64) -> Result<()> {
        let channel = contact(&lead.contact)?
            .split_once(':')
            .ok_or("sales contact channel is unavailable")?
            .0
            .to_string();
        if lead.details.jurisdiction != "US"
            || !policy.jurisdictions.contains(&lead.details.jurisdiction)
        {
            return Err("sales jurisdiction is unknown or outside policy".into());
        }
        if lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || lead.details.permission.recorded_at > now
            || lead.details.permission.reference.trim().is_empty()
            || !lead.details.permission.channels.contains(&channel)
            || !policy.channels.contains(&channel)
        {
            return Err("sales agent requires current channel permission evidence".into());
        }
        if lead.details.data.retain_until <= now
            || !policy.allowed_agents.contains(&agent.pubkey)
            || !lead
                .details
                .data
                .recipients
                .contains(&format!("agent:{}", agent.pubkey))
            || lead
                .details
                .data
                .recipients
                .iter()
                .any(|r| !policy.data_recipients.contains(r))
        {
            return Err("sales agent is outside the current recipient boundary".into());
        }
        self.contact_admitted(lead, &channel)?;
        Ok(())
    }
    pub(super) fn checked_sales_agent(
        &self,
        access: &AgentAccess,
    ) -> Result<(&Lead, &Assignment, &Policy, native::Native)> {
        if self.poisoned {
            return Err("sales store needs recovery".into());
        }
        self.sales_custody()?;
        let lead = self
            .state
            .leads
            .get(&access.lead)
            .ok_or("assigned lead is unavailable")?;
        let grant = lead
            .agent_records
            .assignments
            .get(&access.assignment)
            .ok_or("sales assignment is unavailable")?;
        let now = (self.clock)();
        if !grant.active
            || grant.expires_at <= now
            || grant.credential_sha256 != access.credential_sha256
            || grant.scope_sha256 != scope(lead)?
        {
            return Err(
                "sales assignment is revoked, expired, or its consent scope changed".into(),
            );
        }
        let policy = self.current_sales_policy(&grant.policy_sha256, now)?;
        let native = native::Native::read(
            self.dir.parent().ok_or("host root is unavailable")?,
            &grant.anchor.name,
            now,
            self.native_keys.clone(),
        )?;
        if native.anchor != grant.anchor {
            return Err("native agent charter or identity changed".into());
        }
        self.agent_scope(lead, &native.anchor, policy, now)?;
        self.recheck_sales_agent(access, &native)?;
        Ok((lead, grant, policy, native))
    }
    pub(super) fn recheck_sales_agent(
        &self,
        access: &AgentAccess,
        native: &native::Native,
    ) -> Result<()> {
        self.sales_custody()?;
        native.recheck()?;
        let lead = self
            .state
            .leads
            .get(&access.lead)
            .ok_or("assigned lead is unavailable")?;
        let grant = lead
            .agent_records
            .assignments
            .get(&access.assignment)
            .ok_or("sales assignment is unavailable")?;
        let now = (self.clock)();
        let policy = self.current_sales_policy(&grant.policy_sha256, now)?;
        if !grant.active
            || grant.expires_at <= now
            || grant.anchor.expires_at <= now
            || grant.credential_sha256 != access.credential_sha256
            || grant.scope_sha256 != scope(lead)?
            || grant.anchor != native.anchor
        {
            return Err(
                "sales assignment expired or its native admission changed during the operation"
                    .into(),
            );
        }
        self.agent_scope(lead, &grant.anchor, policy, now)
    }
    pub fn authenticate_sales_agent(&mut self, secret: &str) -> Result<AgentAccess> {
        self.refresh()?;
        token(secret)?;
        let credential_sha256 = digest(secret.as_bytes());
        let found = self
            .state
            .leads
            .values()
            .find_map(|lead| {
                lead.agent_records
                    .assignments
                    .values()
                    .find(|a| a.credential_sha256 == credential_sha256)
                    .map(|a| (lead.id.clone(), a.reference.clone()))
            })
            .ok_or("sales agent credential refused")?;
        let access = AgentAccess {
            lead: found.0,
            assignment: found.1,
            credential_sha256,
        };
        self.checked_sales_agent(&access)?;
        Ok(access)
    }
    pub fn read_sales_agent(&mut self, access: &AgentAccess) -> Result<AgentLead> {
        self.refresh()?;
        let (lead, grant, policy, native) = self.checked_sales_agent(access)?;
        let has = |field| policy.read_fields.contains(&field);
        let result = AgentLead {
            lead: lead.id.clone(),
            revision: lead.revision,
            assignment: grant.reference.clone(),
            agent: grant.anchor.clone(),
            stage: lead.details.stage,
            next: lead.details.next.clone(),
            contact: has(ReadField::Contact).then(|| lead.contact.clone()),
            source: has(ReadField::Source).then(|| lead.source.clone()),
            workflow: has(ReadField::Workflow).then(|| lead.details.workflow.clone()),
            permission: has(ReadField::Permission).then(|| lead.details.permission.clone()),
            customer_decision: has(ReadField::CustomerDecision)
                .then(|| lead.details.customer_decision.clone())
                .flatten(),
            drafts: lead
                .agent_records
                .drafts
                .values()
                .filter(|d| d.assignment == grant.reference)
                .cloned()
                .collect(),
            outbound_authority: false,
        };
        self.recheck_sales_agent(access, &native)?;
        Ok(result)
    }
    pub fn sales_agent_memory(&mut self, access: &AgentAccess) -> Result<MemoryProjection> {
        self.refresh()?;
        let (lead, grant, _, native) = self.checked_sales_agent(access)?;
        let result = MemoryProjection {
            schema: MEMORY_SCHEMA.into(),
            lead_reference: lead
                .agent_records
                .memory_reference
                .clone()
                .ok_or("sales memory reference is missing")?,
            assignment_reference: grant.reference.clone(),
            stage: lead.details.stage,
            has_next_action: lead.details.next.is_some(),
            proposed_drafts: lead
                .agent_records
                .drafts
                .values()
                .filter(|d| d.assignment == grant.reference)
                .map(|d| d.reference.clone())
                .collect(),
            lesson: MemoryLesson::OwnerReviewRequired,
            authority: false,
        };
        self.recheck_sales_agent(access, &native)?;
        Ok(result)
    }
    pub fn apply_sales_agent(&mut self, access: &AgentAccess, bytes: &[u8]) -> Result<Receipt> {
        self.refresh()?;
        let (lead, grant, policy, native) = self.checked_sales_agent(access)?;
        super::privacy::check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "sales agent command is not UTF-8")?,
        )?;
        let command: AgentCommand = parse(bytes)?;
        if command.schema != AGENT_COMMAND_SCHEMA {
            return Err("unsupported sales agent command".into());
        }
        if let AgentOperation::ProposeDraft {
            body,
            check_refs,
            recommendation,
            ..
        } = &command.operation
        {
            self.validate_sales_helper_artifacts_with_native(
                &access.lead,
                &access.assignment,
                check_refs,
                recommendation.as_ref(),
                body,
                &native,
            )?;
        }
        id(&command.id)?;
        let key = digest(format!("sales-agent:{}:{}", access.assignment, command.id).as_bytes());
        let actor = format!("agent:{}", grant.anchor.pubkey);
        let input = digest(bytes);
        if let Some(receipt) = self.agent_history(&key, &actor, &input, false)? {
            self.recheck_sales_agent(access, &native)?;
            return Ok(receipt);
        }
        if command.expected_lead_revision != lead.revision {
            return Err("sales revision conflict".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        let record = next.leads.get_mut(&access.lead).unwrap();
        let outcome;
        let mut admitted_day = None;
        match command.operation {
            AgentOperation::UpdateStage { stage } => {
                if !policy.write_fields.contains(&WriteField::Stage) {
                    return Err("sales agent stage write is outside its field grant".into());
                }
                record.details.stage = stage;
                if stage == Stage::Closed {
                    if record.details.next.is_some()
                        && !policy.write_fields.contains(&WriteField::NextAction)
                    {
                        return Err("closing a lead requires the next-action field grant".into());
                    }
                    record.details.next = None;
                }
                outcome = "sales_agent_stage_updated";
            }
            AgentOperation::UpdateNextAction { next: action } => {
                if !policy.write_fields.contains(&WriteField::NextAction) {
                    return Err("sales agent next-action write is outside its field grant".into());
                }
                record.details.next = action;
                outcome = "sales_agent_next_updated";
            }
            AgentOperation::ProposeDraft {
                body,
                template,
                check_refs,
                recommendation,
            } => {
                if !policy.write_fields.contains(&WriteField::Draft) {
                    return Err("sales agent draft is outside its field grant".into());
                }
                text(&body, 4096)?;
                template.check()?;
                if check_refs.is_empty()
                    || check_refs.len() > 8
                    || record.agent_records.drafts.len() >= MAX_DRAFTS
                {
                    return Err("sales draft requires bounded check references".into());
                }
                for r in &check_refs {
                    r.check()?;
                }
                if let Some(r) = &recommendation {
                    r.check()?;
                }
                // Calendar days use the native business timezone, never Verse time.
                let day = business_day(now)?;
                admitted_day = Some(day);
                let counts = next.agents.draft_days.entry(day).or_default();
                let total: u32 = counts
                    .values()
                    .try_fold(0u32, |sum, n| sum.checked_add(*n))
                    .ok_or("sales draft count overflow")?;
                let used = counts.get(&grant.anchor.pubkey).copied().unwrap_or(0);
                if total >= policy.daily_floor_cap || used >= policy.daily_agent_cap {
                    return Err("sales draft review cap reached".into());
                }
                if counts.len() >= 64 && !counts.contains_key(&grant.anchor.pubkey)
                    || next.agents.draft_days.len() > MAX_DAYS
                {
                    return Err("sales draft day history bound reached".into());
                }
                *next
                    .agents
                    .draft_days
                    .get_mut(&day)
                    .unwrap()
                    .entry(grant.anchor.pubkey.clone())
                    .or_default() += 1;
                let reference = random_token();
                record.agent_records.drafts.insert(
                    reference.clone(),
                    Draft {
                        schema: DRAFT_SCHEMA.into(),
                        reference,
                        assignment: grant.reference.clone(),
                        author: grant.anchor.clone(),
                        lead_revision: lead.revision,
                        policy_sha256: grant.policy_sha256.clone(),
                        playbook: policy.playbook.clone(),
                        template,
                        check_refs,
                        recommendation,
                        body,
                        proposed_at: now,
                        state: DraftState::Proposed,
                        review: None,
                        reviewed_by: None,
                        reviewed_at: None,
                        outbound_authority: false,
                    },
                );
                outcome = "sales_agent_draft_proposed";
            }
        }
        validate(&record.details, now)?;
        validate_contact_permission(&record.contact, &record.details)?;
        record.revision = record
            .revision
            .checked_add(1)
            .ok_or("lead revision overflow")?;
        record.updated_at = now;
        let revision = record.revision;
        native.recheck()?;
        // Authority and consent are rechecked before the only local effect.
        self.recheck_sales_agent(access, &native)?;
        if admitted_day.is_some_and(|day| business_day((self.clock)()).ok() != Some(day)) {
            return Err(
                "sales business date changed before draft recording; retry under the current cap"
                    .into(),
            );
        }
        self.agent_commit(
            next,
            key,
            actor,
            input,
            access.lead.clone(),
            revision,
            outcome,
        )
    }
}
pub(super) fn business_day(now: u64) -> Result<u64> {
    let seconds = i64::try_from(now).map_err(|_| "sales clock exceeds its bound")?;
    let timestamp = jiff::Timestamp::from_second(seconds).map_err(|_| "sales clock is invalid")?;
    let timezone = jiff::tz::TimeZone::get("America/Chicago")
        .map_err(|_| "sales business timezone is unavailable")?;
    timestamp
        .to_zoned(timezone)
        .strftime("%Y%m%d")
        .to_string()
        .parse()
        .map_err(|_| "sales business day is invalid".into())
}
