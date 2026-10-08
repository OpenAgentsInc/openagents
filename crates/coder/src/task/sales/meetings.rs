//! Owner-published slots and bounded private human assignments. No calendar or
//! commercial authority follows a proposal, confirmation, or acceptance.
use super::*;
use agents::{AgentAccess, Anchor};
const MAX_SLOTS: usize = 128;
const MAX_MEETINGS: usize = 128;
pub const PILOT_KIT_SHA_SOURCE: &str = include_str!("../../../../../docs/sales/pilot-kit.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub id: String,
    pub version: u64,
    pub human: String,
    pub start_at: u64,
    pub end_at: u64,
    pub expires_at: u64,
    /// A recorded owner declaration, without calendar attestation.
    pub availability_reference: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Knowledge {
    pub known: Option<String>,
    pub unknown_reason: Option<String>,
}
impl Knowledge {
    fn check(&self) -> Result<()> {
        match (&self.known, &self.unknown_reason) {
            (Some(value), None) | (None, Some(value)) => text(value, 512),
            _ => Err("meeting knowledge needs an explicit value or unknown reason".into()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefInput {
    pub decision_maker: Knowledge,
    pub current_tools: Knowledge,
    pub baseline_unknowns: Vec<String>,
    pub claim_draft: String,
    pub release: String,
    pub proposed_demo: String,
    pub proposed_pilot: String,
    pub acceptance_criteria: Vec<String>,
    pub next_action: NextAction,
    pub review_at: u64,
    pub pilot_kit_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub lead: String,
    pub lead_revision: u64,
    pub permission: Permission,
    pub source: String,
    pub source_at: u64,
    pub contact: String,
    pub account: String,
    pub workflow: String,
    pub baseline_reference: String,
    pub data: DataBoundary,
    pub details: BriefInput,
    pub checked_claims: claims::Draft,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalInput {
    pub id: String,
    pub expected_revision: u64,
    pub lead: String,
    pub expected_lead_revision: u64,
    pub slot: String,
    pub slot_version: u64,
    pub target: String,
    pub brief: Option<BriefInput>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Pending,
    OwnerConfirmed,
    Accepted,
    Declined,
    Retired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meeting {
    pub id: String,
    pub revision: u64,
    pub lead: String,
    pub lead_revision: u64,
    pub scope_sha256: String,
    pub slot: Slot,
    pub target: String,
    pub agent: Option<Anchor>,
    pub phase: Phase,
    pub brief: Option<Brief>,
    pub retain_until: u64,
    pub proposal_sha256: String,
    pub owner_confirmation: Option<String>,
    pub customer_request_reference: Option<String>,
    pub accepted_by: Option<String>,
    pub acceptance_reference: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub meeting: Meeting,
    pub assignment_current: bool,
    pub blockers: Vec<String>,
    pub calendar_authority: bool,
    pub mailbox_authority: bool,
    pub payment_authority: bool,
    pub outbound_authority: bool,
    pub earned_revenue: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Recommendation {
    pub id: String,
    pub revision: u64,
    pub proposal_sha256: String,
    pub owner_confirmation_needed: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct AgentMeeting {
    pub id: String,
    pub revision: u64,
    pub phase: Phase,
    pub proposal_sha256: String,
    pub slot: String,
    pub slot_version: u64,
    pub owner_confirmation_needed: bool,
    pub human_acceptance_needed: bool,
    pub slot_current: bool,
    pub earned_revenue: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    slots: BTreeMap<String, Slot>,
    meetings: BTreeMap<String, Meeting>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.slots.len() > MAX_SLOTS || self.meetings.len() > MAX_MEETINGS {
            return Err("meeting book exceeds its bound".into());
        }
        for (id, slot) in &self.slots {
            super::id(id)?;
            super::id(&slot.human)?;
            if slot.availability_reference.len() != 64 {
                return Err("meeting availability digest changed".into());
            }
            token(&slot.availability_reference)?;
            if *id != slot.id || slot.version == 0 || slot.end_at <= slot.start_at {
                return Err("meeting slot changed".into());
            }
        }
        for (id, m) in &self.meetings {
            super::id(id)?;
            token(&m.scope_sha256)?;
            token(&m.proposal_sha256)?;
            for hash in [&m.customer_request_reference, &m.acceptance_reference]
                .into_iter()
                .flatten()
            {
                token(hash)?;
                if hash.len() != 64 {
                    return Err("meeting evidence digest changed".into());
                }
            }
            if *id != m.id
                || m.revision == 0
                || m.lead_revision == 0
                || m.target != m.slot.human
                || (m.phase == Phase::Accepted
                    && m.accepted_by.as_deref() != Some(m.target.as_str()))
                || (matches!(
                    m.phase,
                    Phase::OwnerConfirmed | Phase::Accepted | Phase::Declined
                ) && (m.owner_confirmation.is_none() || m.customer_request_reference.is_none()))
                || (matches!(m.phase, Phase::Accepted | Phase::Declined)
                    && m.acceptance_reference.is_none())
                || (m.phase == Phase::Retired && m.brief.is_some())
                || m.brief.as_ref().is_some_and(|b| {
                    b.lead != m.lead
                        || b.lead_revision != m.lead_revision
                        || b.data.retain_until != m.retain_until
                })
            {
                return Err("meeting original scope changed".into());
            }
        }
        Ok(())
    }
    pub(super) fn retire(&mut self, lead: &str) {
        for m in self.meetings.values_mut().filter(|m| m.lead == lead) {
            m.brief = None;
            m.phase = Phase::Retired;
        }
    }
    pub(super) fn prune(&mut self, now: u64) -> bool {
        let mut changed = false;
        for m in self
            .meetings
            .values_mut()
            .filter(|m| m.retain_until <= now && m.phase != Phase::Retired)
        {
            m.brief = None;
            m.phase = Phase::Retired;
            changed = true;
        }
        changed
    }
}
fn checked_input(state: &State, value: &impl Serialize) -> Result<()> {
    privacy::check_credentials(
        state,
        &serde_json::to_string(value).map_err(|_| "meeting input serialization failed")?,
    )
}
impl Store {
    /// Project only opaque current proposal references to an admitted agent.
    pub fn sales_agent_meetings(&mut self, access: &AgentAccess) -> Result<Vec<AgentMeeting>> {
        self.refresh()?;
        let (lead, _, _, native) = self.checked_sales_agent(access)?;
        let scope = agents::scope(lead)?;
        let result = self
            .state
            .meetings
            .meetings
            .values()
            .filter(|m| {
                m.lead == lead.id
                    && m.lead_revision == lead.revision
                    && m.scope_sha256 == scope
                    && m.phase != Phase::Retired
            })
            .map(|m| AgentMeeting {
                id: m.id.clone(),
                revision: m.revision,
                phase: m.phase,
                proposal_sha256: m.proposal_sha256.clone(),
                slot: m.slot.id.clone(),
                slot_version: m.slot.version,
                owner_confirmation_needed: m.phase == Phase::Pending,
                human_acceptance_needed: m.phase == Phase::OwnerConfirmed,
                slot_current: m.slot.expires_at > (self.clock)()
                    && m.slot.start_at > (self.clock)()
                    && self
                        .state
                        .meetings
                        .slots
                        .get(&m.slot.id)
                        .is_some_and(|s| s.version == m.slot.version),
                earned_revenue: false,
            })
            .collect();
        self.recheck_sales_agent(access, &native)?;
        Ok(result)
    }
    /// The owner and each named human can read only their own bounded briefs.
    pub fn sales_meetings(
        &mut self,
        access: &Access,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<View>> {
        self.refresh()?;
        let role = self.check(access)?;
        if !(1..=100).contains(&limit) {
            return Err("meeting page limit must be 1 to 100".into());
        }
        let ids = self
            .state
            .meetings
            .meetings
            .values()
            .filter(|m| {
                after.is_none_or(|a| m.id.as_str() > a)
                    && (role == Role::Owner || m.target == access.principal)
            })
            .take(limit)
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        ids.iter()
            .map(|id| self.sales_meeting(access, id))
            .collect()
    }
    /// Recommend a published slot for an existing owner-prepared brief. This
    /// returns opaque proposal references and requires a new owner confirmation.
    pub fn recommend_sales_meeting_slot(
        &mut self,
        access: &AgentAccess,
        meeting: &str,
        revision: u64,
        slot: &str,
        slot_version: u64,
    ) -> Result<Recommendation> {
        self.refresh()?;
        let (lead, _, policy, native) = self.checked_sales_agent(access)?;
        if !policy
            .write_fields
            .contains(&agents::WriteField::NextAction)
        {
            return Err("meeting recommendation requires current next-action permission".into());
        }
        let mut m = self
            .state
            .meetings
            .meetings
            .get(meeting)
            .cloned()
            .ok_or("owner-prepared meeting proposal is unavailable")?;
        if m.phase != Phase::Pending
            || m.revision != revision
            || m.lead != lead.id
            || m.lead_revision != lead.revision
            || m.scope_sha256 != agents::scope(lead)?
        {
            return Err("meeting recommendation scope or pending revision changed".into());
        }
        let published = self
            .state
            .meetings
            .slots
            .get(slot)
            .ok_or("owner-published slot is unavailable")?;
        if published.human != m.target
            || published.version != slot_version
            || published.expires_at <= (self.clock)()
            || published.start_at <= (self.clock)()
        {
            return Err(
                "meeting recommendation slot is stale or outside the named human boundary".into(),
            );
        }
        self.meeting_lead(&m.lead, m.lead_revision, &m.target)?;
        m.slot = published.clone();
        m.agent = Some(native.anchor.clone());
        m.revision = m
            .revision
            .checked_add(1)
            .ok_or("meeting revision exceeds its range")?;
        m.proposal_sha256 = digest(
            &serde_json::to_vec(&(
                &m.id,
                m.revision,
                &m.lead,
                m.lead_revision,
                &m.scope_sha256,
                &m.slot,
                &m.brief,
                &m.agent,
            ))
            .map_err(|_| "meeting recommendation serialization failed")?,
        );
        self.recheck_sales_agent(access, &native)?;
        let result = Recommendation {
            id: m.id.clone(),
            revision: m.revision,
            proposal_sha256: m.proposal_sha256.clone(),
            owner_confirmation_needed: true,
        };
        let mut next = self.state.clone();
        next.meetings.meetings.insert(m.id.clone(), m);
        self.persist(next)?;
        Ok(result)
    }
    pub fn publish_meeting_slot(
        &mut self,
        access: &Access,
        slot: &Slot,
        expected_version: u64,
    ) -> Result<Slot> {
        self.refresh()?;
        self.admin(access)?;
        checked_input(&self.state, slot)?;
        id(&slot.id)?;
        id(&slot.human)?;
        text(&slot.availability_reference, 256)?;
        let now = (self.clock)();
        if slot.version
            != expected_version
                .checked_add(1)
                .ok_or("slot version exceeds its range")?
            || slot.start_at <= now
            || slot.end_at <= slot.start_at
            || slot.end_at - slot.start_at > 86_400
            || slot.expires_at <= now
            || slot.expires_at > slot.start_at
            || !self
                .state
                .principals
                .get(&slot.human)
                .is_some_and(|p| p.active)
        {
            return Err(
                "slot needs current named human and explicit finite owner-published availability"
                    .into(),
            );
        }
        let mut canonical = slot.clone();
        canonical.availability_reference = digest(slot.availability_reference.as_bytes());
        let slot = &canonical;
        let current = self.state.meetings.slots.get(&slot.id);
        if current.is_some_and(|s| {
            s.version == slot.version
                && digest(&serde_json::to_vec(s).unwrap_or_default())
                    == digest(&serde_json::to_vec(slot).unwrap_or_default())
        }) {
            return Ok(slot.clone());
        }
        if current.map_or(0, |s| s.version) != expected_version {
            return Err("meeting slot revision conflict".into());
        }
        if current.is_none() && self.state.meetings.slots.len() >= MAX_SLOTS {
            return Err("meeting slot history is full".into());
        }
        let mut next = self.state.clone();
        next.meetings.slots.insert(slot.id.clone(), slot.clone());
        self.persist(next)?;
        Ok(slot.clone())
    }
    pub fn sales_meeting_slots(&mut self, access: &AgentAccess) -> Result<Vec<Slot>> {
        self.refresh()?;
        let (_, _, _, native) = self.checked_sales_agent(access)?;
        let now = (self.clock)();
        let result = self
            .state
            .meetings
            .slots
            .values()
            .filter(|s| {
                s.expires_at > now
                    && s.start_at > now
                    && self
                        .state
                        .principals
                        .get(&s.human)
                        .is_some_and(|p| p.active)
            })
            .cloned()
            .collect();
        self.recheck_sales_agent(access, &native)?;
        Ok(result)
    }
    fn meeting_lead(&self, lead: &str, revision: u64, target: &str) -> Result<Lead> {
        let lead = self
            .state
            .leads
            .get(lead)
            .ok_or("meeting lead is unavailable")?;
        let now = (self.clock)();
        if lead.revision != revision {
            return Err("meeting lead revision changed; propose and confirm again".into());
        }
        if lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || lead.details.data.retain_until <= now
            || !Self::recipient(&lead.details, target)
        {
            return Err("meeting permission or named human data boundary is unavailable".into());
        }
        let channel = lead
            .contact
            .split_once(':')
            .ok_or("meeting contact channel is unavailable")?
            .0;
        self.contact_admitted(lead, channel)?;
        if !self.state.principals.get(target).is_some_and(|p| p.active) {
            return Err("meeting receiving human is unavailable".into());
        }
        Ok(lead.clone())
    }
    fn meeting_brief(&mut self, access: &Access, lead: &Lead, input: &BriefInput) -> Result<Brief> {
        input.decision_maker.check()?;
        input.current_tools.check()?;
        id(&input.claim_draft)?;
        if input.pilot_kit_sha256 != digest(PILOT_KIT_SHA_SOURCE.as_bytes()) {
            return Err("meeting pilot kit changed".into());
        }
        for value in [
            &input.proposed_demo,
            &input.proposed_pilot,
            &input.next_action.description,
        ] {
            text(value, 1024)?;
        }
        if input.baseline_unknowns.is_empty()
            || input.baseline_unknowns.len() > 8
            || input.acceptance_criteria.is_empty()
            || input.acceptance_criteria.len() > 8
            || input.review_at <= (self.clock)()
            || input.review_at > lead.details.data.retain_until
            || input.next_action.due_at <= (self.clock)()
            || input.next_action.due_at > lead.details.data.retain_until
        {
            return Err("meeting brief needs explicit unknowns, bounded acceptance, next action, and review date".into());
        }
        for value in input
            .baseline_unknowns
            .iter()
            .chain(&input.acceptance_criteria)
        {
            text(value, 512)?;
        }
        let checked_claims =
            self.validate_claim_draft(access, &input.claim_draft, &input.release)?;
        Ok(Brief {
            lead: lead.id.clone(),
            lead_revision: lead.revision,
            permission: lead.details.permission.clone(),
            source: lead.source.clone(),
            source_at: lead.source_at,
            contact: lead.contact.clone(),
            account: lead.details.account.clone(),
            workflow: lead.details.workflow.clone(),
            baseline_reference: lead.details.baseline_reference.clone(),
            data: lead.details.data.clone(),
            details: input.clone(),
            checked_claims,
        })
    }
    /// The owner prepares a private brief. Agents cannot author its customer,
    /// price, decision-maker, or pilot facts through a slot recommendation.
    pub fn propose_sales_meeting(
        &mut self,
        owner: &Access,
        input: &ProposalInput,
    ) -> Result<Meeting> {
        self.refresh()?;
        self.admin(owner)?;
        checked_input(&self.state, input)?;
        id(&input.id)?;
        id(&input.slot)?;
        id(&input.target)?;
        let lead = self.meeting_lead(&input.lead, input.expected_lead_revision, &input.target)?;
        self.readable(owner, &lead)?;
        let slot = self
            .state
            .meetings
            .slots
            .get(&input.slot)
            .cloned()
            .ok_or("owner-published slot is unavailable")?;
        if slot.version != input.slot_version || slot.human != input.target {
            return Err("meeting slot or named human changed".into());
        }
        let current = self.state.meetings.meetings.get(&input.id);
        if current.map_or(0, |m| m.revision) != input.expected_revision {
            return Err("meeting proposal revision conflict".into());
        }
        if current.is_none() && self.state.meetings.meetings.len() >= MAX_MEETINGS {
            return Err("meeting proposal history is full".into());
        }
        let brief = input
            .brief
            .as_ref()
            .map(|b| self.meeting_brief(owner, &lead, b))
            .transpose()?;
        let proposal_sha256 = digest(
            &serde_json::to_vec(&(input, &brief, &slot))
                .map_err(|_| "meeting proposal serialization failed")?,
        );
        let meeting = Meeting {
            id: input.id.clone(),
            revision: input
                .expected_revision
                .checked_add(1)
                .ok_or("meeting revision exceeds its range")?,
            lead: lead.id.clone(),
            lead_revision: lead.revision,
            scope_sha256: agents::scope(&lead)?,
            slot,
            target: input.target.clone(),
            agent: None,
            phase: Phase::Pending,
            brief,
            retain_until: lead.details.data.retain_until,
            proposal_sha256,
            owner_confirmation: None,
            customer_request_reference: None,
            accepted_by: None,
            acceptance_reference: None,
        };
        let mut next = self.state.clone();
        if let Some(brief) = &meeting.brief {
            for value in brief
                .details
                .baseline_unknowns
                .iter()
                .chain(&brief.details.acceptance_criteria)
                .chain([
                    &brief.details.proposed_demo,
                    &brief.details.proposed_pilot,
                    &brief.details.next_action.description,
                ])
                .chain(brief.details.decision_maker.known.iter())
                .chain(brief.details.decision_maker.unknown_reason.iter())
                .chain(brief.details.current_tools.known.iter())
                .chain(brief.details.current_tools.unknown_reason.iter())
            {
                privacy::remember_identifier(&mut next, value)?;
            }
        }
        next.meetings
            .meetings
            .insert(meeting.id.clone(), meeting.clone());
        self.persist(next)?;
        Ok(meeting)
    }
    fn checked_meeting(&mut self, access: &Access, m: &Meeting, require_slot: bool) -> Result<()> {
        let lead = self.meeting_lead(&m.lead, m.lead_revision, &m.target)?;
        if agents::scope(&lead)? != m.scope_sha256
            || m.phase == Phase::Retired
            || m.retain_until <= (self.clock)()
        {
            return Err("meeting original scope is unavailable".into());
        }
        let brief = m.brief.as_ref().ok_or("meeting private brief is missing")?;
        let current =
            self.validate_claim_draft(access, &brief.details.claim_draft, &brief.details.release)?;
        if current.sha256 != brief.checked_claims.sha256 {
            return Err("meeting checked claims changed".into());
        }
        if require_slot
            && (m.slot.expires_at <= (self.clock)()
                || m.slot.start_at <= (self.clock)()
                || self
                    .state
                    .meetings
                    .slots
                    .get(&m.slot.id)
                    .is_none_or(|s| s.version != m.slot.version))
        {
            return Err("meeting slot is stale; publish and confirm a fresh proposal".into());
        }
        Ok(())
    }
    pub fn confirm_sales_meeting(
        &mut self,
        owner: &Access,
        id: &str,
        revision: u64,
        approval: &str,
        customer_request: &str,
    ) -> Result<Meeting> {
        self.refresh()?;
        self.admin(owner)?;
        text(customer_request, 256)?;
        privacy::check_credentials(&self.state, customer_request)?;
        let mut m = self
            .state
            .meetings
            .meetings
            .get(id)
            .cloned()
            .ok_or("meeting proposal is unavailable")?;
        if m.revision != revision || m.proposal_sha256 != approval || m.phase != Phase::Pending {
            return Err("approve the exact current pending meeting proposal".into());
        }
        self.checked_meeting(owner, &m, true)?;
        if self.state.meetings.meetings.values().any(|other| {
            other.id != m.id
                && matches!(other.phase, Phase::OwnerConfirmed | Phase::Accepted)
                && other.slot.human == m.slot.human
                && other.slot.start_at < m.slot.end_at
                && other.slot.end_at > m.slot.start_at
        }) {
            return Err("owner-published meeting slot is already confirmed".into());
        }
        m.phase = Phase::OwnerConfirmed;
        m.owner_confirmation = Some(owner.principal.clone());
        m.customer_request_reference = Some(digest(customer_request.as_bytes()));
        let mut next = self.state.clone();
        next.meetings.meetings.insert(id.into(), m.clone());
        self.persist(next)?;
        Ok(m)
    }
    pub fn decide_sales_meeting(
        &mut self,
        human: &Access,
        id: &str,
        revision: u64,
        approval: &str,
        accept: bool,
        reference: &str,
    ) -> Result<Meeting> {
        self.refresh()?;
        self.check(human)?;
        text(reference, 256)?;
        privacy::check_credentials(&self.state, reference)?;
        let mut m = self
            .state
            .meetings
            .meetings
            .get(id)
            .cloned()
            .ok_or("meeting assignment is unavailable")?;
        if human.principal != m.target
            || m.revision != revision
            || m.proposal_sha256 != approval
            || m.phase != Phase::OwnerConfirmed
        {
            return Err("only the named human may decide the exact confirmed assignment".into());
        }
        self.checked_meeting(human, &m, true)?;
        m.phase = if accept {
            Phase::Accepted
        } else {
            Phase::Declined
        };
        m.accepted_by = accept.then(|| human.principal.clone());
        m.acceptance_reference = Some(digest(reference.as_bytes()));
        let mut next = self.state.clone();
        next.meetings.meetings.insert(id.into(), m.clone());
        self.persist(next)?;
        Ok(m)
    }
    pub fn sales_meeting(&mut self, access: &Access, id: &str) -> Result<View> {
        self.refresh()?;
        let role = self.check(access)?;
        let mut m = self
            .state
            .meetings
            .meetings
            .get(id)
            .cloned()
            .ok_or("meeting is unavailable")?;
        let owner = role == Role::Owner;
        if !owner && access.principal != m.target {
            return Err("private meeting brief access refused".into());
        }
        let mut blockers = Vec::new();
        let current = self.checked_meeting(
            access,
            &m,
            matches!(m.phase, Phase::Pending | Phase::OwnerConfirmed),
        );
        let assignment_current = current.is_ok() && m.phase == Phase::Accepted;
        if let Err(reason) = current {
            blockers.push(reason);
            if !owner {
                m.brief = None;
            }
        }
        match m.phase {
            Phase::Pending => {
                blockers.push("owner confirmation pending".into());
                if !owner {
                    m.brief = None;
                }
            }
            Phase::OwnerConfirmed => blockers.push("named human acceptance pending".into()),
            Phase::Declined => blockers.push("named human declined".into()),
            Phase::Retired => blockers.push("original brief retention expired".into()),
            Phase::Accepted => {}
        }
        Ok(View {
            meeting: m,
            assignment_current,
            blockers,
            calendar_authority: false,
            mailbox_authority: false,
            payment_authority: false,
            outbound_authority: false,
            earned_revenue: false,
        })
    }
}
