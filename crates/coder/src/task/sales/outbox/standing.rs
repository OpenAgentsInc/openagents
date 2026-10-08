//! Standing follow-up policies for explicitly invited threads (REV-73). The
//! book is disabled until the owner grants one exact policy. A policy binds one
//! thread (lead, recipient, mailbox), one follow-up template version, the
//! invitation evidence, wall-clock spacing, an attempt and cost cap, and an
//! expiry. A follow-up proposed under it is approved like a single decision and
//! still dispatches single-use through `outbox_intent` with every recheck. No
//! batch grant, elapsed time, or measurement activates a policy by itself.
use super::*;

pub const POLICY_SCHEMA: &str = "openagents.sales.outbox-standing.v1";
pub const MAX_ATTEMPTS: u32 = 3;
pub const MIN_SPACING_SECS: u64 = 7 * 86_400;
pub const MAX_DURATION_SECS: u64 = 60 * 86_400;
const MAX_POLICIES: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub id: String,
    pub mode: Mode,
    pub lead: String,
    pub recipient: String,
    pub config_sha256: String,
    pub template: agents::Artifact,
    /// Identifier of the owner-reviewed reply that invited further contact.
    pub invitation: String,
    pub spacing_secs: u64,
    pub max_attempts: u32,
    pub maximum_cost_microusd: u64,
    pub expires_at: u64,
    pub qualification_sha256: String,
    pub owner_review_sha256: String,
}
impl Policy {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "standing policy serialization failed")?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyPhase {
    Active,
    Revoked,
    Reset,
    Exhausted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub policy: Policy,
    pub policy_sha256: String,
    pub owner: String,
    pub granted_at: u64,
    pub phase: PolicyPhase,
    pub attempts: u32,
    pub last_attempt_at: Option<u64>,
    pub proposals: Vec<String>,
    pub reference_sha256: Option<String>,
}
/// What measured reviewed-batch operation shows. Meeting it grants nothing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub mode: Mode,
    pub batch: batch::Qualification,
    pub delivered_batches: u32,
    pub policies_active: u32,
    pub enabled: bool,
    pub eligible: bool,
    pub automatic_promotion: bool,
}
impl Qualification {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(&serde_json::to_vec(self).map_err(
            |_| "standing qualification serialization failed",
        )?))
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub policies: BTreeMap<String, Record>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.policies.len() > MAX_POLICIES {
            return Err("standing policy history exceeds its bound".into());
        }
        for (id, r) in &self.policies {
            super::super::id(id)?;
            if r.policy.id != *id
                || r.policy.schema != POLICY_SCHEMA
                || r.attempts > r.policy.max_attempts
                || r.proposals.len() != r.attempts as usize
            {
                return Err("standing policy record is inconsistent".into());
            }
            token(&r.policy_sha256)?;
        }
        Ok(())
    }
    /// The book is enabled only while an active, unexpired policy exists.
    pub fn enabled(&self, now: u64) -> bool {
        self.policies
            .values()
            .any(|r| r.phase == PolicyPhase::Active && r.policy.expires_at > now)
    }
    pub fn covering(&self, proposal: &str) -> Option<&Record> {
        self.policies
            .values()
            .find(|r| r.proposals.iter().any(|p| p == proposal))
    }
    pub(super) fn reset(&mut self, now: u64) {
        for r in self.policies.values_mut() {
            if r.phase == PolicyPhase::Active {
                r.phase = PolicyPhase::Reset;
                r.reference_sha256 = Some(digest(format!("reset:{now}").as_bytes()));
            }
        }
    }
}

impl Store {
    /// Measures reviewed-batch operation. The result is informational.
    pub fn outbox_standing_qualification(
        &mut self,
        access: &Access,
        mode: Mode,
    ) -> Result<Qualification> {
        let batch = self.outbox_batch_qualification(access, mode)?;
        let book = &self.state.outbox;
        let now = (self.clock)();
        let delivered_batches = book
            .batches
            .grants
            .values()
            .filter(|g| {
                g.grant.mode == mode
                    && g.grant.items.iter().all(|i| {
                        book.records
                            .get(&i.proposal)
                            .is_some_and(|r| r.count_consumed && r.phase == Phase::Delivered)
                    })
            })
            .count() as u32;
        let policies_active = book
            .standing
            .policies
            .values()
            .filter(|r| r.phase == PolicyPhase::Active && r.policy.expires_at > now)
            .count() as u32;
        Ok(Qualification {
            mode,
            eligible: batch.eligible && delivered_batches >= 1 && !book.paused,
            batch,
            delivered_batches,
            policies_active,
            enabled: book.standing.enabled(now),
            automatic_promotion: false,
        })
    }
    pub(super) fn outbox_grant_standing(
        &mut self,
        access: &Access,
        policy: Policy,
        now: u64,
    ) -> Result<super::State> {
        super::super::id(&policy.id)?;
        super::super::id(&policy.invitation)?;
        policy.template.check()?;
        token(&policy.config_sha256)?;
        token(&policy.qualification_sha256)?;
        token(&policy.owner_review_sha256)?;
        let qualification = self.outbox_standing_qualification(access, policy.mode)?;
        let book = &self.state.outbox;
        if policy.schema != POLICY_SCHEMA
            || book.standing.policies.len() >= MAX_POLICIES
            || book.standing.policies.contains_key(&policy.id)
            || policy.max_attempts == 0
            || policy.max_attempts > MAX_ATTEMPTS
            || policy.spacing_secs < MIN_SPACING_SECS
            || policy.expires_at <= now
            || policy.expires_at > now + MAX_DURATION_SECS
            || book.standing.policies.values().any(|r| {
                r.phase == PolicyPhase::Active
                    && r.policy.expires_at > now
                    && r.policy.lead == policy.lead
            })
        {
            return Err("standing policy needs one thread, one to three attempts spaced at least a week apart, and a bounded expiry".into());
        }
        if !qualification.eligible || qualification.sha256()? != policy.qualification_sha256 {
            return Err("standing policy requires the exact current reviewed-batch qualification and an explicit owner grant".into());
        }
        let lead = self
            .state
            .leads
            .get(&policy.lead)
            .ok_or("standing policy lead is unavailable")?;
        if lead.details.data.retain_until <= policy.expires_at {
            return Err("standing policy outlives the lead's retained source".into());
        }
        let invitation = self
            .state
            .replies
            .records
            .get(&policy.invitation)
            .ok_or("standing policy invitation reply is unavailable")?;
        let thread = invitation
            .original_thread
            .as_ref()
            .ok_or("standing policy invitation has no original thread")?;
        let original = self
            .state
            .outbox
            .records
            .get(&thread.proposal)
            .ok_or("standing policy invitation thread is unavailable")?;
        let original_message = original
            .subject
            .as_ref()
            .map(|s| &s.proposal.message)
            .ok_or("standing policy original subject was minimized")?;
        if invitation.lead.as_deref() != Some(policy.lead.as_str())
            || invitation.review_sha256.is_none()
            || !matches!(
                invitation.owner_label,
                Some(replies::Label::Interested | replies::Label::Question)
            )
            || invitation.safety != replies::Safety::Ordinary
            || original.mode != policy.mode
            || original_message.recipient != policy.recipient
            || original_message.config_sha256 != policy.config_sha256
            || self.state.replies.blocked_leads.contains(&policy.lead)
        {
            return Err("standing policy requires an owner-reviewed interested or question reply on the same thread and mailbox".into());
        }
        let policy_sha256 = policy.sha256()?;
        let mut next = self.state.clone();
        next.outbox.standing.policies.insert(
            policy.id.clone(),
            Record {
                policy,
                policy_sha256,
                owner: access.principal().into(),
                granted_at: now,
                phase: PolicyPhase::Active,
                attempts: 0,
                last_attempt_at: None,
                proposals: Vec::new(),
                reference_sha256: None,
            },
        );
        Ok(next)
    }
    /// Proposes one follow-up under an active standing policy and approves it
    /// as that policy's owner did. The proposal must be a follow-up on the
    /// policy's thread, template, and mailbox, within its cost, attempt, and
    /// spacing bounds. Dispatch stays separate and single-use.
    pub fn outbox_standing_follow_up(
        &mut self,
        access: &Access,
        policy_id: &str,
        proposal: Proposal,
        keys: &dyn email::MailboxCredentials,
    ) -> Result<Subject> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        let record = self
            .state
            .outbox
            .standing
            .policies
            .get(policy_id)
            .ok_or("standing policy is unavailable")?
            .clone();
        let policy = &record.policy;
        if record.phase != PolicyPhase::Active
            || policy.expires_at <= now
            || record.owner != access.principal()
            || self.state.outbox.paused
        {
            return Err("standing policy is inactive, expired, revoked, or paused".into());
        }
        if record.attempts >= policy.max_attempts {
            return Err("standing policy attempts are exhausted".into());
        }
        if record
            .last_attempt_at
            .is_some_and(|at| now < at.saturating_add(policy.spacing_secs))
        {
            return Err("standing policy spacing has not elapsed".into());
        }
        if record
            .proposals
            .iter()
            .filter_map(|p| self.state.outbox.records.get(p))
            .any(|r| {
                !r.count_consumed
                    || matches!(r.phase, Phase::Unknown | Phase::Failed | Phase::HardBounce)
            })
        {
            return Err("standing policy waits on an unresolved earlier attempt".into());
        }
        let message = &proposal.message;
        if proposal.kind != MessageKind::FollowUp
            || message.lead != policy.lead
            || message.recipient != policy.recipient
            || message.config_sha256 != policy.config_sha256
            || message.template != policy.template
            || !proposal.attachments.is_empty()
            || proposal.maximum_cost_microusd > policy.maximum_cost_microusd
            || message.expires_at > policy.expires_at
            || self.state.outbox.records.contains_key(&proposal.id)
        {
            return Err("standing follow-up must match the policy's thread, template, mailbox, cost, and expiry exactly".into());
        }
        // The canonical follow-up plan names the exact invited thread. The
        // no-response guard does not apply: this thread was answered.
        let artifact = proposal
            .follow_up_reference
            .as_ref()
            .ok_or("standing follow-up requires the canonical follow-up reference")?;
        let plan = self
            .state
            .replies
            .follow_ups
            .get(&artifact.sha256)
            .ok_or("standing follow-up plan is unavailable")?;
        let invitation = self
            .state
            .replies
            .records
            .get(&policy.invitation)
            .ok_or("standing policy invitation reply is unavailable")?;
        if plan.artifact()? != *artifact
            || plan.lead != policy.lead
            || plan.mode != policy.mode
            || Some(&plan.thread) != invitation.original_thread.as_ref()
            || plan.retain_until <= (self.clock)()
        {
            return Err("standing follow-up plan must name the invited thread".into());
        }
        if self
            .state
            .replies
            .records
            .values()
            .any(|r| r.lead.as_deref() == Some(policy.lead.as_str()) && r.id != policy.invitation)
        {
            return Err(
                "a later reply on the thread needs a human; standing follow-up stops".into(),
            );
        }
        let subject = self.propose_sales_outbox(access, proposal, keys)?;
        if subject.mode != policy.mode {
            return Err("standing follow-up mode differs from the policy".into());
        }
        let subject_sha256 = subject.sha256()?;
        let mut next = self.state.clone();
        let row = next
            .outbox
            .records
            .get_mut(&subject.proposal.id)
            .ok_or("standing follow-up record is unavailable")?;
        row.decision = Some(Decision {
            subject_sha256,
            owner: access.principal().into(),
            approved: true,
            at: now,
        });
        row.phase = Phase::Approved;
        let record = next
            .outbox
            .standing
            .policies
            .get_mut(policy_id)
            .ok_or("standing policy is unavailable")?;
        record.attempts += 1;
        record.last_attempt_at = Some(now);
        record.proposals.push(subject.proposal.id.clone());
        if record.attempts >= record.policy.max_attempts {
            record.phase = PolicyPhase::Exhausted;
        }
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        self.persist(next)?;
        Ok(subject)
    }
    /// A standing follow-up dispatches only while its policy is active or
    /// exhausted by this very attempt, and still unexpired.
    pub(super) fn outbox_standing_current(&self, proposal: &str, now: u64) -> Result<()> {
        if let Some(record) = self.state.outbox.standing.covering(proposal)
            && (!matches!(record.phase, PolicyPhase::Active | PolicyPhase::Exhausted)
                || record.policy.expires_at <= now)
        {
            return Err("standing policy expired, was revoked, or was reset".into());
        }
        Ok(())
    }
}
pub(super) fn revoke(next: &mut super::State, id: &str, reference_sha256: String) -> Result<()> {
    token(&reference_sha256)?;
    let record = next
        .outbox
        .standing
        .policies
        .get_mut(id)
        .ok_or("standing policy is unavailable")?;
    if !matches!(record.phase, PolicyPhase::Active | PolicyPhase::Exhausted) {
        return Err("standing policy is not active".into());
    }
    record.phase = PolicyPhase::Revoked;
    record.reference_sha256 = Some(reference_sha256);
    for proposal in record.proposals.clone() {
        if let Some(row) = next.outbox.records.get_mut(&proposal)
            && row.phase == Phase::Approved
            && !row.count_consumed
        {
            row.phase = Phase::Invalidated;
        }
    }
    Ok(())
}
